use crate::{
    agent::{AgentEvent, AgentJob, EventCallback},
    CompletionProvider,
};
use anyhow::Result;
use finsight_core::{
    models::NewCategorization,
    repos::{categorizations, rules},
    Db,
};
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

const LLM_BATCH_SIZE: usize = 20;

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
    let import_id = match &job {
        AgentJob::CategorizeImport { import_id } => Some(import_id.clone()),
        AgentJob::CategorizeAll => None,
    };

    // Load data needed for categorization on a blocking thread
    let (uncategorized, active_rules, categories, recent_examples) = {
        let db = db.clone();
        let import_id_clone = import_id.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let uncategorized = load_uncategorized(&mut *conn, import_id_clone.as_deref())?;
            let active_rules = rules::list_active(&mut *conn)?;
            let categories = load_categories(&mut *conn)?;
            let recent_examples = load_recent_examples(&mut *conn)?;
            Ok::<_, anyhow::Error>((uncategorized, active_rules, categories, recent_examples))
        })
        .await??
    };

    let total = uncategorized.len() as u32;
    let mut remaining: Vec<(String, String, i64)> = Vec::new(); // (txn_id, merchant_raw, amount_cents)
    let mut categorized: u32 = 0;

    // Step 1: Rule pass
    for (txn_id, merchant_raw, amount_cents) in &uncategorized {
        let matched = active_rules.iter().find(|r| {
            let pat = r.pattern.to_lowercase();
            let merch = merchant_raw.to_lowercase();
            // Simple LIKE: leading/trailing % = contains, otherwise exact
            if pat.starts_with('%') && pat.ends_with('%') {
                merch.contains(&pat[1..pat.len()-1])
            } else if pat.starts_with('%') {
                merch.ends_with(&pat[1..])
            } else if pat.ends_with('%') {
                merch.starts_with(&pat[..pat.len()-1])
            } else {
                merch == pat
            }
        });

        if let Some(rule) = matched {
            let cat_id = rule.category_id.clone();
            let txn_id = txn_id.clone();
            let db = db.clone();
            tokio::task::spawn_blocking(move || {
                let mut conn = db.get()?;
                categorizations::insert(&mut *conn, NewCategorization {
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
        let user_prompt = build_user_prompt(chunk);
        let raw = provider.complete_json(&system_prompt, &user_prompt).await?;
        // All three provider impls (Ollama, OpenAiCompat, Anthropic) return a flat JSON array.
        let results: Vec<LlmResult> = serde_json::from_value(raw)?;

        for r in &results {
            let txn_id = r.txn_id.clone();
            let cat_id = r.category_id.clone();
            let confidence = r.confidence;
            let rationale = r.rationale.clone();
            let model = provider.model_id().to_string();
            let db = db.clone();
            tokio::task::spawn_blocking(move || {
                let mut conn = db.get()?;
                categorizations::insert(&mut *conn, NewCategorization {
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
            }).await??;
            categorized += 1;
        }
        on_event(AgentEvent::CategorizationProgress {
            import_id: import_id.clone(),
            done: categorized,
            total,
        });
    }

    let final_skipped = total.saturating_sub(categorized);
    on_event(AgentEvent::CategorizationComplete {
        import_id: import_id.clone(),
        categorized,
        skipped: final_skipped,
    });

    Ok(())
}

// ── helpers ────────────────────────────────────────────────────────────────

fn load_uncategorized(
    conn: &mut rusqlite::Connection,
    _import_id: Option<&str>,
) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT id, merchant_raw, amount_cents FROM transactions \
         WHERE category_id IS NULL ORDER BY posted_at DESC",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

fn load_categories(conn: &mut rusqlite::Connection) -> Result<Vec<(String, String, String)>> {
    // (id, label, group_label)
    let mut stmt = conn.prepare(
        "SELECT c.id, c.label, COALESCE(g.label, '') \
         FROM categories c LEFT JOIN category_groups g ON g.id = c.group_id \
         WHERE c.archived_at IS NULL ORDER BY g.sort_order, c.sort_order",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
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
    for r in rows { out.push(r?); }
    Ok(out)
}

fn build_system_prompt(
    categories: &[(String, String, String)],
    recent_examples: &[(String, String)],
) -> String {
    let cats_json = json!(categories.iter().map(|(id, label, group)| {
        json!({"id": id, "label": label, "group_label": group})
    }).collect::<Vec<_>>());
    let examples_json = json!(recent_examples.iter().map(|(merchant, cat)| {
        json!({"merchant_raw": merchant, "category_label": cat})
    }).collect::<Vec<_>>());
    format!(
        "You are a personal finance transaction categorizer. Classify each transaction into \
         exactly one of the provided categories. Respond with a valid JSON array only — \
         no markdown, no explanation outside the array.\n\nCategories:\n{}\n\nRecent examples from this user (for calibration):\n{}",
        cats_json, examples_json
    )
}

fn build_user_prompt(txns: &[(String, String, i64)]) -> String {
    let items: Vec<_> = txns.iter().map(|(id, merchant, amount)| {
        json!({"txn_id": id, "merchant_raw": merchant, "amount_cents": amount})
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
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','Daily',0)", []).unwrap();
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
            seed_db(&mut *conn);
            rules::insert(&mut *conn, NewRule {
                pattern: "CHIPOTLE".to_string(),
                category_id: "cat1".to_string(),
                source: "user".to_string(),
            }).unwrap();
        }
        let events: Arc<Mutex<Vec<AgentEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([]),
        });
        run_job(
            &db,
            AgentJob::CategorizeAll,
            provider,
            Arc::new(move |e| { events_clone.lock().unwrap().push(e); }),
        ).await.unwrap();

        let conn = db.get().unwrap();
        let cat_id: Option<String> = conn.query_row(
            "SELECT category_id FROM transactions WHERE id='t1'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(cat_id.as_deref(), Some("cat1"));
    }

    #[tokio::test]
    async fn llm_pass_writes_category_and_ai_confidence() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_db(&mut *conn);
            // No rules — forces LLM path
        }
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "gpt-test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.87, "rationale": "Fast food"}]),
        });
        run_job(
            &db,
            AgentJob::CategorizeAll,
            provider,
            Arc::new(|_| {}),
        ).await.unwrap();

        let conn = db.get().unwrap();
        let (cat_id, confidence): (Option<String>, Option<f64>) = conn.query_row(
            "SELECT category_id, ai_confidence FROM transactions WHERE id='t1'",
            [], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(cat_id.as_deref(), Some("cat1"));
        assert!((confidence.unwrap() - 0.87).abs() < 0.01);
    }
}
