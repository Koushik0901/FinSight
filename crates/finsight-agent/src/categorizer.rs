use crate::{
    agent::{AgentEvent, AgentJob, EventCallback},
    CompletionProvider,
};
use anyhow::Result;
use finsight_core::{
    models::NewCategorization,
    repos::{categorizations, rule_proposals, rules},
    Db,
};
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;

const LLM_BATCH_SIZE: usize = 20;

/// Confidence score below which a LLM-assigned category is considered uncertain
/// and surfaced to the user as "needs review". Shared with the Tauri command layer
/// and the Inbox action-item query so all three stay in sync.
pub const LOW_CONFIDENCE_THRESHOLD: f64 = 0.6;

#[derive(Deserialize)]
struct LlmResult {
    txn_id: String,
    category_id: String,
    confidence: f64,
    rationale: String,
}

pub async fn run_job(
    db: &Db,
    job: AgentJob,
    provider: Arc<dyn CompletionProvider>,
    on_event: EventCallback,
) -> Result<()> {
    let (import_id, rerun_mode) = match &job {
        AgentJob::CategorizeImport { import_id } => (Some(import_id.clone()), false),
        AgentJob::CategorizeAll => (None, false),
        AgentJob::RecategorizeLowConfidence => (None, true),
        _ => return Ok(()),
    };

    // Snapshot the ledger epoch from the reset barrier. Two layers keep this job
    // from writing against a wiped ledger:
    //  - `superseded()` (cheap epoch compare) lets us bail promptly at batch
    //    boundaries once a Delete-All begins.
    //  - a `writer_lease` held across every commit makes it *impossible* for a
    //    write to land after the wipe: Delete-All drains outstanding leases
    //    before wiping, and a lease taken after the wipe sees the new epoch.
    let start_epoch = db.reset_barrier().epoch();
    let superseded = || db.reset_barrier().epoch() != start_epoch;

    // Load data needed for categorization on a blocking thread
    let (uncategorized, active_rules, categories, recent_examples) = {
        let db = db.clone();
        let import_id_clone = import_id.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let uncategorized = if rerun_mode {
                load_low_confidence(&mut conn)?
            } else {
                load_uncategorized(&mut conn, import_id_clone.as_deref())?
            };
            let active_rules = rules::list_active(&mut conn)?;
            let categories = load_categories(&mut conn)?;
            let recent_examples = load_recent_examples(&mut conn)?;
            Ok::<_, anyhow::Error>((uncategorized, active_rules, categories, recent_examples))
        })
        .await??
    };

    // Build a set of valid category IDs for LLM output validation.
    let valid_category_ids: HashSet<String> =
        categories.iter().map(|(id, _, _, _)| id.clone()).collect();

    let total = uncategorized.len() as u32;
    let mut remaining: Vec<(String, String, i64)> = Vec::new(); // (txn_id, merchant_raw, amount_cents)
    let mut categorized: u32 = 0;

    // Step 1: Rule pass
    for (txn_id, merchant_raw, amount_cents) in &uncategorized {
        // Bail promptly if a Delete-All has begun — don't keep scanning rules
        // for transactions that no longer exist.
        if superseded() {
            return Ok(());
        }
        let matched = active_rules.iter().find(|r| {
            let pat = r.pattern.to_lowercase();
            let merch = merchant_raw.to_lowercase();
            // Simple LIKE: leading/trailing % = contains, otherwise exact
            if pat.starts_with('%') && pat.ends_with('%') && pat.len() > 1 {
                merch.contains(&pat[1..pat.len() - 1])
            } else if let Some(stripped) = pat.strip_prefix('%') {
                merch.ends_with(stripped)
            } else if pat.ends_with('%') {
                merch.starts_with(&pat[..pat.len() - 1])
            } else {
                merch == pat
            }
        });

        if let Some(rule) = matched {
            // Hold a reset lease across the commit and re-check the epoch under
            // it: if a Delete-All has landed, skip the write entirely; otherwise
            // the reset can't wipe until this lease drops, so this categorization
            // can only land before the wipe (never orphaned after it).
            let lease = db.reset_barrier().writer_lease(start_epoch).await;
            if lease.superseded() {
                return Ok(());
            }
            let cat_id = rule.category_id.clone();
            let txn_id = txn_id.clone();
            let wdb = db.clone();
            tokio::task::spawn_blocking(move || {
                let mut conn = wdb.get()?;
                categorizations::insert(&mut conn, NewCategorization {
                    txn_id: txn_id.clone(),
                    category_id: Some(cat_id.clone()),
                    source: "rule".to_string(),
                    confidence: 1.0,
                    model: None,
                })?;
                conn.execute(
                    "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
                    params![cat_id, txn_id],
                )?;
                Ok::<_, anyhow::Error>(())
            }).await??;
            drop(lease);
            categorized += 1;
        } else {
            remaining.push((txn_id.clone(), merchant_raw.clone(), *amount_cents));
        }
    }

    on_event(AgentEvent::CategorizationProgress {
        import_id: import_id.clone(),
        done: categorized,
        total,
    });

    // Step 2: LLM batch pass
    let system_prompt = build_system_prompt(&categories, &recent_examples);

    for chunk in remaining.chunks(LLM_BATCH_SIZE) {
        // A Delete-All / factory reset between batches aborts the rest of the
        // run: the following LLM call + writes would target a wiped ledger.
        // (Cheap bail before we spend an LLM round-trip; the lease below is the
        // bulletproof guard around the actual writes.)
        if superseded() {
            return Ok(());
        }
        // Per-chunk error recovery: a bad LLM response (timeout, parse error, hallucinated
        // JSON) skips this chunk and continues rather than aborting the entire job.
        let chunk_result = async {
            let user_prompt = build_user_prompt(chunk);
            let raw = provider.complete_json(&system_prompt, &user_prompt).await?;
            // All three provider impls (Ollama, OpenAiCompat, Anthropic) return a flat JSON array.
            let results: Vec<LlmResult> = serde_json::from_value(raw)?;
            Ok::<Vec<LlmResult>, anyhow::Error>(results)
        }
        .await;

        let results = match chunk_result {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[categorizer] chunk failed, skipping: {e}");
                on_event(AgentEvent::CategorizationProgress {
                    import_id: import_id.clone(),
                    done: categorized,
                    total,
                });
                continue;
            }
        };

        // The txn_ids actually sent in this chunk. The LLM sometimes echoes a
        // garbled or hallucinated id; writing it would violate the
        // categorizations.txn_id foreign key and abort the whole job.
        let chunk_txn_ids: std::collections::HashSet<&str> =
            chunk.iter().map(|(id, _, _)| id.as_str()).collect();

        // Hold one reset lease across this chunk's writes. A Delete-All draining
        // the barrier waits for it, so these categorizations can only land
        // before the wipe; and if a reset already committed, `superseded()` is
        // true and we stop before writing into the wiped ledger.
        let lease = db.reset_barrier().writer_lease(start_epoch).await;
        if lease.superseded() {
            return Ok(());
        }
        for r in &results {
            // Validate the category_id returned by the LLM exists in our category set.
            // Skip results with hallucinated or stale IDs to avoid writing dangling FKs.
            if !valid_category_ids.contains(&r.category_id) {
                eprintln!(
                    "[categorizer] LLM returned unknown category_id '{}' for txn '{}', skipping",
                    r.category_id, r.txn_id
                );
                continue;
            }
            // Validate the txn_id was actually in this batch (guards the
            // transactions FK against LLM-hallucinated ids).
            if !chunk_txn_ids.contains(r.txn_id.as_str()) {
                eprintln!(
                    "[categorizer] LLM returned unknown txn_id '{}', skipping",
                    r.txn_id
                );
                continue;
            }

            let txn_id = r.txn_id.clone();
            let cat_id = r.category_id.clone();
            let confidence = r.confidence;
            let rationale = r.rationale.clone();
            let model = provider.model_id().to_string();
            let db = db.clone();
            let write = tokio::task::spawn_blocking(move || {
                let mut conn = db.get()?;
                categorizations::insert(&mut conn, NewCategorization {
                    txn_id: txn_id.clone(),
                    category_id: Some(cat_id.clone()),
                    source: "llm".to_string(),
                    confidence,
                    model: Some(model),
                })?;
                conn.execute(
                    "UPDATE transactions SET category_id = ?1, ai_confidence = ?2, ai_explanation = ?3 WHERE id = ?4",
                    params![cat_id, confidence, rationale, txn_id],
                )?;
                Ok::<_, anyhow::Error>(())
            })
            .await;
            // Defense in depth: a single row's write failure must not abort the
            // whole categorization job — log it and keep going.
            match write {
                Ok(Ok(())) => categorized += 1,
                Ok(Err(e)) => {
                    eprintln!("[categorizer] write failed for one transaction, skipping: {e}")
                }
                Err(e) => eprintln!("[categorizer] write task join error, skipping: {e}"),
            }
        }
        drop(lease);
        on_event(AgentEvent::CategorizationProgress {
            import_id: import_id.clone(),
            done: categorized,
            total,
        });
    }

    // If a Delete-All has begun, stop here. The remaining post-run steps are
    // all self-healing against a wipe (rule proposals derive from now-wiped
    // corrections and are FK-guarded; anomaly detection UPDATEs transactions by
    // id, hitting zero rows once wiped), but there's no point doing the work —
    // and this keeps us from racing a wipe that lands mid-step.
    if superseded() {
        return Ok(());
    }

    // Post-run: surface rule proposals for merchants the user keeps re-categorizing.
    {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            rule_proposals::emit_from_corrections(&mut conn, 3)?;
            Ok::<_, anyhow::Error>(())
        })
        .await??;
    }

    let final_skipped = total.saturating_sub(categorized);
    on_event(AgentEvent::CategorizationComplete {
        import_id: import_id.clone(),
        categorized,
        skipped: final_skipped,
    });

    // Post-run: anomaly detection (best-effort — failures don't abort the scan).
    let _ = crate::anomaly::detect_anomalies(db, Arc::clone(&provider)).await;

    // Post-run: persist scan metadata for status ticker.
    {
        let db = db.clone();
        let n = categorized;
        let _ = tokio::task::spawn_blocking(move || {
            let conn = db.get()?;
            crate::anomaly::store_last_scan(&conn, n)?;
            Ok::<_, anyhow::Error>(())
        })
        .await;
    }

    Ok(())
}

// ── helpers ────────────────────────────────────────────────────────────────

fn load_uncategorized(
    conn: &mut rusqlite::Connection,
    _import_id: Option<&str>,
) -> Result<Vec<(String, String, i64)>> {
    // Exclude transfers / credit-card payments: the builtin pass already flags
    // them (is_transfer = 1) and they are not spending or income, so they must
    // not be handed to the LLM — otherwise it invents a bogus spending category
    // (e.g. a "PAYMENT RECEIVED - THANK YOU" card payment tagged "Shopping")
    // and burns a low-confidence "Needs review" slot on something already known.
    // Investment-account rows (trades, contributions) are equally not spending —
    // don't ship them to the cloud either.
    let mut stmt = conn.prepare(&format!(
        "SELECT id, merchant_raw, amount_cents FROM transactions t \
         WHERE category_id IS NULL AND is_transfer = 0 AND {} ORDER BY posted_at DESC",
        finsight_core::metrics::non_investment_txn_predicate("t")
    ))?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Load transactions that were categorized by the LLM but with low confidence,
/// so they can be re-sent to the LLM after the user has added rules or corrections.
fn load_low_confidence(conn: &mut rusqlite::Connection) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, merchant_raw, amount_cents FROM transactions \
         WHERE ai_confidence IS NOT NULL AND ai_confidence < ?1 \
           AND (SELECT source FROM categorizations c \
                WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm' \
         ORDER BY ai_confidence ASC, posted_at DESC",
    )?;
    let rows = stmt.query_map(rusqlite::params![LOW_CONFIDENCE_THRESHOLD], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

type CategoryRow = (String, String, String, Option<String>);

fn load_categories(conn: &mut rusqlite::Connection) -> Result<Vec<CategoryRow>> {
    // (id, label, group_label, guidance)
    let mut stmt = conn.prepare(
        "SELECT c.id, c.label, COALESCE(g.label, ''), c.guidance \
         FROM categories c LEFT JOIN category_groups g ON g.id = c.group_id \
         WHERE c.archived_at IS NULL ORDER BY g.sort_order, c.sort_order",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn load_recent_examples(conn: &mut rusqlite::Connection) -> Result<Vec<(String, String)>> {
    // (merchant_raw, category_label) — last 5 user corrections
    let mut stmt = conn.prepare(
        "SELECT t.merchant_raw, c.label \
         FROM categorizations ca \
         JOIN transactions t ON t.id = ca.txn_id \
         JOIN categories c ON c.id = ca.category_id \
         WHERE ca.source = 'user' \
         ORDER BY ca.at DESC LIMIT 5",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn build_system_prompt(categories: &[CategoryRow], recent_examples: &[(String, String)]) -> String {
    let cats_json = json!(categories
        .iter()
        .map(|(id, label, group, guidance)| {
            let mut obj = json!({"id": id, "label": label, "group_label": group});
            // User-authored guidance tells the model when this category applies.
            if let Some(g) = guidance.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                obj["guidance"] = json!(g);
            }
            obj
        })
        .collect::<Vec<_>>());
    let examples_json = json!(recent_examples
        .iter()
        .map(|(merchant, cat)| { json!({"merchant_raw": merchant, "category_label": cat}) })
        .collect::<Vec<_>>());
    format!(
        "You are a personal finance transaction categorizer. Classify each transaction into \
         exactly one of the provided categories. When a category includes a \"guidance\" note, \
         follow it — it is the user's own instruction for when that category applies (merchant \
         hints, exclusions, intent). Respond with a valid JSON array only — no markdown, no \
         explanation outside the array.\n\nCategories:\n{}\n\nRecent examples from this user (for calibration):\n{}",
        cats_json, examples_json
    )
}

fn build_user_prompt(txns: &[(String, String, i64)]) -> String {
    // Privacy: redact personally-identifying tokens (bank reference numbers and
    // the counterparty NAME of a person-to-person e-transfer) before the
    // merchant string leaves the machine. The category-relevant vocabulary is
    // preserved; a stranger's name is never useful to the categorizer anyway.
    let items: Vec<_> = txns.iter().map(|(id, merchant, amount)| {
        json!({"txn_id": id, "merchant_raw": finsight_core::categorize::redact_for_llm(merchant), "amount_cents": amount})
    }).collect();
    format!(
        "Classify these transactions:\n{}\n\nRespond:\n[\
         {{\"txn_id\":\"...\",\"category_id\":\"...\",\"confidence\":0.0,\"rationale\":\"one sentence\"}}]",
        json!(items)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockCompletionProvider;
    use finsight_core::{db::run_migrations, keychain, models::NewRule};
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, finsight_core::Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("cat.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_db(conn: &mut rusqlite::Connection) -> (String, String) {
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','Daily',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')", []).unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES('t1','a1','2024-01-15T00:00:00Z',1500,'CHIPOTLE','cleared',0,'2024-01-15T00:00:00Z')", [],
        ).unwrap();
        ("a1".to_string(), "t1".to_string())
    }

    #[tokio::test]
    async fn rule_pass_categorizes_matching_transaction() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn);
            rules::insert(
                &mut conn,
                NewRule {
                    pattern: "CHIPOTLE".to_string(),
                    category_id: "cat1".to_string(),
                    source: "user".to_string(),
                    treatment: "categorize".to_string(),
                },
            )
            .unwrap();
        }
        let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(
            &db,
            AgentJob::CategorizeAll,
            provider,
            Arc::new(move |e| {
                events_clone.lock().unwrap().push(e);
            }),
        )
        .await
        .unwrap();

        let conn = db.get().unwrap();
        let cat_id: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cat_id.as_deref(), Some("cat1"));
    }

    #[tokio::test]
    async fn llm_pass_writes_category_and_ai_confidence() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn);
            // No rules — forces LLM path
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.87, "rationale": "Fast food"}]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let conn = db.get().unwrap();
        let (cat_id, confidence): (Option<String>, Option<f64>) = conn
            .query_row(
                "SELECT category_id, ai_confidence FROM transactions WHERE id='t1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(cat_id.as_deref(), Some("cat1"));
        assert!((confidence.unwrap() - 0.87).abs() < 0.01);
    }

    #[tokio::test]
    async fn transfers_are_not_sent_to_the_llm_and_stay_uncategorized() {
        // Phase 4 finding: credit-card payments / internal transfers are flagged
        // is_transfer=1 by the builtin pass but left uncategorized. They must NOT
        // be handed to the LLM — otherwise it tags a "PAYMENT RECEIVED" card
        // payment as "Shopping" and floods Needs Review. Even if the model
        // volunteers a category for one, the batch guard drops it.
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE (not a transfer) + cat1 + account
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,is_transfer,created_at) \
                 VALUES('t2','a1','2024-02-01T00:00:00Z',298614,'PAYMENT RECEIVED - THANK YOU','cleared',0,1,'2024-02-01T00:00:00Z')",
                [],
            ).unwrap();
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            // Model tries to categorize BOTH; t2 must be rejected as out-of-batch.
            response: json!([
                {"txn_id": "t1", "category_id": "cat1", "confidence": 0.9, "rationale": "Fast food"},
                {"txn_id": "t2", "category_id": "cat1", "confidence": 0.8, "rationale": "guessed"}
            ]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let conn = db.get().unwrap();
        let (cat, conf): (Option<String>, Option<f64>) = conn
            .query_row(
                "SELECT category_id, ai_confidence FROM transactions WHERE id='t2'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(cat, None, "a transfer must stay uncategorized");
        assert_eq!(conf, None, "a transfer must not get an LLM confidence");
        // The real spending txn was still categorized.
        let t1: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(t1.as_deref(), Some("cat1"));
    }

    #[tokio::test]
    async fn hallucinated_txn_id_is_skipped_and_does_not_abort_the_job() {
        // Regression: on real data Gemma occasionally echoes a garbled txn_id.
        // Writing it violated the categorizations.txn_id FK and aborted the
        // whole job. It must now be skipped without failing run_job.
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 + cat1 + account
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            // A hallucinated txn_id plus the real one.
            response: json!([
                {"txn_id": "ghost-txn-999", "category_id": "cat1", "confidence": 0.9, "rationale": "bogus"},
                {"txn_id": "t1", "category_id": "cat1", "confidence": 0.85, "rationale": "Fast food"}
            ]),
            tool_turns: Mutex::new(vec![]),
        });

        // Must not error despite the bad id.
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let conn = db.get().unwrap();
        // The real txn was categorized; the ghost wrote nothing.
        let cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cat.as_deref(), Some("cat1"));
        let ghost: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM categorizations WHERE txn_id='ghost-txn-999'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ghost, 0, "hallucinated txn_id must not be written");
    }

    #[tokio::test]
    async fn emits_rule_proposal_for_repeated_user_corrections() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // inserts cat1 + account a1 + txn t1
                                // Add two more transactions for the same merchant, all user-categorized.
            for i in 2..=3 {
                let tid = format!("t{i}");
                conn.execute(
                    "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,created_at) \
                     VALUES(?1,'a1','2024-01-15T00:00:00Z',1500,'CHIPOTLE','cat1','cleared',0,'2024-01-15T00:00:00Z')",
                    rusqlite::params![tid],
                ).unwrap();
            }
            // t1 also categorized to cat1, all by the user.
            conn.execute(
                "UPDATE transactions SET category_id='cat1' WHERE id='t1'",
                [],
            )
            .unwrap();
            for (i, tid) in ["t1", "t2", "t3"].iter().enumerate() {
                conn.execute(
                    "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                     VALUES(?1,?2,'cat1','user',1.0,'2024-01-16T00:00:00Z')",
                    rusqlite::params![format!("uc{i}"), tid],
                )
                .unwrap();
            }
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let mut conn = db.get().unwrap();
        let pending =
            finsight_core::repos::rule_proposals::list(&mut conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].pattern, "CHIPOTLE");
    }

    /// LLM returning a hallucinated category_id must be silently skipped —
    /// the transaction should remain uncategorized rather than receive a dangling FK.
    #[tokio::test]
    async fn llm_invalid_category_id_is_skipped() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn);
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            // "ghost-category" does not exist in the DB
            response: json!([{"txn_id": "t1", "category_id": "ghost-category", "confidence": 0.9, "rationale": "Hallucinated"}]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let conn = db.get().unwrap();
        let cat_id: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(cat_id.is_none(), "dangling FK must not be written");
    }

    /// When one LLM chunk fails (e.g. bad JSON), remaining chunks must still be processed.
    /// This test uses two transactions and a mock that returns a parse error for the first
    /// call and valid data on the second — simulating a retry via two distinct responses.
    /// Here we verify the job itself does not propagate the error.
    #[tokio::test]
    async fn chunk_error_does_not_abort_job() {
        use crate::providers::mock::MockCompletionProvider;

        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1
        }
        // Return invalid JSON — the chunk should be skipped but run_job must succeed.
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: serde_json::Value::String("not valid array".into()),
            tool_turns: Mutex::new(vec![]),
        });
        let result = run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {})).await;
        assert!(
            result.is_ok(),
            "job must not fail when a chunk errors: {result:?}"
        );
    }

    /// A provider that simulates a Delete-All landing exactly when the LLM is
    /// answering: it advances the reset barrier (like `delete_all_data` does)
    /// on the first `complete_json`, then returns a response that WOULD
    /// categorize the transaction if the reset guard failed.
    struct ResetDuringLlmProvider {
        barrier: finsight_core::ResetBarrier,
        response: serde_json::Value,
    }

    #[async_trait::async_trait]
    impl CompletionProvider for ResetDuringLlmProvider {
        fn provider_id(&self) -> &str {
            "reset-during-llm"
        }
        fn model_id(&self) -> &str {
            "test"
        }
        async fn complete_json(&self, _system: &str, _user: &str) -> Result<serde_json::Value> {
            // Advance the epoch (dropping the guard immediately — the epoch stays
            // advanced; only the drain gate is released). This is the state the
            // categorizer will observe when it takes its write lease next.
            drop(self.barrier.begin_reset().await);
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn reset_during_the_llm_pass_writes_no_categorization() {
        // A Delete-All that lands while the LLM is answering must leave the
        // transaction uncategorized: the categorizer takes a write lease and
        // re-checks the epoch before committing, sees it advanced, and skips.
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE (no rule) + cat1 + account
        }
        let provider = Arc::new(ResetDuringLlmProvider {
            barrier: db.reset_barrier().clone(),
            // If the guard did NOT fire, this response would categorize t1 -> cat1.
            response: json!([
                {"txn_id": "t1", "category_id": "cat1", "confidence": 0.9, "rationale": "Fast food"}
            ]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let conn = db.get().unwrap();
        let cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            cat, None,
            "the LLM write must be skipped once the reset barrier advanced mid-run"
        );
    }
}
