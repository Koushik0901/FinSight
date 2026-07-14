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
    was_flagged: bool,
    /// User marked this flagged charge as reviewed-and-fine — it still counts
    /// toward its merchant's baseline but must never be re-flagged.
    dismissed: bool,
}

/// Recompute `is_anomaly` for every expense transaction from live data.
/// Returns the number of transactions now flagged as anomalous.
pub fn recompute_anomalies(conn: &mut Connection) -> CoreResult<u32> {
    recompute(conn, None)
}

/// Account-scoped recompute for the import cascade. Only merchants that appear
/// in `account_id` can have been affected by importing into it (an outlier is
/// judged against that merchant's *whole* cross-account history, so a merchant
/// absent from this account is untouched by this import). Recomputing only
/// those groups — and clearing only their flags — is provably equivalent to a
/// full recompute for the affected rows while leaving every other merchant's
/// flags exactly as they were, so it avoids re-sorting/re-flagging the entire
/// ledger on each import. Returns the number of transactions flagged within the
/// recomputed groups.
pub fn recompute_anomalies_for_account(conn: &mut Connection, account_id: &str) -> CoreResult<u32> {
    recompute(conn, Some(account_id))
}

/// Mark a flagged transaction as reviewed-and-fine, or un-dismiss it. Dismissing
/// clears the current flag; the detector will not re-flag it while dismissed
/// (it still counts toward its merchant's baseline). Un-dismissing lets the next
/// recompute flag it again if it is still an outlier.
pub fn set_dismissed(conn: &Connection, txn_id: &str, dismissed: bool) -> CoreResult<()> {
    if dismissed {
        conn.execute(
            "UPDATE transactions SET anomaly_dismissed = 1, is_anomaly = 0 WHERE id = ?1",
            rusqlite::params![txn_id],
        )?;
    } else {
        conn.execute(
            "UPDATE transactions SET anomaly_dismissed = 0 WHERE id = ?1",
            rusqlite::params![txn_id],
        )?;
    }
    Ok(())
}

/// Shared core. When `scope_account` is `None`, recompute every merchant group
/// and clear every prior flag (the authoritative full pass). When it is `Some`,
/// touch only groups that have a member in that account: clear flags on their
/// rows and re-flag just them, leaving all other transactions' `is_anomaly`
/// untouched. The in-scope key set is built inline during the single load pass
/// (no extra query, merchants normalized once) so the scoped path is no slower
/// than the full one on a single-account ledger.
fn recompute(conn: &mut Connection, scope_account: Option<&str>) -> CoreResult<u32> {
    // Load expenses (exclude transfers — a large transfer is not an anomaly).
    // A group's outlier judgement needs its *full* cross-account membership, so
    // even the scoped pass loads all expenses and filters which groups to act
    // on; it just does far less sorting/writing.
    let mut touched: std::collections::HashSet<String> = std::collections::HashSet::new();
    let rows: Vec<Row> = {
        // Investment-account rows (BUY/SELL trades) are excluded: a large trade
        // is not "unusual spending" — it isn't spending at all.
        let mut stmt = conn.prepare(&format!(
            "SELECT id, merchant_raw, amount_cents, is_anomaly, account_id, \
                    COALESCE(anomaly_dismissed, 0) FROM transactions t \
             WHERE amount_cents < 0 AND is_transfer = 0 AND {}",
            crate::metrics::non_investment_txn_predicate("t")
        ))?;
        let mapped = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)? != 0,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)? != 0,
            ))
        })?;
        let mut out = Vec::new();
        for row in mapped {
            let (id, raw, amount, was_flagged, account_id, dismissed) = row?;
            let merchant_key = normalize_merchant(&raw);
            // Build the in-scope key set for free: any group with a member in
            // the target account could have been shifted by importing into it.
            if let Some(aid) = scope_account {
                if account_id == aid && !merchant_key.is_empty() {
                    touched.insert(merchant_key.clone());
                }
            }
            out.push(Row {
                id,
                abs_cents: amount.unsigned_abs() as f64,
                merchant_key,
                was_flagged,
                dismissed,
            });
        }
        out
    };
    let scoped = scope_account.is_some();

    // Group by normalized merchant, keeping only the groups in scope.
    let mut groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, row) in rows.iter().enumerate() {
        if row.merchant_key.is_empty() {
            continue;
        }
        if scoped && !touched.contains(&row.merchant_key) {
            continue;
        }
        groups.entry(row.merchant_key.clone()).or_default().push(i);
    }

    // Flag robust outliers within each in-scope group.
    let mut flagged: Vec<(String, String)> = Vec::new(); // (txn_id, reason)
    for (_key, idxs) in &groups {
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

        for &i in idxs {
            // A dismissed charge still shapes the baseline above, but the user
            // has said it's fine — never re-flag it.
            if rows[i].dismissed {
                continue;
            }
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

    let tx = conn.transaction()?;
    // Clear prior flags. Full pass: clear everything stale in one statement.
    // Scoped pass: clear only rows belonging to the in-scope groups, so other
    // merchants' flags survive untouched.
    if scoped {
        // Only rows that are CURRENTLY flagged need clearing, and only within
        // touched groups — proportional to the (small) set of existing
        // anomalies in those groups, not every touched row.
        let mut clear =
            tx.prepare_cached("UPDATE transactions SET is_anomaly = 0 WHERE id = ?1")?;
        for idxs in groups.values() {
            for &i in idxs {
                if rows[i].was_flagged {
                    clear.execute([&rows[i].id])?;
                }
            }
        }
    } else {
        tx.execute("UPDATE transactions SET is_anomaly = 0 WHERE is_anomaly = 1", [])?;
    }
    // Persist fresh flags + a deterministic explanation.
    {
        let mut set_flag = tx.prepare_cached(
            "UPDATE transactions SET is_anomaly = 1, ai_explanation = ?1 WHERE id = ?2",
        )?;
        for (id, reason) in &flagged {
            set_flag.execute(rusqlite::params![reason, id])?;
        }
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

    /// Insert with an explicit id + account so two ledgers can be built
    /// byte-identically and compared row-for-row.
    fn ins(conn: &Connection, id: &str, account: &str, merchant: &str, cents: i64) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(?1,?2,'2026-01-01T00:00:00Z',?3,?4,0,'cleared',datetime('now'))",
            rusqlite::params![id, account, cents, merchant],
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
    fn dismissed_anomaly_is_not_reflagged() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for i in 0..8 {
            ins(&conn, &format!("n{i}"), "a", "STARBUCKS  800", -1000);
        }
        ins(&conn, "outlier", "a", "STARBUCKS  800", -18000);

        assert_eq!(recompute_anomalies(&mut conn).unwrap(), 1);
        let flagged: i64 = conn
            .query_row("SELECT is_anomaly FROM transactions WHERE id='outlier'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(flagged, 1);

        // User dismisses it → flag cleared, marked dismissed.
        set_dismissed(&conn, "outlier", true).unwrap();
        let (isa, dis): (i64, i64) = conn
            .query_row(
                "SELECT is_anomaly, anomaly_dismissed FROM transactions WHERE id='outlier'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((isa, dis), (0, 1));

        // Recompute must NOT re-flag a dismissed charge.
        assert_eq!(recompute_anomalies(&mut conn).unwrap(), 0, "dismissed anomaly stays dismissed");

        // Un-dismissing makes it flaggable again on the next recompute.
        set_dismissed(&conn, "outlier", false).unwrap();
        assert_eq!(recompute_anomalies(&mut conn).unwrap(), 1, "un-dismissed outlier is flaggable again");
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

    #[test]
    fn scoped_recompute_matches_full_and_preserves_untouched_flags() {
        // Build two byte-identical ledgers that diverge only at the FINAL
        // recompute: one runs the authoritative full pass, the other the
        // account-scoped pass the import cascade uses. Their resulting
        // is_anomaly / ai_explanation state must match row-for-row.
        fn build() -> (TempDir, Db) {
            let (dir, db) = fresh(); // seeds account 'a'
            {
                let conn = db.get().unwrap();
                conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('b','me','B','Credit','CardB','USD','#fff',datetime('now'))", []).unwrap();
            }
            let mut conn = db.get().unwrap();
            // GYM_B lives ONLY in account b, with a pre-existing outlier.
            for i in 0..6 {
                ins(&conn, &format!("gymb{i}"), "b", "GYM MEMBERSHIP B", -2000);
            }
            ins(&conn, "gymb_out", "b", "GYM MEMBERSHIP B", -20000);
            // SHARED lives in BOTH accounts (group spans a+b), normal so far.
            for i in 0..3 {
                ins(&conn, &format!("sha{i}"), "a", "SHARED STORE", -3000);
            }
            for i in 0..3 {
                ins(&conn, &format!("shb{i}"), "b", "SHARED STORE", -3000);
            }
            // Baseline: flags the GYM_B outlier so it is already correct.
            recompute_anomalies(&mut conn).unwrap();
            // "Import into account a": A-only COFFEE with an outlier, plus a
            // SHARED outlier that shifts the cross-account shared group.
            for i in 0..8 {
                ins(&conn, &format!("cof{i}"), "a", "COFFEE HUT", -1000);
            }
            ins(&conn, "cof_out", "a", "COFFEE HUT", -50000);
            ins(&conn, "sha_out", "a", "SHARED STORE", -40000);
            (dir, db)
        }

        let (_d1, db1) = build();
        let (_d2, db2) = build();
        {
            let mut c = db1.get().unwrap();
            recompute_anomalies(&mut c).unwrap();
        }
        {
            let mut c = db2.get().unwrap();
            recompute_anomalies_for_account(&mut c, "a").unwrap();
        }

        let flags = |db: &Db| -> Vec<(String, i64, Option<String>)> {
            let conn = db.get().unwrap();
            let mut stmt = conn
                .prepare("SELECT id, is_anomaly, ai_explanation FROM transactions ORDER BY id")
                .unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .unwrap();
            rows.map(|r| r.unwrap()).collect()
        };
        let full = flags(&db1);
        let scoped = flags(&db2);
        assert_eq!(
            full, scoped,
            "account-scoped recompute must match the full recompute row-for-row"
        );

        // Sanity: both outliers introduced by the import are flagged, and the
        // untouched GYM_B outlier (only in account b) stays flagged under the
        // scoped pass — it was never cleared.
        let is_flagged = |id: &str| full.iter().find(|(i, _, _)| i == id).unwrap().1 == 1;
        assert!(is_flagged("cof_out"), "A-only import outlier must flag");
        assert!(is_flagged("sha_out"), "shared-group import outlier must flag");
        assert!(is_flagged("gymb_out"), "untouched merchant's flag must survive");
    }
}
