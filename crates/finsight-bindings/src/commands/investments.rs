//! Frontend bridge to `finsight-core::investments` — ledger-derived positions
//! and the portfolio estimate for investment accounts. Read-only: setting the
//! account balance from the estimate goes through the existing
//! `set_account_balance` command so the write stays an explicit user action.

use crate::error::AppResult;
use crate::AppState;
use finsight_core::investments::{InvestmentSummary, Position};

#[tauri::command]
#[specta::specta]
pub async fn list_account_positions(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<Vec<Position>> {
    finsight_api::commands::investments::list_account_positions(&state.api, account_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_investment_summary(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<InvestmentSummary> {
    finsight_api::commands::investments::get_investment_summary(&state.api, account_id).await
}
