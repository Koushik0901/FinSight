use crate::CompletionProvider;
use anyhow::Result;
use finsight_core::{
    anomaly::{statistical_outlier_candidates, AnomalyCandidate},
    settings, Db,
};
use rusqlite::params;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

const BATCH_SIZE: usize = 20;

#[derive(Deserialize)]
struct LlmAnomalyResult {
    txn_id: String,
    is_anomaly: bool,
    reason: String,
}

/// Detect anomalous transactions using a two-phase approach on top of the one
/// authoritative statistics engine ([`finsight_core::anomaly`]):
///
/// 1. **Candidates:** [`statistical_outlier_candidates`] applies the same
///    median/MAD thresholds and the same exclusions (transfers, settle-up rows,
///    investment-account trades, user-dismissed charges) as the deterministic
///    recompute. There is deliberately no second statistical implementation
///    here: a divergent one could re-flag charges the user explicitly
///    dismissed and overwrite the authoritative detector's verdicts.
/// 2. **LLM confirmation:** candidates are sent to the LLM in batches with
///    their historical baseline; only transactions the LLM confirms get
///    flagged, with the LLM's human-readable reason stored as explanation.
///
/// Only *not-yet-flagged* transactions are reviewed (the same scope the
/// deterministic recompute just (re)computed from live data). Writes hold a
/// reset-barrier lease across a single transactional commit, so a concurrent
/// Delete-All can never leave these flags orphaned on a wiped ledger.
/// Returns the number of anomalies written.
pub async fn detect_anomalies(db: &Db, provider: Arc<dyn CompletionProvider>) -> Result<u32> {
    // Snapshot the barrier epoch before any LLM round-trip: if a Delete-All
    // lands while we await the provider, the lease below reports superseded
    // and we skip writing entirely.
    let start_epoch = db.reset_barrier().epoch();

    // ── Step 1: candidates from the shared core analysis ────────────────────
    let candidates: Vec<AnomalyCandidate> = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.get()?;
            statistical_outlier_candidates(&conn)
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
                    "historical_median_cents": c.typical_cents,
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

    // ── Step 3: transactional write under the reset barrier ─────────────────
    if confirmed.is_empty() {
        return Ok(0);
    }
    let lease = db.reset_barrier().writer_lease(start_epoch).await;
    if lease.superseded() {
        return Ok(0);
    }
    let db = db.clone();
    let written = tokio::task::spawn_blocking(move || {
        let mut conn = db.get()?;
        let tx = conn.transaction()?;
        let mut written = 0u32;
        {
            // The dismissal guard is belt-and-braces (candidates already exclude
            // dismissed rows): between listing and writing, a dismissal may land.
            let mut stmt = tx.prepare(
                "UPDATE transactions SET is_anomaly = 1, ai_explanation = ?1 \
                 WHERE id = ?2 AND COALESCE(anomaly_dismissed, 0) = 0",
            )?;
            for (txn_id, reason) in &confirmed {
                written += stmt.execute(params![reason, txn_id])? as u32;
            }
        }
        tx.commit()?;
        Ok::<_, anyhow::Error>(written)
    })
    .await??;

    Ok(written)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::mock::MockCompletionProvider;

    use serde_json::json;
    use std::sync::Mutex;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, finsight_core::Db) {
        let (dir, db) = finsight_core::testing::migrated_db();
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
            // 6 normal transactions at ~$15 (the shared core criteria need a
            // group of >= 6), plus one $200 outlier.
            seed_merchant(
                &mut conn,
                "a1",
                "COSTCO",
                &[-1500, -1600, -1400, -1550, -1450, -1500],
            );
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

        let count = detect_anomalies(&db, provider).await.unwrap();

        assert_eq!(count, 1);
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
            // Only 2 transactions — below the shared 6-occurrence threshold
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

    #[tokio::test]
    async fn never_flags_a_dismissed_charge() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_merchant(
                &mut conn,
                "a3",
                "DISMISSED CO",
                &[-1500, -1600, -1400, -1550, -1450, -1500],
            );
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES('dismissed-1','a3',date('now', '-1 days'),-20000,'DISMISSED CO','cleared',0,date('now', '-1 days'))",
                [],
            ).unwrap();
            finsight_core::anomaly::set_dismissed(&conn, "dismissed-1", true).unwrap();
        }

        // The LLM would confirm the charge if it were ever asked.
        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([{"txn_id": "dismissed-1", "is_anomaly": true, "reason": "large"}]),
            tool_turns: Mutex::new(vec![]),
        });

        let count = detect_anomalies(&db, provider).await.unwrap();
        assert_eq!(count, 0, "dismissed charges must never be re-flagged");
        let conn = db.get().unwrap();
        let (isa, dis): (i64, i64) = conn
            .query_row(
                "SELECT is_anomaly, anomaly_dismissed FROM transactions WHERE id='dismissed-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((isa, dis), (0, 1));
    }

    #[tokio::test]
    async fn does_not_re_review_already_flagged_rows() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed_merchant(
                &mut conn,
                "a4",
                "FLAGGED CO",
                &[-1500, -1600, -1400, -1550, -1450, -1500],
            );
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
                 VALUES('flagged-1','a4',date('now', '-1 days'),-20000,'FLAGGED CO','cleared',1,date('now', '-1 days'))",
                [],
            ).unwrap();
        }

        let provider = Arc::new(MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([{"txn_id": "flagged-1", "is_anomaly": true, "reason": "llm reason"}]),
            tool_turns: Mutex::new(vec![]),
        });

        // Already-flagged rows are outside the review scope; the deterministic
        // explanation written by the core detector stays untouched.
        let count = detect_anomalies(&db, provider).await.unwrap();
        assert_eq!(count, 0);
        let conn = db.get().unwrap();
        let (isa, why): (i64, Option<String>) = conn
            .query_row(
                "SELECT is_anomaly, ai_explanation FROM transactions WHERE id='flagged-1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(isa, 1);
        assert_ne!(why.as_deref(), Some("llm reason"));
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
