//! The prescriptive layer: turn the decomposition into an honest "path back".
//! One-off spend self-corrects; recurring/emerging drivers are the levers you
//! can trim; anything below what trimming reaches is structural — and we say so
//! plainly rather than over-promising a number the user is attached to.

use crate::error::CoreResult;
use crate::spending::decompose::{decompose, Filter};
use crate::spending::{baseline, Driver, Persistence, Window};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
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
    /// The one-off drivers that lapse on their own — shown as "leave them".
    pub self_correcting: Vec<Driver>,
    /// Drivers the user accepted (expected/investment) — kept in the floor,
    /// surfaced so they can be reviewed and undone; not levers, not self-correcting.
    pub accepted: Vec<Driver>,
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
    // Trimming your recent *increases* returns you to your normal — it cannot
    // take you below it. Clamp the projected floor at the baseline so a target
    // below your normal reads as structural, not "within reach". This also
    // corrects the median-headline vs mean-driver mismatch (positive deltas can
    // exceed the median gap when some merchants stopped), which would otherwise
    // over-promise reachability. Surfaced by live-app validation on real data.
    let projected_after = (recent - self_correcting - recoverable).max(baseline_monthly);

    let levers: Vec<Driver> = d
        .drivers
        .iter()
        .filter(|dr| {
            dr.delta_cents > 0
                && dr.user_verdict.is_none()
                && matches!(dr.persistence, Persistence::Recurring | Persistence::Emerging)
        })
        .cloned()
        .collect();
    let self_correcting_drivers: Vec<Driver> = d
        .drivers
        .iter()
        .filter(|dr| {
            dr.delta_cents > 0
                && (dr.user_verdict.as_deref() == Some("one_off")
                    || (dr.user_verdict.is_none() && dr.persistence == Persistence::OneOff))
        })
        .cloned()
        .collect();
    let accepted: Vec<Driver> = d
        .drivers
        .iter()
        .filter(|dr| {
            dr.delta_cents > 0
                && matches!(dr.user_verdict.as_deref(), Some("expected") | Some("investment"))
        })
        .cloned()
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
        self_correcting: self_correcting_drivers,
        accepted,
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
        assert!(p.self_correcting.iter().any(|d| d.display == "FLAIR AIRLINES"), "the one-off flight shows in self_correcting");
        assert!(!p.self_correcting.iter().any(|d| d.display == "SAVE ON FOODS"), "the recurring grocery is not self-correcting");
    }

    #[test]
    fn reachable_target_has_no_structural_gap() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_scenario(&conn);
        let p = plan_spending_reduction(&conn, "2026-01", Some(220_000)).unwrap();
        assert_eq!(p.structural_gap_cents, None, "$2,200 >= the $2,000 trimming floor");
    }

    #[test]
    fn accepted_annotation_is_not_self_correcting_and_stays_in_the_floor() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -100_000, "SAVE ON FOODS  EDMONTON, AB");
            ins(&conn, &format!("2025-{:02}", i + 1), -10_000, "AMAZON  ONLINE, ON");
        }
        ins(&conn, "2026-01", -100_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-01", -60_000, "AMAZON  ONLINE, ON"); // recurring, elevated +$500

        // The user says the Amazon spend is an accepted cost they're keeping.
        crate::spending::annotate::set_annotation(
            &conn,
            &crate::merchant::canonical_merchant_key("AMAZON  ONLINE, ON"),
            "expected",
            None,
        )
        .unwrap();

        let p = plan_spending_reduction(&conn, "2026-01", Some(90_000)).unwrap();
        assert_eq!(p.self_correcting_cents, 0, "a kept cost is NOT self-correcting");
        assert!(p.levers.is_empty(), "a kept cost is not a lever");
        assert!(p.accepted.iter().any(|d| d.display == "AMAZON"), "the kept driver is surfaced for review/undo");
        // The $500 Amazon rise stays in the floor (not subtracted as if it lapses),
        // so the projection is the full recent spend, not an understated floor.
        assert_eq!(p.projected_after_levers_cents, 160_000);
        assert!(p.structural_gap_cents.is_some(), "target below the real floor stays structural");
    }

    #[test]
    fn projected_floor_never_undershoots_the_baseline() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        // Baseline: $1,000 groceries + a $500 sub every month → normal $1,500/mo.
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -100_000, "SAVE ON FOODS  EDMONTON, AB");
            ins(&conn, &format!("2025-{:02}", i + 1), -50_000, "OLD SUB  ONLINE, ON");
        }
        // Target month: groceries flat, the sub STOPPED (a tailwind), plus one
        // $800 one-off. The positive delta ($800) exceeds the gap because the
        // stopped sub is a negative delta — so the naive floor dips below the
        // $1,500 baseline. It must clamp at the baseline, not over-promise.
        ins(&conn, "2026-01", -100_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-01", -80_000, "GADGET SHOP  ONLINE, ON");
        let p = plan_spending_reduction(&conn, "2026-01", Some(120_000)).unwrap();
        assert!(
            p.projected_after_levers_cents >= p.baseline_monthly_cents,
            "the floor can't dip below your normal (projected {}, baseline {})",
            p.projected_after_levers_cents,
            p.baseline_monthly_cents
        );
        // $1,200 target sits below the $1,500 floor → structural, not "reachable".
        assert_eq!(p.structural_gap_cents, Some(p.baseline_monthly_cents - 120_000));
    }
}
