use crate::{
    error::{AppError, AppResult},
    AppState,
};
use chrono::{Duration, Utc};
use finsight_core::repos::run;
use rusqlite::params;
use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JourneyStatus {
    pub milestones: Vec<JourneyMilestone>,
    pub current_stage: u8,
    pub completed_count: u8,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JourneyMilestone {
    pub stage: u8,
    pub name: String,
    pub description: String,
    pub status: String,
    pub progress_pct: u8,
    pub detail: String,
    pub action_prompt: String,
}

fn fmt_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}${:.0}", cents.abs() as f64 / 100.0)
}

fn clamp_pct(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

#[tauri::command]
#[specta::specta]
pub async fn get_journey_status(state: tauri::State<'_, AppState>) -> AppResult<JourneyStatus> {
    let db = (*state.db).clone();

    run(&db, move |conn| {
        let now = Utc::now();
        let month = now.format("%Y-%m").to_string();
        let rolling_cutoff = (now - Duration::days(90)).to_rfc3339();
        let past_30_date = (now - Duration::days(30)).format("%Y-%m-%d").to_string();

        let accounts_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM accounts WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        let transactions_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?;

        let liquid_balance_cents: i64 = conn.query_row(
            "SELECT COALESCE(SUM(COALESCE(
                 (SELECT balance_cents FROM account_balances b
                  WHERE b.account_id = a.id ORDER BY b.as_of_date DESC LIMIT 1), 0
             )), 0) FROM accounts a WHERE a.archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;

        let active_debt_goal_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM goals
             WHERE type = 'debt-payoff'
               AND archived_at IS NULL
               AND (target_cents - current_cents) > 0",
            [],
            |r| r.get(0),
        )?;

        let starter_goal_cents: i64 = conn.query_row(
            "SELECT COALESCE(MAX(CASE
                WHEN target_cents >= 100000 THEN current_cents
                ELSE 0
             END), 0)
             FROM goals
             WHERE archived_at IS NULL",
            [],
            |r| r.get(0),
        )?;

        let budgets_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM budgets WHERE month = ?1 AND amount_cents > 0",
            params![month],
            |r| r.get(0),
        )?;

        let (debt_remaining_cents, debt_target_cents): (i64, i64) = conn.query_row(
            "SELECT
                COALESCE(SUM(MAX(target_cents - current_cents, 0)), 0),
                COALESCE(SUM(target_cents), 0)
             FROM goals
             WHERE type = 'debt-payoff'
               AND archived_at IS NULL",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        let (rolling_income_total, rolling_expense_total): (i64, i64) = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN amount_cents > 0 THEN amount_cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0)
             FROM transactions
             WHERE posted_at >= ?1",
            params![rolling_cutoff],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        let avg_monthly_expense_cents = rolling_expense_total / 3;
        let avg_monthly_income_cents = rolling_income_total / 3;
        let avg_savings_rate_pct = if avg_monthly_income_cents > 0 {
            (((avg_monthly_income_cents - avg_monthly_expense_cents).max(0) * 100)
                / avg_monthly_income_cents)
                .clamp(0, 100)
        } else {
            0
        };

        let current_net_worth_cents: i64 = conn.query_row(
            "SELECT
                COALESCE((
                    SELECT SUM(COALESCE(
                        (SELECT balance_cents FROM account_balances b
                         WHERE b.account_id = a.id ORDER BY b.as_of_date DESC LIMIT 1), 0
                    ))
                    FROM accounts a
                    WHERE a.archived_at IS NULL
                ), 0)
                + COALESCE((SELECT SUM(value_cents) FROM manual_assets), 0)
                - COALESCE((SELECT SUM(balance_cents) FROM liabilities), 0)",
            [],
            |r| r.get(0),
        )?;

        let snapshot_30_days_ago: Option<i64> = conn
            .query_row(
                "SELECT total_cents
                 FROM net_worth_snapshots
                 WHERE date <= ?1
                 ORDER BY date DESC
                 LIMIT 1",
                params![past_30_date],
                |r| r.get(0),
            )
            .ok();

        let stage1_completed = accounts_count >= 1 && transactions_count >= 30;
        let stage1_progress = clamp_pct(
            ((accounts_count.min(1) as f64) / 1.0) * 50.0
                + ((transactions_count.min(30) as f64) / 30.0) * 50.0,
        );
        let stage1_detail = format!(
            "{} account{} linked · {} of 30 transactions imported",
            accounts_count,
            if accounts_count == 1 { "" } else { "s" },
            transactions_count.min(30)
        );

        let starter_goal_progress = (starter_goal_cents.min(100_000) as f64 / 100_000.0) * 100.0;
        let liquid_progress = (liquid_balance_cents.clamp(0, 100_000) as f64 / 100_000.0) * 100.0;
        let stage2_completed =
            starter_goal_cents >= 100_000 || (liquid_balance_cents >= 100_000 && active_debt_goal_count < 2);
        let mut stage2_progress = clamp_pct(starter_goal_progress.max(liquid_progress));
        if !stage2_completed && stage2_progress == 100 {
            stage2_progress = 99;
        }
        let stage2_detail = if starter_goal_cents > 0 {
            format!(
                "Starter fund goal: {} of $1,000 target",
                fmt_money(starter_goal_cents.min(100_000))
            )
        } else if liquid_balance_cents >= 100_000 && active_debt_goal_count >= 2 {
            format!(
                "You already have {} set aside, but {} debt payoff goals are still active",
                fmt_money(liquid_balance_cents),
                active_debt_goal_count
            )
        } else {
            format!(
                "Liquid cash: {} of $1,000 target",
                fmt_money(liquid_balance_cents.clamp(0, 100_000))
            )
        };

        let stage3_completed = budgets_count >= 1;
        let stage3_progress = if stage3_completed { 100 } else { 0 };
        let stage3_detail = if stage3_completed {
            format!(
                "{} budget envelope{} set for {}",
                budgets_count,
                if budgets_count == 1 { "" } else { "s" },
                month
            )
        } else {
            format!("No budget envelopes set for {} yet", month)
        };

        let stage4_completed = debt_remaining_cents <= 0;
        let stage4_progress = if debt_target_cents > 0 {
            clamp_pct(
                ((debt_target_cents - debt_remaining_cents).max(0) as f64 / debt_target_cents as f64)
                    * 100.0,
            )
        } else {
            100
        };
        let stage4_detail = if debt_remaining_cents > 0 {
            format!(
                "{} debt payoff goal{} active · {} remaining",
                active_debt_goal_count,
                if active_debt_goal_count == 1 { "" } else { "s" },
                fmt_money(debt_remaining_cents)
            )
        } else {
            "No active debt payoff goals remain".to_string()
        };

        let full_emergency_target_cents = avg_monthly_expense_cents.saturating_mul(3);
        let stage5_completed =
            full_emergency_target_cents > 0 && liquid_balance_cents >= full_emergency_target_cents;
        let stage5_progress = if full_emergency_target_cents > 0 {
            clamp_pct(
                (liquid_balance_cents.max(0).min(full_emergency_target_cents) as f64
                    / full_emergency_target_cents as f64)
                    * 100.0,
            )
        } else {
            0
        };
        let stage5_detail = if full_emergency_target_cents > 0 {
            format!(
                "Emergency fund: {} of {} target",
                fmt_money(liquid_balance_cents.max(0).min(full_emergency_target_cents)),
                fmt_money(full_emergency_target_cents)
            )
        } else {
            "Need at least 90 days of expense history to size your full emergency fund".to_string()
        };

        let stage6_completed = avg_monthly_income_cents > 0 && avg_savings_rate_pct >= 15;
        let stage6_progress = if avg_monthly_income_cents > 0 {
            clamp_pct((avg_savings_rate_pct.min(15) as f64 / 15.0) * 100.0)
        } else {
            0
        };
        let stage6_detail = if avg_monthly_income_cents > 0 {
            format!("Average savings rate over the last 90 days: {}%", avg_savings_rate_pct)
        } else {
            "No recent income data yet".to_string()
        };

        let stage7_upward = snapshot_30_days_ago
            .map(|past| current_net_worth_cents > past)
            .unwrap_or(false);
        let stage7_completed = current_net_worth_cents > 0 && stage7_upward;
        let stage7_progress = if current_net_worth_cents <= 0 {
            0
        } else if let Some(past) = snapshot_30_days_ago {
            if current_net_worth_cents > past {
                100
            } else {
                60
            }
        } else {
            50
        };
        let stage7_detail = if let Some(past) = snapshot_30_days_ago {
            let delta = current_net_worth_cents - past;
            if delta >= 0 {
                format!(
                    "Net worth is {} · up {} over the last 30 days",
                    fmt_money(current_net_worth_cents),
                    fmt_money(delta)
                )
            } else {
                format!(
                    "Net worth is {} · down {} over the last 30 days",
                    fmt_money(current_net_worth_cents),
                    fmt_money(-delta)
                )
            }
        } else if current_net_worth_cents > 0 {
            format!(
                "Net worth is {} · keep recording snapshots to confirm the 30-day trend",
                fmt_money(current_net_worth_cents)
            )
        } else {
            format!("Net worth is {}", fmt_money(current_net_worth_cents))
        };

        let mut milestones = vec![
            (
                1u8,
                "Know Your Numbers".to_string(),
                "Connect an account and bring in enough transaction history to see your real patterns."
                    .to_string(),
                stage1_completed,
                stage1_progress,
                stage1_detail,
                "Help me connect my accounts, import at least 30 transactions, and start tracking my real spending patterns in FinSight.".to_string(),
            ),
            (
                2u8,
                "Starter Emergency Fund".to_string(),
                "Build your first $1,000 buffer so surprises stop becoming crises.".to_string(),
                stage2_completed,
                stage2_progress,
                stage2_detail,
                "I want to build a $1,000 starter emergency fund. Help me free up cash and decide where to keep it.".to_string(),
            ),
            (
                3u8,
                "Budget Every Dollar".to_string(),
                "Give this month's income a clear job so your plan is intentional.".to_string(),
                stage3_completed,
                stage3_progress,
                stage3_detail,
                "Help me build a zero-based budget in FinSight so every dollar of income has a job this month.".to_string(),
            ),
            (
                4u8,
                "Debt-Free".to_string(),
                "Knock out active payoff goals until no high-friction debt remains.".to_string(),
                stage4_completed,
                stage4_progress,
                stage4_detail,
                "Help me create a debt payoff plan and prioritize my remaining balances using a realistic monthly strategy.".to_string(),
            ),
            (
                5u8,
                "Full Emergency Fund".to_string(),
                "Grow your cash reserve to cover at least three months of essential expenses.".to_string(),
                stage5_completed,
                stage5_progress,
                stage5_detail,
                "Help me build a full emergency fund worth three months of expenses and figure out how much to save each month.".to_string(),
            ),
            (
                6u8,
                "Saving 15%+".to_string(),
                "Move from occasional saving to consistently keeping 15% or more of your income.".to_string(),
                stage6_completed,
                stage6_progress,
                stage6_detail,
                "My goal is to save at least 15% of my income. Help me find room in my plan and automate the habit.".to_string(),
            ),
            (
                7u8,
                "Building Assets".to_string(),
                "Keep net worth positive and rising so your money starts compounding for you.".to_string(),
                stage7_completed,
                stage7_progress,
                stage7_detail,
                "Help me grow my assets, improve my net worth trend, and decide what to focus on next for long-term wealth.".to_string(),
            ),
        ];

        let completed_count = milestones.iter().filter(|m| m.3).count() as u8;
        let current_stage = milestones
            .iter()
            .find(|m| !m.3)
            .map(|m| m.0)
            .unwrap_or(7);

        let milestones = milestones
            .drain(..)
            .map(|(stage, name, description, completed, progress_pct, detail, action_prompt)| {
                let status = if completed {
                    "completed"
                } else if stage == current_stage {
                    "current"
                } else {
                    "upcoming"
                };

                JourneyMilestone {
                    stage,
                    name,
                    description,
                    status: status.to_string(),
                    progress_pct,
                    detail,
                    action_prompt,
                }
            })
            .collect();

        Ok(JourneyStatus {
            milestones,
            current_stage,
            completed_count,
        })
    })
    .await
    .map_err(AppError::from)
}
