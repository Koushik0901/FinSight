//! Frontend bridge to `finsight-core::investments` — ledger-derived positions
//! and the portfolio estimate for investment accounts. Read-only: setting the
//! account balance from the estimate goes through the existing
//! `set_account_balance` command so the write stays an explicit user action.

use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::investments::{self, InvestmentSummary, Position};
use finsight_core::repos::run;

#[tauri::command]
#[specta::specta]
pub async fn list_account_positions(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<Vec<Position>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        investments::positions_for_account(conn, &account_id)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_investment_summary(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<InvestmentSummary> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        investments::summary_for_account(conn, &account_id)
    })
    .await
    .map_err(AppError::from)
}
