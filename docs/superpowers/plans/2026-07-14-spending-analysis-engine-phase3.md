# Spending Analysis Engine — Phase 3 (plan_spending_reduction) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Add the last core tool, `plan_spending_reduction` — the honest "path back". Given an elevated month and an optional monthly target, it separates one-off spend (self-corrects, no action) from the recurring "levers" you can trim, projects where trimming lands you, and states plainly when a target is *below* what trimming reaches (a structural floor, not more cuts). This completes the 8th acceptance-tree question ("how do I get back?") and the Copilot's 5-tool vocabulary.

**Architecture:** Pure orchestration over the shipped engine — `plan` calls `baseline::trailing` + `decompose` and re-frames the result; no new analysis primitives, no new tables. All in `finsight-core::spending`; a thin agent-tool wrapper. The "levers" are exactly the decomposition's recurring/emerging drivers (annotations already fold accepted ones out, from Phase 2).

**Tech Stack:** Rust (rusqlite, serde), the `finsight-agent` reasoning-tool trait.

**Prereq:** Phases 1 & 2 merged to `main` (`finsight-core::spending::{stats,baseline,decompose,classify,annotate}` + `explain_spending_change`/`classify_spending_period`/`annotate_spending_driver`). Spec: `docs/superpowers/specs/2026-07-14-spending-analysis-engine-design.md` §7 (`plan_spending_reduction` — "recoverable levers summing toward an honest target … must not over-promise") and §14 (honest: recurring-reducible ≈ $2.2k floor, $1.5k is structural).

**Scope note:** Follow-on (NOT here): the thin readout Tauri command, the Path Back screen, proactive Inbox surfacing, a dedicated eval spike fixture, and per-currency `month_total` (deferred multi-currency debt).

---

## File structure

- Modify `crates/finsight-core/src/spending/baseline.rs` — add `latest_activity_month()` helper (so the tool can default "the current period").
- Create `crates/finsight-core/src/spending/plan.rs` — `SpendingPlan`, `plan_spending_reduction`.
- Modify `crates/finsight-core/src/spending/mod.rs` — declare `pub mod plan;`.
- Modify `crates/finsight-agent/src/reasoning/tools/spending.rs` — add the `plan_spending_reduction` tool.
- Modify `crates/finsight-agent/src/reasoning/tools/mod.rs` — register it.

Locked return type (used across tasks):
```rust
pub struct SpendingPlan {
    pub currency: String,
    pub recent_monthly_cents: i64,
    pub baseline_monthly_cents: i64,
    pub self_correcting_cents: i64,
    pub recoverable_recurring_cents: i64,
    pub projected_after_levers_cents: i64,
    pub levers: Vec<Driver>,
    pub target_monthly_cents: Option<i64>,
    pub structural_gap_cents: Option<i64>,
    pub note: String,
}
```

---

### Task 1: `baseline::latest_activity_month` helper

**Files:** Modify `crates/finsight-core/src/spending/baseline.rs`

- [ ] **Step 1: Add the helper** at the end of `baseline.rs` (before the `#[cfg(test)]` module), next to `month_total`/`trailing`:
```rust
/// The most recent calendar month (`YYYY-MM`) with any spending activity, or
/// None if the ledger has none. Lets a caller default "the current period".
pub fn latest_activity_month(conn: &Connection) -> CoreResult<Option<String>> {
    let pred = crate::metrics::non_investment_txn_predicate("t");
    let sql = format!(
        "SELECT MAX(substr(t.posted_at,1,7)) FROM transactions t \
         WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND {pred}"
    );
    let ym: Option<String> = conn.query_row(&sql, [], |r| r.get(0))?;
    Ok(ym)
}
```

- [ ] **Step 2: Add a test** inside the existing `#[cfg(test)] mod tests` (reuse `fresh()`/`ins()`):
```rust
    #[test]
    fn latest_activity_month_finds_the_newest_month() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        assert_eq!(latest_activity_month(&conn).unwrap(), None);
        ins(&conn, "2025-03", -1000, "A  X, BC");
        ins(&conn, "2026-02", -1000, "B  Y, BC");
        assert_eq!(latest_activity_month(&conn).unwrap().as_deref(), Some("2026-02"));
    }
```

- [ ] **Step 3: Verify + commit.**
Run: `cargo test -p finsight-core --lib spending::baseline` — expect all PASS.
```
git add crates/finsight-core/src/spending/baseline.rs
git commit -m "feat(spending): latest_activity_month helper

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: `plan_spending_reduction` core

**Files:** Create `crates/finsight-core/src/spending/plan.rs`; Modify `crates/finsight-core/src/spending/mod.rs`

- [ ] **Step 1: Declare the module.** In `mod.rs`, add `pub mod plan;` with the other module declarations.

- [ ] **Step 2: Create `crates/finsight-core/src/spending/plan.rs`:**
```rust
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

    // Baseline: 12 months of $2,000 groceries (recurring). Target month 2026-01:
    // groceries elevated to $2,500 (recurring lever, +$500) plus a $900 one-off
    // flight. So self-correcting = $900, recoverable = $500, floor = $2,000.
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
        // Target $1,500 is below the $2,000 floor → $500 is structural.
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
```

- [ ] **Step 3: Verify + commit.**
Run: `cargo test -p finsight-core --lib spending::plan` — expect 2 PASS. If a subtotal differs, STOP and report the full `SpendingPlan` debug dump; do not adjust assertions to force a pass.
```
git add crates/finsight-core/src/spending/plan.rs crates/finsight-core/src/spending/mod.rs
git commit -m "feat(spending): plan_spending_reduction — honest path back (levers vs structural)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: `plan_spending_reduction` agent tool

**Files:** Modify `crates/finsight-agent/src/reasoning/tools/spending.rs`, `crates/finsight-agent/src/reasoning/tools/mod.rs`

- [ ] **Step 1: Add the tool** to `spending.rs` (after `annotate_spending_driver`, before the `#[cfg(test)]` module):
```rust
pub fn plan_spending_reduction() -> std::sync::Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "plan_spending_reduction"
        }
        fn description(&self) -> &str {
            "Build an HONEST path back toward a spending target. Given `period` (YYYY-MM, the elevated month; omit to use the most recent month) and optional `target_monthly_cents`, it separates one-off spend (self_correcting_cents — falls off on its own, no action) from the recurring 'levers' you can trim (recoverable_recurring_cents + the `levers` list), projects where trimming lands you (projected_after_levers_cents), and sets structural_gap_cents when the target is BELOW what trimming can reach — meaning the rest is a structural floor, not more cuts. Read the `note`. Use for 'how do I get back to $X' / 'how do I cut my spending'. Every number is precomputed — quote the *_display values and never claim a target is reachable when structural_gap_cents is set."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "period":{"type":"string","description":"Elevated month YYYY-MM. Omit to use the most recent month with activity."},
                "target_monthly_cents":{"type":"integer","description":"Optional monthly spend goal in cents (e.g. 150000 for $1,500/mo)."}
            }})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let period = match args["period"].as_str() {
                Some(p) if p.len() >= 7 => p.to_string(),
                _ => match finsight_core::spending::baseline::latest_activity_month(ctx.conn)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                {
                    Some(ym) => ym,
                    None => return Ok(json!({"error":"no_data","note":"No spending activity to plan from."})),
                },
            };
            let target = args["target_monthly_cents"].as_i64();
            let plan = finsight_core::spending::plan::plan_spending_reduction(ctx.conn, &period, target)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let mut v = serde_json::to_value(plan)?;
            v["period"] = json!(period);
            Ok(v)
        }
    }
    std::sync::Arc::new(T)
}
```

- [ ] **Step 2: Add a test** inside the `#[cfg(test)] mod tests` in `spending.rs` (reuse `fresh()`/`ins()`):
```rust
    #[test]
    fn plan_tool_flags_structural_target() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        for i in 0..12 {
            ins(&conn, &format!("2025-{:02}", i + 1), -200_000, "SAVE ON FOODS  EDMONTON, AB");
        }
        ins(&conn, "2026-01", -250_000, "SAVE ON FOODS  EDMONTON, AB");
        ins(&conn, "2026-01", -90_000, "FLAIR AIRLINES  BURNABY, BC");
        let mut changes = Vec::new();
        let mut drafts = Vec::new();
        let mut ctx = ToolContext { conn: &mut conn, changes: &mut changes, draft_actions: &mut drafts };
        let out = plan_spending_reduction()
            .execute(&mut ctx, json!({"period":"2026-01","target_monthly_cents":150_000}))
            .unwrap();
        assert_eq!(out["structural_gap_cents"], 50_000);
        assert_eq!(out["self_correcting_cents"], 90_000);
        assert_eq!(out["period"], "2026-01");
    }
```

- [ ] **Step 3: Register** in `mod.rs`, after the other spending registrations:
```rust
    tools.register(spending::plan_spending_reduction());
```

- [ ] **Step 4: Verify + commit.**
Run: `cargo test -p finsight-agent --lib reasoning::tools::spending` then `cargo test -p finsight-agent --lib`.
```
git add crates/finsight-agent/src/reasoning/tools/spending.rs crates/finsight-agent/src/reasoning/tools/mod.rs
git commit -m "feat(agent): plan_spending_reduction tool — completes the engine vocabulary

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Full green-bar verification

- [ ] **Step 1: Run the affected surface.**
```
cargo test -p finsight-core --lib spending
cargo test -p finsight-agent --lib reasoning::tools
```
Expected: all PASS.

- [ ] **Step 2: Clippy the new module.**
Run: `cargo clippy -p finsight-core --lib 2>&1 | grep -iE "spending/plan|plan\.rs|plan::"` — expect no output. Fix any that appear (explicit `git add`, no `-A`).

---

## Self-review

**Spec coverage:**
- §7 `plan_spending_reduction` — Task 2 (core) + Task 3 (tool): recoverable levers + honest reachable estimate; `structural_gap_cents` is the "must not over-promise" guarantee made mechanical. ✓
- §14 honesty — a target below the trimming floor yields a structural gap and a note that says so, exactly the "$1.5k is structural" framing. ✓
- §5 reconciliation — pure orchestration over `baseline::trailing` + `decompose`; no re-derived numbers; annotations already fold accepted drivers out of the levers (Phase 2). ✓
- §3 zero-arithmetic — the tool returns precomputed `_cents` (auto-`_display`) and a prose `note`; the description forbids over-claiming when `structural_gap_cents` is set. ✓
- §11 acceptance tree row 8 ("how do I get back?") — answered. ✓

**Placeholder scan:** none.

**Type consistency:** `SpendingPlan` defined once (plan.rs), serialized by the tool. `plan_spending_reduction(conn, period_ym, target_monthly_cents)` signature identical at both call sites. `levers: Vec<Driver>` reuses the existing `Driver` (whose `persistence` already reflects annotations from Phase 2). `baseline::latest_activity_month` added Task 1, consumed Task 3.

**Design note (deliberate):** `recent_monthly_cents` is the target month's monthly-equivalent (median headline baseline vs mean-based driver deltas still don't sum — documented on `DecomposeResult.gap_cents` since Phase 1). `plan` reports `recent`, `baseline`, `self_correcting`, `recoverable`, and `projected_after` as distinct precomputed figures; the LLM narrates, never sums.

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-14-spending-analysis-engine-phase3.md`. Continuing with **Subagent-Driven** execution (same as Phases 1–2) unless you say otherwise.
