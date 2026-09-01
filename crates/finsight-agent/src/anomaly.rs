use crate::CompletionProvider;
use anyhow::Result;
use finsight_core::{
    anomaly::{statistical_outlier_candidates, AnomalyCandidate},
    settings, Db,
};
use rusqlite::params;
use std::sync::Arc;

#[allow(dead_code)]
const BATCH_SIZE: usize = 20;


fn fmt_money(cents: i64) -> String {
    format!("${:.2}", cents as f64 / 100.0)
}

fn anomaly_reason(c: &AnomalyCandidate) -> String {
    let mult = if c.typical_cents != 0 {
        c.amount_cents.abs() as f64 / c.typical_cents.abs() as f64
    } else {
        0.0
    };
    let amount = fmt_money(c.amount_cents.abs());
    let median = fmt_money(c.typical_cents.abs());
    if mult >= 2.0 {
        format!("{} {} is {:.1}× your median {} — outlier", c.merchant_raw, amount, mult, median)
    } else {
        format!("{} {} vs median {} — unusual", c.merchant_raw, amount, median)
    }
}


/// Detect anomalous transactions deterministically — no LLM.
///
/// 1. **Candidates:** [`statistical_outlier_candidates`] (median/MAD, same
///    exclusions as `recompute_anomalies`).
/// 2. **Confirmation:** every candidate is confirmed with a templated
///    `anomaly_reason` (`"{merchant} $X is N× median $Y — outlier"`), more
///    auditable than LLM prose and aligned with `finance.rs` metric
///    explanations.
///
/// Only *not-yet-flagged* transactions are reviewed. Writes hold a
/// reset-barrier lease. `provider` is kept for API compat but ignored.
/// Returns the number of anomalies written.
pub async fn detect_anomalies(db: &Db, provider: Arc<dyn CompletionProvider>) -> Result<u32> {
    // Snapshot the barrier epoch before any provider round-trip.
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

    // ── Step 2: deterministic confirmation — all statistical candidates are
    // confirmed with a templated human-readable reason (no LLM). This is
    // more auditable than an LLM hallucination and matches the MAD
    // explanation style in finance.rs MetricExplanation.
    let confirmed: Vec<(String, String)> = candidates
        .iter()
        .map(|c| (c.txn_id.clone(), anomaly_reason(c)))
        .collect();

    // Allow unused provider param for API compat — caller in categorizer.rs
    // still passes Arc<dyn CompletionProvider>. Suppress warning.
    let _ = &provider;


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
