use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_agent::context::build_context;
use finsight_core::models::AgentMemory;
use finsight_core::repos::{agent_memory, run};
use serde::Serialize;
use specta::Type;

#[tauri::command]
#[specta::specta]
pub async fn list_agent_memory(state: tauri::State<'_, AppState>) -> AppResult<Vec<AgentMemory>> {
    let db = (*state.db).clone();
    run(&db, agent_memory::list).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn forget_agent_memory(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| agent_memory::forget(conn, &id))
        .await
        .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthScoreBreakdown {
    pub savings_rate_pts: u8,
    pub emergency_fund_pts: u8,
    pub debt_ratio_pts: u8,
    pub goal_progress_pts: u8,
    pub budget_adherence_pts: u8,
    pub savings_rate_pct: i64,
    pub emergency_fund_months: f64,
    pub debt_to_income_pct: i64,
    pub avg_goal_pct: i64,
    pub budget_adherence_pct: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthScore {
    pub total: u8,
    pub grade: String,
    pub breakdown: HealthScoreBreakdown,
    pub tips: Vec<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn get_financial_health_score(
    state: tauri::State<'_, AppState>,
) -> AppResult<HealthScore> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let ctx = build_context(conn);
        // Score against the user's configured targets, not hardcoded numbers, so
        // the scorecard reflects the goals set in Settings → Financial targets.
        let assumptions = finsight_core::metrics::assumptions(conn);
        let savings_target = assumptions.target_savings_rate_pct;
        let ef_target = assumptions.emergency_fund_target_months;

        let sr = ctx.cashflow.savings_rate_pct;
        // Full credit at the target, half credit at halfway to it.
        let savings_rate_pts: u8 = if sr >= savings_target {
            25
        } else if sr >= savings_target / 2 {
            15
        } else {
            0
        };

        let ef = ctx.wellness.emergency_fund_months;
        let emergency_fund_pts: u8 = if ef >= ef_target {
            25
        } else if ef >= ef_target / 2.0 {
            15
        } else if ef >= 1.0 {
            8
        } else {
            0
        };

        let annual_income = ctx.cashflow.avg_monthly_income_cents * 12;
        let debt_to_income_pct = if annual_income > 0 {
            (ctx.wellness.total_debt_cents * 100) / annual_income
        } else {
            0
        };
        let debt_ratio_pts = if ctx.wellness.total_debt_cents == 0 {
            20
        } else if debt_to_income_pct < 20 {
            15
        } else if debt_to_income_pct < 40 {
            8
        } else {
            0
        };

        let avg_goal_pct = if ctx.goals.is_empty() {
            50
        } else {
            ctx.goals.iter().map(|g| g.pct_complete).sum::<i64>() / ctx.goals.len() as i64
        };
        let goal_progress_pts = ((avg_goal_pct.clamp(0, 100) * 15) / 100).min(15) as u8;

        let total_budget_categories: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM budgets WHERE month = ?1",
                rusqlite::params![ctx.budget.month],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let overage_count = ctx.budget.overages.len() as i64;
        let budget_adherence_pts = if overage_count == 0 {
            15
        } else if overage_count <= 2 {
            8
        } else if overage_count <= 4 {
            4
        } else {
            0
        };
        let budget_adherence_pct = if total_budget_categories > 0 {
            (((total_budget_categories - overage_count).max(0)) * 100) / total_budget_categories
        } else if ctx.budget.overages.is_empty() {
            100
        } else {
            50
        };

        let total = savings_rate_pts
            + emergency_fund_pts
            + debt_ratio_pts
            + goal_progress_pts
            + budget_adherence_pts;
        let grade = match total {
            85..=100 => "A",
            70..=84 => "B",
            55..=69 => "C",
            40..=54 => "D",
            _ => "F",
        }
        .to_string();

        let mut tips = Vec::new();
        if savings_rate_pts < 25 {
            tips.push(format!(
                "Increase savings rate to ≥{}% (currently {}%)",
                savings_target, sr
            ));
        }
        if emergency_fund_pts < 25 {
            tips.push(format!(
                "Build emergency fund to {:.0}+ months (currently {:.1} months)",
                ef_target, ef
            ));
        }
        if debt_ratio_pts < 20 && ctx.wellness.total_debt_cents > 0 {
            tips.push(
                "Reduce debt using the Snowball method (pay smallest balance first)".to_string(),
            );
        }
        if goal_progress_pts < 10 {
            tips.push("Increase monthly contributions to active goals".to_string());
        }
        if budget_adherence_pts < 15 {
            tips.push(format!(
                "{} budget categor{} over limit — review spending",
                overage_count,
                if overage_count == 1 { "y" } else { "ies" }
            ));
        }
        tips.truncate(3);

        Ok(HealthScore {
            total,
            grade,
            breakdown: HealthScoreBreakdown {
                savings_rate_pts,
                emergency_fund_pts,
                debt_ratio_pts,
                goal_progress_pts,
                budget_adherence_pts,
                savings_rate_pct: sr,
                emergency_fund_months: ef,
                debt_to_income_pct,
                avg_goal_pct,
                budget_adherence_pct,
            },
            tips,
        })
    })
    .await
    .map_err(AppError::from)
}
