//! Window-vs-baseline decomposition: rank the drivers of a period's spend
//! against "your normal" and tag each on two axes — mechanism (where the
//! delta came from) and persistence (will it repeat). The LLM never computes
//! these; it narrates them.

use crate::error::CoreResult;
use crate::spending::baseline::{self, Baseline};
use crate::spending::{DecomposeResult, Driver, Mechanism, Persistence, PersistenceSubtotals, Window};
use rusqlite::Connection;
use std::collections::HashSet;

/// Ratio at/above which a change in ticket size or frequency is "up"/"down".
const CHANGE_RATIO: f64 = 1.3;

/// Minimum periods of history before we claim "what changed" vs just "where it went".
const MIN_BASELINE_MONTHS: i64 = 3;

/// How the target window is filtered before ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    New,
    Elevated,
}

/// Per-merchant recent vs baseline monthly figures → a mechanism.
pub(crate) fn classify_mechanism(
    recent_monthly: i64,
    base_monthly: i64,
    recent_txns_pm: f64,
    base_txns_pm: f64,
) -> Mechanism {
    if base_monthly == 0 && recent_monthly > 0 {
        return Mechanism::New;
    }
    if recent_monthly == 0 && base_monthly > 0 {
        return Mechanism::Stopped;
    }
    let recent_ticket = if recent_txns_pm > 0.0 { recent_monthly as f64 / recent_txns_pm } else { 0.0 };
    let base_ticket = if base_txns_pm > 0.0 { base_monthly as f64 / base_txns_pm } else { 0.0 };
    let freq = if base_txns_pm > 0.0 { recent_txns_pm / base_txns_pm } else { f64::INFINITY };
    let price = if base_ticket > 0.0 { recent_ticket / base_ticket } else { f64::INFINITY };
    let freq_up = freq >= CHANGE_RATIO;
    let price_up = price >= CHANGE_RATIO;
    let freq_dn = freq <= 1.0 / CHANGE_RATIO;
    let price_dn = price <= 1.0 / CHANGE_RATIO;
    match (freq_up, price_up, freq_dn, price_dn) {
        (true, true, _, _) => Mechanism::Mixed,
        (true, _, _, _) => Mechanism::FrequencyUp,
        (_, true, _, _) => Mechanism::PriceUp,
        (_, _, true, _) => Mechanism::FrequencyDown,
        (_, _, _, true) => Mechanism::PriceDown,
        _ => Mechanism::Flat,
    }
}

/// Persistence from cheap structural signals (Phase 1). A later plan refines
/// this with recurring.rs cadence + user annotations.
pub(crate) fn classify_persistence(
    mechanism: Mechanism,
    active_months: i64,
    total_txns: i64,
    target_txns: i64,
) -> Persistence {
    if active_months >= 4 {
        return Persistence::Recurring;
    }
    if matches!(mechanism, Mechanism::New) && target_txns >= 2 {
        return Persistence::Emerging;
    }
    if total_txns <= 2 {
        return Persistence::OneOff;
    }
    Persistence::Uncertain
}

/// Decompose `target` against `reference` (a pre-computed baseline — either the
/// trailing-normal or a single comparison month). `min_ratio` applies only to
/// `Filter::Elevated`.
pub fn decompose(
    conn: &Connection,
    target: &Window,
    reference: &Baseline,
    filter: Filter,
    min_ratio: f64,
    limit: usize,
) -> CoreResult<DecomposeResult> {
    // The target window's own per-merchant aggregates (same exclusions/normalization).
    // `get(..7)` (not `[..7]`) so a hand-built Window with a short start can't panic.
    let start_ym = target.start.get(..7).unwrap_or(target.start.as_str());
    let end_ym = target.end.get(..7).unwrap_or(target.end.as_str());
    let target_bl = baseline::compute(conn, start_ym, end_ym)?;
    let verdicts = crate::spending::annotate::annotations(conn)?;
    let months = target.months.max(1.0);

    let mut keys: HashSet<String> = target_bl.per_merchant.keys().cloned().collect();
    keys.extend(reference.per_merchant.keys().cloned());

    let mut drivers: Vec<Driver> = Vec::new();
    let mut target_total: i64 = 0;

    for key in &keys {
        let t = target_bl.per_merchant.get(key);
        let b = reference.per_merchant.get(key);
        let recent_monthly = t.map(|m| m.monthly_cents).unwrap_or(0);
        let base_monthly = b.map(|m| m.monthly_cents).unwrap_or(0);
        let recent_pm = t.map(|m| m.txns_per_month).unwrap_or(0.0);
        let base_pm = b.map(|m| m.txns_per_month).unwrap_or(0.0);
        target_total += (recent_monthly as f64 * months).round() as i64;

        let mechanism = classify_mechanism(recent_monthly, base_monthly, recent_pm, base_pm);
        let active = t
            .map(|m| m.active_months)
            .unwrap_or(0)
            .max(b.map(|m| m.active_months).unwrap_or(0));
        let total_txns = ((recent_pm + base_pm) * months).round() as i64;
        let target_txns = (recent_pm * months).round() as i64;
        let computed = classify_persistence(mechanism, active, total_txns, target_txns);
        let user_verdict = verdicts.get(key).cloned();
        // A sticky verdict overrides the computed persistence (spec §6): an
        // accepted driver is never a recurring lever — in the field AND the total.
        let persistence = if user_verdict.is_some() { crate::spending::Persistence::OneOff } else { computed };

        let driver = Driver {
            merchant_key: key.clone(),
            display: t.or(b).map(|m| m.display.clone()).unwrap_or_else(|| key.clone()),
            category: t
                .and_then(|m| m.category.clone())
                .or_else(|| b.and_then(|m| m.category.clone())),
            delta_cents: recent_monthly - base_monthly,
            recent_monthly_cents: recent_monthly,
            base_monthly_cents: base_monthly,
            recent_txns_per_month: recent_pm,
            base_txns_per_month: base_pm,
            mechanism,
            persistence,
            user_verdict,
        };
        if passes(&driver, filter, min_ratio) {
            drivers.push(driver);
        }
    }

    drivers.sort_by(|a, b| b.delta_cents.cmp(&a.delta_cents));

    // Subtotals over the full (filtered) driver set BEFORE truncation, so
    // "how much of the increase will recur" reflects every driver.
    let mut subtotals = PersistenceSubtotals::default();
    for d in drivers.iter().filter(|d| d.delta_cents > 0) {
        match d.persistence {
            Persistence::Recurring => subtotals.recurring_cents += d.delta_cents,
            Persistence::OneOff => subtotals.one_off_cents += d.delta_cents,
            Persistence::Emerging => subtotals.emerging_cents += d.delta_cents,
            Persistence::Uncertain => subtotals.uncertain_cents += d.delta_cents,
        }
    }
    drivers.truncate(limit);

    let baseline_monthly = reference.grand_monthly_median_cents;
    let note = if reference.months < MIN_BASELINE_MONTHS {
        format!(
            "Only {} month(s) of baseline history — showing where money went, but the 'what changed' tags are low-confidence.",
            reference.months
        )
    } else if reference.mixed_currency || target_bl.mixed_currency {
        format!("Multiple currencies present; analyzing {} only.", reference.currency)
    } else {
        String::new()
    };

    Ok(DecomposeResult {
        currency: reference.currency.clone(),
        target_total_cents: target_total,
        baseline_monthly_cents: baseline_monthly,
        gap_cents: (target_total as f64 / months).round() as i64 - baseline_monthly,
        drivers,
        persistence_subtotals: subtotals,
        note,
    })
}

fn passes(d: &Driver, filter: Filter, min_ratio: f64) -> bool {
    match filter {
        Filter::All => d.delta_cents != 0,
        Filter::New => d.mechanism == Mechanism::New,
        Filter::Elevated => {
            d.base_monthly_cents > 0
                && d.recent_monthly_cents as f64 >= d.base_monthly_cents as f64 * min_ratio
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use crate::merchant::canonical_merchant_key;
    use tempfile::TempDir;

    #[test]
    fn mechanism_distinguishes_new_price_frequency() {
        assert_eq!(classify_mechanism(5000, 0, 1.0, 0.0), Mechanism::New);
        assert_eq!(classify_mechanism(0, 5000, 0.0, 1.0), Mechanism::Stopped);
        assert_eq!(classify_mechanism(20000, 10000, 1.0, 1.0), Mechanism::PriceUp);
        assert_eq!(classify_mechanism(11000, 7000, 11.0, 7.0), Mechanism::FrequencyUp);
        assert_eq!(classify_mechanism(10000, 9800, 2.0, 2.0), Mechanism::Flat);
    }

    #[test]
    fn persistence_reads_structure() {
        assert_eq!(classify_persistence(Mechanism::PriceUp, 8, 20, 3), Persistence::Recurring);
        assert_eq!(classify_persistence(Mechanism::New, 1, 3, 3), Persistence::Emerging);
        assert_eq!(classify_persistence(Mechanism::New, 1, 1, 1), Persistence::OneOff);
    }

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("d.sqlcipher"), &key).unwrap();
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

    #[test]
    fn decompose_finds_new_and_elevated_drivers() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-05", -40_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-05", -60_000, "FLAIR AIRLINES  BURNABY, BC");

        let base = baseline::compute(&conn, "2025-01", "2026-01").unwrap();
        let may = Window::for_month("2026-05");
        let out = decompose(&conn, &may, &base, Filter::All, 2.0, 20).unwrap();

        assert_eq!(out.drivers[0].display, "FLAIR AIRLINES");
        assert_eq!(out.drivers[0].mechanism, Mechanism::New);
        let groc = out.drivers.iter().find(|d| d.display == "SAVE ON FOODS").unwrap();
        assert_eq!(groc.recent_monthly_cents, 40_000);
        assert_eq!(groc.base_monthly_cents, 20_000);

        let only_new = decompose(&conn, &may, &base, Filter::New, 2.0, 20).unwrap();
        assert!(only_new.drivers.iter().all(|d| d.mechanism == Mechanism::New));
        assert!(only_new.drivers.iter().any(|d| d.display == "FLAIR AIRLINES"));

        let elevated = decompose(&conn, &may, &base, Filter::Elevated, 2.0, 20).unwrap();
        assert!(elevated.drivers.iter().any(|d| d.display == "SAVE ON FOODS"));
    }

    #[test]
    fn cold_start_emits_low_confidence_note() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        ins(&conn, "2026-03", -50_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-04", -50_000, "SAVE ON FOODS  EDMONTON, AB");
        let base = baseline::compute(&conn, "2025-05", "2026-05").unwrap();
        let may = Window::for_month("2026-05");
        let out = decompose(&conn, &may, &base, Filter::All, 2.0, 20).unwrap();
        assert!(out.note.to_lowercase().contains("month"), "cold-start note should fire, got: {:?}", out.note);
    }

    #[test]
    fn annotation_drops_driver_from_recurring_levers() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -10_000, "AMAZON  ONLINE, ON");
        }
        ins(&conn, "2026-05", -40_000, "AMAZON  ONLINE, ON");
        let base = baseline::compute(&conn, "2025-01", "2026-01").unwrap();
        let may = Window::for_month("2026-05");

        let before = decompose(&conn, &may, &base, Filter::All, 2.0, 20).unwrap();
        assert!(before.persistence_subtotals.recurring_cents > 0, "amazon is a lever before annotation");

        crate::spending::annotate::set_annotation(&conn, &canonical_merchant_key("AMAZON  ONLINE, ON"), "expected", None).unwrap();
        let after = decompose(&conn, &may, &base, Filter::All, 2.0, 20).unwrap();
        let amz = after.drivers.iter().find(|d| d.display == "AMAZON").unwrap();
        assert_eq!(amz.user_verdict.as_deref(), Some("expected"));
        assert_eq!(amz.persistence, Persistence::OneOff, "annotation overrides the driver's persistence too");
        assert_eq!(after.persistence_subtotals.recurring_cents, 0, "annotated driver leaves the levers");
    }
}
