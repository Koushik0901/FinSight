//! The prescriptive layer: turn the decomposition into an honest "path back".
//! One-off spend self-corrects; recurring/emerging drivers are the levers you
//! can trim; anything below what trimming reaches is structural — and we say so
//! plainly rather than over-promising a number the user is attached to.

use crate::error::CoreResult;
use crate::spending::decompose::{decompose, Filter};
use crate::spending::{baseline, Driver, Persistence, Window};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingPlan {
    pub currency: String,
    /// The elevated month's spend (monthly-equivalent).
    pub recent_monthly_cents: i64,
    /// Your robust normal (median of the trailing months).
    pub baseline_monthly_cents: i64,
    /// One-off drivers that fall off on their own — no action needed.
    pub self_correcting_cents: i64,
    /// Recurring/emerging drivers you could trim — the levers.
    pub recoverable_recurring_cents: i64,
    /// Where you'd land if the one-offs lapse AND you trim every lever.
    pub projected_after_levers_cents: i64,
    /// The specific recurring levers to act on, ranked by size.
    pub levers: Vec<Driver>,
    pub target_monthly_cents: Option<i64>,
    /// Present only with a target BELOW what trimming reaches: the remaining
    /// gap is structural (a floor / fixed commitments), not more trimming.
    pub structural_gap_cents: Option<i64>,
    pub note: String,
}

pub fn plan_spending_reduction(
    conn: &Connection,
    period_ym: &str,
    target_monthly_cents: Option<i64>,
) -> CoreResult<SpendingPlan> {
    let base = baseline::trailing(conn, period_ym, 12)?;
    let target_win = Window::for_month(period_ym);
    let d = decompose(conn, &target_win, &base, Filter::All, 2.0, 50)?;

    let months = target_win.months.max(1.0);
    let recent = (d.target_total_cents as f64 / months).round() as i64;
    let baseline_monthly = d.baseline_monthly_cents;
    let self_correcting = d.persistence_subtotals.one_off_cents;
    let recoverable =
        d.persistence_subtotals.recurring_cents + d.persistence_subtotals.emerging_cents;
    let projected_after = recent - self_correcting - recoverable;

    let levers: Vec<Driver> = d
        .drivers
        .into_iter()
        .filter(|dr| {
            dr.delta_cents > 0
                && matches!(dr.persistence, Persistence::Recurring | Persistence::Emerging)
        })
        .collect();

    let (structural_gap_cents, note) = match target_monthly_cents {
        Some(t) if t >= projected_after => (
            None,
            "Your target is within reach: letting the one-offs lapse and trimming the recurring levers below gets you there.".to_string(),
        ),
        Some(t) => (
            Some(projected_after - t),
            "Even after the one-offs lapse and you trim every recurring lever, you'd land above your target — the remaining gap is structural (your normal floor and fixed commitments), not something these cuts reach. Going lower means a deliberate change, not more trimming.".to_string(),
        ),
        None => (
            None,
            "The one-off part self-corrects; the recurring levers below are what's yours to trim.".to_string(),
        ),
    };

    Ok(SpendingPlan {
        currency: d.currency,
        recent_monthly_cents: recent,
        baseline_monthly_cents: baseline_monthly,
        self_correcting_cents: self_correcting,
        recoverable_recurring_cents: recoverable,
        projected_after_levers_cents: projected_after,
        levers,
        target_monthly_cents,
        structural_gap_cents,
        note,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("p.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        {
            let conn = db.get().unwrap();
            conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a','me','B','Credit','Card','USD','#fff',datetime('now'))", []).unwrap();
        }
        (dir, db)
    }

    fn ins(conn: &Connection, ym: &str, cents: i64, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,0,'cleared',datetime('now'))",
            rusqlite::params![format!("{ym}-15T12:00:00Z"), cents, merchant],
        ).unwrap();
    }

    fn seed_scenario(conn: &Connection) {
        for i in 0..12 {
            ins(conn, &format!("2025-{:02}", i + 1), -200_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(conn, "2026-01", -250_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(conn, "2026-01", -90_000, "FLAIR AIRLINES  BURNABY, BC");
    }

    #[test]
    fn splits_self_correcting_from_recoverable_and_flags_structural_target() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_scenario(&conn);
        let p = plan_spending_reduction(&conn, "2026-01", Some(150_000)).unwrap();
        assert_eq!(p.recent_monthly_cents, 340_000);
        assert_eq!(p.baseline_monthly_cents, 200_000);
        assert_eq!(p.self_correcting_cents, 90_000, "the flight is one-off");
        assert_eq!(p.recoverable_recurring_cents, 50_000, "groceries +$500 is recurring");
        assert_eq!(p.projected_after_levers_cents, 200_000);
        assert!(p.levers.iter().any(|d| d.display == "SAVE ON FOODS"));
        assert!(!p.levers.iter().any(|d| d.display == "FLAIR AIRLINES"), "one-offs are not levers");
        assert_eq!(p.structural_gap_cents, Some(50_000));
    }

    #[test]
    fn reachable_target_has_no_structural_gap() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_scenario(&conn);
        let p = plan_spending_reduction(&conn, "2026-01", Some(220_000)).unwrap();
        assert_eq!(p.structural_gap_cents, None, "$2,200 >= the $2,000 trimming floor");
    }
}
