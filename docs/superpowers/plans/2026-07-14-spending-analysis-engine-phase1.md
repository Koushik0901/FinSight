# Spending Analysis Engine — Phase 1 (walking skeleton) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the core spending-analysis engine (`finsight-core::spending`) and expose it through one composable Copilot tool, `explain_spending_change`, so the Copilot can deterministically answer "why was <month> so high?", "which merchants are new vs my normal?", "what doubled?", "how much of it will recur?", and "compare <month A> to <month B>".

**Architecture:** All computation lives in `finsight-core::spending` (pure, deterministic, `no_std`-of-Tauri, unit-tested) so every future consumer reconciles to the same numbers. A thin `Tool` wrapper in `finsight-agent` calls it; the LLM only selects the tool and narrates its pre-computed, pre-classified output (zero arithmetic). Drivers are clustered on the existing `canonical_merchant_key` normalizer and compared against a robust "your normal" baseline; each driver is tagged on two axes — mechanism (new/stopped/price/frequency) and persistence (one-off/recurring/emerging/uncertain).

**Tech Stack:** Rust (rusqlite, serde, chrono), the existing `finsight-agent` reasoning-tool trait, `finsight-eval` harness.

**Scope note:** This is the first of several plans for the engine (spec: `docs/superpowers/specs/2026-07-14-spending-analysis-engine-design.md`). Phase 1 delivers working, testable software on its own. Follow-on plans (not in this document): `classify_spending_period`; sticky `annotate_spending_driver` + migration; `get_spending_baseline` + `plan_spending_reduction`; the thin readout Tauri command + the Path Back screen; proactive Insights.

---

## File structure

- Create `crates/finsight-core/src/spending/mod.rs` — module root: shared types (`Window`, `Mechanism`, `Persistence`, `Driver`, `PersistenceSubtotals`, `DecomposeResult`) + submodule declarations.
- Create `crates/finsight-core/src/spending/stats.rs` — `median`, `mad` robust helpers.
- Create `crates/finsight-core/src/spending/baseline.rs` — `MerchantBaseline`, `Baseline`, `compute`, `for_month`.
- Create `crates/finsight-core/src/spending/decompose.rs` — `decompose` (window-vs-baseline driver decomposition + two-axis tagging) and its filters.
- Modify `crates/finsight-core/src/lib.rs` — add `pub mod spending;`.
- Create `crates/finsight-agent/src/reasoning/tools/spending.rs` — the `explain_spending_change` `Tool`.
- Modify `crates/finsight-agent/src/reasoning/tools/mod.rs` — declare `pub mod spending;` and register the tool in `standard_toolset()`.
- Modify `crates/finsight-eval/src/seed.rs` — add a deterministic "hot month" (one brand-new merchant + one doubled merchant) so the engine has a known regime to detect.
- Create `eval/subset_spending.jsonl` — two benchmark cases exercising the tool end-to-end.

Type contract locked here (used across all tasks):

```rust
pub struct Window { pub start: String, pub end: String, pub months: f64 }   // [start,end) YYYY-MM-DD

pub enum Mechanism { New, Stopped, PriceUp, PriceDown, FrequencyUp, FrequencyDown, Mixed, Flat }
pub enum Persistence { OneOff, Recurring, Emerging, Uncertain }

pub struct Driver {
    pub merchant_key: String, pub display: String, pub category: Option<String>,
    pub delta_cents: i64, pub recent_monthly_cents: i64, pub base_monthly_cents: i64,
    pub recent_txns_per_month: f64, pub base_txns_per_month: f64,
    pub mechanism: Mechanism, pub persistence: Persistence,
}
pub struct PersistenceSubtotals { pub recurring_cents: i64, pub one_off_cents: i64, pub emerging_cents: i64, pub uncertain_cents: i64 }
pub struct DecomposeResult {
    pub currency: String, pub target_total_cents: i64, pub baseline_monthly_cents: i64,
    pub gap_cents: i64, pub drivers: Vec<Driver>, pub persistence_subtotals: PersistenceSubtotals, pub note: String,
}
```

---

### Task 1: Robust statistics helpers

**Files:**
- Create: `crates/finsight-core/src/spending/stats.rs`
- Create: `crates/finsight-core/src/spending/mod.rs` (stub declaring `pub mod stats;`)
- Modify: `crates/finsight-core/src/lib.rs` (add `pub mod spending;`)

- [ ] **Step 1: Create the module stub and register it**

In `crates/finsight-core/src/spending/mod.rs`:

```rust
//! Spending Analysis Engine — deterministic "what changed vs your normal".
pub mod stats;
```

In `crates/finsight-core/src/lib.rs`, add alongside the other `pub mod` lines (e.g. near `pub mod recurring;`):

```rust
pub mod spending;
```

- [ ] **Step 2: Write the failing test**

In `crates/finsight-core/src/spending/stats.rs`:

```rust
//! Robust statistics (median / MAD) — the same principle anomaly.rs uses,
//! reused so "your normal" resists a few hot months poisoning it.

/// Median of a slice. Returns 0.0 for an empty slice.
pub fn median(xs: &[f64]) -> f64 {
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

/// Median absolute deviation about `med`. Returns 0.0 for an empty slice.
pub fn mad(xs: &[f64], med: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let devs: Vec<f64> = xs.iter().map(|x| (x - med).abs()).collect();
    median(&devs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_ignores_outliers_that_mean_would_chase() {
        // 12 months: eleven ~2000 and one 9000 spike.
        let mut months = vec![2000.0; 11];
        months.push(9000.0);
        assert_eq!(median(&months), 2000.0, "median stays at the true normal");
        let mean = months.iter().sum::<f64>() / months.len() as f64;
        assert!(mean > 2500.0, "the mean is dragged up by the spike");
    }

    #[test]
    fn mad_measures_spread_about_the_median() {
        assert_eq!(median(&[]), 0.0);
        assert_eq!(mad(&[5.0], 5.0), 0.0);
        assert_eq!(mad(&[1.0, 2.0, 3.0, 4.0, 5.0], 3.0), 1.0);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail, then pass**

Run: `cargo test -p finsight-core --lib spending::stats::tests`
Expected: PASS (the impl is included above — this task pairs the impl with its test since it is pure arithmetic with no dependencies). If it does not compile, fix `lib.rs` module registration.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/spending/mod.rs crates/finsight-core/src/spending/stats.rs crates/finsight-core/src/lib.rs
git commit -m "feat(spending): robust median/MAD stats helpers"
```

---

### Task 2: The "your normal" baseline — types + month helpers

**Files:**
- Create: `crates/finsight-core/src/spending/baseline.rs`
- Modify: `crates/finsight-core/src/spending/mod.rs` (add `pub mod baseline;` and the `Window` type)

- [ ] **Step 1: Add `Window` + month helpers with a failing test**

In `crates/finsight-core/src/spending/mod.rs`, append:

```rust
pub mod baseline;

/// A half-open date window `[start, end)` in `YYYY-MM-DD`, plus the number of
/// whole calendar months it spans (used to convert a window total into a
/// monthly-equivalent so a 1-month window and a 12-month baseline compare).
#[derive(Debug, Clone)]
pub struct Window {
    pub start: String,
    pub end: String,
    pub months: f64,
}

impl Window {
    /// The single calendar month `ym` (`YYYY-MM`) as a `[first, next-first)` window.
    pub fn for_month(ym: &str) -> Window {
        let (y, m) = parse_ym(ym);
        let start = format!("{y:04}-{m:02}-01");
        let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
        Window { start, end: format!("{ny:04}-{nm:02}-01"), months: 1.0 }
    }
}

/// Parse `YYYY-MM` into `(year, month)`. Defaults to `(1970, 1)` on garbage so
/// callers never panic on user/LLM input.
pub fn parse_ym(ym: &str) -> (i32, u32) {
    let mut it = ym.split('-');
    let y = it.next().and_then(|s| s.parse().ok()).unwrap_or(1970);
    let m = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    (y, m.clamp(1, 12))
}

/// Count of whole calendar months in `[start_ym, end_ym)` (both `YYYY-MM`).
pub fn months_between(start_ym: &str, end_ym: &str) -> i64 {
    let (sy, sm) = parse_ym(start_ym);
    let (ey, em) = parse_ym(end_ym);
    ((ey * 12 + em as i32) - (sy * 12 + sm as i32)) as i64
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn for_month_builds_a_half_open_window() {
        let w = Window::for_month("2026-05");
        assert_eq!(w.start, "2026-05-01");
        assert_eq!(w.end, "2026-06-01");
        assert_eq!(w.months, 1.0);
        let w = Window::for_month("2026-12");
        assert_eq!(w.end, "2027-01-01");
    }

    #[test]
    fn months_between_counts_calendar_months() {
        assert_eq!(months_between("2025-04", "2026-04"), 12);
        assert_eq!(months_between("2026-05", "2026-06"), 1);
    }
}
```

- [ ] **Step 2: Run to verify pass**

Run: `cargo test -p finsight-core --lib spending::window_tests`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/src/spending/mod.rs
git commit -m "feat(spending): Window type and month arithmetic helpers"
```

---

### Task 3: Baseline computation (the "your normal" model)

**Files:**
- Modify: `crates/finsight-core/src/spending/baseline.rs`

- [ ] **Step 1: Write the failing test**

In `crates/finsight-core/src/spending/baseline.rs`:

```rust
//! "Your normal": robust per-merchant monthly baselines + the grand monthly
//! median, computed on read from the ledger. Clusters on `canonical_merchant_key`
//! (the same normalizer categorization/recurring use) so a merchant's variants
//! collapse to one stream. Honors the metrics-layer exclusions (transfers and
//! investment activity are never spending).

use crate::error::CoreResult;
use crate::merchant::canonical_merchant_key;
use crate::spending::{months_between, stats};
use rusqlite::Connection;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MerchantBaseline {
    pub display: String,
    pub category: Option<String>,
    /// Mean monthly spend for this merchant over the baseline (total ÷ months).
    pub monthly_cents: i64,
    /// Mean transactions per month over the baseline.
    pub txns_per_month: f64,
    /// Distinct calendar months this merchant had any spend in.
    pub active_months: i64,
}

#[derive(Debug, Clone)]
pub struct Baseline {
    /// Whole calendar months the baseline spans.
    pub months: i64,
    /// Robust "normal" monthly spend: median of the per-month grand totals.
    pub grand_monthly_median_cents: i64,
    /// Keyed by `canonical_merchant_key`.
    pub per_merchant: HashMap<String, MerchantBaseline>,
    /// Dominant account currency in the window (v1 analyzes one currency).
    pub currency: String,
    /// True when more than one currency appeared (drives a caller warning).
    pub mixed_currency: bool,
}

struct Row {
    key: String,
    display: String,
    ym: String,
    amount_abs: i64,
    category: Option<String>,
    currency: String,
}

/// Load expense rows in `[start, end)` (YYYY-MM-DD), normalized + clustered.
fn load_rows(conn: &Connection, start: &str, end: &str) -> CoreResult<Vec<Row>> {
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT t.merchant_raw, substr(t.posted_at,1,7) AS ym, t.amount_cents, \
                (SELECT label FROM categories c WHERE c.id = t.category_id), \
                COALESCE(a.currency, 'USD') \
         FROM transactions t JOIN accounts a ON a.id = t.account_id \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND {pred} \
           AND substr(t.posted_at,1,10) >= ?1 AND substr(t.posted_at,1,10) < ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![start, end], |r| {
            let raw: String = r.get(0)?;
            Ok(Row {
                key: canonical_merchant_key(&raw),
                display: crate::merchant::split_display(&raw),
                ym: r.get(1)?,
                amount_abs: r.get::<_, i64>(2)?.unsigned_abs() as i64,
                category: r.get(3)?,
                currency: r.get(4)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Compute the baseline over `[start_ym, end_ym)` (both `YYYY-MM`).
pub fn compute(conn: &Connection, start_ym: &str, end_ym: &str) -> CoreResult<Baseline> {
    let (sy, sm) = crate::spending::parse_ym(start_ym);
    let (ey, em) = crate::spending::parse_ym(end_ym);
    let start = format!("{sy:04}-{sm:02}-01");
    let end = format!("{ey:04}-{em:02}-01");
    let months = months_between(start_ym, end_ym).max(1);
    let rows = load_rows(conn, &start, &end)?;

    // Dominant currency.
    let mut cur_tot: HashMap<String, i64> = HashMap::new();
    for r in &rows {
        *cur_tot.entry(r.currency.clone()).or_default() += r.amount_abs;
    }
    let mixed_currency = cur_tot.len() > 1;
    let currency = cur_tot
        .iter()
        .max_by_key(|(_, v)| **v)
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| "USD".to_string());

    // Per (merchant, month) and per-month grand totals — dominant currency only.
    let mut m_month: HashMap<String, HashMap<String, (i64, i64)>> = HashMap::new(); // key -> ym -> (sum, count)
    let mut m_display: HashMap<String, String> = HashMap::new();
    let mut m_cat: HashMap<String, Option<String>> = HashMap::new();
    let mut grand: HashMap<String, i64> = HashMap::new(); // ym -> sum
    for r in rows.into_iter().filter(|r| r.currency == currency) {
        let e = m_month.entry(r.key.clone()).or_default().entry(r.ym.clone()).or_insert((0, 0));
        e.0 += r.amount_abs;
        e.1 += 1;
        m_display.entry(r.key.clone()).or_insert(r.display);
        m_cat.entry(r.key.clone()).or_insert(r.category);
        *grand.entry(r.ym).or_default() += r.amount_abs;
    }

    let per_merchant = m_month
        .into_iter()
        .map(|(key, by_month)| {
            let total: i64 = by_month.values().map(|(s, _)| *s).sum();
            let count: i64 = by_month.values().map(|(_, c)| *c).sum();
            let mb = MerchantBaseline {
                display: m_display.remove(&key).unwrap_or_else(|| key.clone()),
                category: m_cat.remove(&key).flatten(),
                monthly_cents: total / months,
                txns_per_month: count as f64 / months as f64,
                active_months: by_month.len() as i64,
            };
            (key, mb)
        })
        .collect();

    // Robust grand monthly: median over ALL baseline months, counting months
    // with no spend as 0 so a couple of quiet months pull the normal down, not
    // up. Build the full month vector from the span, not just months present.
    let mut monthly_totals: Vec<f64> = Vec::with_capacity(months as usize);
    for i in 0..months {
        let idx = sy * 12 + (sm as i32 - 1) + i as i32;
        let ym = format!("{:04}-{:02}", idx.div_euclid(12), idx.rem_euclid(12) + 1);
        monthly_totals.push(*grand.get(&ym).unwrap_or(&0) as f64);
    }
    let grand_monthly_median_cents = stats::median(&monthly_totals).round() as i64;

    Ok(Baseline {
        months,
        grand_monthly_median_cents,
        per_merchant,
        currency,
        mixed_currency,
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
        let db = Db::open(&dir.path().join("b.sqlcipher"), &key).unwrap();
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
    fn baseline_is_robust_and_per_merchant() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        // 12 months: $2,000 groceries each month, plus one $9,000 spike month.
        for i in 0..12 {
            let ym = format!("2025-{:02}", i + 1);
            ins(&conn, &ym, -200_000, "SAVE ON FOODS #1 EDMONTON, AB");
        }
        ins(&conn, "2025-06", -700_000, "FLAIR AIRLINES BURNABY, BC"); // spike in June

        let b = compute(&conn, "2025-01", "2026-01").unwrap();
        assert_eq!(b.months, 12);
        // Grand monthly median stays ~ the groceries level, not dragged up by the spike.
        assert!(b.grand_monthly_median_cents <= 220_000, "median resists the spike: {}", b.grand_monthly_median_cents);
        let groceries = b.per_merchant.get(&canonical_merchant_key("SAVE ON FOODS #1 EDMONTON, AB")).unwrap();
        assert_eq!(groceries.monthly_cents, 200_000);
        assert_eq!(groceries.active_months, 12);
    }
}
```

Note: this test references `crate::merchant::split_display`. If that helper does not exist, add it in Step 2.

- [ ] **Step 2: Ensure `merchant::split_display` exists (add if missing)**

Check: `grep -n "pub fn split_display" crates/finsight-core/src/merchant.rs`

If absent, add to `crates/finsight-core/src/merchant.rs`:

```rust
/// A human-facing merchant label: the segment before the first run of 2+
/// spaces (statement city/padding), trimmed. Used for display next to a
/// canonical key. Falls back to the whole string.
pub fn split_display(raw: &str) -> String {
    raw.split("  ").next().unwrap_or(raw).trim().to_string()
}
```

(If a similar helper already exists with a different name, use that name in `baseline.rs` instead and skip this step.)

- [ ] **Step 3: Run to verify it fails, then passes**

Run: `cargo test -p finsight-core --lib spending::baseline::tests::baseline_is_robust_and_per_merchant`
Expected first: FAIL to compile (`compute` / `split_display` missing) → after Steps 1–2, PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/spending/baseline.rs crates/finsight-core/src/spending/mod.rs crates/finsight-core/src/merchant.rs
git commit -m "feat(spending): robust per-merchant baseline model"
```

---

### Task 4: Two-axis tagging — mechanism + persistence

**Files:**
- Create: `crates/finsight-core/src/spending/decompose.rs`
- Modify: `crates/finsight-core/src/spending/mod.rs` (add `pub mod decompose;` + the shared enums/structs)

- [ ] **Step 1: Add the shared result types to `mod.rs`**

In `crates/finsight-core/src/spending/mod.rs`, append:

```rust
pub mod decompose;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mechanism { New, Stopped, PriceUp, PriceDown, FrequencyUp, FrequencyDown, Mixed, Flat }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Persistence { OneOff, Recurring, Emerging, Uncertain }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Driver {
    pub merchant_key: String,
    pub display: String,
    pub category: Option<String>,
    pub delta_cents: i64,
    pub recent_monthly_cents: i64,
    pub base_monthly_cents: i64,
    pub recent_txns_per_month: f64,
    pub base_txns_per_month: f64,
    pub mechanism: Mechanism,
    pub persistence: Persistence,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistenceSubtotals {
    pub recurring_cents: i64,
    pub one_off_cents: i64,
    pub emerging_cents: i64,
    pub uncertain_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposeResult {
    pub currency: String,
    pub target_total_cents: i64,
    pub baseline_monthly_cents: i64,
    pub gap_cents: i64,
    pub drivers: Vec<Driver>,
    pub persistence_subtotals: PersistenceSubtotals,
    pub note: String,
}
```

- [ ] **Step 2: Write the failing test for classification**

In `crates/finsight-core/src/spending/decompose.rs`:

```rust
//! Window-vs-baseline decomposition: rank the drivers of a period's spend
//! against "your normal" and tag each on two axes — mechanism (where the
//! delta came from) and persistence (will it repeat). The LLM never computes
//! these; it narrates them.

use crate::spending::{Mechanism, Persistence};

/// Ratio at/above which a change in ticket size or frequency is "up"/"down".
const CHANGE_RATIO: f64 = 1.3;

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
        return Persistence::Emerging; // new and already repeating within the window
    }
    if total_txns <= 2 {
        return Persistence::OneOff;
    }
    Persistence::Uncertain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mechanism_distinguishes_new_price_frequency() {
        assert_eq!(classify_mechanism(5000, 0, 1.0, 0.0), Mechanism::New);
        assert_eq!(classify_mechanism(0, 5000, 0.0, 1.0), Mechanism::Stopped);
        // Same ~1 txn/mo, ticket doubled → PriceUp.
        assert_eq!(classify_mechanism(20000, 10000, 1.0, 1.0), Mechanism::PriceUp);
        // Same ticket, 5x the visits → FrequencyUp.
        assert_eq!(classify_mechanism(23500, 9500, 11.0, 7.0), Mechanism::FrequencyUp);
        // Steady.
        assert_eq!(classify_mechanism(10000, 9800, 2.0, 2.0), Mechanism::Flat);
    }

    #[test]
    fn persistence_reads_structure() {
        assert_eq!(classify_persistence(Mechanism::PriceUp, 8, 20, 3), Persistence::Recurring);
        assert_eq!(classify_persistence(Mechanism::New, 1, 3, 3), Persistence::Emerging);
        assert_eq!(classify_persistence(Mechanism::New, 1, 1, 1), Persistence::OneOff);
    }
}
```

- [ ] **Step 3: Run to verify it fails, then passes**

Run: `cargo test -p finsight-core --lib spending::decompose::tests`
Expected first: FAIL to compile (module/types missing) → after Step 1, PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/spending/mod.rs crates/finsight-core/src/spending/decompose.rs
git commit -m "feat(spending): two-axis mechanism + persistence classifiers"
```

---

### Task 5: `decompose` — assemble ranked, tagged drivers vs the baseline

**Files:**
- Modify: `crates/finsight-core/src/spending/decompose.rs`

- [ ] **Step 1: Write the failing integration test**

Append to `crates/finsight-core/src/spending/decompose.rs` (above the existing `#[cfg(test)]` block, add the public function; then extend the test module):

```rust
use crate::error::CoreResult;
use crate::spending::baseline::{self, Baseline};
use crate::spending::{Driver, DecomposeResult, PersistenceSubtotals, Window};
use rusqlite::Connection;
use std::collections::HashMap;

/// How the target window is filtered before ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter { All, New, Elevated }

/// Minimum periods of history before we will claim "what changed" rather than
/// just "where it's going" (cold-start honesty).
const MIN_BASELINE_MONTHS: i64 = 3;

/// Decompose `target` against `reference` (a pre-computed baseline — either the
/// trailing-12-month normal or a single comparison month). `min_ratio` applies
/// only to `Filter::Elevated`.
pub fn decompose(
    conn: &Connection,
    target: &Window,
    reference: &Baseline,
    filter: Filter,
    min_ratio: f64,
    limit: usize,
) -> CoreResult<DecomposeResult> {
    // Target-window per-merchant aggregates, same normalization + exclusions.
    let target_bl = baseline::compute(conn, &ym_of(&target.start), &ym_end_of(target))?;
    let months = target.months.max(1.0);

    let mut keys: std::collections::HashSet<String> = HashSet::from_iter_keys(&target_bl, reference);
    let mut drivers: Vec<Driver> = Vec::new();
    let mut target_total: i64 = 0;

    for key in keys.drain() {
        let t = target_bl.per_merchant.get(&key);
        let b = reference.per_merchant.get(&key);
        let recent_monthly = t.map(|m| m.monthly_cents).unwrap_or(0);
        let base_monthly = b.map(|m| m.monthly_cents).unwrap_or(0);
        let recent_pm = t.map(|m| m.txns_per_month).unwrap_or(0.0);
        let base_pm = b.map(|m| m.txns_per_month).unwrap_or(0.0);
        target_total += (recent_monthly as f64 * months).round() as i64;

        let mechanism = classify_mechanism(recent_monthly, base_monthly, recent_pm, base_pm);
        let active = t.map(|m| m.active_months).unwrap_or(0) + b.map(|m| m.active_months).unwrap_or(0);
        let total_txns = ((recent_pm + base_pm) * months).round() as i64;
        let target_txns = (recent_pm * months).round() as i64;
        let persistence = classify_persistence(mechanism, active, total_txns, target_txns);

        let driver = Driver {
            merchant_key: key.clone(),
            display: t.or(b).map(|m| m.display.clone()).unwrap_or(key),
            category: t.and_then(|m| m.category.clone()).or_else(|| b.and_then(|m| m.category.clone())),
            delta_cents: recent_monthly - base_monthly,
            recent_monthly_cents: recent_monthly,
            base_monthly_cents: base_monthly,
            recent_txns_per_month: recent_pm,
            base_txns_per_month: base_pm,
            mechanism,
            persistence,
        };
        if passes(&driver, filter, min_ratio) {
            drivers.push(driver);
        }
    }

    drivers.sort_by(|a, b| b.delta_cents.cmp(&a.delta_cents));
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
        format!("Only {} month(s) of baseline history — showing where money went, but the 'what changed' tags are low-confidence.", reference.months)
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

fn ym_of(date: &str) -> String { date[..7].to_string() }
fn ym_end_of(w: &Window) -> String { w.end[..7].to_string() }

trait HashSetFromKeys { fn from_iter_keys(t: &Baseline, r: &Baseline) -> Self; }
impl HashSetFromKeys for std::collections::HashSet<String> {
    fn from_iter_keys(t: &Baseline, r: &Baseline) -> Self {
        t.per_merchant.keys().chain(r.per_merchant.keys()).cloned().collect()
    }
}
use std::collections::HashSet;
```

Then extend the `#[cfg(test)]` module with:

```rust
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

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
        // Baseline Jan..Dec 2025: $200/mo groceries every month.
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS EDMONTON, AB");
        }
        // May 2026 (the hot month): groceries doubled + a brand-new flight.
        ins(&conn, "2026-05", -40_000, "SAVE ON FOODS EDMONTON, AB");
        ins(&conn, "2026-05", -60_000, "FLAIR AIRLINES BURNABY, BC");

        let base = baseline::compute(&conn, "2025-01", "2026-01").unwrap();
        let may = Window::for_month("2026-05");
        let out = decompose(&conn, &may, &base, Filter::All, 2.0, 20).unwrap();

        // Flight is the biggest driver, tagged New.
        assert_eq!(out.drivers[0].display, "FLAIR AIRLINES");
        assert_eq!(out.drivers[0].mechanism, Mechanism::New);
        // Groceries present and elevated (doubled).
        let groc = out.drivers.iter().find(|d| d.display == "SAVE ON FOODS").unwrap();
        assert_eq!(groc.recent_monthly_cents, 40_000);
        assert_eq!(groc.base_monthly_cents, 20_000);

        // Filter=New returns only the flight.
        let only_new = decompose(&conn, &may, &base, Filter::New, 2.0, 20).unwrap();
        assert!(only_new.drivers.iter().all(|d| d.mechanism == Mechanism::New));
        assert!(only_new.drivers.iter().any(|d| d.display == "FLAIR AIRLINES"));

        // Filter=Elevated (>=2x) returns groceries.
        let elevated = decompose(&conn, &may, &base, Filter::Elevated, 2.0, 20).unwrap();
        assert!(elevated.drivers.iter().any(|d| d.display == "SAVE ON FOODS"));
    }
```

- [ ] **Step 2: Run to verify it fails, then passes**

Run: `cargo test -p finsight-core --lib spending::decompose::tests::decompose_finds_new_and_elevated_drivers`
Expected first: FAIL to compile → after wiring, PASS. Also run the whole module: `cargo test -p finsight-core --lib spending`.

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/src/spending/decompose.rs
git commit -m "feat(spending): decompose window vs baseline into ranked tagged drivers"
```

---

### Task 6: The `explain_spending_change` Copilot tool

**Files:**
- Create: `crates/finsight-agent/src/reasoning/tools/spending.rs`
- Modify: `crates/finsight-agent/src/reasoning/tools/mod.rs`

- [ ] **Step 1: Write the tool (mirrors the `Tool` impls in `read.rs`)**

Create `crates/finsight-agent/src/reasoning/tools/spending.rs`:

```rust
use crate::reasoning::tools::{Tool, ToolContext};
use anyhow::Result;
use finsight_core::spending::baseline::{self, Baseline};
use finsight_core::spending::decompose::{decompose, Filter};
use finsight_core::spending::Window;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn explain_spending_change() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "explain_spending_change"
        }
        fn description(&self) -> &str {
            "Explain WHAT CHANGED in a month's spending versus the user's normal — the ranked drivers of the difference, each tagged with a mechanism (new / price_up / frequency_up / stopped) and a persistence (recurring / one_off / emerging). Use for 'why was <month> so high', 'what's new this month vs my usual', 'what doubled', 'how much of the increase will recur', and 'compare <month> to <other month>'. `period` is a YYYY-MM month. By default it compares against the trailing-12-month baseline; pass `reference` (YYYY-MM) to compare two specific months. Every number is precomputed — quote the *_display strings and the persistence_subtotals; do not add or divide amounts yourself."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "period": {"type":"string","description":"Target month, YYYY-MM (e.g. 2026-05)."},
                "reference": {"type":"string","description":"Optional comparison month YYYY-MM. Omit to compare against the trailing-12-month normal."},
                "filter": {"type":"string","enum":["all","new","elevated"],"default":"all","description":"'new' = only merchants absent from the baseline; 'elevated' = only merchants at least min_ratio× their usual."},
                "min_ratio": {"type":"number","default":2.0,"description":"Threshold for filter='elevated'."},
                "limit": {"type":"integer","default":12}
            },"required":["period"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let period = args["period"].as_str().unwrap_or("").to_string();
            if period.len() < 7 {
                return Ok(json!({"error":"bad_period","note":"period must be YYYY-MM"}));
            }
            let filter = match args["filter"].as_str().unwrap_or("all") {
                "new" => Filter::New,
                "elevated" => Filter::Elevated,
                _ => Filter::All,
            };
            let min_ratio = args["min_ratio"].as_f64().unwrap_or(2.0);
            let limit = args["limit"].as_i64().unwrap_or(12).clamp(1, 50) as usize;

            // Reference: an explicit month, else the trailing 12 months ending
            // at the month BEFORE `period` (so the target isn't in its own baseline).
            let reference: Baseline = match args["reference"].as_str() {
                Some(rm) if rm.len() >= 7 => {
                    let (ry, rmn) = finsight_core::spending::parse_ym(rm);
                    let end = if rmn == 12 { format!("{}-01", ry + 1) } else { format!("{ry:04}-{:02}", rmn + 1) };
                    baseline::compute(ctx.conn, rm, &end).map_err(|e| anyhow::anyhow!(e.to_string()))?
                }
                _ => {
                    let (py, pm) = finsight_core::spending::parse_ym(&period);
                    let end = format!("{py:04}-{pm:02}"); // exclusive: month before period
                    let start_idx = py * 12 + (pm as i32 - 1) - 12;
                    let start = format!("{:04}-{:02}", start_idx.div_euclid(12), start_idx.rem_euclid(12) + 1);
                    baseline::compute(ctx.conn, &start, &end).map_err(|e| anyhow::anyhow!(e.to_string()))?
                }
            };

            let target = Window::for_month(&period);
            let out = decompose(ctx.conn, &target, &reference, filter, min_ratio, limit)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Ok(serde_json::to_value(out)?)
        }
    }
    Arc::new(T)
}
```

- [ ] **Step 2: Register the module + tool**

In `crates/finsight-agent/src/reasoning/tools/mod.rs`:
- Add near the top, beside `pub mod act; pub mod read;`:

```rust
pub mod spending;
```

- Inside `standard_toolset()`, add before the `act::` registrations:

```rust
    tools.register(spending::explain_spending_change());
```

- [ ] **Step 3: Write a tool-level test (mirrors `read.rs` tests)**

Append to `crates/finsight-agent/src/reasoning/tools/spending.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::messages::{AgentChange, AgentDraftAction};
    use finsight_core::{db::run_migrations, keychain, Db};
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
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
    fn tool_reports_new_flight_as_top_driver() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -20_000, "SAVE ON FOODS EDMONTON, AB");
        }
        ins(&conn, "2026-05", -60_000, "FLAIR AIRLINES BURNABY, BC");

        let mut changes: Vec<AgentChange> = Vec::new();
        let mut drafts: Vec<AgentDraftAction> = Vec::new();
        let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
        let out = explain_spending_change().execute(&mut ctx, json!({"period":"2026-05"})).unwrap();

        let drivers = out["drivers"].as_array().unwrap();
        assert_eq!(drivers[0]["display"], "FLAIR AIRLINES");
        assert_eq!(drivers[0]["mechanism"], "new");
    }
}
```

- [ ] **Step 4: Run to verify it fails, then passes**

Run: `cargo test -p finsight-agent --lib reasoning::tools::spending`
Expected first: FAIL to compile (tool missing) → after Steps 1–2, PASS.

- [ ] **Step 5: Verify the tool is registered in the shipped + eval toolset**

Run: `cargo test -p finsight-agent --lib` (all agent tests) and confirm `standard_toolset()` compiles with the new registration. There are no TypeScript bindings to regenerate — agent tools are not specta commands.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-agent/src/reasoning/tools/spending.rs crates/finsight-agent/src/reasoning/tools/mod.rs
git commit -m "feat(agent): explain_spending_change tool over the spending engine"
```

---

### Task 7: Deterministic spike in the eval household + benchmark cases

**Files:**
- Modify: `crates/finsight-eval/src/seed.rs`
- Create: `eval/subset_spending.jsonl`

- [ ] **Step 1: Read the seed to find where monthly history is inserted**

Run: `grep -n "first_of_month_back\|insert_txn\|fn seed" crates/finsight-eval/src/seed.rs`
Read the surrounding block to see how the six most-recent complete months are seeded (the deterministic history the benchmark asserts against).

- [ ] **Step 2: Add a known regime to the most-recent complete month**

In `crates/finsight-eval/src/seed.rs`, inside `seed(...)`, after the normal monthly history loop, add (using the file's existing `first_of_month_back`, `day_in`, `insert_txn` helpers and the account id used for the card — confirm the id from Step 1, shown here as `"visa"`):

```rust
    // ── Deterministic spending-analysis regime: the most-recent complete month
    // runs hot with one brand-NEW merchant and one DOUBLED merchant, so the
    // spending engine has a fixed, hand-verifiable spike to decompose.
    let hot = first_of_month_back(Utc::now().date_naive(), 0); // most-recent complete month start
    insert_txn(conn, "visa", day_in(hot, 10), -60000, "FLAIR AIRLINES BURNABY, BC", None); // NEW: $600 flight
    insert_txn(conn, "visa", day_in(hot, 20), -40000, "SAVE ON FOODS EDMONTON, AB", Some("groceries")); // doubled vs $200 normal
```

(If the seed anchors "the current month" differently, match its existing convention; the requirement is: exactly one merchant absent from prior months and one at ~2× its prior monthly amount.)

- [ ] **Step 3: Run the eval crate's tests to confirm the seed still builds**

Run: `cargo test -p finsight-eval`
Expected: PASS (seed compiles; existing reference-fact tests unaffected — the new rows are in a month the existing questions don't assert totals for; if any total assertion breaks, move the hot rows to a fresh merchant/category the existing facts don't sum).

- [ ] **Step 4: Add benchmark cases**

Create `eval/subset_spending.jsonl` (match the field shape of an existing case — inspect one first with `head -n 1 eval/benchmark.jsonl`). Two cases:

```jsonl
{"id":"spend-why-hot-month","question":"Why was my spending higher than usual last month? What were the biggest drivers?","expects":["FLAIR","new","SAVE ON FOODS"],"notes":"Should call explain_spending_change for last month and name the new flight and the doubled groceries as the drivers."}
{"id":"spend-whats-new","question":"Which merchants did I spend on last month that I don't normally?","expects":["FLAIR"],"notes":"Should call explain_spending_change with filter=new and surface the flight."}
```

Adjust the field names (`id`/`question`/`expects`) to whatever `benchmark.jsonl` actually uses.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-eval/src/seed.rs eval/subset_spending.jsonl
git commit -m "test(eval): seed a deterministic spending spike + explain_spending_change cases"
```

---

### Task 8: Full green-bar verification

- [ ] **Step 1: Run the whole affected test surface**

Run:
```bash
cargo test -p finsight-core --lib spending
cargo test -p finsight-agent --lib reasoning::tools
cargo test -p finsight-eval
```
Expected: all PASS.

- [ ] **Step 2: Confirm no clippy regressions on the new code**

Run: `cargo clippy -p finsight-core -p finsight-agent --all-targets 2>&1 | grep -A3 spending`
Expected: no warnings referencing the new `spending` modules. Fix any that appear.

- [ ] **Step 3: Manual smoke via the eval harness (optional, needs OPENROUTER_API_KEY)**

Per memory, iterate harness-only on the tiny subset first: run the spending subset through the harness without the judge and read the trace to confirm the model calls `explain_spending_change` for "last month" and narrates the pre-computed drivers rather than doing its own arithmetic. Do not run the full judged benchmark for a Phase-1 change.

- [ ] **Step 4: Final commit if any fixes were made**

```bash
git add -A
git commit -m "chore(spending): phase-1 green bar"
```

---

## Self-review

**Spec coverage (Phase 1 subset of `2026-07-14-spending-analysis-engine-design.md`):**
- §3 zero-arithmetic — tool description forbids LLM math; all numbers precomputed and `_display`-augmented by the existing `augment_cents_fields`. ✓
- §4 self-referential baselines — every driver judged vs its own `per_merchant` history. ✓
- §4 locale-safe money — dominant-currency selection + `mixed_currency` note; per-currency analysis; no cross-currency sums. ✓ (full minor-unit exponent handling remains app-wide debt, out of scope.)
- §4 cold-start honesty — `MIN_BASELINE_MONTHS` note when history is thin. ✓
- §5 reconciliation — all math in `finsight-core::spending`; the tool is a thin wrapper. ✓
- §6 two-axis taxonomy — `Mechanism` + `Persistence`, both required, per driver. ✓ (`emerging` present; recurring.rs refinement + sticky annotations are follow-on plans, noted.)
- §7 `explain_spending_change` — implemented, covering acceptance-tree rows 1–4 and 6 (why / new / doubled / how-much-recurring via `persistence_subtotals` / compare-two-months via `reference`). ✓
- §11 acceptance tree — rows for `classify`, `plan`, and `annotate` are explicitly deferred to follow-on plans (stated in Scope note). ✓
- §13 eval — deterministic seeded spike + subset cases. ✓

**Placeholder scan:** No "TBD"/"handle errors"/"similar to". The two intentional "confirm the id / field names" notes (Task 7) are grounded verification steps against real files, with concrete fallback instructions — not blanks.

**Type consistency:** `Window`, `Mechanism`, `Persistence`, `Driver`, `PersistenceSubtotals`, `DecomposeResult` defined once in `mod.rs` and used unchanged in `baseline.rs`, `decompose.rs`, and the tool. `canonical_merchant_key` (verified to exist) is the single clustering key across baseline + decompose. `baseline::compute(conn, start_ym, end_ym)` signature is identical at all call sites. Tool params (`period`, `reference`, `filter`, `min_ratio`, `limit`) match `decompose`'s `Filter`/`min_ratio`/`limit`.

**Known assumptions to verify during execution:** `merchant::split_display` may need adding (Task 3 Step 2); the eval card's exact JSON field names and the card account id in `seed.rs` (Task 7) must be read from the real files before writing.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-14-spending-analysis-engine-phase1.md`. Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
