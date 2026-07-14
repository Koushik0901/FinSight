# Spending Analysis Engine — Phase 2 (classify + sticky annotations) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two capabilities on top of the shipped Phase 1 engine: `classify_spending_period` (is a month `normal` / `episodic_spike` / `regime_shift`, with evidence) and the "propose → confirm → remember" loop — a sticky `annotate_spending_driver` verdict (one_off / expected / investment) that `decompose` honours everywhere so a user-accepted driver drops out of the "levers."

**Architecture:** Same layering as Phase 1 — all computation in `finsight-core::spending`; thin agent-tool wrappers. Classification reuses the robust baseline (median + a new MAD band). Annotations are a new keyed table (migration V047) with a small core module; `decompose` loads them and overrides each driver's effective persistence. The annotate tool is a **direct write** (a reversible user verdict, like `transfer_override`/anomaly-dismiss), not a draft — the draft/undo UX belongs to the fast-follow screen.

**Tech Stack:** Rust (rusqlite, serde, chrono), refinery migrations, the `finsight-agent` reasoning-tool trait.

**Prereq:** Phase 1 is merged to `main` (`finsight-core::spending::{stats,baseline,decompose}` + `explain_spending_change`). Spec: `docs/superpowers/specs/2026-07-14-spending-analysis-engine-design.md` (§6 emerging/annotation override, §7 classify + annotate, §10 migration, §11 tree rows).

**Scope note:** Follow-on (NOT here): `plan_spending_reduction`, the thin readout Tauri command, the Path Back screen, proactive Inbox surfacing, a dedicated eval spike fixture.

---

## File structure

- Modify `crates/finsight-core/src/spending/baseline.rs` — add `grand_monthly_mad_cents` to `Baseline`; add `trailing()` and `month_total()` helpers (the trailing-window math, moved out of the agent layer per the Phase 1 review).
- Create `crates/finsight-core/src/spending/classify.rs` — `PeriodClass`, `PeriodAssessment`, `classify_spending_period`.
- Create `crates/finsight-core/migrations/V047__spending_driver_annotations.sql`.
- Create `crates/finsight-core/src/spending/annotate.rs` — `set_annotation`, `clear_annotation`, `annotations`.
- Modify `crates/finsight-core/src/spending/mod.rs` — declare `classify`/`annotate`; add `user_verdict` to `Driver`.
- Modify `crates/finsight-core/src/spending/decompose.rs` — load + honour annotations.
- Modify `crates/finsight-agent/src/reasoning/tools/spending.rs` — refactor `explain` to use `baseline::trailing`; add `classify_spending_period` (read) + `annotate_spending_driver` (write) tools.
- Modify `crates/finsight-agent/src/reasoning/tools/mod.rs` — register the two new tools.

Locked constants/types (used across tasks):
```rust
// classify.rs
const MIN_BASELINE_MONTHS: i64 = 3;   // matches decompose.rs
const BAND_K: f64 = 2.5;              // robust sigmas above median = "elevated"
const MAD_TO_SIGMA: f64 = 1.4826;
enum PeriodClass { Normal, EpisodicSpike, RegimeShift, InsufficientHistory }
```

---

### Task 1: Baseline extensions — MAD band + trailing/month-total helpers

**Files:** Modify `crates/finsight-core/src/spending/baseline.rs`

- [ ] **Step 1: Add `grand_monthly_mad_cents` to the `Baseline` struct.** In the `pub struct Baseline { … }`, add after `grand_monthly_median_cents`:

```rust
    /// Robust spread (MAD) of the per-month grand totals — the volatility band
    /// classify uses to tell an episodic spike from a new regime.
    pub grand_monthly_mad_cents: i64,
```

- [ ] **Step 2: Populate it in `compute`.** Find the block that computes the median:
```rust
    let grand_monthly_median_cents = stats::median(&monthly_totals).round() as i64;
```
Replace with:
```rust
    let med = stats::median(&monthly_totals);
    let grand_monthly_median_cents = med.round() as i64;
    let grand_monthly_mad_cents = stats::mad(&monthly_totals, med).round() as i64;
```
Then in the returned `Ok(Baseline { … })`, add the field `grand_monthly_mad_cents,` next to `grand_monthly_median_cents,`.

- [ ] **Step 3: Add the `trailing` and `month_total` helpers** at the end of `baseline.rs` (before the `#[cfg(test)]` module):

```rust
/// The trailing `months`-month baseline ending the month BEFORE `period_ym`
/// (so the target month is never inside its own baseline). This is the
/// canonical "your normal" window; the agent tools and classify all use it.
pub fn trailing(conn: &Connection, period_ym: &str, months: i64) -> CoreResult<Baseline> {
    let (py, pm) = crate::spending::parse_ym(period_ym);
    let end = format!("{py:04}-{pm:02}"); // exclusive end = the period month itself
    let start_idx = py * 12 + (pm as i32 - 1) - months as i32;
    let start = format!("{:04}-{:02}", start_idx.div_euclid(12), start_idx.rem_euclid(12) + 1);
    compute(conn, &start, &end)
}

/// Total expense (positive cents) in one calendar month `ym` (`YYYY-MM`),
/// applying the same exclusions as the baseline (transfers + investment out).
pub fn month_total(conn: &Connection, ym: &str) -> CoreResult<i64> {
    let (y, m) = crate::spending::parse_ym(ym);
    let start = format!("{y:04}-{m:02}-01");
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let end = format!("{ny:04}-{nm:02}-01");
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT COALESCE(SUM(-t.amount_cents), 0) FROM transactions t \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND {pred} \
           AND substr(t.posted_at,1,10) >= ?1 AND substr(t.posted_at,1,10) < ?2"
    );
    let total: i64 = conn.query_row(&sql, rusqlite::params![start, end], |r| r.get(0))?;
    Ok(total)
}
```

- [ ] **Step 4: Add tests** to the `#[cfg(test)] mod tests` in `baseline.rs` (reuse its `fresh()`/`ins()` helpers):

```rust
    #[test]
    fn trailing_excludes_the_target_month_and_month_total_sums_it() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-01", -99_000, "FLAIR AIRLINES  BURNABY, BC"); // the target month

        let base = trailing(&conn, "2026-01", 12).unwrap(); // [2025-01, 2026-01)
        assert_eq!(base.months, 12);
        // The Jan flight must NOT be in the baseline.
        assert!(base.per_merchant.get(&canonical_merchant_key("FLAIR AIRLINES  BURNABY, BC")).is_none());
        assert!(base.grand_monthly_mad_cents >= 0);

        assert_eq!(month_total(&conn, "2026-01").unwrap(), 99_000);
    }
```

- [ ] **Step 5: Verify + commit.**
Run: `cargo test -p finsight-core --lib spending::baseline` — expect all PASS (existing + new).
```
git add crates/finsight-core/src/spending/baseline.rs
git commit -m "feat(spending): baseline MAD band + trailing/month_total helpers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: `classify_spending_period` core

**Files:** Create `crates/finsight-core/src/spending/classify.rs`; Modify `crates/finsight-core/src/spending/mod.rs`

- [ ] **Step 1: Declare the module.** In `crates/finsight-core/src/spending/mod.rs`, add with the other module decls (near `pub mod decompose;`): `pub mod classify;`

- [ ] **Step 2: Create `crates/finsight-core/src/spending/classify.rs`:**

```rust
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

    // Fill 2025-01..2025-12 with a steady `base` and return classify for a 2026-01 target.
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
        ins(&conn, "2026-01", -205_000, "SAVE ON FOODS  EDMONTON, AB"); // ~normal
        let a = classify_spending_period(&conn, "2026-01").unwrap();
        assert_eq!(a.class, PeriodClass::Normal, "{a:?}");
    }

    #[test]
    fn isolated_big_month_is_episodic_spike() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        setup_year(&conn, 200_000);
        ins(&conn, "2026-01", -900_000, "FLAIR AIRLINES  BURNABY, BC"); // one hot month, nothing recent
        let a = classify_spending_period(&conn, "2026-01").unwrap();
        assert_eq!(a.class, PeriodClass::EpisodicSpike, "{a:?}");
        assert!(a.period_total_cents > a.upper_band_cents);
    }

    #[test]
    fn sustained_elevation_is_regime_shift() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        setup_year(&conn, 200_000);
        // The three months before the target are ALSO hot → sustained.
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
```

- [ ] **Step 3: Verify + commit.**
Run: `cargo test -p finsight-core --lib spending::classify` — expect 4 PASS. If a class assertion differs, STOP and report the `PeriodAssessment` debug dump; do not change thresholds to force a pass without reporting.
```
git add crates/finsight-core/src/spending/classify.rs crates/finsight-core/src/spending/mod.rs
git commit -m "feat(spending): classify a month as normal / episodic spike / regime shift

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: `classify_spending_period` agent tool + refactor `explain` onto `baseline::trailing`

**Files:** Modify `crates/finsight-agent/src/reasoning/tools/spending.rs`, `crates/finsight-agent/src/reasoning/tools/mod.rs`

- [ ] **Step 1: DRY the trailing-window math.** In `spending.rs`, in `explain_spending_change`'s `execute`, replace the default-reference branch (the `_ =>` arm that hand-computes `start`/`end` and calls `baseline::compute`) with:
```rust
                _ => finsight_core::spending::baseline::trailing(ctx.conn, &period, 12)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?,
```
Leave the explicit-`reference`-month arm unchanged. Run `cargo test -p finsight-agent --lib reasoning::tools::spending` — the existing `tool_reports_new_flight_as_top_driver` test must still PASS (behaviour is identical).

- [ ] **Step 2: Add the classify tool** to `spending.rs`:
```rust
pub fn classify_spending_period() -> std::sync::Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "classify_spending_period"
        }
        fn description(&self) -> &str {
            "Judge whether a month is normal, an episodic one-off spike, or a sustained new regime, versus the user's own trailing history. Use for 'was last month a blip or my new normal?'. `period` is YYYY-MM. Returns the class plus evidence (the month's total, the normal median, the upper band, and how many recent months were also elevated) — all precomputed; quote the *_display values, don't recompute."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "period":{"type":"string","description":"Month to judge, YYYY-MM."}
            },"required":["period"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let period = args["period"].as_str().unwrap_or("");
            if period.len() < 7 {
                return Ok(json!({"error":"bad_period","note":"period must be YYYY-MM"}));
            }
            let a = finsight_core::spending::classify::classify_spending_period(ctx.conn, period)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Ok(serde_json::to_value(a)?)
        }
    }
    std::sync::Arc::new(T)
}
```

- [ ] **Step 2b: Add a test** to the `#[cfg(test)] mod tests` in `spending.rs` (reuse its `fresh()`/`ins()` helpers):
```rust
    #[test]
    fn classify_tool_flags_episodic_spike() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-01", -900_000, "FLAIR AIRLINES  BURNABY, BC");
        let mut changes = Vec::new();
        let mut drafts = Vec::new();
        let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
        let out = classify_spending_period().execute(&mut ctx, json!({"period":"2026-01"})).unwrap();
        assert_eq!(out["class"], "episodic_spike");
    }
```

- [ ] **Step 3: Register** in `crates/finsight-agent/src/reasoning/tools/mod.rs`, next to the existing `tools.register(spending::explain_spending_change());`:
```rust
    tools.register(spending::classify_spending_period());
```

- [ ] **Step 4: Verify + commit.**
Run: `cargo test -p finsight-agent --lib reasoning::tools::spending` — expect all PASS. Then `cargo test -p finsight-agent --lib` to confirm the toolset still builds.
```
git add crates/finsight-agent/src/reasoning/tools/spending.rs crates/finsight-agent/src/reasoning/tools/mod.rs
git commit -m "feat(agent): classify_spending_period tool; DRY trailing window into core

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Migration V047 + `annotate` core module

**Files:** Create `crates/finsight-core/migrations/V047__spending_driver_annotations.sql`, `crates/finsight-core/src/spending/annotate.rs`; Modify `crates/finsight-core/src/spending/mod.rs`

- [ ] **Step 1: Confirm the migration number.** Run `ls crates/finsight-core/migrations/ | sort | tail -3`. The highest must be `V046__…`. If a `V047__…` already exists (e.g. merged from another branch), STOP and report — use the next free number and tell the controller.

- [ ] **Step 2: Create `crates/finsight-core/migrations/V047__spending_driver_annotations.sql`:**
```sql
-- Sticky user verdicts on a spending "driver" (a normalized merchant). Lets the
-- engine LEARN the user's life: a flagged driver marked one_off / expected /
-- investment stops being treated as a recurring lever, across chat + screen +
-- future recomputes. Keyed by canonical_merchant_key (the same clustering key
-- the baseline/decompose use). Mirrors the transfer_override sticky-verdict idea.
CREATE TABLE spending_driver_annotations (
    merchant_key TEXT PRIMARY KEY,
    verdict      TEXT NOT NULL,          -- 'one_off' | 'expected' | 'investment'
    note         TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 3: Declare the module.** In `crates/finsight-core/src/spending/mod.rs`, add `pub mod annotate;` with the other module decls.

- [ ] **Step 4: Create `crates/finsight-core/src/spending/annotate.rs`:**
```rust
//! Sticky user verdicts on spending drivers (keyed by canonical merchant key).
//! The "remember" half of propose → confirm → remember: once set, every
//! decompose honours it so an accepted driver drops out of the "levers".

use crate::error::CoreResult;
use rusqlite::Connection;
use std::collections::HashMap;

/// The verdicts a user can stick on a driver.
pub const VERDICTS: [&str; 3] = ["one_off", "expected", "investment"];

/// Upsert a verdict for a merchant key. `verdict` must be one of [`VERDICTS`].
pub fn set_annotation(conn: &Connection, merchant_key: &str, verdict: &str, note: Option<&str>) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO spending_driver_annotations(merchant_key, verdict, note, created_at, updated_at) \
         VALUES(?1, ?2, ?3, datetime('now'), datetime('now')) \
         ON CONFLICT(merchant_key) DO UPDATE SET verdict = ?2, note = ?3, updated_at = datetime('now')",
        rusqlite::params![merchant_key, verdict, note],
    )?;
    Ok(())
}

/// Remove a verdict (the driver returns to computed persistence).
pub fn clear_annotation(conn: &Connection, merchant_key: &str) -> CoreResult<()> {
    conn.execute(
        "DELETE FROM spending_driver_annotations WHERE merchant_key = ?1",
        rusqlite::params![merchant_key],
    )?;
    Ok(())
}

/// All current verdicts as `merchant_key -> verdict`.
pub fn annotations(conn: &Connection) -> CoreResult<HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT merchant_key, verdict FROM spending_driver_annotations")?;
    let map = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("an.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn set_update_clear_roundtrip() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        set_annotation(&conn, "flair airlines", "one_off", Some("a trip")).unwrap();
        assert_eq!(annotations(&conn).unwrap().get("flair airlines").unwrap(), "one_off");
        // Upsert updates in place, no duplicate row.
        set_annotation(&conn, "flair airlines", "expected", None).unwrap();
        let m = annotations(&conn).unwrap();
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("flair airlines").unwrap(), "expected");
        clear_annotation(&conn, "flair airlines").unwrap();
        assert!(annotations(&conn).unwrap().is_empty());
    }
}
```

- [ ] **Step 5: Verify + commit.**
Run: `cargo test -p finsight-core --lib spending::annotate` — expect PASS. Also `cargo test -p finsight-core --lib spending::baseline` to confirm the new migration didn't disturb existing seeds.
```
git add crates/finsight-core/migrations/V047__spending_driver_annotations.sql crates/finsight-core/src/spending/annotate.rs crates/finsight-core/src/spending/mod.rs
git commit -m "feat(spending): sticky driver annotations table + core module (V047)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Honour annotations in `decompose`

**Files:** Modify `crates/finsight-core/src/spending/mod.rs`, `crates/finsight-core/src/spending/decompose.rs`

- [ ] **Step 1: Add `user_verdict` to `Driver`.** In `mod.rs`, in `pub struct Driver`, add after `pub persistence: Persistence,`:
```rust
    /// A sticky user verdict (one_off / expected / investment) if the user has
    /// annotated this merchant; overrides how it counts toward the "levers".
    pub user_verdict: Option<String>,
```

- [ ] **Step 2: Load + apply annotations in `decompose`.** In `decompose.rs`, inside `decompose`, right after `let target_bl = baseline::compute(...)?;`, add:
```rust
    let verdicts = crate::spending::annotate::annotations(conn)?;
```
In the `Driver { … }` construction, add the field:
```rust
            user_verdict: verdicts.get(key).cloned(),
```
Then change the persistence-subtotals loop so an annotated driver is treated as accepted (counts as one-off, never a lever). Replace:
```rust
    for d in drivers.iter().filter(|d| d.delta_cents > 0) {
        match d.persistence {
            Persistence::Recurring => subtotals.recurring_cents += d.delta_cents,
            Persistence::OneOff => subtotals.one_off_cents += d.delta_cents,
            Persistence::Emerging => subtotals.emerging_cents += d.delta_cents,
            Persistence::Uncertain => subtotals.uncertain_cents += d.delta_cents,
        }
    }
```
with:
```rust
    for d in drivers.iter().filter(|d| d.delta_cents > 0) {
        // A user-annotated driver is accepted — it never counts as a recurring
        // "lever"; fold it into one-off so the levers total drops it.
        let effective = if d.user_verdict.is_some() { Persistence::OneOff } else { d.persistence };
        match effective {
            Persistence::Recurring => subtotals.recurring_cents += d.delta_cents,
            Persistence::OneOff => subtotals.one_off_cents += d.delta_cents,
            Persistence::Emerging => subtotals.emerging_cents += d.delta_cents,
            Persistence::Uncertain => subtotals.uncertain_cents += d.delta_cents,
        }
    }
```

- [ ] **Step 3: Add a test** to the `#[cfg(test)] mod tests` in `decompose.rs` (reuse its `fresh()`/`ins()` helpers):
```rust
    #[test]
    fn annotation_drops_driver_from_recurring_levers() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        // A recurring-looking rising merchant across the baseline + target.
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
        assert_eq!(after.persistence_subtotals.recurring_cents, 0, "annotated driver leaves the levers");
    }
```

- [ ] **Step 4: Verify + commit.**
Run: `cargo test -p finsight-core --lib spending::decompose` — expect all PASS (existing + new). The existing decompose tests must still pass (they set no annotations → `user_verdict` is `None`).
```
git add crates/finsight-core/src/spending/mod.rs crates/finsight-core/src/spending/decompose.rs
git commit -m "feat(spending): decompose honours sticky annotations (accepted = not a lever)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: `annotate_spending_driver` agent tool (direct write)

**Files:** Modify `crates/finsight-agent/src/reasoning/tools/spending.rs`, `crates/finsight-agent/src/reasoning/tools/mod.rs`

- [ ] **Step 1: Add the tool** to `spending.rs`. Unlike the `draft_*` act tools this one writes directly — it's a reversible user verdict (same class as `transfer_override`), so it persists immediately and records a change:
```rust
pub fn annotate_spending_driver() -> std::sync::Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "annotate_spending_driver"
        }
        fn description(&self) -> &str {
            "Remember the user's verdict on a spending driver so it stops showing as a recurring lever everywhere. Pass the `merchant_key` exactly as returned by explain_spending_change. `verdict`: one_off (a one-time thing), expected (a known/accepted cost), investment (spending the user considers an investment), or reset (forget a prior verdict). This WRITES immediately and is remembered across sessions. Only call it when the user has actually told you their verdict."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "merchant_key":{"type":"string","description":"canonical merchant key from explain_spending_change output"},
                "verdict":{"type":"string","enum":["one_off","expected","investment","reset"]},
                "note":{"type":"string"}
            },"required":["merchant_key","verdict"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            use crate::reasoning::messages::AgentChange;
            let key = args["merchant_key"].as_str().unwrap_or("").trim();
            let verdict = args["verdict"].as_str().unwrap_or("");
            if key.is_empty() {
                return Ok(json!({"error":"missing_merchant_key"}));
            }
            let note = args["note"].as_str();
            if verdict == "reset" {
                finsight_core::spending::annotate::clear_annotation(ctx.conn, key)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            } else if finsight_core::spending::annotate::VERDICTS.contains(&verdict) {
                finsight_core::spending::annotate::set_annotation(ctx.conn, key, verdict, note)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            } else {
                return Ok(json!({"error":"bad_verdict","note":"verdict must be one_off, expected, investment, or reset"}));
            }
            ctx.changes.push(AgentChange {
                kind: "spending_annotation".to_string(),
                description: format!("Marked '{key}' as {verdict}"),
            });
            Ok(json!({"saved": true, "merchant_key": key, "verdict": verdict}))
        }
    }
    std::sync::Arc::new(T)
}
```

- [ ] **Step 2: Add a test** to `spending.rs` tests (reuse `fresh()`/`ins()`):
```rust
    #[test]
    fn annotate_tool_writes_a_sticky_verdict() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let mut changes = Vec::new();
        let mut drafts = Vec::new();
        {
            let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
            let out = annotate_spending_driver()
                .execute(&mut ctx, json!({"merchant_key":"flair airlines","verdict":"one_off"}))
                .unwrap();
            assert_eq!(out["saved"], true);
        }
        assert_eq!(
            finsight_core::spending::annotate::annotations(&conn).unwrap().get("flair airlines").unwrap(),
            "one_off"
        );
        assert_eq!(changes.len(), 1);
    }
```

- [ ] **Step 3: Register** in `mod.rs`, next to the other spending registrations:
```rust
    tools.register(spending::annotate_spending_driver());
```

- [ ] **Step 4: Verify + commit.**
Run: `cargo test -p finsight-agent --lib reasoning::tools::spending` then `cargo test -p finsight-agent --lib`.
```
git add crates/finsight-agent/src/reasoning/tools/spending.rs crates/finsight-agent/src/reasoning/tools/mod.rs
git commit -m "feat(agent): annotate_spending_driver — sticky verdict write (remember loop)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: Full green-bar verification

- [ ] **Step 1: Run the affected surface.**
```
cargo test -p finsight-core --lib spending
cargo test -p finsight-agent --lib reasoning::tools
```
Expected: all PASS.

- [ ] **Step 2: Clippy the new code.**
Run: `cargo clippy -p finsight-core --lib 2>&1 | grep -iE "spending/(classify|annotate)|classify::|annotate::"` — expect no output (no warnings). Fix any that appear. (Do NOT `git add -A`; if a fix is needed, add the specific file.)

- [ ] **Step 3: Commit any fixes** with an explicit `git add <file>` of only the changed spending files.

---

## Self-review

**Spec coverage:**
- §7 `classify_spending_period` — Task 2 (core) + Task 3 (tool); normal/episodic/regime + insufficient-history. ✓
- §4/§6 `emerging` + annotation override; §10 migration; §11 "that Flair burst was one-time → leaves levers everywhere" — Tasks 4–6 (migration + annotate module + decompose honouring + write tool). ✓
- §4 cold-start honesty — classify returns `InsufficientHistory` under `MIN_BASELINE_MONTHS` (Task 2, tested). ✓
- §5 reconciliation — all logic in `finsight-core::spending`; tools are thin wrappers; the trailing-window math is now a core helper (Task 1/3), fixing the Phase 1 review's "window math in the agent layer" note. ✓
- §3 zero-arithmetic — both tools return precomputed `_cents` (auto-`_display`); the annotate tool only writes a verdict. ✓

**Placeholder scan:** none. The one discovery step (Task 4 Step 1, confirm migration number) has a concrete STOP-and-report fallback.

**Type consistency:** `PeriodClass`/`PeriodAssessment` defined once (classify.rs) and used unchanged in the tool. `Baseline.grand_monthly_mad_cents` added in Task 1, consumed in Task 2. `Driver.user_verdict` added in Task 5 Step 1, populated in Task 5 Step 2, asserted in Task 5 Step 3 and Task 6. `annotate::{set_annotation, clear_annotation, annotations, VERDICTS}` signatures identical at all call sites (decompose, the tool). Migration is `V047` (verified highest in `main` is `V046`).

**Known assumption to verify at execution:** migration number is free in `main` (Task 4 Step 1 guards it).

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-14-spending-analysis-engine-phase2.md`. Continuing with **Subagent-Driven** execution (same as Phase 1) unless you say otherwise.
