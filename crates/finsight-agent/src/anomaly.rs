use crate::CompletionProvider;
use anyhow::Result;
use finsight_core::{settings, Db};
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

const BATCH_SIZE: usize = 20;

struct Candidate {
    txn_id: String,
    merchant_raw: String,
    amount_cents: i64,
    median_cents: i64,
    p75_cents: i64,
}

#[derive(Deserialize)]
struct LlmAnomalyResult {
    txn_id: String,
    is_anomaly: bool,
    reason: String,
}

/// Detect anomalous transactions using a two-phase approach:
///
/// 1. **Statistical pre-filter (IQR):** For each merchant with ≥ 3 historical
///    transactions, compute Q1/Q3 of `abs(amount_cents)`. Flag candidates where
///    `abs(amount) > Q3 + 1.5 * IQR`. Merchants with < 3 transactions are skipped.
///
/// 2. **LLM confirmation:** Send candidates to the LLM in batches with their
///    historical baseline. The LLM returns `is_anomaly` + a human-readable reason.
///    Only transactions confirmed by the LLM get `is_anomaly = 1`.
///
/// Evaluates all recent (≤ 90 days) transactions not yet flagged as anomalies.
/// Returns the number of anomalies flagged.
pub async fn detect_anomalies(db: &Db, provider: Arc<dyn CompletionProvider>) -> Result<u32> {
    // ── Step 1: statistical pre-filter ──────────────────────────────────────
    let candidates: Vec<Candidate> = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.get()?;
            find_statistical_candidates(&conn)
        })
        .await??
    };

    if candidates.is_empty() {
        return Ok(0);
    }

    // ── Step 2: LLM confirmation in batches ──────────────────────────────────
    let mut confirmed: Vec<(String, String)> = Vec::new(); // (txn_id, reason)

    let system = "You are a transaction anomaly reviewer for a personal finance app. \
You will receive a list of transactions that look statistically unusual compared to \
the user's history with each merchant. Decide which are genuinely anomalous (e.g. \
a much larger charge than usual, a duplicate, or a clear outlier). \
Respond with a valid JSON array only — no markdown, no explanation outside the array. \
Each item: {\"txn_id\": \"...\", \"is_anomaly\": true/false, \"reason\": \"one sentence\"}";

    for chunk in candidates.chunks(BATCH_SIZE) {
        let items: Vec<_> = chunk
            .iter()
            .map(|c| {
                json!({
                    "txn_id": c.txn_id,
                    "merchant_raw": c.merchant_raw,
                    "amount_cents": c.amount_cents,
                    "historical_median_cents": c.median_cents,
                    "historical_p75_cents": c.p75_cents,
                })
            })
            .collect();

        let user = format!(
            "Review these transactions for anomalies:\n{}\n\n\
             Respond:\n[{{\"txn_id\":\"...\",\"is_anomaly\":true,\"reason\":\"...\"}}]",
            json!(items)
        );

        let raw = provider.complete_json(system, &user).await?;
        let results: Vec<LlmAnomalyResult> = serde_json::from_value(raw)?;
        for r in results {
            if r.is_anomaly {
                confirmed.push((r.txn_id, r.reason));
            }
        }
    }

    let count = confirmed.len() as u32;

    // ── Step 3: write results ────────────────────────────────────────────────
    if !confirmed.is_empty() {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.get()?;
            for (txn_id, reason) in &confirmed {
                conn.execute(
                    "UPDATE transactions SET is_anomaly = 1, ai_explanation = ?1 WHERE id = ?2",
                    params![reason, txn_id],
                )?;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
    }

    Ok(count)
}

/// Store last scan metadata in settings KV after a completed categorization run.
pub fn store_last_scan(
    conn: &rusqlite::Connection,
    categorized: u32,
) -> finsight_core::error::CoreResult<()> {
    let now = chrono::Utc::now().to_rfc3339();
    settings::set(conn, "agent.last_scan_at", &now)?;
    settings::set(conn, "agent.last_scan_categorized", &(categorized as i64))?;
    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn find_statistical_candidates(conn: &rusqlite::Connection) -> Result<Vec<Candidate>> {
    // Fetch recent, not-yet-flagged transactions.
    let txns: Vec<(String, String, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT id, merchant_raw, amount_cents \
             FROM transactions \
             WHERE posted_at >= date('now', '-90 days') AND is_anomaly = 0",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut candidates = Vec::new();

    for (txn_id, merchant_raw, amount_cents) in txns {
        let abs_amount = amount_cents.unsigned_abs() as i64;

        // Get historical amounts for this merchant (all except this transaction).
        let mut hist_stmt = conn.prepare(
            "SELECT ABS(amount_cents) FROM transactions \
             WHERE merchant_raw = ?1 AND id != ?2 \
             ORDER BY ABS(amount_cents)",
        )?;
        let hist: Vec<i64> = hist_stmt
            .query_map(params![merchant_raw, txn_id], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        if hist.len() < 3 {
            continue; // not enough baseline
        }

        let n = hist.len();
        let q1 = hist[n / 4];
        let q3 = hist[(3 * n) / 4];
        let median = hist[n / 2];
        let iqr = q3 - q1;
        let upper_fence = q3 + (iqr * 3 / 2); // Q3 + 1.5 * IQR

        if abs_amount > upper_fence && iqr > 0 {
            candidates.push(Candidate {
                txn_id,
                merchant_raw,
                amount_cents,
                median_cents: median,
                p75_cents: q3,
            });
        }
    }

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockCompletionProvider;
    use finsight_core::{db::run_migrations, keychain};
    use serde_json::json;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, finsight_core::Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("an.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_merchant(
        conn: &mut rusqlite::Connection,
        account_id: &str,
        merchant: &str,
        amounts: &[i64],
    ) {
        conn.execute(
            "INSERT OR IGNORE INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES(?1,'Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')",
            params![account_id],
        ).unwrap();
        for (i, &amt) in amounts.iter().enumerate() {
            let id = format!("{account_id}-{merchant}-{i}");
            // Use recent dates so they fall within the 90-day detection window
            let days_ago = (i as i64 + 1) * 10;
            let posted = format!("date('now', '-{days_ago} days')");
            let sql = format!(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES(?1,?2,{posted},?3,?4,'cleared',0,{posted})"
            );
            conn.execute(&sql, params![id, account_id, amt, merchant])
                .unwrap();
        }
    }

    #[tokio::test]
    async fn flags_outlier_when_llm_confirms() {
        let (_d, db) = fresh_db();
        let outlier_id;
        {
            let mut conn = db.get().unwrap();
            // 4 normal transactions at ~$15, plus one $200 outlier
            seed_merchant(&mut conn, "a1", "COSTCO", &[-1500, -1600, -1400, -1550]);
            // Insert the outlier separately so we know its ID
            outlier_id = "outlier-1".to_string();
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES(?1,'a1',date('now', '-1 days'),-20000,'COSTCO','cleared',0,date('now', '-1 days'))",
                params![outlier_id],
            ).unwrap();
        }

        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([{"txn_id": "outlier-1", "is_anomaly": true, "reason": "Unusually large charge"}]),
            tool_turns: Mutex::new(vec![]),
        });

        let _count = detect_anomalies(&db, provider).await.unwrap();

        let conn = db.get().unwrap();
        let is_anomaly: i64 = conn
            .query_row(
                "SELECT is_anomaly FROM transactions WHERE id='outlier-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_anomaly, 1);
    }

    #[tokio::test]
    async fn skips_sparse_merchant() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            // Only 2 transactions — below the 3-occurrence threshold
            seed_merchant(&mut conn, "a2", "RARE_STORE", &[-5000, -50000]);
        }

        // Mock that would confirm anomaly if called — but it shouldn't be called
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([{"txn_id": "a2-RARE_STORE-1", "is_anomaly": true, "reason": "test"}]),
            tool_turns: Mutex::new(vec![]),
        });

        let count = detect_anomalies(&db, provider).await.unwrap();
        assert_eq!(count, 0, "sparse merchant should not produce anomalies");
    }

    #[test]
    fn store_last_scan_writes_settings() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        store_last_scan(&conn, 42).unwrap();

        let at: Option<String> = settings::get(&conn, "agent.last_scan_at").unwrap();
        let n: Option<i64> = settings::get(&conn, "agent.last_scan_categorized").unwrap();
        assert!(at.is_some());
        assert_eq!(n, Some(42));
    }
}
