//! Deterministic statistical anomaly detection (Phase 6 #5).
//!
//! Invariant: anomalies must be computed from *actual transaction patterns*,
//! never random or stale flags. This module flags an expense as anomalous when
//! its magnitude is a robust statistical outlier versus that merchant's own
//! history — using the median and MAD (median absolute deviation), which are
//! resistant to the very outliers we are trying to find.
//!
//! Recomputation always clears prior flags first, so `is_anomaly` reflects a
//! fresh, current computation (no stale flags survive a re-run or re-import).

use crate::error::CoreResult;
use crate::merchant::normalize_merchant;
use rusqlite::Connection;

/// Minimum charges from a merchant before we can judge one as unusual.
const MIN_HISTORY: usize = 6;
/// Robustness multiplier: how many robust sigmas above the median.
const K_SIGMA: f64 = 5.0;
/// A flagged charge must be at least this multiple of the typical charge.
const MIN_MULTIPLE: f64 = 2.5;
/// …and at least this many cents above the typical charge (avoids flagging
/// small merchants where everything is a few dollars).
const MIN_ABS_DELTA_CENTS: f64 = 4_000.0;
/// Consistency constant making MAD comparable to a standard deviation.
const MAD_TO_SIGMA: f64 = 1.4826;

struct Row {
    id: String,
    abs_cents: f64,
    merchant_key: String,
}

/// Recompute `is_anomaly` for every expense transaction from live data.
/// Returns the number of transactions now flagged as anomalous.
pub fn recompute_anomalies(conn: &mut Connection) -> CoreResult<u32> {
    // 1. Clear all prior flags so nothing stale survives.
    conn.execute("UPDATE transactions SET is_anomaly = 0 WHERE is_anomaly = 1", [])?;

    // 2. Load expenses (exclude transfers — a large transfer is not an anomaly).
    let rows: Vec<Row> = {
        let mut stmt = conn.prepare(
            "SELECT id, merchant_raw, amount_cents FROM transactions \
             WHERE amount_cents < 0 AND is_transfer = 0",
        )?;
        let mapped = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in mapped {
            let (id, raw, amount) = row?;
            out.push(Row {
                id,
                abs_cents: amount.unsigned_abs() as f64,
                merchant_key: normalize_merchant(&raw),
            });
        }
        out
    };

    // 3. Group by normalized merchant.
    let mut groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        if row.merchant_key.is_empty() {
            continue;
        }
        groups.entry(row.merchant_key.clone()).or_default().push(i);
    }

    // 4. Flag robust outliers within each group.
    let mut flagged: Vec<(String, String)> = Vec::new(); // (txn_id, reason)
    for (_key, idxs) in groups {
        if idxs.len() < MIN_HISTORY {
            continue;
        }
        let amounts: Vec<f64> = idxs.iter().map(|&i| rows[i].abs_cents).collect();
        let med = median(&amounts);
        if med <= 0.0 {
            continue;
        }
        let deviations: Vec<f64> = amounts.iter().map(|a| (a - med).abs()).collect();
        let mad = median(&deviations);
        let robust_sigma = (MAD_TO_SIGMA * mad).max(med * 0.10); // floor: 10% of median
        let threshold = med + K_SIGMA * robust_sigma;

        for &i in &idxs {
            let a = rows[i].abs_cents;
            if a > threshold && a >= MIN_MULTIPLE * med && (a - med) >= MIN_ABS_DELTA_CENTS {
                let reason = format!(
                    "This ${:.2} charge is {:.1}× your typical ${:.2} at this merchant.",
                    a / 100.0,
                    a / med,
                    med / 100.0
                );
                flagged.push((rows[i].id.clone(), reason));
            }
        }
    }

    // 5. Persist flags + a deterministic explanation.
    let tx = conn.transaction()?;
    for (id, reason) in &flagged {
        tx.execute(
            "UPDATE transactions SET is_anomaly = 1, ai_explanation = ?1 WHERE id = ?2",
            rusqlite::params![reason, id],
        )?;
    }
    tx.commit()?;

    Ok(flagged.len() as u32)
}

fn median(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("anom.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        conn_seed(&db);
        (dir, db)
    }

    fn conn_seed(db: &Db) {
        let conn = db.get().unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a','me','B','Credit','Card','USD','#fff',datetime('now'))", []).unwrap();
    }

    fn insert(conn: &Connection, merchant: &str, cents: i64, is_transfer: i64) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'a','2026-01-01T00:00:00Z',?1,?2,?3,'cleared',datetime('now'))",
            rusqlite::params![cents, merchant, is_transfer],
        )
        .unwrap();
    }

    #[test]
    fn flags_a_large_outlier_against_the_merchants_own_history() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        // 8 typical ~$10 coffees, then one $180 charge.
        for _ in 0..8 {
            insert(&conn, "STARBUCKS  800", -1000, 0);
        }
        insert(&conn, "STARBUCKS  800", -18000, 0);

        let n = recompute_anomalies(&mut conn).unwrap();
        assert_eq!(n, 1);
        let (flagged, reason): (i64, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), MAX(ai_explanation) FROM transactions WHERE is_anomaly = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(flagged, 1);
        assert!(reason.unwrap().contains("typical"));
    }

    #[test]
    fn does_not_flag_normal_variation_or_thin_history() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        // Normal variation, plenty of history — nothing extreme.
        for c in [-1000, -1200, -900, -1100, -1000, -1300, -950] {
            insert(&conn, "TIM HORTONS  EDM", c, 0);
        }
        // Thin history: a big charge but only 2 occurrences → can't judge.
        insert(&conn, "RARE VENDOR  X", -500, 0);
        insert(&conn, "RARE VENDOR  X", -90000, 0);

        let n = recompute_anomalies(&mut conn).unwrap();
        assert_eq!(n, 0, "no anomaly on normal variation or thin history");
    }

    #[test]
    fn large_transfers_are_never_anomalies() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for _ in 0..8 {
            insert(&conn, "Internet Withdrawal to Tangerine", -5000, 1);
        }
        insert(&conn, "Internet Withdrawal to Tangerine", -500000, 1); // huge transfer
        let n = recompute_anomalies(&mut conn).unwrap();
        assert_eq!(n, 0, "transfers are excluded from anomaly detection");
    }

    #[test]
    fn recompute_clears_stale_flags() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for _ in 0..8 {
            insert(&conn, "STARBUCKS  800", -1000, 0);
        }
        insert(&conn, "STARBUCKS  800", -18000, 0);
        assert_eq!(recompute_anomalies(&mut conn).unwrap(), 1);
        // Manually mark an unrelated txn stale, then recompute: it must clear.
        conn.execute("UPDATE transactions SET is_anomaly = 1 WHERE amount_cents = -1000", []).unwrap();
        let n = recompute_anomalies(&mut conn).unwrap();
        assert_eq!(n, 1, "recompute must clear stale flags and reflect only current outliers");
    }
}
