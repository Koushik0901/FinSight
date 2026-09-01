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
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

const LLM_BATCH_SIZE: usize = 20;

/// Confidence score below which a LLM-assigned category is considered uncertain
/// and surfaced to the user as "needs review". Shared with the Tauri command layer
/// and the Inbox action-item query so all three stay in sync.
pub const LOW_CONFIDENCE_THRESHOLD: f64 = 0.6;

fn routing_threshold(conn: &rusqlite::Connection) -> f64 {
    // Immich-style per-task routing: `llm_routing.fasttextThreshold` (camelCase via serde).
    // Falls back to the const when the setting is absent or malformed.
    if let Ok(Some(v)) = finsight_core::settings::get::<serde_json::Value>(conn, "llm_routing") {
        if let Some(t) = v.get("fasttextThreshold").and_then(|x| x.as_f64()) {
            if (0.0..=1.0).contains(&t) {
                return t;
            }
        }
        if let Some(t) = v.get("fasttext_threshold").and_then(|x| x.as_f64()) {
            if (0.0..=1.0).contains(&t) {
                return t;
            }
        }
    }
    LOW_CONFIDENCE_THRESHOLD
}

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
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let uncategorized = if rerun_mode {
                load_low_confidence(&mut conn)?
            } else {
                load_uncategorized(&mut conn)?
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
            // 'transfer'/'settle_up' rules (see repos::transactions::
            // apply_verdict_to_matching) persist a counterparty verdict, not a
            // category — their category_id is "" and must never be written
            // to a transaction's category_id (empty string, and it would
            // violate transactions.category_id's FK to categories(id) besides
            // being semantically wrong). Only 'categorize' rules apply here;
            // repos::rules::apply_treatment_rules handles the other two.
            if r.treatment != "categorize" {
                return false;
            }
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
            // One atomic unit per transaction: the audit row, the canonical
            // column, and the proposal resolution must land together, or a
            // crash between them leaves a half-recorded categorization.
            tokio::task::spawn_blocking(move || {
                let mut conn = wdb.get()?;
                finsight_core::repos::atomic(&mut conn, |conn| {
                    categorizations::insert(conn, NewCategorization {
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
                    // Issue #87: this is a canonical write through a path that is
                    // NOT accept/correct/reject, so it must resolve any live
                    // proposal — same contract as `repos::transactions::update`.
                    // It matters in rerun mode (`RecategorizeLowConfidence`),
                    // where `load_low_confidence` re-selects rows that may still
                    // carry a pending proposal: a user who adds a merchant rule
                    // and hits "Re-check" is giving a deliberate, stronger signal
                    // than the stale LLM guess. Without this, the proposal stays
                    // `pending` and clicking Accept later REVERTS the user's own
                    // rule back to that guess. Unconditional (no `rerun_mode`
                    // branch) — `resolve_for_txn` already no-ops unless a pending
                    // proposal exists.
                    finsight_core::repos::category_proposals::resolve_for_txn(
                        conn,
                        &txn_id,
                        Some(&cat_id),
                    )
                })
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

    // Step 1.5: fastText local classifier (feature `fasttext-local`)
    // Tries to categorize `remaining` without an LLM round-trip. Threshold
    // is the per-task `llm_routing.fasttextThreshold` (Immich-style) when
    // set, else LOW_CONFIDENCE_THRESHOLD. Only predictions with
    // prob >= threshold and a known category slug are accepted.
    let remaining_for_llm: Vec<(String, String, i64)> = {
        #[cfg(feature = "fasttext-local")]
        {
            if remaining.is_empty() {
                Vec::new()
            } else {
                // Build slug -> category_id map (training labels are
                // lowercased category names, e.g. "groceries").
                let slug_to_id: HashMap<String, String> = categories
                    .iter()
                    .map(|(id, name, _, _)| (name.to_lowercase(), id.clone()))
                    .collect();
                let data_dir = std::env::var("FINSIGHT_DATA_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."));
                match crate::fasttext_predict::get_fasttext_model(&data_dir).await {
                    Ok(model) => {
                        let threshold: f64 = {
                            let conn = db.get().ok();
                            conn.as_ref().map(|c| routing_threshold(c)).unwrap_or(LOW_CONFIDENCE_THRESHOLD)
                        };
                        let mut accepted: Vec<(String, String, f64)> = Vec::new();
                        let mut fallthrough: Vec<(String, String, i64)> = Vec::new();
                        for (txn_id, merchant_raw, amount_cents) in remaining {
                            if superseded() {
                                return Ok(());
                            }
                            let text =
                                crate::fasttext_predict::merchant_text_for_model(
                                    &merchant_raw,
                                    amount_cents,
                                );
                            let pred = tokio::task::spawn_blocking({
                                let m = model.clone();
                                let t = text.clone();
                                move || m.predict(&t)
                            })
                            .await
                            .ok()
                            .flatten();
                            if let Some((slug, prob)) = pred {
                                let slug_norm = slug.trim_start_matches("__label__").to_lowercase();
                                if slug_norm == "__exclude" || slug_norm == "exclude" {
                                    fallthrough.push((txn_id, merchant_raw, amount_cents));
                                    continue;
                                }
                                if prob < threshold {
                                    fallthrough.push((txn_id, merchant_raw, amount_cents));
                                    continue;
                                }
                                if let Some(cat_id) = slug_to_id.get(&slug_norm) {
                                    accepted.push((txn_id, cat_id.clone(), prob));
                                } else {
                                    tracing::warn!(
                                        "[categorizer] fasttext unknown slug '{}' for '{}'",
                                        slug_norm,
                                        merchant_raw
                                    );
                                    fallthrough.push((txn_id, merchant_raw, amount_cents));
                                }
                            } else {
                                fallthrough.push((txn_id, merchant_raw, amount_cents));
                            }
                        }
                        // Bulk-write fastText acceptances (one txn per chunk pattern,
                        // but single atomic batch here is fine - all or nothing).
                        if !accepted.is_empty() {
                            let db_for_ft = db.clone();
                            let start_epoch_ft = start_epoch;
                            let lease = db.reset_barrier().writer_lease(start_epoch_ft).await;
                            if lease.superseded() {
                                return Ok(());
                            }
                            let write_ft = tokio::task::spawn_blocking(move || {
                                let mut conn = db_for_ft.get()?;
                                let tx = conn.transaction()?;
                                {
                                    let mut ins_cat = tx.prepare_cached(
                                        "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) VALUES(?1, ?2, ?3, 'fasttext', ?4, 'merchant_ft.bin', ?5)",
                                    )?;
                                    let mut upd_txn = tx.prepare_cached(
                                        "UPDATE transactions SET category_id = ?1, ai_confidence = ?2, ai_explanation = ?3 WHERE id = ?4",
                                    )?;
                                    for (txn_id, cat_id, prob) in &accepted {
                                        let cid = uuid::Uuid::new_v4().to_string();
                                        let now = chrono::Utc::now().to_rfc3339();
                                        let rationale = format!("fasttext {prob:.2}");
                                        ins_cat.execute(rusqlite::params![
                                            cid, txn_id, cat_id, prob, now
                                        ])?;
                                        upd_txn.execute(rusqlite::params![
                                            cat_id, prob, rationale, txn_id
                                        ])?;
                                        let prop_id = uuid::Uuid::new_v4().to_string();
                                        let candidates =
                                            json!([{"category_id": cat_id, "confidence": prob}])
                                                .to_string();
                                        let status = if *prob < threshold {
                                            "pending"
                                        } else {
                                            "accepted"
                                        };
                                        tx.execute(
                                            "INSERT INTO category_proposals (id, txn_id, proposed_category_id, source, confidence, rationale, candidates_json, status, applied, model, created_at, reviewed_at) VALUES (?1, ?2, ?3, 'fasttext', ?4, ?5, ?6, ?7, 1, 'merchant_ft.bin', ?8, NULL) ON CONFLICT(txn_id) DO UPDATE SET id=excluded.id, proposed_category_id=excluded.proposed_category_id, source=excluded.source, confidence=excluded.confidence, rationale=excluded.rationale, candidates_json=excluded.candidates_json, status=excluded.status, applied=excluded.applied, model=excluded.model, created_at=excluded.created_at, reviewed_at=NULL WHERE category_proposals.status='pending'",
                                            rusqlite::params![
                                                prop_id, txn_id, cat_id, prob, rationale, candidates, status, now
                                            ],
                                        )?;
                                    }
                                }
                                tx.commit()?;
                                Ok::<usize, anyhow::Error>(accepted.len())
                            })
                            .await;
                            match write_ft {
                                Ok(Ok(n)) => {
                                    categorized += n as u32;
                                    tracing::info!("[categorizer] fasttext categorized {n} txns");
                                    on_event(AgentEvent::CategorizationProgress {
                                        import_id: import_id.clone(),
                                        done: categorized,
                                        total,
                                    });
                                }
                                Ok(Err(e)) => {
                                    tracing::error!("[categorizer] fasttext write failed: {e}");
                                    // keep fallthrough only — accepted had no merchant_raw
                                    // to re-queue; they'll be retried as LLM on next run
                                    // if still uncategorized
                                }
                                Err(e) => {
                                    tracing::error!("[categorizer] fasttext join error: {e}");
                                }
                            }
                            drop(lease);
                        }
                        fallthrough
                    }
                    Err(e) => {
                        tracing::warn!("[categorizer] fasttext unavailable, using LLM: {e}");
                        remaining
                    }
                }
            }
        }
        #[cfg(not(feature = "fasttext-local"))]
        {
            remaining
        }
    };

    // Step 2: LLM batch pass (now over fastText fallthrough only)
    // Immich-style routing: if `llm_routing.categorization` is null/unconfigured,
    // we skip LLM entirely (deterministic: remaining stays uncategorized,
    // surfaced as "needs review" via threshold). This is the 90-95% token cut.
    let should_llm = {
        if let Ok(conn) = db.get() {
            if let Ok(Some(v)) = finsight_core::settings::get::<serde_json::Value>(&conn, "llm_routing") {
                // `categorization: null` or missing → skip LLM
                !v.get("categorization").map_or(true, |x| x.is_null())
            } else {
                true // no routing config → use passed provider (global default)
            }
        } else {
            true
        }
    };
    if !should_llm {
        tracing::info!("[categorizer] categorization LLM skipped by routing (deterministic)");
    } else {
        let system_prompt = build_system_prompt(&categories, &recent_examples);

        for chunk in remaining_for_llm.chunks(LLM_BATCH_SIZE) {

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
            let results: Vec<LlmResult> = serde_json::from_value(raw)?;
            Ok::<Vec<LlmResult>, anyhow::Error>(results)
        }
        .await;

        let results = match chunk_result {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("[categorizer] chunk failed, skipping: {e}");
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
        // Collect valid results first — category and txn validation outside the
        // transaction so only well-formed rows enter the atomic batch.
        let model_id = provider.model_id().to_string();
        let mut valid: Vec<(String, String, f64, String)> = Vec::new();
        for r in &results {
            if !valid_category_ids.contains(&r.category_id) {
                tracing::warn!(
                    "[categorizer] LLM returned unknown category_id '{}' for txn '{}', skipping",
                    r.category_id,
                    r.txn_id
                );
                continue;
            }
            if !chunk_txn_ids.contains(r.txn_id.as_str()) {
                tracing::warn!(
                    "[categorizer] LLM returned unknown txn_id '{}', skipping",
                    r.txn_id
                );
                continue;
            }
            valid.push((
                r.txn_id.clone(),
                r.category_id.clone(),
                r.confidence,
                r.rationale.clone(),
            ));
        }
        if !valid.is_empty() {
            let db_for_chunk = db.clone();
            let valid_for_task = valid.clone();
            let model_for_task = model_id.clone();
            // One transaction for the whole chunk — like repos/rules.rs:90-104.
            // A mid-batch failure rolls back the entire chunk rather than leaving
            let write_res = tokio::task::spawn_blocking(move || {
                let mut conn = db_for_chunk.get()?;
                let tx = conn.transaction()?;
                {
                    let mut insert_cat = tx.prepare_cached(
                        "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) VALUES(?1, ?2, ?3, 'llm', ?4, ?5, ?6)",
                    )?;
                    let mut update_txn = tx.prepare_cached(
                        "UPDATE transactions SET category_id = ?1, ai_confidence = ?2, ai_explanation = ?3 WHERE id = ?4",
                    )?;
                    for (txn_id, cat_id, confidence, rationale) in &valid_for_task {
                        let cid = uuid::Uuid::new_v4().to_string();
                        let now = chrono::Utc::now().to_rfc3339();
                        insert_cat.execute(rusqlite::params![cid, txn_id, cat_id, confidence, model_for_task.clone(), now])?;
                        update_txn.execute(rusqlite::params![cat_id, confidence, rationale, txn_id])?;
                        let status = if *confidence < LOW_CONFIDENCE_THRESHOLD { "pending" } else { "accepted" };
                        let prop_id = uuid::Uuid::new_v4().to_string();
                        let candidates = json!([{"category_id": cat_id.clone(), "confidence": confidence}]).to_string();
                        tx.execute(
                            "INSERT INTO category_proposals \
                             VALUES (?1, ?2, ?3, 'llm', ?4, ?5, ?6, ?7, 1, ?8, ?9, NULL) \
                             ON CONFLICT(txn_id) DO UPDATE SET \
                                id = excluded.id, \
                                proposed_category_id = excluded.proposed_category_id, \
                                source = excluded.source, \
                                confidence = excluded.confidence, \
                                rationale = excluded.rationale, \
                                candidates_json = excluded.candidates_json, \
                                status = excluded.status, \
                                applied = excluded.applied, \
                                model = excluded.model, \
                                created_at = excluded.created_at, \
                                reviewed_at = NULL \
                             WHERE category_proposals.status = 'pending'",
                            rusqlite::params![
                                prop_id,
                                txn_id,
                                cat_id,
                                confidence,
                                rationale,
                                candidates,
                                status,
                                model_for_task.clone(),
                                now
                            ],
                        )?;
                    }
                }
                tx.commit()?;
                Ok::<_, anyhow::Error>(valid_for_task.len() as u32)
            })
            .await;
            match write_res {
                Ok(Ok(n)) => categorized += n,
                Ok(Err(e)) => {
                    tracing::error!("[categorizer] chunk transaction failed, rolled back: {e}");
                    if let Ok(conn) = db.get() {
                        let mut warnings: Vec<String> =
                            finsight_core::settings::get(&conn, "data.agent_warnings")
                                .unwrap_or(None)
                                .unwrap_or_default();
                        warnings.push(format!("categorizer chunk failed: {e}"));
                        let _ =
                            finsight_core::settings::set(&conn, "data.agent_warnings", &warnings);
                    }
                }
                Err(e) => {
                    tracing::error!("[categorizer] chunk task join error: {e}");
                    if let Ok(conn) = db.get() {
                        let mut warnings: Vec<String> =
                            finsight_core::settings::get(&conn, "data.agent_warnings")
                                .unwrap_or(None)
                                .unwrap_or_default();
                        warnings.push(format!("categorizer join error: {e}"));
                        let _ =
                            finsight_core::settings::set(&conn, "data.agent_warnings", &warnings);
                    }
                }
            }
        }
        drop(lease);
        on_event(AgentEvent::CategorizationProgress {
            import_id: import_id.clone(),
            done: categorized,
            total,
        });
    }
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

    // Post-run: anomaly detection (best-effort — failures don't abort the scan,
    // but they must not vanish silently). Anomaly writes already honor the
    // ResetBarrier lease (Bound B exception — see anomaly::detect_anomalies).
    if let Err(e) = crate::anomaly::detect_anomalies(db, Arc::clone(&provider)).await {
        tracing::error!("[categorizer] post-scan anomaly detection failed: {e}");
        // Surface as a durable Inbox-style warning so the failure is visible
        // beyond the server log. Best-effort: a failure to record the warning
        // must not mask the scan result.
        if let Ok(conn) = db.get() {
            let mut warnings: Vec<String> =
                finsight_core::settings::get(&conn, "data.agent_warnings")
                    .unwrap_or(None)
                    .unwrap_or_default();
            warnings.push(format!("anomaly detection: {e}"));
            let _ = finsight_core::settings::set(&conn, "data.agent_warnings", &warnings);
        }
    }

    // Post-run: persist scan metadata for status ticker.
    {
        let db_outer = db.clone();
        let db_for_task = db.clone();
        let n = categorized;
        let res = tokio::task::spawn_blocking(move || {
            let conn = db_for_task.get()?;
            crate::anomaly::store_last_scan(&conn, n)?;
            Ok::<_, anyhow::Error>(())
        })
        .await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::error!("[categorizer] store_last_scan failed: {e}");
                if let Ok(conn) = db_outer.get() {
                    let mut warnings: Vec<String> =
                        finsight_core::settings::get(&conn, "data.agent_warnings")
                            .unwrap_or(None)
                            .unwrap_or_default();
                    warnings.push(format!("store_last_scan: {e}"));
                    let _ = finsight_core::settings::set(&conn, "data.agent_warnings", &warnings);
                }
            }
            Err(e) => {
                tracing::error!("[categorizer] store_last_scan join error: {e}");
                if let Ok(conn) = db_outer.get() {
                    let mut warnings: Vec<String> =
                        finsight_core::settings::get(&conn, "data.agent_warnings")
                            .unwrap_or(None)
                            .unwrap_or_default();
                    warnings.push(format!("store_last_scan join: {e}"));
                    let _ = finsight_core::settings::set(&conn, "data.agent_warnings", &warnings);
                }
            }
        }
    }

    Ok(())
}

// ── helpers ────────────────────────────────────────────────────────────────

/// The same population [`load_uncategorized`] selects, exposed for the centroid
/// proposal pass (issue #92).
///
/// Shared rather than reimplemented on purpose: the exclusions here are
/// invariants from epic #74 (never categorize a transfer, never an investment
/// row), and a second copy of that predicate is a second place for them to
/// drift out of sync. The semantic pass must be bound by exactly the same rules
/// as the LLM pass.
pub(crate) fn load_uncategorized_for_proposals(
    conn: &mut rusqlite::Connection,
) -> Result<Vec<(String, String, i64)>> {
    load_uncategorized(conn)
}

fn load_uncategorized(conn: &mut rusqlite::Connection) -> Result<Vec<(String, String, i64)>> {
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
    // The `NOT EXISTS` clause is an EFFICIENCY filter, not a correctness one.
    //
    // `category_proposals::upsert` already refuses to resurrect a resolved
    // proposal (accepted/corrected/rejected) back into `pending` — that guard
    // is what makes "reject" durable and it stays exactly as it is. But
    // `reject_category_proposal` deliberately does not clear
    // `transactions.ai_confidence` (a canonical-column mutation, explicitly
    // deferred), so a rejected transaction kept matching this query forever.
    //
    // Every re-check therefore re-sent it to the LLM, the model produced a
    // proposal, and `upsert` discarded it by design. The outcome was already
    // correct; what it cost was a round trip and real tokens on every re-check,
    // for every rejection, forever — scaling with how diligently the user
    // reviews, and billed directly to self-hosters on their own API key.
    //
    // Only `pending` is excluded from the exclusion: a transaction with no
    // proposal, or one still awaiting review, is genuinely re-checkable.
    let mut stmt = conn.prepare(
        "SELECT id, merchant_raw, amount_cents FROM transactions \
         WHERE ai_confidence IS NOT NULL AND ai_confidence < ?1 \
           AND (SELECT source FROM categorizations c \
                WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm' \
           AND NOT EXISTS ( \
                SELECT 1 FROM category_proposals p \
                WHERE p.txn_id = transactions.id AND p.status <> 'pending') \
         ORDER BY ai_confidence ASC, posted_at DESC",
    )?;
    let rows = stmt.query_map(rusqlite::params![LOW_CONFIDENCE_THRESHOLD], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
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
    use finsight_core::models::NewRule;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, finsight_core::Db) {
        let (dir, db) = finsight_core::testing::migrated_db();
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

    /// Issue #102: a rejected proposal must not be re-sent to the LLM.
    ///
    /// `reject_category_proposal` deliberately leaves `ai_confidence` alone, so
    /// a rejected transaction kept matching this loader forever. Every re-check
    /// re-sent it, the model answered, and `upsert`'s resurrection guard threw
    /// the answer away — correct, and paid for in tokens on every run.
    ///
    /// This asserts on `load_low_confidence`'s output rather than on the DB
    /// afterwards, exactly as the issue requires: the loader's return value IS
    /// the batch handed to the provider, and the end state was already correct
    /// before this fix, so a state assertion would pass either way and prove
    /// nothing.
    #[test]
    fn recheck_skips_resolved_proposals_but_keeps_pending_and_unproposed() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_db(&mut conn); // t1 CHIPOTLE, cat1, a1

        // Three low-confidence transactions whose latest categorization is the
        // LLM's — identical as far as the old query was concerned.
        for id in ["t_rejected", "t_pending", "t_none"] {
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at,ai_confidence) \
                 VALUES(?1,'a1','2024-02-01T00:00:00Z',-2200,'MYSTERY','cleared',0,'2024-02-01T00:00:00Z',0.30)",
                rusqlite::params![id],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES(?1,?2,'cat1','llm',0.30,'2024-02-01T00:00:00Z')",
                rusqlite::params![format!("c-{id}"), id],
            )
            .unwrap();
        }

        let mut propose = |txn: &str, status: &str| {
            finsight_core::repos::category_proposals::upsert(
                &mut conn,
                finsight_core::models::NewCategoryProposal {
                    txn_id: txn.to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.30,
                    rationale: None,
                    candidates_json: None,
                    status: status.to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();
        };
        propose("t_rejected", "rejected");
        propose("t_pending", "pending");
        // t_none deliberately has no proposal row at all.

        let batch = load_low_confidence(&mut conn).unwrap();
        let mut ids: Vec<String> = batch.into_iter().map(|(id, _, _)| id).collect();
        ids.sort();

        assert_eq!(
            ids,
            vec!["t_none".to_string(), "t_pending".to_string()],
            "a rejected proposal must not cost another LLM round trip, while a \
             pending one and an unproposed transaction are still genuinely re-checkable"
        );
    }

    /// The exclusion must key on "resolved", not on "has a proposal" — an
    /// accepted or corrected transaction is equally pointless to re-send, and
    /// over-filtering to `pending`-only would be the same bug in reverse.
    #[test]
    fn recheck_skips_accepted_and_corrected_proposals_too() {
        for status in ["accepted", "corrected"] {
            let (_d, db) = fresh_db();
            let mut conn = db.get().unwrap();
            seed_db(&mut conn);
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at,ai_confidence) \
                 VALUES('t2','a1','2024-02-01T00:00:00Z',-900,'MYSTERY','cleared',0,'2024-02-01T00:00:00Z',0.20)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES('c2','t2','cat1','llm',0.20,'2024-02-01T00:00:00Z')",
                [],
            )
            .unwrap();
            finsight_core::repos::category_proposals::upsert(
                &mut conn,
                finsight_core::models::NewCategoryProposal {
                    txn_id: "t2".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.20,
                    rationale: None,
                    candidates_json: None,
                    status: status.to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();

            let ids: Vec<String> = load_low_confidence(&mut conn)
                .unwrap()
                .into_iter()
                .map(|(id, _, _)| id)
                .collect();
            assert!(
                !ids.contains(&"t2".to_string()),
                "a '{status}' proposal is resolved — re-sending it buys nothing"
            );
        }
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

    // ── Issue #87 (Slice 1): proposal + provenance foundation ───────────────

    #[tokio::test]
    async fn llm_pass_emits_a_pending_proposal_below_threshold_and_auto_accepts_above() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE + cat1 + account
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES('t2','a1','2024-01-16T00:00:00Z',900,'STARBUCKS','cleared',0,'2024-01-16T00:00:00Z')",
                [],
            ).unwrap();
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([
                {"txn_id": "t1", "category_id": "cat1", "confidence": 0.4, "rationale": "maybe food"},
                {"txn_id": "t2", "category_id": "cat1", "confidence": 0.9, "rationale": "clearly coffee"}
            ]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let mut conn = db.get().unwrap();
        // Below threshold: canonical is still written (additive), but the
        // proposal is "pending" — this is the row the review queue reads.
        let low = finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t1")
            .unwrap()
            .expect("a proposal was recorded for the low-confidence write");
        assert_eq!(low.status, "pending");
        assert!(
            low.applied,
            "the LLM pass still auto-writes canonical today"
        );
        assert_eq!(low.proposed_category_id, "cat1");
        assert_eq!(low.source, "llm");
        assert!(low.reviewed_at.is_none());
        let t1_cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            t1_cat.as_deref(),
            Some("cat1"),
            "additive: canonical write happens regardless of confidence"
        );

        // At/above threshold: auto-accepted, not sitting in the review queue,
        // and NOT a human decision (reviewed_at stays NULL).
        let high = finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t2")
            .unwrap()
            .expect("a proposal was recorded for the high-confidence write too");
        assert_eq!(high.status, "accepted");
        assert!(
            high.reviewed_at.is_none(),
            "auto-accept is not a human review"
        );

        assert_eq!(
            finsight_core::repos::category_proposals::count(&mut conn, "pending").unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn rule_pass_does_not_emit_a_category_proposal() {
        // Only the LLM pass is in scope for issue #87 — the rule pass's
        // canonical write is unaccompanied by a proposal row.
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
        assert!(
            finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t1")
                .unwrap()
                .is_none(),
            "the rule pass must not create a category_proposals row"
        );
    }

    #[tokio::test]
    async fn transfers_are_never_proposed_by_the_llm_pass() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE (not a transfer)
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,is_transfer,created_at) \
                 VALUES('t2','a1','2024-02-01T00:00:00Z',298614,'PAYMENT RECEIVED - THANK YOU','cleared',0,1,'2024-02-01T00:00:00Z')",
                [],
            ).unwrap();
            // A user-confirmed transfer verdict (transfer_override) on a third row.
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,is_transfer,transfer_override,created_at) \
                 VALUES('t3','a1','2024-02-02T00:00:00Z',-50000,'INTERNET TRANSFER 123','cleared',0,1,1,'2024-02-02T00:00:00Z')",
                [],
            ).unwrap();
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([
                {"txn_id": "t1", "category_id": "cat1", "confidence": 0.9, "rationale": "Fast food"},
                {"txn_id": "t2", "category_id": "cat1", "confidence": 0.8, "rationale": "guessed"},
                {"txn_id": "t3", "category_id": "cat1", "confidence": 0.8, "rationale": "guessed"}
            ]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let mut conn = db.get().unwrap();
        for txn_id in ["t2", "t3"] {
            assert!(
                finsight_core::repos::category_proposals::get_for_txn(&mut conn, txn_id)
                    .unwrap()
                    .is_none(),
                "no proposal must ever be recorded for a transfer/transfer_override row ({txn_id})"
            );
        }
        // The real spending txn still got proposed.
        assert!(
            finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t1")
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn investment_rows_are_never_proposed() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE + cat1 + checking account a1
            conn.execute(
                "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
                 VALUES('inv','Me','Brokerage','Investment','Brokerage','USD','#000','manual','2024-01-01T00:00:00Z')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,activity_type,created_at) \
                 VALUES('t2','inv','2024-02-01T00:00:00Z',-100000,'BUY VTI','cleared',0,'Trade','2024-02-01T00:00:00Z')",
                [],
            ).unwrap();
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([
                {"txn_id": "t1", "category_id": "cat1", "confidence": 0.9, "rationale": "Fast food"},
                {"txn_id": "t2", "category_id": "cat1", "confidence": 0.8, "rationale": "guessed"}
            ]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let mut conn = db.get().unwrap();
        assert!(
            finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t2")
                .unwrap()
                .is_none(),
            "an investment-account trade must never get a category proposal"
        );
    }

    #[tokio::test]
    async fn user_categorized_rows_are_never_reproposed() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE + cat1
            conn.execute(
                "UPDATE transactions SET category_id='cat1' WHERE id='t1'",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES('c1','t1','cat1','user',1.0,'2024-01-16T00:00:00Z')",
                [],
            )
            .unwrap();
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            // Even if the model tried to weigh in, t1 is excluded from the
            // uncategorized batch sent to it (category_id is already set).
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.4, "rationale": "guess"}]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let mut conn = db.get().unwrap();
        assert!(
            finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t1")
                .unwrap()
                .is_none(),
            "a user-set category must never be shadowed by a fresh AI proposal"
        );
    }

    #[tokio::test]
    async fn abstains_on_archived_category_no_dangling_proposal() {
        // An archived category is excluded from `valid_category_ids`, so an
        // LLM response naming it is treated exactly like a hallucinated id:
        // skipped, with neither a canonical write nor a proposal row.
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn);
            conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat-archived','g1','Old','#000',1)", []).unwrap();
            conn.execute(
                "UPDATE categories SET archived_at = '2024-01-01T00:00:00Z' WHERE id = 'cat-archived'",
                [],
            )
            .unwrap();
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat-archived", "confidence": 0.9, "rationale": "guess"}]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let mut conn = db.get().unwrap();
        let cat_id: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            cat_id.is_none(),
            "must abstain rather than write an archived category"
        );
        assert!(
            finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t1")
                .unwrap()
                .is_none(),
            "must abstain rather than leave a dangling proposal referencing an archived category"
        );
    }

    /// Regression (review finding 1a), end to end: reject → "Re-check" must
    /// stay rejected.
    ///
    /// `reject_category_proposal` deliberately does not touch
    /// `transactions.ai_confidence`, so a rejected row used to be re-selected
    /// and re-proposed by every `RecategorizeLowConfidence` run — which the
    /// Inbox action item's own copy invites the user to trigger — with the
    /// `upsert` guard the only thing standing between that and a silently
    /// erased rejection.
    ///
    /// Issue #102 added the `NOT EXISTS` filter in `load_low_confidence`, so
    /// the rejected row is no longer selected in the first place. The end
    /// result this test protects is unchanged, but it now holds for a stronger
    /// reason — the LLM is never asked — and the assertion below flipped
    /// accordingly: `ai_confidence` must stay at the ORIGINAL 0.4, because the
    /// re-check no longer re-processes t1 at all.
    ///
    /// The `upsert` resurrection guard remains the correctness backstop and is
    /// still regression-tested directly, independently of this pipeline, by
    /// `finsight_core::repos::category_proposals::tests::
    /// upsert_does_not_resurrect_a_resolved_proposal`. This filter is an
    /// efficiency layer in front of it, never a replacement.
    #[tokio::test]
    async fn a_rejected_proposal_survives_a_recheck_that_reproposes_it() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE + cat1 + a1 (no rules -> LLM path)
        }
        // First pass: low confidence -> pending proposal.
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.4, "rationale": "maybe food"}]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        // The user rejects it. This is exactly what
        // `commands::category_proposals::reject_category_proposal` does:
        // resolve the proposal, leave canonical (and ai_confidence) alone.
        let (proposal_id, rejected_at) = {
            let mut conn = db.get().unwrap();
            let p = finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t1")
                .unwrap()
                .unwrap();
            assert_eq!(p.status, "pending");
            finsight_core::repos::category_proposals::set_status(&mut conn, &p.id, "rejected")
                .unwrap();
            let after = finsight_core::repos::category_proposals::get(&mut conn, &p.id)
                .unwrap()
                .unwrap();
            (p.id, after.reviewed_at)
        };

        // "Re-check": the LLM re-proposes the very same category at a
        // slightly different confidence.
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.45, "rationale": "still maybe food"}]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(
            &db,
            AgentJob::RecategorizeLowConfidence,
            provider,
            Arc::new(|_| {}),
        )
        .await
        .unwrap();

        let mut conn = db.get().unwrap();
        // Issue #102: t1 must NOT have been re-processed. The mock provider
        // would have written 0.45; seeing the original 0.4 is the proof that
        // the rejected row never entered the batch, so no tokens were spent
        // producing a proposal that `upsert` would only have discarded.
        let conf: Option<f64> = conn
            .query_row(
                "SELECT ai_confidence FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            (conf.unwrap() - 0.4).abs() < 1e-9,
            "a rejected row must not be re-sent to the LLM on re-check (got {:?}; \
             0.45 would mean it was re-processed and the answer thrown away)",
            conf
        );

        // …and yet the rejection is fully intact.
        let after = finsight_core::repos::category_proposals::get(&mut conn, &proposal_id)
            .unwrap()
            .expect("the rejected row was not replaced by a fresh one");
        assert_eq!(
            after.status, "rejected",
            "a re-proposal must not resurrect a rejection"
        );
        assert_eq!(
            after.reviewed_at, rejected_at,
            "the human decision timestamp must not be cleared or re-stamped"
        );
        assert_eq!(
            finsight_core::repos::category_proposals::count(&mut conn, "pending").unwrap(),
            0,
            "the rejected transaction must not reappear in the review queue"
        );
    }

    /// Regression (review finding 1b): the rule pass in RERUN mode writes
    /// canonical directly, and must resolve the live proposal.
    ///
    /// The scenario: t1 has a pending proposal for cat1 ("Food"). The user
    /// adds a merchant rule mapping CHIPOTLE -> cat2 — a deliberate, stronger
    /// signal than a 0.4-confidence guess — and hits "Re-check".
    /// `load_low_confidence` re-selects t1 (it has no `category_id IS NULL`
    /// filter), Step 1's rule pass matches and writes cat2 + nulls
    /// `ai_confidence`. Without `resolve_for_txn` the proposal stayed
    /// `pending` with `proposed_category_id = cat1`, so t1 lingered in the
    /// review queue and clicking Accept reverted the user's own rule.
    ///
    /// This is also a genuine parity gap that
    /// `needs_review_population_matches_the_legacy_predicate_exactly` does
    /// not cover: that test only exercises a `transactions::update`-driven
    /// correction, never a rule-pass one in rerun mode. Under the legacy
    /// `ai_confidence`-based predicate this row drops out; under the
    /// proposal-backed one it would have stayed.
    #[tokio::test]
    async fn rule_pass_in_rerun_mode_resolves_the_pending_proposal_it_overwrites() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE + cat1 + a1
            conn.execute(
                "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat2','g1','Groceries','#0f0',1)",
                [],
            )
            .unwrap();
        }
        // First pass: no rules, LLM proposes cat1 at low confidence.
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.4, "rationale": "maybe food"}]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        let proposal_id = {
            let mut conn = db.get().unwrap();
            let p = finsight_core::repos::category_proposals::get_for_txn(&mut conn, "t1")
                .unwrap()
                .unwrap();
            assert_eq!(p.status, "pending");
            assert_eq!(p.proposed_category_id, "cat1");
            // The user now teaches the app the real answer.
            rules::insert(
                &mut conn,
                NewRule {
                    pattern: "CHIPOTLE".to_string(),
                    category_id: "cat2".to_string(),
                    source: "user".to_string(),
                    treatment: "categorize".to_string(),
                },
            )
            .unwrap();
            p.id
        };

        // "Re-check". The rule matches in Step 1, so `remaining` is empty and
        // the LLM is never consulted — hence the empty mock response.
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(
            &db,
            AgentJob::RecategorizeLowConfidence,
            provider,
            Arc::new(|_| {}),
        )
        .await
        .unwrap();

        let mut conn = db.get().unwrap();
        let (cat, conf): (Option<String>, Option<f64>) = conn
            .query_row(
                "SELECT category_id, ai_confidence FROM transactions WHERE id='t1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            cat.as_deref(),
            Some("cat2"),
            "the rule wins the canonical write"
        );
        assert_eq!(conf, None, "the rule pass nulls the LLM confidence");

        let after = finsight_core::repos::category_proposals::get(&mut conn, &proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            after.status, "corrected",
            "the rule's canonical write must resolve the proposal it overwrote"
        );
        assert!(after.reviewed_at.is_some());
        assert_eq!(
            finsight_core::repos::category_proposals::count(&mut conn, "pending").unwrap(),
            0,
            "accepting a stale proposal must not be able to revert the user's rule"
        );

        // Parity with the legacy predicate on THIS shape (the one the existing
        // parity test never exercises): ai_confidence is now NULL, so the old
        // query drops t1 — and so must the proposal-backed one.
        let legacy: Vec<String> = conn
            .prepare(
                "SELECT id FROM transactions \
                 WHERE ai_confidence IS NOT NULL AND ai_confidence < 0.6 \
                   AND (SELECT source FROM categorizations c \
                        WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm'",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            legacy.is_empty(),
            "sanity: the legacy predicate drops a rule-corrected row"
        );
        let listed = finsight_core::repos::transactions::list(
            &mut conn,
            finsight_core::repos::transactions::TxnFilter {
                filter_preset: Some("needs_review".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            listed.is_empty(),
            "the needs_review screen must agree with the legacy predicate on this shape too"
        );
    }

    /// Acceptance criterion #2, the load-bearing one: the repointed
    /// `category_proposals`-backed review population must be a ROW-FOR-ROW
    /// match for today's `ai_confidence IS NOT NULL AND ai_confidence < 0.6
    /// AND latest source = 'llm'` predicate, on a realistic mix of sources —
    /// not just same-shape, same-COUNT.
    #[tokio::test]
    async fn needs_review_population_matches_the_legacy_predicate_exactly() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut conn); // t1 CHIPOTLE + cat1 + a1 (t1 reused as the low-confidence LLM row below)
                                // t2: rule-categorized (source='rule'), never touches the LLM.
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,created_at) \
                 VALUES('t2','a1','2024-01-02T00:00:00Z',-1200,'NETFLIX','cat1','cleared',0,'2024-01-02T00:00:00Z')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES('rc1','t2','cat1','rule',1.0,'2024-01-02T00:00:00Z')",
                [],
            )
            .unwrap();
            // t3: user-categorized directly (source='user'), never touched by AI.
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,created_at) \
                 VALUES('t3','a1','2024-01-03T00:00:00Z',-1500,'RENT','cat1','cleared',0,'2024-01-03T00:00:00Z')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES('uc1','t3','cat1','user',1.0,'2024-01-03T00:00:00Z')",
                [],
            )
            .unwrap();
            // t4: builtin-source categorization (crates/finsight-core/src/
            // categorize.rs writes source='builtin', confidence 1.0) — never
            // an LLM decision, so never in the review queue.
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,created_at) \
                 VALUES('t4','a1','2024-01-04T00:00:00Z',-2200,'HYDRO ONE','cat1','cleared',0,'2024-01-04T00:00:00Z')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                 VALUES('bc1','t4','cat1','builtin',1.0,'2024-01-04T00:00:00Z')",
                [],
            )
            .unwrap();
            // t5: will be LLM-categorized at low confidence, THEN manually
            // corrected via the ordinary edit path — the sneaky case: it must
            // drop out of the review population entirely.
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES('t5','a1','2024-01-05T00:00:00Z',-800,'MYSTERY SHOP','cleared',0,'2024-01-05T00:00:00Z')",
                [],
            ).unwrap();
            // t6: LLM-categorized ABOVE the threshold — the AI touched it, but
            // confidently, so it must NOT be in the review queue. This is the
            // row that catches the "review count balloons to everything the
            // LLM ever touched" regression.
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES('t6','a1','2024-01-06T00:00:00Z',-1100,'OBVIOUS GROCER','cleared',0,'2024-01-06T00:00:00Z')",
                [],
            ).unwrap();
        }

        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([
                {"txn_id": "t1", "category_id": "cat1", "confidence": 0.4, "rationale": "low"},
                {"txn_id": "t5", "category_id": "cat1", "confidence": 0.35, "rationale": "low"},
                {"txn_id": "t6", "category_id": "cat1", "confidence": 0.95, "rationale": "high"}
            ]),
            tool_turns: Mutex::new(vec![]),
        });
        run_job(&db, AgentJob::CategorizeAll, provider, Arc::new(|_| {}))
            .await
            .unwrap();

        // Now the user manually recategorizes t5 through the ordinary edit
        // path (repos::transactions::update), same as the transaction drawer.
        {
            let mut conn = db.get().unwrap();
            finsight_core::repos::transactions::update(
                &mut conn,
                "t5",
                finsight_core::models::TxnPatch {
                    category_id: Some(Some("cat1".to_string())),
                    ..Default::default()
                },
            )
            .unwrap();
        }

        let mut conn = db.get().unwrap();

        // The OLD predicate, computed directly against the same seeded data.
        let mut legacy_stmt = conn
            .prepare(
                "SELECT id FROM transactions \
                 WHERE ai_confidence IS NOT NULL AND ai_confidence < 0.6 \
                   AND (SELECT source FROM categorizations c \
                        WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm' \
                 ORDER BY id",
            )
            .unwrap();
        let legacy: Vec<String> = legacy_stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        drop(legacy_stmt);

        // The NEW predicate: category_proposals.status = 'pending'.
        let mut new_stmt = conn
            .prepare(
                "SELECT txn_id FROM category_proposals WHERE status = 'pending' ORDER BY txn_id",
            )
            .unwrap();
        let fresh: Vec<String> = new_stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        drop(new_stmt);

        // The seeded mix, and why exactly one row qualifies:
        //   t1  LLM @ 0.40  (below threshold, untouched since)  -> IN
        //   t2  rule source, confidence 1.0                     -> out
        //   t3  user source, confidence 1.0                     -> out
        //   t4  builtin source, confidence 1.0                  -> out
        //   t5  LLM @ 0.35 then user-corrected via `update`     -> out
        //   t6  LLM @ 0.95  (above threshold)                   -> out
        assert_eq!(
            legacy,
            vec!["t1".to_string()],
            "sanity: only t1 matches the legacy predicate (t5 corrected, t6 above threshold, t2/t3/t4 never an LLM decision)"
        );
        assert_eq!(
            fresh, legacy,
            "the proposal-backed review population must exactly match the legacy predicate, row for row"
        );

        // And the repos::transactions::list "needs_review" preset (what the
        // Transactions screen actually queries) agrees too.
        let listed = finsight_core::repos::transactions::list(
            &mut conn,
            finsight_core::repos::transactions::TxnFilter {
                filter_preset: Some("needs_review".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let listed_ids: Vec<String> = listed.into_iter().map(|t| t.id).collect();
        assert_eq!(listed_ids, vec!["t1".to_string()]);

        // …as does `commands::agent::get_needs_review_count`'s underlying
        // count (the badge) — one population, three surfaces.
        assert_eq!(
            finsight_core::repos::category_proposals::count(&mut conn, "pending").unwrap(),
            legacy.len() as i64
        );
    }
}
