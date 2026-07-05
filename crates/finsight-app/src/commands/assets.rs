use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::{Duration, Utc};
use finsight_core::models::{
    AccountType, ManualAsset, ManualAssetPatch, NetWorthPoint, NewManualAsset,
};
use finsight_core::repos::{accounts, manual_assets, net_worth, run};
use serde::Serialize;
use specta::Type;

#[tauri::command]
#[specta::specta]
pub async fn list_manual_assets(state: tauri::State<'_, AppState>) -> AppResult<Vec<ManualAsset>> {
    let db = (*state.db).clone();
    run(&db, manual_assets::list).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_manual_asset(
    state: tauri::State<'_, AppState>,
    input: NewManualAsset,
) -> AppResult<ManualAsset> {
    let db = (*state.db).clone();
    run(&db, move |conn| manual_assets::create(conn, input))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_manual_asset(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: ManualAssetPatch,
) -> AppResult<ManualAsset> {
    let db = (*state.db).clone();
    run(&db, move |conn| manual_assets::update(conn, &id, patch))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_manual_asset(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| manual_assets::delete(conn, &id))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn record_net_worth_snapshot(state: tauri::State<'_, AppState>) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, net_worth::record_today)
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_net_worth_history(
    state: tauri::State<'_, AppState>,
    days: u32,
) -> AppResult<Vec<NetWorthPoint>> {
    let db = (*state.db).clone();
    run(&db, move |conn| net_worth::list_history(conn, days))
        .await
        .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DebtPayoffMonth {
    pub month: i32,
    pub month_label: String,
    pub liability_id: String,
    pub liability_name: String,
    pub payment_cents: i64,
    pub interest_cents: i64,
    pub principal_cents: i64,
    pub remaining_balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DebtPayoffSummary {
    pub account_id: String,
    pub account_name: String,
    pub initial_balance_cents: i64,
    pub total_interest_cents: i64,
    pub payoff_month_label: String,
    pub months_to_payoff: i32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DebtPayoffResult {
    pub strategy: String,
    pub extra_monthly_cents: i64,
    pub total_interest_cents: i64,
    pub total_months: i32,
    pub payoff_date_label: String,
    pub summaries: Vec<DebtPayoffSummary>,
}

#[tauri::command]
#[specta::specta]
pub async fn compute_debt_payoff(
    state: tauri::State<'_, AppState>,
    extra_monthly_cents: i64,
) -> AppResult<Vec<DebtPayoffResult>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        // Debt is a Credit/Loan-type Account with a negative balance — not a
        // separate liabilities-table row. "Amount owed" is the positive
        // magnitude of that negative balance.
        let debts: Vec<(String, String, i64, Option<f64>, Option<i64>)> = accounts::list_summaries(conn)?
            .into_iter()
            .filter(|a| {
                a.balance_known
                    && matches!(a.r#type, AccountType::Credit | AccountType::Loan)
                    && a.balance_cents < 0
            })
            .map(|a| (a.id, a.name, -a.balance_cents, a.apr_pct, a.min_payment_cents))
            .collect();
        if debts.is_empty() {
            return Ok(vec![]);
        }

        let now = Utc::now();
        let base_payment_budget: i64 = debts
            .iter()
            .map(|(_, _, _, _, min_payment_cents)| min_payment_cents.unwrap_or(0).max(1_000))
            .sum::<i64>()
            + extra_monthly_cents.max(0);

        let mut results = Vec::new();
        for strategy in ["snowball", "avalanche"] {
            let mut debt_states: Vec<(String, String, i64, f64, i64)> = debts
                .iter()
                .map(|(id, name, amount_owed, apr_pct, min_payment_cents)| {
                    (
                        id.clone(),
                        name.clone(),
                        *amount_owed,
                        apr_pct.unwrap_or(0.0),
                        min_payment_cents.unwrap_or(0).max(1_000),
                    )
                })
                .collect();

            if strategy == "snowball" {
                debt_states.sort_by_key(|d| d.2);
            } else {
                debt_states.sort_by(|a, b| {
                    b.3.partial_cmp(&a.3)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.2.cmp(&b.2))
                });
            }

            let mut balances: Vec<i64> = debt_states.iter().map(|d| d.2).collect();
            let min_payments: Vec<i64> = debt_states.iter().map(|d| d.4).collect();
            let rates: Vec<f64> = debt_states.iter().map(|d| d.3).collect();
            let mut summaries: Vec<DebtPayoffSummary> = debt_states
                .iter()
                .map(|(id, name, balance, _, _)| DebtPayoffSummary {
                    account_id: id.clone(),
                    account_name: name.clone(),
                    initial_balance_cents: *balance,
                    total_interest_cents: 0,
                    payoff_month_label: String::new(),
                    months_to_payoff: 0,
                })
                .collect();

            let mut month = 0_i32;
            let mut total_interest_cents = 0_i64;
            let max_months = 360;

            while balances.iter().any(|b| *b > 0) && month < max_months {
                month += 1;
                let mut accrued_interest = vec![0_i64; balances.len()];
                let mut remaining_budget = base_payment_budget;

                for i in 0..balances.len() {
                    if balances[i] <= 0 {
                        continue;
                    }
                    let monthly_rate = rates[i] / 100.0 / 12.0;
                    let interest = ((balances[i] as f64) * monthly_rate).round() as i64;
                    accrued_interest[i] = interest.max(0);
                    balances[i] += accrued_interest[i];
                    total_interest_cents += accrued_interest[i];
                    summaries[i].total_interest_cents += accrued_interest[i];
                }

                for i in 0..balances.len() {
                    if balances[i] <= 0 {
                        continue;
                    }
                    let min_payment = min_payments[i].min(balances[i]);
                    balances[i] -= min_payment;
                    remaining_budget -= min_payment;
                    if balances[i] == 0 && summaries[i].payoff_month_label.is_empty() {
                        let payoff_date = now + Duration::days(month as i64 * 30);
                        summaries[i].payoff_month_label = payoff_date.format("%Y-%m").to_string();
                        summaries[i].months_to_payoff = month;
                    }
                }

                while remaining_budget > 0 {
                    let Some(target_idx) = balances.iter().position(|b| *b > 0) else {
                        break;
                    };
                    let extra_payment = remaining_budget.min(balances[target_idx]);
                    balances[target_idx] -= extra_payment;
                    remaining_budget -= extra_payment;
                    if balances[target_idx] == 0
                        && summaries[target_idx].payoff_month_label.is_empty()
                    {
                        let payoff_date = now + Duration::days(month as i64 * 30);
                        summaries[target_idx].payoff_month_label =
                            payoff_date.format("%Y-%m").to_string();
                        summaries[target_idx].months_to_payoff = month;
                    }
                }
            }

            let payoff_date = now + Duration::days(month as i64 * 30);
            results.push(DebtPayoffResult {
                strategy: strategy.to_string(),
                extra_monthly_cents,
                total_interest_cents,
                total_months: month,
                payoff_date_label: payoff_date.format("%Y-%m").to_string(),
                summaries,
            });
        }

        Ok(results)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_uncelebrated_milestones(state: tauri::State<'_, AppState>) -> AppResult<Vec<i64>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let milestones = vec![
            1_000_000_i64,
            2_500_000,
            5_000_000,
            10_000_000,
            25_000_000,
            50_000_000,
            100_000_000,
        ];
        let total_accounts: i64 = conn
            .query_row(
                "SELECT COALESCE(SUM(COALESCE(
                    (SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY b.as_of_date DESC, CASE source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END LIMIT 1),
                    0
                 )), 0)
                 FROM accounts a
                 WHERE a.archived_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let manual_assets_total: i64 = conn
            .query_row("SELECT COALESCE(SUM(value_cents), 0) FROM manual_assets", [], |r| r.get(0))
            .unwrap_or(0);
        // Debt is already folded into total_accounts as negative Credit/Loan
        // balances — no separate liabilities subtraction needed anymore.
        let net_worth = total_accounts + manual_assets_total;

        let mut new_milestones = Vec::new();
        for threshold in milestones {
            if net_worth < threshold {
                continue;
            }
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM net_worth_milestones WHERE threshold_cents = ?1",
                    rusqlite::params![threshold],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if exists == 0 {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO net_worth_milestones(threshold_cents, achieved_at) VALUES(?1, ?2)",
                    rusqlite::params![threshold, Utc::now().to_rfc3339()],
                );
                new_milestones.push(threshold);
            }
        }
        Ok(new_milestones)
    })
    .await
    .map_err(AppError::from)
}
