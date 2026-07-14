//! Period classification: is a month within your normal band, an isolated
//! (episodic) spike, or a sustained new regime? Robust, evidence-based, and
//! honest about thin history (spec §4 cold-start rule).

use crate::error::CoreResult;
use crate::spending::baseline;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const MIN_BASELINE_MONTHS: i64 = 3;
const BAND_K: f64 = 2.5;
const MAD_TO_SIGMA: f64 = 1.4826;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodClass {
    Normal,
    EpisodicSpike,
    RegimeShift,
    InsufficientHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodAssessment {
    pub class: PeriodClass,
    pub period_total_cents: i64,
    pub baseline_monthly_cents: i64,
    /// median + BAND_K robust sigmas — spend above this is "elevated".
    pub upper_band_cents: i64,
    /// How many of the 3 months before `period` were also above the band.
    pub elevated_recent_months: i64,
    pub baseline_months: i64,
    pub note: String,
}

/// The robust upper band for a baseline: median + K·σ, with σ = 1.4826·MAD
/// floored at 15% of the median so a perfectly flat history still has slack.
fn upper_band(median: i64, mad: i64) -> i64 {
    let sigma = (MAD_TO_SIGMA * mad as f64).max(median as f64 * 0.15);
    median + (BAND_K * sigma).round() as i64
}

/// First-of-month `ym` shifted back `n` months.
fn month_back(period_ym: &str, n: i64) -> String {
    let (y, m) = crate::spending::parse_ym(period_ym);
    let idx = y * 12 + (m as i32 - 1) - n as i32;
    format!("{:04}-{:02}", idx.div_euclid(12), idx.rem_euclid(12) + 1)
}

pub fn classify_spending_period(conn: &Connection, period_ym: &str) -> CoreResult<PeriodAssessment> {
    let base = baseline::trailing(conn, period_ym, 12)?;
    let period_total = baseline::month_total(conn, period_ym)?;
    let band = upper_band(base.grand_monthly_median_cents, base.grand_monthly_mad_cents);

    if base.months < MIN_BASELINE_MONTHS {
        return Ok(PeriodAssessment {
            class: PeriodClass::InsufficientHistory,
            period_total_cents: period_total,
            baseline_monthly_cents: base.grand_monthly_median_cents,
            upper_band_cents: band,
            elevated_recent_months: 0,
            baseline_months: base.months,
            note: format!(
                "Only {} month(s) of history — can't yet judge normal vs. spike vs. regime.",
                base.months
            ),
        });
    }

    // Are the 3 months before `period` also above the band? (sustained = regime)
    let mut elevated_recent = 0;
    for n in 1..=3 {
        let ym = month_back(period_ym, n);
        if baseline::month_total(conn, &ym)? > band {
            elevated_recent += 1;
        }
    }

    let class = if period_total <= band {
        PeriodClass::Normal
    } else if elevated_recent >= 2 {
        PeriodClass::RegimeShift
    } else {
        PeriodClass::EpisodicSpike
    };

    let note = match class {
        PeriodClass::Normal => "Within your normal range.".to_string(),
        PeriodClass::EpisodicSpike => "A one-month spike — surrounding months are within your normal band.".to_string(),
        PeriodClass::RegimeShift => "A sustained step up — recent months are also elevated, not a one-off.".to_string(),
        PeriodClass::InsufficientHistory => String::new(),
    };

    Ok(PeriodAssessment {
        class,
        period_total_cents: period_total,
        baseline_monthly_cents: base.grand_monthly_median_cents,
        upper_band_cents: band,
        elevated_recent_months: elevated_recent,
        baseline_months: base.months,
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
        let db = Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
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

    fn setup_year(conn: &Connection, base_cents: i64) {
        for i in 0..12 {
            ins(conn, &format!("2025-{:02}", i + 1), -base_cents, "SAVE ON FOODS  EDMONTON, AB");
        }
    }

    #[test]
    fn normal_month_is_normal() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        setup_year(&conn, 200_000);
        ins(&conn, "2026-01", -205_000, "SAVE ON FOODS  EDMONTON, AB");
        let a = classify_spending_period(&conn, "2026-01").unwrap();
        assert_eq!(a.class, PeriodClass::Normal, "{a:?}");
    }

    #[test]
    fn isolated_big_month_is_episodic_spike() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        setup_year(&conn, 200_000);
        ins(&conn, "2026-01", -900_000, "FLAIR AIRLINES  BURNABY, BC");
        let a = classify_spending_period(&conn, "2026-01").unwrap();
        assert_eq!(a.class, PeriodClass::EpisodicSpike, "{a:?}");
        assert!(a.period_total_cents > a.upper_band_cents);
    }

    #[test]
    fn sustained_elevation_is_regime_shift() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        setup_year(&conn, 200_000);
        for ym in ["2025-11", "2025-12", "2026-01"] {
            ins(&conn, ym, -700_000, "NEW LIFESTYLE  VANCOUVER, BC");
        }
        let a = classify_spending_period(&conn, "2026-01").unwrap();
        assert_eq!(a.class, PeriodClass::RegimeShift, "{a:?}");
        assert!(a.elevated_recent_months >= 2);
    }

    #[test]
    fn thin_history_is_insufficient() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        ins(&conn, "2025-12", -200_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-01", -900_000, "FLAIR AIRLINES  BURNABY, BC");
        let a = classify_spending_period(&conn, "2026-01").unwrap();
        assert_eq!(a.class, PeriodClass::InsufficientHistory, "{a:?}");
    }
}
