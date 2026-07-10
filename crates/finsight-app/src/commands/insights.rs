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
        // Virtual "Unbudgeted" envelope: spend with no category bypasses every
        // real envelope, so a "0 overages" score is false comfort when a big
        // chunk of the month is uncategorized. Count material uncategorized spend
        // (> 5% of this month's income, floor $50) as one more overage so budget
        // adherence reflects the spend the envelopes never saw.
        let month_start = format!("{}-01", ctx.budget.month);
        let unbudgeted_cents: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
                 WHERE category_id IS NULL AND amount_cents < 0 AND is_transfer = 0 AND posted_at >= ?1",
                rusqlite::params![month_start],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let income_this_month = ctx.cashflow.this_month_income_cents.max(0);
        // Only counts when the user actually budgets — that's where uncategorized
        // spend produces the "false under-budget" comfort. With no budgets at all,
        // the budget component is already neutral.
        let unbudgeted_material = unbudgeted_cents > (income_this_month / 20).max(5_000)
            && total_budget_categories > 0;
        let unbudgeted_overage = if unbudgeted_material { 1 } else { 0 };

        let overage_count = ctx.budget.overages.len() as i64 + unbudgeted_overage;
        let budget_adherence_pts = if overage_count == 0 {
            15
        } else if overage_count <= 2 {
            8
        } else if overage_count <= 4 {
            4
        } else {
            0
        };
        // The virtual Unbudgeted envelope joins the denominator when it fires.
        let effective_categories = total_budget_categories + unbudgeted_overage;
        let budget_adherence_pct = if effective_categories > 0 {
            (((effective_categories - overage_count).max(0)) * 100) / effective_categories
        } else {
            // No budgets set at all → adherence is not applicable; stay neutral.
            100
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
        if unbudgeted_material {
            tips.push(
                "Some of this month's spending has no category, so it isn't tracked against any budget — categorize it for an accurate score.".to_string(),
            );
        }
        let real_overages = ctx.budget.overages.len();
        if budget_adherence_pts < 15 && real_overages > 0 {
            tips.push(format!(
                "{} budget categor{} over limit — review spending",
                real_overages,
                if real_overages == 1 { "y" } else { "ies" }
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
