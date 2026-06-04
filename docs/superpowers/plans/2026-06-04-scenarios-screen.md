# Scenarios Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a natural-language what-if planner screen (`/scenarios`) that projects cash-flow trajectories deterministically, with optional LLM parsing of free-text input.

**Architecture:** A pure projection engine in `finsight-core` (`forecast.rs`) consumes a small `ScenarioParams` struct plus a financial `Snapshot` and produces a `Projection` (trajectories, runway delta, verdict, considerations). Preset chips build params in the frontend (no LLM). Free text is the only path that calls the configured LLM, degrading to a friendly nudge when no provider is set. Scenarios can be saved to a new `scenarios` table.

**Tech Stack:** Rust (rusqlite, refinery migrations, specta) · React 18 + TypeScript + Vite · tanstack-query · sonner · vitest.

---

## File structure

| File | Responsibility |
|------|----------------|
| `crates/finsight-core/migrations/V005__scenarios.sql` | Create `scenarios` table |
| `crates/finsight-core/src/forecast.rs` | Pure projection engine + runway helper + types + unit tests |
| `crates/finsight-core/src/repos/scenarios.rs` | CRUD for `scenarios` table + repo test |
| `crates/finsight-core/src/lib.rs` | Register `forecast` module |
| `crates/finsight-core/src/repos/mod.rs` | Register `scenarios` repo module |
| `crates/finsight-app/src/commands/scenarios.rs` | Tauri commands + DTOs + LLM extraction |
| `crates/finsight-app/src/commands/mod.rs` | Register `scenarios` command module |
| `crates/finsight-app/src/lib.rs` | Register commands in specta builder |
| `ui/src/api/bindings.ts` | Regenerated (never hand-edited) |
| `ui/src/api/hooks/useScenarios.ts` | tanstack-query wrappers |
| `ui/src/screens/Scenarios.tsx` | The screen + SVG dual-line chart |
| `ui/src/screens/Scenarios.test.tsx` | Frontend test |
| `ui/src/App.tsx` | `/scenarios` route |
| `ui/src/components/Sidebar.tsx` | Nav entry (§15a) |

---

## Task 1: Migration + scenarios repo (core)

**Files:**
- Create: `crates/finsight-core/migrations/V005__scenarios.sql`
- Create: `crates/finsight-core/src/repos/scenarios.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the migration**

Create `crates/finsight-core/migrations/V005__scenarios.sql`:

```sql
-- V005: saved what-if scenarios
CREATE TABLE scenarios (
  id          TEXT PRIMARY KEY,
  description TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_scenarios_created ON scenarios(created_at);
```

- [ ] **Step 2: Register the repo module**

In `crates/finsight-core/src/repos/mod.rs`, add the module declaration in alphabetical position (after `rules;`):

```rust
pub mod rules;
pub mod scenarios;
pub mod transactions;
```

- [ ] **Step 3: Write the repo with a failing round-trip test**

Create `crates/finsight-core/src/repos/scenarios.rs`:

```rust
use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ScenarioRow {
    pub id: String,
    pub description: String,
    pub result_json: String,
    pub created_at: String,
}

pub fn insert(conn: &mut Connection, description: &str, result_json: &str) -> CoreResult<ScenarioRow> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO scenarios(id, description, result_json, created_at) VALUES(?1, ?2, ?3, ?4)",
        params![id, description, result_json, now],
    )?;
    Ok(ScenarioRow {
        id,
        description: description.to_string(),
        result_json: result_json.to_string(),
        created_at: now,
    })
}

pub fn list(conn: &mut Connection) -> CoreResult<Vec<ScenarioRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, description, result_json, created_at FROM scenarios ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ScenarioRow {
            id: r.get(0)?,
            description: r.get(1)?,
            result_json: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM scenarios WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("a.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_list_delete_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let row = insert(&mut conn, "What if I buy a car?", r#"{"verdict":true}"#).unwrap();
        let listed = list(&mut conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].description, "What if I buy a car?");
        assert_eq!(listed[0].result_json, r#"{"verdict":true}"#);
        delete(&mut conn, &row.id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p finsight-core --lib repos::scenarios::tests::insert_list_delete_round_trip`
Expected: PASS (migration applies, round-trip works).

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/migrations/V005__scenarios.sql crates/finsight-core/src/repos/scenarios.rs crates/finsight-core/src/repos/mod.rs
git commit -m "feat(core): scenarios table + repo (TODO §1)"
```

---

## Task 2: Projection engine (core)

**Files:**
- Create: `crates/finsight-core/src/forecast.rs`
- Modify: `crates/finsight-core/src/lib.rs`

- [ ] **Step 1: Register the module**

In `crates/finsight-core/src/lib.rs`, add `pub mod forecast;` after `pub mod error;`:

```rust
pub mod db;
pub mod error;
pub mod forecast;
pub mod keychain;
```

- [ ] **Step 2: Write the engine with failing unit tests**

Create `crates/finsight-core/src/forecast.rs`:

```rust
//! Pure deterministic projection engine for what-if scenarios.
//! No DB, no LLM — given a financial snapshot and parameters, project trajectories.

/// Runway is capped here so "covered indefinitely" doesn't produce absurd numbers.
pub const RUNWAY_CAP_DAYS: i64 = 3650;

/// Parameters describing a scenario. Built directly from preset chips, or
/// extracted from free text by the LLM in the app layer.
#[derive(Debug, Clone, Default)]
pub struct ScenarioParams {
    /// e.g. -50 means "cut income by 50%".
    pub income_delta_pct: i32,
    /// Recurring monthly outflow change. Positive = more outflow (e.g. add to
    /// savings); negative = less outflow (e.g. eliminate dining).
    pub monthly_expense_delta_cents: i64,
    /// One-off cost in cents, applied at `start_month_offset`.
    pub one_time_cents: i64,
    /// Months from now the change begins (0 = immediately).
    pub start_month_offset: u32,
    /// Human label echoed back for display.
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct GoalInfo {
    pub name: String,
    pub remaining_cents: i64,
    pub monthly_cents: i64,
}

/// Current financial state the projection runs against.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub balance_cents: i64,
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub goals: Vec<GoalInfo>,
}

#[derive(Debug, Clone)]
pub struct Projection {
    pub baseline_monthly: Vec<i64>,
    pub scenario_monthly: Vec<i64>,
    pub runway_change_days: i64,
    pub monthly_impact_cents: i64,
    pub verdict: bool,
    pub goals_affected: Vec<String>,
    pub considerations: Vec<String>,
}

/// Days a balance lasts given an outflow over a period. Shared formula:
/// `avg_daily = outflow / period_days`, `runway = balance / avg_daily`.
/// TODO §3d's Today runway stat calls this with (balance, expenses_this_month, day_of_month).
pub fn runway_days(balance_cents: i64, period_outflow_cents: i64, period_days: i64) -> i64 {
    if period_outflow_cents <= 0 || period_days <= 0 {
        return RUNWAY_CAP_DAYS;
    }
    let daily = period_outflow_cents as f64 / period_days as f64;
    let days = (balance_cents as f64 / daily).floor() as i64;
    days.clamp(0, RUNWAY_CAP_DAYS)
}

fn fmt_money(cents: i64) -> String {
    format!("${:.0}", (cents.abs() as f64) / 100.0)
}

fn fmt_runway(days: i64) -> String {
    if days >= RUNWAY_CAP_DAYS {
        "10+ years".to_string()
    } else if days >= 365 {
        format!("{:.1} years", days as f64 / 365.0)
    } else {
        format!("{} days", days)
    }
}

pub fn project(s: &Snapshot, p: &ScenarioParams, months: u32) -> Projection {
    let n = months.max(1) as usize;
    let start = (p.start_month_offset as usize).min(n.saturating_sub(1));

    let base_income = s.avg_monthly_income_cents;
    let base_expense = s.avg_monthly_expense_cents;
    let base_net = base_income - base_expense;

    let scen_income =
        (base_income as f64 * (1.0 + p.income_delta_pct as f64 / 100.0)).round() as i64;
    let scen_expense = base_expense + p.monthly_expense_delta_cents;
    let scen_net = scen_income - scen_expense;

    let mut baseline_monthly = Vec::with_capacity(n);
    let mut scenario_monthly = Vec::with_capacity(n);
    let mut bal = s.balance_cents;
    let mut sbal = s.balance_cents;
    for i in 0..n {
        bal += base_net;
        let month_net = if i >= start { scen_net } else { base_net };
        sbal += month_net;
        if i == start {
            sbal -= p.one_time_cents;
        }
        baseline_monthly.push(bal);
        scenario_monthly.push(sbal);
    }

    // Runway uses NET outflow (expense - income), so income cuts shorten it.
    let base_outflow = (base_expense - base_income).max(0);
    let scen_outflow = (scen_expense - scen_income).max(0);
    let base_runway = runway_days(s.balance_cents, base_outflow, 30);
    let scen_runway = runway_days(s.balance_cents - p.one_time_cents, scen_outflow, 30);
    let runway_change_days = scen_runway - base_runway;

    let monthly_impact_cents = scen_net - base_net;

    let verdict = scenario_monthly.iter().all(|&v| v >= 0);

    // Goals affected: distribute the monthly shortfall proportionally across goals.
    let total_goal_monthly: i64 = s.goals.iter().map(|g| g.monthly_cents.max(0)).sum();
    let shortfall = (base_net - scen_net).max(0);
    let mut goals_affected = Vec::new();
    if shortfall > 0 && total_goal_monthly > 0 {
        for g in &s.goals {
            if g.monthly_cents <= 0 || g.remaining_cents <= 0 {
                continue;
            }
            let share = (shortfall as f64 * (g.monthly_cents as f64 / total_goal_monthly as f64))
                .round() as i64;
            let new_monthly = g.monthly_cents - share;
            let base_eta = (g.remaining_cents + g.monthly_cents - 1) / g.monthly_cents;
            if new_monthly <= 0 {
                goals_affected.push(format!("{}: paused", g.name));
            } else {
                let scen_eta = (g.remaining_cents + new_monthly - 1) / new_monthly;
                let slip = scen_eta - base_eta;
                if slip > 0 {
                    goals_affected.push(format!("{}: +{} mo", g.name, slip));
                }
            }
        }
    }

    let considerations = build_considerations(
        s,
        n,
        base_runway,
        scen_runway,
        runway_change_days,
        monthly_impact_cents,
        &scenario_monthly,
        &goals_affected,
        verdict,
    );

    Projection {
        baseline_monthly,
        scenario_monthly,
        runway_change_days,
        monthly_impact_cents,
        verdict,
        goals_affected,
        considerations,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_considerations(
    s: &Snapshot,
    n: usize,
    base_runway: i64,
    scen_runway: i64,
    runway_change: i64,
    monthly_impact: i64,
    scenario_monthly: &[i64],
    goals_affected: &[String],
    verdict: bool,
) -> Vec<String> {
    let mut out = Vec::new();

    if runway_change < -1 {
        out.push(format!(
            "Runway shortens by {} days — from {} to {}.",
            runway_change.abs(),
            fmt_runway(base_runway),
            fmt_runway(scen_runway)
        ));
    } else if runway_change > 1 {
        out.push(format!(
            "Runway extends by {} days — from {} to {}.",
            runway_change,
            fmt_runway(base_runway),
            fmt_runway(scen_runway)
        ));
    } else {
        out.push("Runway is essentially unchanged.".to_string());
    }

    if s.avg_monthly_expense_cents > 0 {
        let today_months = s.balance_cents as f64 / s.avg_monthly_expense_cents as f64;
        let low = *scenario_monthly.iter().min().unwrap_or(&s.balance_cents);
        let low_months = low as f64 / s.avg_monthly_expense_cents as f64;
        out.push(format!(
            "Your savings cover ~{:.1} months of expenses today; this scenario draws that to ~{:.1} months at its lowest.",
            today_months.max(0.0),
            low_months.max(0.0)
        ));
    }

    if monthly_impact < 0 {
        out.push(format!(
            "This costs about {} more per month than your current plan.",
            fmt_money(monthly_impact)
        ));
    } else if monthly_impact > 0 {
        out.push(format!(
            "This frees about {} per month versus your current plan.",
            fmt_money(monthly_impact)
        ));
    }

    if !goals_affected.is_empty() {
        out.push(format!(
            "Affects {} goal(s): {}.",
            goals_affected.len(),
            goals_affected.join(", ")
        ));
    }

    if verdict {
        out.push(format!(
            "Your projected balance stays positive across the {}-month horizon.",
            n
        ));
    } else {
        let k = scenario_monthly
            .iter()
            .position(|&v| v < 0)
            .map(|i| i + 1)
            .unwrap_or(n);
        out.push(format!(
            "Your projected balance would go negative around month {} — you'd need to adjust spending or income.",
            k
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            balance_cents: 2_000_000, // $20k
            avg_monthly_income_cents: 600_000, // $6k
            avg_monthly_expense_cents: 400_000, // $4k
            goals: vec![GoalInfo {
                name: "House Fund".into(),
                remaining_cents: 1_200_000,
                monthly_cents: 100_000,
            }],
        }
    }

    #[test]
    fn runway_zero_burn_is_capped() {
        assert_eq!(runway_days(100_000, 0, 30), RUNWAY_CAP_DAYS);
    }

    #[test]
    fn runway_basic_division() {
        // $3000 balance, $3000/mo outflow over 30 days => 30 days.
        assert_eq!(runway_days(300_000, 300_000, 30), 30);
    }

    #[test]
    fn neutral_scenario_is_coverable() {
        let p = ScenarioParams::default();
        let proj = project(&snap(), &p, 12);
        assert_eq!(proj.baseline_monthly.len(), 12);
        assert_eq!(proj.scenario_monthly.len(), 12);
        assert!(proj.verdict);
        // Neutral params => trajectories identical.
        assert_eq!(proj.baseline_monthly, proj.scenario_monthly);
    }

    #[test]
    fn income_cut_shortens_runway() {
        let p = ScenarioParams { income_delta_pct: -100, ..Default::default() };
        let proj = project(&snap(), &p, 12);
        // With no income, net outflow becomes positive => finite, shorter runway.
        assert!(proj.runway_change_days < 0);
    }

    #[test]
    fn one_time_purchase_reduces_scenario_balance() {
        let p = ScenarioParams { one_time_cents: 500_000, ..Default::default() };
        let proj = project(&snap(), &p, 12);
        // First month scenario balance is $5k below baseline.
        assert_eq!(proj.baseline_monthly[0] - proj.scenario_monthly[0], 500_000);
    }

    #[test]
    fn large_one_time_on_low_balance_is_not_coverable() {
        let mut s = snap();
        s.balance_cents = 100_000; // $1k
        let p = ScenarioParams { one_time_cents: 3_500_000, ..Default::default() };
        let proj = project(&s, &p, 12);
        assert!(!proj.verdict);
        assert!(!proj.considerations.is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p finsight-core --lib forecast::`
Expected: PASS (6 tests).

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/forecast.rs crates/finsight-core/src/lib.rs
git commit -m "feat(core): deterministic scenario projection engine (TODO §1)"
```

---

## Task 3: Scenario commands — chip path (app)

**Files:**
- Create: `crates/finsight-app/src/commands/scenarios.rs`
- Modify: `crates/finsight-app/src/commands/mod.rs`
- Modify: `crates/finsight-app/src/lib.rs`
- Regenerate: `ui/src/api/bindings.ts`

- [ ] **Step 1: Register the command module**

In `crates/finsight-app/src/commands/mod.rs`, add `pub mod scenarios;` (alphabetical, after `reports;`). Confirm the existing ordering and insert accordingly.

- [ ] **Step 2: Write the commands file (chip path; free-text returns a typed error for now)**

Create `crates/finsight-app/src/commands/scenarios.rs`:

```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::forecast::{self, GoalInfo, ScenarioParams, Snapshot};
use finsight_core::repos::{accounts, goals, run, scenarios as scenarios_repo};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioResult {
    pub verdict: bool,
    pub runway_change_days: i64,
    pub monthly_impact_cents: i64,
    pub considerations: Vec<String>,
    pub baseline_monthly: Vec<i64>,
    pub scenario_monthly: Vec<i64>,
    pub goals_affected: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioParamsInput {
    pub income_delta_pct: i32,
    pub monthly_expense_delta_cents: i64,
    pub one_time_cents: i64,
    pub start_month_offset: u32,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedScenario {
    pub id: String,
    pub description: String,
    pub result: ScenarioResult,
    pub created_at: String,
}

fn projection_to_result(proj: forecast::Projection) -> ScenarioResult {
    ScenarioResult {
        verdict: proj.verdict,
        runway_change_days: proj.runway_change_days,
        monthly_impact_cents: proj.monthly_impact_cents,
        considerations: proj.considerations,
        baseline_monthly: proj.baseline_monthly,
        scenario_monthly: proj.scenario_monthly,
        goals_affected: proj.goals_affected,
    }
}

async fn build_snapshot(state: &AppState) -> AppResult<Snapshot> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let accts = accounts::list_summaries(conn)?;
        let balance: i64 = accts.iter().map(|a| a.balance_cents).sum();

        let (sum_income, sum_expense, active_months): (i64, i64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(inc),0), COALESCE(SUM(exp),0), COUNT(*) FROM (\
               SELECT strftime('%Y-%m', posted_at) mo,\
                      SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END) inc,\
                      SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END) exp\
               FROM transactions\
               WHERE posted_at >= date('now','-12 months')\
               GROUP BY mo)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let am = active_months.max(1);

        let goal_infos = goals::list(conn)?
            .into_iter()
            .map(|g| GoalInfo {
                name: g.name,
                remaining_cents: (g.target_cents - g.current_cents).max(0),
                monthly_cents: g.monthly_cents,
            })
            .collect();

        Ok(Snapshot {
            balance_cents: balance,
            avg_monthly_income_cents: sum_income / am,
            avg_monthly_expense_cents: sum_expense / am,
            goals: goal_infos,
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn run_scenario(
    state: tauri::State<'_, AppState>,
    description: String,
    months: u32,
    params: Option<ScenarioParamsInput>,
) -> AppResult<ScenarioResult> {
    let snapshot = build_snapshot(&state).await?;

    let core_params = match params {
        Some(p) => ScenarioParams {
            income_delta_pct: p.income_delta_pct,
            monthly_expense_delta_cents: p.monthly_expense_delta_cents,
            one_time_cents: p.one_time_cents,
            start_month_offset: p.start_month_offset,
            label: p.label,
        },
        None => {
            return Err(AppError::new(
                "scenario.no_provider",
                "Configure an AI provider in Settings to ask free-text scenarios, or pick a suggested scenario.",
            ))
        }
    };

    let proj = forecast::project(&snapshot, &core_params, months);
    Ok(projection_to_result(proj))
}

#[tauri::command]
#[specta::specta]
pub async fn save_scenario(
    state: tauri::State<'_, AppState>,
    description: String,
    result: ScenarioResult,
) -> AppResult<SavedScenario> {
    let db = (*state.db).clone();
    let result_json =
        serde_json::to_string(&result).map_err(|e| AppError::new("scenario.serialize", e.to_string()))?;
    let row = run(&db, move |conn| {
        scenarios_repo::insert(conn, &description, &result_json)
    })
    .await
    .map_err(AppError::from)?;
    Ok(SavedScenario {
        id: row.id,
        description: row.description,
        result,
        created_at: row.created_at,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn list_scenario_history(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SavedScenario>> {
    let db = (*state.db).clone();
    let rows = run(&db, scenarios_repo::list).await.map_err(AppError::from)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let result: ScenarioResult = serde_json::from_str(&row.result_json)
            .map_err(|e| AppError::new("scenario.parse", e.to_string()))?;
        out.push(SavedScenario {
            id: row.id,
            description: row.description,
            result,
            created_at: row.created_at,
        });
    }
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_scenario(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| scenarios_repo::delete(conn, &id))
        .await
        .map_err(AppError::from)
}
```

- [ ] **Step 3: Register commands in the specta builder**

In `crates/finsight-app/src/lib.rs`, inside `collect_commands![...]`, add these four lines after `commands::reports::get_month_totals,`:

```rust
        commands::scenarios::run_scenario,
        commands::scenarios::save_scenario,
        commands::scenarios::list_scenario_history,
        commands::scenarios::delete_scenario,
```

- [ ] **Step 4: Build to verify it compiles**

Run: `cargo build -p finsight-app`
Expected: compiles cleanly (no errors).

- [ ] **Step 5: Regenerate TypeScript bindings**

Run (from repo root): `cargo run -p finsight-tauri --bin export_bindings`
Expected: `ui/src/api/bindings.ts` updated with `runScenario`, `saveScenario`, `listScenarioHistory`, `deleteScenario`, and the `ScenarioResult` / `ScenarioParamsInput` / `SavedScenario` types.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-app/src/commands/scenarios.rs crates/finsight-app/src/commands/mod.rs crates/finsight-app/src/lib.rs ui/src/api/bindings.ts
git commit -m "feat(app): scenario commands — chip path + save/list/delete (TODO §1)"
```

---

## Task 4: LLM free-text extraction (app)

**Files:**
- Modify: `crates/finsight-app/src/commands/scenarios.rs`

- [ ] **Step 1: Add the extraction helper**

In `crates/finsight-app/src/commands/scenarios.rs`, add this function above `run_scenario`:

```rust
async fn extract_params_via_llm(
    state: &AppState,
    description: &str,
    snapshot: &Snapshot,
) -> AppResult<ScenarioParams> {
    let provider = state.agent_provider.read().unwrap().clone();
    let Some(provider) = provider else {
        return Err(AppError::new(
            "scenario.no_provider",
            "Configure an AI provider in Settings to ask free-text scenarios, or pick a suggested scenario.",
        ));
    };

    let system = "You convert a personal-finance what-if question into JSON parameters. \
Respond ONLY with JSON of this exact shape: \
{\"income_delta_pct\": <int>, \"monthly_expense_delta_cents\": <int>, \"one_time_cents\": <int>, \"start_month_offset\": <int>, \"label\": <string>}. \
income_delta_pct is the percent change to monthly income (e.g. -50 to halve it). \
monthly_expense_delta_cents is the recurring monthly outflow change in cents: positive means MORE outflow (extra spending or saving), negative means LESS. \
one_time_cents is a single one-off cost in cents. \
start_month_offset is how many months from now the change begins (0 if immediate). \
label is a short title for the scenario.";

    let user = format!(
        "Question: {description}\nContext: average monthly income {} cents, average monthly expense {} cents.",
        snapshot.avg_monthly_income_cents, snapshot.avg_monthly_expense_cents
    );

    let v = provider
        .complete_json(system, &user)
        .await
        .map_err(|e| AppError::new("scenario.llm", e.to_string()))?;

    Ok(ScenarioParams {
        income_delta_pct: v.get("income_delta_pct").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
        monthly_expense_delta_cents: v
            .get("monthly_expense_delta_cents")
            .and_then(|x| x.as_i64())
            .unwrap_or(0),
        one_time_cents: v.get("one_time_cents").and_then(|x| x.as_i64()).unwrap_or(0),
        start_month_offset: v.get("start_month_offset").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        label: v
            .get("label")
            .and_then(|x| x.as_str())
            .unwrap_or(description)
            .to_string(),
    })
}
```

- [ ] **Step 2: Wire the free-text path into `run_scenario`**

Replace the `None => { return Err(...) }` arm in `run_scenario` with:

```rust
        None => extract_params_via_llm(&state, &description, &snapshot).await?,
```

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build -p finsight-app`
Expected: compiles cleanly.

- [ ] **Step 4: Run the full Rust suite**

Run: `cargo test --workspace`
Expected: all tests pass (the keychain round-trip test may be intermittently flaky on Windows — re-run if only that fails).

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/scenarios.rs
git commit -m "feat(app): LLM free-text extraction for scenarios with graceful fallback (TODO §1)"
```

---

## Task 5: Frontend hooks

**Files:**
- Create: `ui/src/api/hooks/useScenarios.ts`

- [ ] **Step 1: Write the hooks**

Create `ui/src/api/hooks/useScenarios.ts`:

```typescript
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ScenarioResult,
  type ScenarioParamsInput,
  type SavedScenario,
} from "../client";

export function useScenarioHistory() {
  return useQuery<SavedScenario[]>({
    queryKey: ["scenario-history"],
    queryFn: async () => {
      const result = await commands.listScenarioHistory();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useRunScenario() {
  return useMutation({
    mutationFn: async ({
      description,
      months,
      params,
    }: {
      description: string;
      months: number;
      params: ScenarioParamsInput | null;
    }) => {
      const result = await commands.runScenario(description, months, params);
      if (result.status === "error") {
        const err = new Error(result.error.message) as Error & { code?: string };
        err.code = result.error.code;
        throw err;
      }
      return result.data;
    },
  });
}

export function useSaveScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({
      description,
      result,
    }: {
      description: string;
      result: ScenarioResult;
    }) => {
      const res = await commands.saveScenario(description, result);
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["scenario-history"] });
    },
  });
}

export function useDeleteScenario() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const res = await commands.deleteScenario(id);
      if (res.status === "error") throw new Error(res.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["scenario-history"] });
    },
  });
}
```

> Note: `commands.runScenario` signature is `(description, months, params)` — verify argument order against the regenerated `bindings.ts` from Task 3 Step 5 and adjust if specta emitted a different order.

- [ ] **Step 2: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/hooks/useScenarios.ts
git commit -m "feat(ui): scenario tanstack-query hooks (TODO §1)"
```

---

## Task 6: Scenarios screen

**Files:**
- Create: `ui/src/screens/Scenarios.tsx`

- [ ] **Step 1: Write the screen**

Create `ui/src/screens/Scenarios.tsx`. It mirrors `Reports.tsx` conventions (`.screen`, `.screen-header`, `.screen-eyebrow`, `.toolbar`, `.card`, `.stat`, `money` class for privacy). The dining chip resolves a real amount from `listCategoriesWithSpending`.

```tsx
import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { commands, type ScenarioResult, type ScenarioParamsInput } from "../api/client";
import {
  useScenarioHistory,
  useRunScenario,
  useSaveScenario,
  useDeleteScenario,
} from "../api/hooks/useScenarios";
import * as I from "../components/Icons";

type Range = "6" | "12" | "24";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

function useDiningMonthly() {
  return useQuery<number>({
    queryKey: ["dining-monthly"],
    queryFn: async () => {
      const res = await commands.listCategoriesWithSpending();
      if (res.status === "error") throw new Error(res.error.message);
      const match = res.data.find((c) => /dining|restaurant|food|eat/i.test(c.label));
      return match?.thisMonthCents ?? 40000;
    },
    staleTime: 60_000,
  });
}

// ── Dual-line forecast chart ───────────────────────────────────────────────

function ForecastChart({
  baseline,
  scenario,
  range,
}: {
  baseline: number[];
  scenario: number[];
  range: Range;
}) {
  const count = range === "6" ? 6 : range === "24" ? 24 : 12;
  const base = baseline.slice(0, count);
  const scen = scenario.slice(0, count);
  const all = [...base, ...scen];
  const max = Math.max(...all, 1);
  const min = Math.min(...all, 0);
  const span = max - min || 1;
  const W = 100 / Math.max(base.length - 1, 1);
  const stressing = (scen[scen.length - 1] ?? 0) < (base[base.length - 1] ?? 0);
  const color = stressing ? "var(--negative)" : "var(--accent)";

  const path = (vals: number[]) =>
    vals
      .map((v, i) => {
        const x = i * W;
        const y = 38 - ((v - min) / span) * 34;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");

  return (
    <div style={{ background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)", padding: "22px 8px 8px" }}>
      <svg viewBox="0 0 100 42" preserveAspectRatio="none" style={{ width: "100%", height: 200, display: "block" }}>
        <line x1="0" y1={(38 - ((0 - min) / span) * 34).toFixed(1)} x2="100" y2={(38 - ((0 - min) / span) * 34).toFixed(1)} stroke="var(--hairline)" strokeWidth="0.4" />
        <path d={path(base)} fill="none" stroke="var(--ink)" strokeWidth="1" />
        <path d={path(scen)} fill="none" stroke={color} strokeWidth="1.2" strokeDasharray="2.5 2" />
      </svg>
      <div style={{ display: "flex", gap: 16, fontSize: 12, color: "var(--ink-mute)", padding: "8px 12px 0" }}>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ width: 14, height: 2, background: "var(--ink)", display: "inline-block" }} />current path
        </span>
        <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ width: 14, height: 2, background: color, display: "inline-block" }} />with scenario
        </span>
      </div>
    </div>
  );
}

// ── Results panel ──────────────────────────────────────────────────────────

function Results({
  description,
  result,
  onSaved,
  onDiscard,
}: {
  description: string;
  result: ScenarioResult;
  onSaved: () => void;
  onDiscard: () => void;
}) {
  const [range, setRange] = useState<Range>("12");
  const save = useSaveScenario();
  const coverable = result.verdict;

  return (
    <div style={{ marginTop: 24 }}>
      <div
        className="card"
        style={{
          borderColor: coverable ? "var(--accent)" : "var(--negative)",
        }}
      >
        <div className="screen-eyebrow" style={{ marginBottom: 10 }}>Verdict</div>
        <div style={{ fontSize: 22, fontWeight: 600, marginBottom: 6 }}>
          {coverable ? "You can do this — here's what changes." : "Not without trade-offs — here's what would give."}
        </div>
        <div className="muted" style={{ fontSize: 14 }}>&ldquo;{description}&rdquo;</div>

        <div className="stat-row" style={{ marginTop: 20 }}>
          <div className="stat">
            <div className="label">Runway change</div>
            <div className={`value figure ${result.runwayChangeDays >= 0 ? "" : "neg"}`}>
              {result.runwayChangeDays >= 0 ? "+" : ""}
              {result.runwayChangeDays} days
            </div>
          </div>
          <div className="stat">
            <div className="label">Monthly impact</div>
            <div className={`value figure money ${result.monthlyImpactCents >= 0 ? "" : "neg"}`}>
              {fmt(result.monthlyImpactCents)}
            </div>
          </div>
          <div className="stat">
            <div className="label">Goals affected</div>
            <div className="value figure">{result.goalsAffected.length}</div>
          </div>
        </div>
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 16 }}>
        <div className="toolbar">
          {(["6", "12", "24"] as Range[]).map((r) => (
            <button key={r} className={range === r ? "on" : ""} onClick={() => setRange(r)}>{r}M</button>
          ))}
        </div>
      </div>
      <div style={{ marginTop: 8 }}>
        <ForecastChart baseline={result.baselineMonthly} scenario={result.scenarioMonthly} range={range} />
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1.4fr 1fr", gap: 16, marginTop: 16 }}>
        <div className="card">
          <div className="screen-eyebrow" style={{ marginBottom: 12 }}>Worth knowing</div>
          <ol style={{ margin: 0, paddingLeft: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 10 }}>
            {result.considerations.map((c, i) => (
              <li key={i} style={{ display: "grid", gridTemplateColumns: "22px 1fr", gap: 10, fontSize: 13.5, color: "var(--ink-2)", lineHeight: 1.5 }}>
                <span style={{ fontFamily: "var(--mono)", fontSize: 11, color: "var(--ink-mute)" }}>{i + 1}</span>
                <span>{c}</span>
              </li>
            ))}
          </ol>
        </div>
        <div className="card" style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div className="screen-eyebrow" style={{ marginBottom: 4 }}>What to do</div>
          <button
            className="btn primary"
            disabled={save.isPending}
            onClick={async () => {
              try {
                await save.mutateAsync({ description, result });
                toast.success("Scenario saved", { description });
                onSaved();
              } catch (e) {
                toast.error("Could not save scenario");
              }
            }}
          >
            <I.Sparkle /> Save this scenario
          </button>
          <button className="btn ghost" onClick={onDiscard}>
            <I.X /> Discard
          </button>
          <div className="muted" style={{ fontSize: 12, marginTop: 6, lineHeight: 1.5 }}>
            All scenarios are local — nothing happens to your real money until you explicitly apply changes.
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main screen ────────────────────────────────────────────────────────────

export default function Scenarios() {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<{ description: string; result: ScenarioResult } | null>(null);
  const run = useRunScenario();
  const del = useDeleteScenario();
  const { data: history = [] } = useScenarioHistory();
  const { data: diningMonthly = 40000 } = useDiningMonthly();

  const chips: { label: string; params: ScenarioParamsInput }[] = useMemo(
    () => [
      { label: "Cut income 50%", params: { incomeDeltaPct: -50, monthlyExpenseDeltaCents: 0, oneTimeCents: 0, startMonthOffset: 0, label: "Cut income 50%" } },
      { label: "Eliminate dining out", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: -diningMonthly, oneTimeCents: 0, startMonthOffset: 0, label: "Eliminate dining out" } },
      { label: "Buy a car $35k", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 0, oneTimeCents: 3_500_000, startMonthOffset: 0, label: "Buy a car $35k" } },
      { label: "Add $500/mo to savings", params: { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 50_000, oneTimeCents: 0, startMonthOffset: 0, label: "Add $500/mo to savings" } },
    ],
    [diningMonthly]
  );

  const runWith = async (description: string, params: ScenarioParamsInput | null) => {
    try {
      const result = await run.mutateAsync({ description, months: 24, params });
      setActive({ description, result });
    } catch (e) {
      const code = (e as { code?: string }).code;
      if (code === "scenario.no_provider") {
        toast.error("Free-text needs an AI provider", {
          description: "Configure one in Settings, or pick a suggested scenario below.",
        });
      } else {
        toast.error("Could not run scenario", { description: (e as Error).message });
      }
    }
  };

  return (
    <div className="screen">
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">Scenarios · run any what-if</div>
          <h1>Imagine a future, see the math.</h1>
        </div>
      </div>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (query.trim()) void runWith(query.trim(), null);
        }}
        style={{ marginTop: 16 }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "16px 20px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)" }}>
          <I.Sparkle style={{ color: "var(--accent)" }} />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="What if I take a 6-month sabbatical?"
            aria-label="Scenario question"
            style={{ flex: 1, background: "transparent", border: 0, outline: 0, fontSize: 16, color: "var(--ink)" }}
          />
          <button type="submit" className="btn primary" disabled={run.isPending}>
            {run.isPending ? "Running…" : "Run"}
          </button>
        </div>
      </form>

      <div style={{ marginTop: 18 }}>
        <div className="screen-eyebrow" style={{ marginBottom: 10 }}>Or pick a starting point</div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {chips.map((c) => (
            <button key={c.label} className="chip" onClick={() => void runWith(c.label, c.params)}>
              {c.label}
            </button>
          ))}
        </div>
      </div>

      {active && (
        <Results
          description={active.description}
          result={active.result}
          onSaved={() => undefined}
          onDiscard={() => setActive(null)}
        />
      )}

      <div style={{ marginTop: 32 }}>
        <div className="screen-eyebrow" style={{ marginBottom: 10 }}>Recent scenarios you've run</div>
        <div className="card flush">
          {history.length === 0 ? (
            <div style={{ padding: 32, textAlign: "center", color: "var(--ink-faint)", fontSize: 13 }}>
              No scenarios saved. Run one above to keep it here.
            </div>
          ) : (
            history.map((h) => (
              <div key={h.id} style={{ display: "grid", gridTemplateColumns: "1fr auto auto auto", gap: 16, padding: "14px 20px", borderBottom: "1px solid var(--hairline)", alignItems: "center" }}>
                <div>
                  <div style={{ fontSize: 14 }}>{h.description}</div>
                  <span className={`chip ${h.result.verdict ? "positive" : "warning"}`} style={{ marginTop: 4 }}>
                    {h.result.verdict ? "Coverable" : "Not coverable"}
                  </span>
                </div>
                <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>
                  {new Date(h.createdAt).toLocaleDateString()}
                </span>
                <button className="btn ghost sm" onClick={() => setActive({ description: h.description, result: h.result })}>
                  Re-run
                </button>
                <button
                  className="btn ghost sm"
                  aria-label={`Delete ${h.description}`}
                  onClick={async () => {
                    await del.mutateAsync(h.id);
                    toast("Scenario deleted");
                  }}
                >
                  <I.Trash />
                </button>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
```

> Notes: (1) Verify the `ScenarioParamsInput` field names against the regenerated `bindings.ts` (camelCase: `incomeDeltaPct`, `monthlyExpenseDeltaCents`, `oneTimeCents`, `startMonthOffset`, `label`). (2) Verify `CategoryWithSpending` exposes `thisMonthCents` and `label` (per CLAUDE.md it is camelCase). (3) "Re-run" on a saved scenario re-displays the stored result rather than recomputing — acceptable for v1.

- [ ] **Step 2: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors. Fix any field-name mismatches against `bindings.ts`.

- [ ] **Step 3: Commit**

```bash
git add ui/src/screens/Scenarios.tsx
git commit -m "feat(ui): Scenarios screen with dual-line forecast chart (TODO §1)"
```

---

## Task 7: Route + sidebar nav (§15a)

**Files:**
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/components/Sidebar.tsx`

- [ ] **Step 1: Add the route**

In `ui/src/App.tsx`, add the import alongside the other screen imports:

```tsx
import Scenarios from "./screens/Scenarios";
```

And add the route between goals and reports inside `<Routes>`:

```tsx
              <Route path="/goals"        element={<Goals />} />
              <Route path="/scenarios"    element={<Scenarios />} />
              <Route path="/reports"      element={<Reports />} />
```

- [ ] **Step 2: Add the nav entry**

In `ui/src/components/Sidebar.tsx`, add to `NAV_MAIN` between `goals` and `reports`:

```tsx
  { id: "goals",        path: "/goals",         label: "Goals",        Icon: I.Goal },
  { id: "scenarios",    path: "/scenarios",     label: "Scenarios",    Icon: I.Bolt },
  { id: "reports",      path: "/reports",       label: "Reports",      Icon: I.Spark },
```

- [ ] **Step 3: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/App.tsx ui/src/components/Sidebar.tsx
git commit -m "feat(ui): add Scenarios route + sidebar nav (TODO §1, §15a)"
```

---

## Task 8: Frontend test

**Files:**
- Create: `ui/src/screens/Scenarios.test.tsx`

- [ ] **Step 1: Write the test**

Create `ui/src/screens/Scenarios.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Scenarios from "./Scenarios";
import { createWrapper } from "../test-utils";

const runMutate = vi.fn();

vi.mock("../api/hooks/useScenarios", () => ({
  useScenarioHistory: vi.fn(() => ({ data: [] })),
  useRunScenario: vi.fn(() => ({ mutateAsync: runMutate, isPending: false })),
  useSaveScenario: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteScenario: vi.fn(() => ({ mutateAsync: vi.fn() })),
}));

vi.mock("../api/client", () => ({
  commands: {
    listCategoriesWithSpending: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: [{ label: "Dining", thisMonthCents: 30000 }] }),
  },
}));

const RESULT = {
  verdict: true,
  runwayChangeDays: -20,
  monthlyImpactCents: -50000,
  considerations: ["Runway shortens by 20 days."],
  baselineMonthly: [100000, 110000, 120000],
  scenarioMonthly: [100000, 105000, 110000],
  goalsAffected: ["House Fund: +2 mo"],
};

describe("Scenarios screen", () => {
  beforeEach(() => {
    runMutate.mockReset();
    runMutate.mockResolvedValue(RESULT);
  });

  it("renders the header and suggested chips", () => {
    render(<Scenarios />, { wrapper: createWrapper() });
    expect(screen.getByText("Imagine a future, see the math.")).toBeInTheDocument();
    expect(screen.getByText("Cut income 50%")).toBeInTheDocument();
  });

  it("running a chip shows the verdict panel", async () => {
    render(<Scenarios />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Buy a car $35k"));
    await waitFor(() => expect(runMutate).toHaveBeenCalled());
    await waitFor(() =>
      expect(screen.getByText("You can do this — here's what changes.")).toBeInTheDocument()
    );
    expect(screen.getByText("Runway shortens by 20 days.")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test**

Run: `cd ui && npx vitest run src/screens/Scenarios.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 3: Commit**

```bash
git add ui/src/screens/Scenarios.test.tsx
git commit -m "test(ui): Scenarios screen render + chip-run (TODO §1)"
```

---

## Task 9: Full verification + mark TODO done

**Files:**
- Modify: `docs/TODO.md`

- [ ] **Step 1: Run the full Rust suite**

Run: `cargo test --workspace`
Expected: all pass (keychain round-trip may be flaky on Windows — re-run if only that fails).

- [ ] **Step 2: Run the full frontend suite**

Run: `cd ui && npx vitest run`
Expected: all pass (was 51 tests + the 2 new = 53).

- [ ] **Step 3: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 4: Mark §1 and §15a done in the TODO**

In `docs/TODO.md`:
- Change the `## 1. Scenarios screen` heading to append ` ✅ DONE`.
- Change `### 15a. Scenarios in nav` to append ` ✅ DONE`.
- In the priority table, set the Scenarios row Status to ✅ Done.

- [ ] **Step 5: Commit**

```bash
git add docs/TODO.md
git commit -m "docs: mark Scenarios screen (§1, §15a) done"
```

---

## Self-review notes

- **Spec coverage:** verdict/runway/monthly-impact/considerations/baseline/scenario/goals-affected → Task 2 engine + Task 3 result type. `run_scenario`/`save_scenario`/`list_scenario_history`/`delete_scenario` → Task 3. LLM-parse-with-fallback → Task 4. Screen (input, chips, verdict card, impact grid, chart, considerations, save/discard, history, re-run) → Task 6. Route + nav (§15a) → Task 7. V005 migration → Task 1. Tests → Tasks 1, 2, 8, 9.
- **Out of scope (per spec):** "Add constraints to forecast" and "Set reminder" buttons, LLM-authored prose. Not in any task — intentional.
- **Type consistency:** `ScenarioResult`/`ScenarioParamsInput`/`SavedScenario` defined in Task 3, consumed identically in Tasks 5/6/8. Core `ScenarioParams`/`Snapshot`/`GoalInfo`/`Projection` defined in Task 2, consumed in Task 3. `runScenario(description, months, params)` arg order flagged for verification against generated bindings in Tasks 5 & 6.
- **Known v1 simplification:** the dining chip resolves its amount from `listCategoriesWithSpending` (falls back to $400) rather than a backend category lookup, keeping the params seam pure.
