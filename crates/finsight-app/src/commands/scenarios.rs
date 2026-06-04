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
        None => extract_params_via_llm(&state, &description, &snapshot).await?,
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
