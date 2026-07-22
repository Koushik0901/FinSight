use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::forecast::{self, ScenarioParams, Snapshot};
use finsight_core::repos::{run, scenarios as scenarios_repo};
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

// Serialize as well as Deserialize: the resolved params travel back to the UI
// (so a free-text scenario can be saved) and are persisted with the scenario.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioParamsInput {
    pub income_delta_pct: i32,
    pub monthly_expense_delta_cents: i64,
    pub one_time_cents: i64,
    pub start_month_offset: u32,
    pub label: String,
}

/// A run's result together with the resolved params, so the UI can save a
/// scenario it ran from free text (where the params were resolved server-side).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RanScenario {
    pub result: ScenarioResult,
    pub params: ScenarioParamsInput,
    pub months: u32,
}

/// A compact view of the baseline a scenario was computed against, for display
/// and for showing the user what moved when a scenario goes stale.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BaselineSummary {
    pub balance_cents: i64,
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub goal_count: i64,
}

/// A saved scenario with everything needed to compare and act on it. The
/// `original_*` fields are exactly what was saved; `current_result`/`is_stale`
/// recompute it against TODAY's baseline so every compared scenario shares one
/// baseline (consistent by construction) while the original stays distinct.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedScenarioDetail {
    pub id: String,
    pub description: String,
    pub created_at: String,
    pub months: u32,
    /// `None` for legacy result-only rows saved before durable scenarios.
    pub params: Option<ScenarioParamsInput>,
    pub original_result: ScenarioResult,
    pub original_baseline: Option<BaselineSummary>,
    /// The scenario re-run against the current baseline. `None` when the row
    /// lacks params/baseline (legacy) and can't be recomputed.
    pub current_result: Option<ScenarioResult>,
    /// Whether the current baseline differs materially from the saved one.
    pub is_stale: Option<bool>,
    pub recomputable: bool,
    /// A REVISED set of assumptions the user is evaluating (issue #73), or `None`.
    pub revised_params: Option<ScenarioParamsInput>,
    /// The revised params run against the CURRENT baseline — same baseline as
    /// `current_result`, so the difference between the two is purely the effect
    /// of the assumption edit (never confused with live-data drift). `None` when
    /// there is no revision or the row can't be recomputed.
    pub revised_result: Option<ScenarioResult>,
}

/// One proposed plan change from promoting a scenario — a suggestion for the
/// user to review, never an applied mutation.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanChange {
    pub title: String,
    pub detail: String,
    pub current_cents: Option<i64>,
    pub proposed_cents: Option<i64>,
}

/// The reviewable result of promoting a scenario. Deliberately carries NO write
/// path: promoting produces suggestions only, so exploration can never silently
/// change live budgets, goals, or debt plans.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioPlanProposal {
    pub scenario_id: String,
    pub description: String,
    pub changes: Vec<PlanChange>,
    pub note: String,
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

fn to_core_params(p: &ScenarioParamsInput) -> ScenarioParams {
    ScenarioParams {
        income_delta_pct: p.income_delta_pct,
        monthly_expense_delta_cents: p.monthly_expense_delta_cents,
        one_time_cents: p.one_time_cents,
        start_month_offset: p.start_month_offset,
        label: p.label.clone(),
    }
}

fn from_core_params(p: &ScenarioParams) -> ScenarioParamsInput {
    ScenarioParamsInput {
        income_delta_pct: p.income_delta_pct,
        monthly_expense_delta_cents: p.monthly_expense_delta_cents,
        one_time_cents: p.one_time_cents,
        start_month_offset: p.start_month_offset,
        label: p.label.clone(),
    }
}

fn baseline_summary(s: &Snapshot) -> BaselineSummary {
    BaselineSummary {
        balance_cents: s.balance_cents,
        avg_monthly_income_cents: s.avg_monthly_income_cents,
        avg_monthly_expense_cents: s.avg_monthly_expense_cents,
        goal_count: s.goals.len() as i64,
    }
}

fn fmt_money(cents: i64) -> String {
    format!("${:.0}", (cents.unsigned_abs() as f64) / 100.0)
}

/// The current financial baseline the projections run against. Delegates to the
/// shared `finsight-core` builder so save-time, compare-time, and the Copilot
/// all use one identical baseline — otherwise staleness would compare apples to
/// oranges.
async fn build_snapshot(state: &ApiState) -> AppResult<Snapshot> {
    let db = (*state.db).clone();
    run(&db, scenarios_repo::build_baseline)
        .await
        .map_err(AppError::from)
}

async fn extract_params_via_llm(
    state: &ApiState,
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

    let obj = v.as_object().ok_or_else(|| {
        AppError::new(
            "scenario.llm_parse",
            "The AI returned an unexpected response. Try rephrasing your question, e.g. \"What if I cut rent by $300?\"",
        )
    })?;
    let recognized = [
        "income_delta_pct",
        "monthly_expense_delta_cents",
        "one_time_cents",
        "start_month_offset",
    ]
    .iter()
    .any(|k| obj.contains_key(*k));
    if !recognized {
        return Err(AppError::new(
            "scenario.llm_parse",
            "Couldn't interpret that as a financial scenario. Try rephrasing, e.g. \"What if I cut rent by $300?\"",
        ));
    }

    Ok(ScenarioParams {
        income_delta_pct: v.get("income_delta_pct").and_then(|x| x.as_i64()).unwrap_or(0) as i32,
        monthly_expense_delta_cents: v.get("monthly_expense_delta_cents").and_then(|x| x.as_i64()).unwrap_or(0),
        one_time_cents: v.get("one_time_cents").and_then(|x| x.as_i64()).unwrap_or(0),
        start_month_offset: v.get("start_month_offset").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
        label: v.get("label").and_then(|x| x.as_str()).unwrap_or(description).to_string(),
    })
}

pub async fn run_scenario(
    state: &ApiState,
    description: String,
    months: u32,
    params: Option<ScenarioParamsInput>,
) -> AppResult<RanScenario> {
    let snapshot = build_snapshot(state).await?;
    let core_params = match &params {
        Some(p) => to_core_params(p),
        None => extract_params_via_llm(state, &description, &snapshot).await?,
    };
    let proj = forecast::project(&snapshot, &core_params, months);
    Ok(RanScenario {
        result: projection_to_result(proj),
        params: from_core_params(&core_params),
        months,
    })
}

/// Save a scenario durably: capture the current baseline, re-project the params
/// against it, and store params + baseline + result together so the scenario
/// can later be recomputed, compared, and checked for staleness.
pub async fn save_scenario(
    state: &ApiState,
    description: String,
    params: ScenarioParamsInput,
    months: u32,
) -> AppResult<SavedScenarioDetail> {
    let baseline = build_snapshot(state).await?;
    let core_params = to_core_params(&params);
    let result = projection_to_result(forecast::project(&baseline, &core_params, months));

    let result_json = serde_json::to_string(&result).map_err(|e| AppError::new("scenario.serialize", e.to_string()))?;
    let params_json = serde_json::to_string(&params).map_err(|e| AppError::new("scenario.serialize", e.to_string()))?;
    let baseline_json = serde_json::to_string(&baseline).map_err(|e| AppError::new("scenario.serialize", e.to_string()))?;

    let db = (*state.db).clone();
    let row = run(&db, move |conn| {
        scenarios_repo::insert(conn, &description, &result_json, Some(&params_json), Some(&baseline_json), Some(months as i64))
    })
    .await
    .map_err(AppError::from)?;

    // Just saved → current baseline IS the saved one, so not stale.
    Ok(detail_from_row(row, &baseline))
}

/// Revise a saved scenario's assumptions (issue #73). Stores a second set of
/// what-if params alongside the immutable original; the returned detail carries
/// the recalculated `revised_result` next to the original and current results.
/// Never touches the active plan, and never overwrites the original params.
pub async fn revise_scenario(
    state: &ApiState,
    id: String,
    params: ScenarioParamsInput,
) -> AppResult<SavedScenarioDetail> {
    let current = build_snapshot(state).await?;
    let revised_json =
        serde_json::to_string(&params).map_err(|e| AppError::new("scenario.serialize", e.to_string()))?;
    let db = (*state.db).clone();

    let id_load = id.clone();
    let existing = run(&db, move |conn| scenarios_repo::get(conn, &id_load))
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::new("scenario.not_found", "That scenario no longer exists."))?;
    // Legacy result-only rows have no assumptions to revise.
    if existing.params_json.is_none() || existing.baseline_json.is_none() {
        return Err(AppError::new(
            "scenario.legacy",
            "This scenario was saved before durable scenarios, so its assumptions can't be revised. Re-run and save it to enable this.",
        ));
    }

    let id_set = id.clone();
    let row = run(&db, move |conn| {
        scenarios_repo::set_revised_params(conn, &id_set, Some(&revised_json))?;
        scenarios_repo::get(conn, &id_set)
    })
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::new("scenario.not_found", "That scenario no longer exists."))?;

    Ok(detail_from_row(row, &current))
}

/// Discard a scenario's revision, reverting to the original assumptions only.
pub async fn clear_scenario_revision(state: &ApiState, id: String) -> AppResult<SavedScenarioDetail> {
    let current = build_snapshot(state).await?;
    let db = (*state.db).clone();
    let id2 = id.clone();
    let row = run(&db, move |conn| {
        scenarios_repo::set_revised_params(conn, &id2, None)?;
        scenarios_repo::get(conn, &id2)
    })
    .await
    .map_err(AppError::from)?
    .ok_or_else(|| AppError::new("scenario.not_found", "That scenario no longer exists."))?;
    Ok(detail_from_row(row, &current))
}

/// Turn a stored row into a comparable detail: recompute against `current` and
/// flag staleness. Pure and infallible — malformed legacy JSON degrades to a
/// non-recomputable row rather than failing the whole list.
fn detail_from_row(row: scenarios_repo::ScenarioRow, current: &Snapshot) -> SavedScenarioDetail {
    let original_result: ScenarioResult = serde_json::from_str(&row.result_json).unwrap_or(ScenarioResult {
        verdict: false,
        runway_change_days: 0,
        monthly_impact_cents: 0,
        considerations: vec!["This saved result could not be read.".to_string()],
        baseline_monthly: Vec::new(),
        scenario_monthly: Vec::new(),
        goals_affected: Vec::new(),
    });
    let params: Option<ScenarioParamsInput> = row.params_json.as_deref().and_then(|s| serde_json::from_str(s).ok());
    let baseline: Option<Snapshot> = row.baseline_json.as_deref().and_then(|s| serde_json::from_str(s).ok());
    let months = row.months.unwrap_or(12).clamp(1, 120) as u32;

    let (current_result, is_stale) = match (&params, &baseline) {
        (Some(p), Some(b)) => {
            let cur = projection_to_result(forecast::project(current, &to_core_params(p), months));
            (Some(cur), Some(forecast::baseline_materially_changed(b, current)))
        }
        _ => (None, None),
    };

    // A revision (issue #73): the revised params run against the SAME current
    // baseline, so `current_result` vs `revised_result` isolates the assumption
    // edit. Only meaningful when the row is recomputable.
    let revised_params: Option<ScenarioParamsInput> =
        row.revised_params_json.as_deref().and_then(|s| serde_json::from_str(s).ok());
    let revised_result = match (&revised_params, current_result.is_some()) {
        (Some(rp), true) => Some(projection_to_result(forecast::project(current, &to_core_params(rp), months))),
        _ => None,
    };

    SavedScenarioDetail {
        id: row.id,
        description: row.description,
        created_at: row.created_at,
        months,
        recomputable: current_result.is_some(),
        params,
        original_result,
        original_baseline: baseline.as_ref().map(baseline_summary),
        current_result,
        is_stale,
        revised_params,
        revised_result,
    }
}

/// Active saved scenarios, each recomputed against the current baseline so a
/// comparison across them is consistent by construction.
pub async fn list_saved_scenarios(state: &ApiState) -> AppResult<Vec<SavedScenarioDetail>> {
    let current = build_snapshot(state).await?;
    let db = (*state.db).clone();
    let rows = run(&db, scenarios_repo::list).await.map_err(AppError::from)?;
    Ok(rows.into_iter().map(|row| detail_from_row(row, &current)).collect())
}

pub async fn duplicate_scenario(state: &ApiState, id: String) -> AppResult<Option<SavedScenarioDetail>> {
    let current = build_snapshot(state).await?;
    let db = (*state.db).clone();
    let row = run(&db, move |conn| scenarios_repo::duplicate(conn, &id)).await.map_err(AppError::from)?;
    Ok(row.map(|r| detail_from_row(r, &current)))
}

pub async fn archive_scenario(state: &ApiState, id: String, archived: bool) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| scenarios_repo::set_archived(conn, &id, archived))
        .await
        .map_err(AppError::from)
}

/// Promote a scenario into a REVIEWABLE set of proposed plan changes. This
/// writes nothing: it grounds each proposal in the current baseline and hands
/// them back for the user to approve and apply themselves.
pub async fn promote_scenario(state: &ApiState, id: String) -> AppResult<ScenarioPlanProposal> {
    let current = build_snapshot(state).await?;
    let db = (*state.db).clone();
    let id2 = id.clone();
    let row = run(&db, move |conn| scenarios_repo::get(conn, &id2))
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::new("scenario.not_found", "That scenario no longer exists."))?;

    let params: ScenarioParamsInput = row
        .params_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .ok_or_else(|| {
            AppError::new(
                "scenario.legacy",
                "This scenario was saved before plan proposals existed, so it can't be promoted. Re-run and save it to enable this.",
            )
        })?;

    let original_result: ScenarioResult = serde_json::from_str(&row.result_json).unwrap_or(ScenarioResult {
        verdict: false,
        runway_change_days: 0,
        monthly_impact_cents: 0,
        considerations: Vec::new(),
        baseline_monthly: Vec::new(),
        scenario_monthly: Vec::new(),
        goals_affected: Vec::new(),
    });

    let mut changes = Vec::new();

    if params.monthly_expense_delta_cents != 0 {
        let cur = current.avg_monthly_expense_cents;
        let proposed = cur + params.monthly_expense_delta_cents;
        let (title, detail) = if params.monthly_expense_delta_cents < 0 {
            (
                "Trim monthly spending".to_string(),
                format!("Reduce your typical monthly spending by about {} — e.g. adjust the budgets it comes from.", fmt_money(params.monthly_expense_delta_cents)),
            )
        } else {
            (
                "Commit more each month".to_string(),
                format!("Set aside or spend about {} more each month, matching this scenario.", fmt_money(params.monthly_expense_delta_cents)),
            )
        };
        changes.push(PlanChange { title, detail, current_cents: Some(cur), proposed_cents: Some(proposed) });
    }

    if params.income_delta_pct != 0 {
        let cur = current.avg_monthly_income_cents;
        let proposed = ((cur as f64) * (1.0 + params.income_delta_pct as f64 / 100.0)).round() as i64;
        changes.push(PlanChange {
            title: "Plan around an income change".to_string(),
            detail: format!("This scenario assumes your monthly income changes by {}%. Update your plan if that becomes real.", params.income_delta_pct),
            current_cents: Some(cur),
            proposed_cents: Some(proposed),
        });
    }

    if params.one_time_cents != 0 {
        changes.push(PlanChange {
            title: "Set aside for a one-time amount".to_string(),
            detail: format!(
                "Plan for a one-off of about {}{}.",
                fmt_money(params.one_time_cents),
                if params.start_month_offset > 0 { format!(" in ~{} month(s)", params.start_month_offset) } else { String::new() }
            ),
            current_cents: None,
            proposed_cents: Some(params.one_time_cents),
        });
    }

    for affected in &original_result.goals_affected {
        changes.push(PlanChange {
            title: "Revisit a goal".to_string(),
            detail: format!("{affected} — adjust its contribution or target if you go ahead."),
            current_cents: None,
            proposed_cents: None,
        });
    }

    if changes.is_empty() {
        changes.push(PlanChange {
            title: "No changes to your plan".to_string(),
            detail: "This scenario doesn't imply any change to your monthly commitments.".to_string(),
            current_cents: None,
            proposed_cents: None,
        });
    }

    Ok(ScenarioPlanProposal {
        scenario_id: id,
        description: row.description,
        changes,
        note: "These are suggestions for your review — nothing has been changed. Apply each one yourself if you decide to go ahead.".to_string(),
    })
}

pub async fn delete_scenario(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| scenarios_repo::delete(conn, &id))
        .await
        .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::forecast::Snapshot;

    fn snap(income: i64, expense: i64) -> Snapshot {
        Snapshot {
            balance_cents: 2_000_000,
            avg_monthly_income_cents: income,
            avg_monthly_expense_cents: expense,
            goals: vec![],
        }
    }

    fn params(income_delta_pct: i32, expense_delta: i64) -> ScenarioParamsInput {
        ScenarioParamsInput {
            income_delta_pct,
            monthly_expense_delta_cents: expense_delta,
            one_time_cents: 0,
            start_month_offset: 0,
            label: "t".into(),
        }
    }

    fn row_with(
        original: &ScenarioParamsInput,
        baseline: &Snapshot,
        revised: Option<&ScenarioParamsInput>,
    ) -> scenarios_repo::ScenarioRow {
        let orig_result = projection_to_result(forecast::project(baseline, &to_core_params(original), 24));
        scenarios_repo::ScenarioRow {
            id: "s".into(),
            description: "d".into(),
            result_json: serde_json::to_string(&orig_result).unwrap(),
            created_at: "2026-01-01T00:00:00Z".into(),
            params_json: Some(serde_json::to_string(original).unwrap()),
            baseline_json: Some(serde_json::to_string(baseline).unwrap()),
            months: Some(24),
            archived_at: None,
            revised_params_json: revised.map(|r| serde_json::to_string(r).unwrap()),
        }
    }

    #[test]
    fn revision_recomputes_a_distinct_result_and_preserves_the_original() {
        let baseline = snap(500_000, 300_000);
        let original = params(0, 0);
        // Revised: income cut 50% — a materially different outcome.
        let revised = params(-50, 0);
        let row = row_with(&original, &baseline, Some(&revised));
        let orig_impact = projection_to_result(forecast::project(&baseline, &to_core_params(&original), 24)).monthly_impact_cents;

        // Recompute against the SAME baseline so any difference is the assumption edit alone.
        let detail = detail_from_row(row, &baseline);

        assert_eq!(detail.original_result.monthly_impact_cents, orig_impact, "original preserved");
        assert!(detail.revised_params.is_some());
        let current = detail.current_result.as_ref().expect("recomputable");
        let revised_res = detail.revised_result.as_ref().expect("revision recomputed");
        assert_ne!(
            revised_res.runway_change_days, current.runway_change_days,
            "an income cut must change the projected outcome vs the original assumptions"
        );
    }

    #[test]
    fn no_revision_yields_no_revised_result() {
        let baseline = snap(500_000, 300_000);
        let original = params(0, -20_000);
        let detail = detail_from_row(row_with(&original, &baseline, None), &baseline);
        assert!(detail.revised_result.is_none());
        assert!(detail.revised_params.is_none());
    }

    #[test]
    fn legacy_row_cannot_carry_a_revised_result() {
        // A revision on a row with no baseline can't be recomputed → no revised result.
        let baseline = snap(500_000, 300_000);
        let mut row = row_with(&params(0, 0), &baseline, Some(&params(-50, 0)));
        row.baseline_json = None;
        row.params_json = None;
        let detail = detail_from_row(row, &baseline);
        assert!(detail.current_result.is_none());
        assert!(detail.revised_result.is_none(), "not recomputable → no revised result");
    }
}
