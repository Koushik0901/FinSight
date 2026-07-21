use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{
    NewPlannedTransaction, PlannedTransaction, PlannedTransactionPatch, PlannedTxnFilter,
};

#[tauri::command]
#[specta::specta]
pub async fn list_planned_transactions(
    state: tauri::State<'_, AppState>,
    filter: PlannedTxnFilter,
) -> AppResult<Vec<PlannedTransaction>> {
    finsight_api::commands::planned_transactions::list_planned_transactions(&state.api, filter)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_planned_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<Option<PlannedTransaction>> {
    finsight_api::commands::planned_transactions::get_planned_transaction(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_planned_transaction(
    state: tauri::State<'_, AppState>,
    input: NewPlannedTransaction,
) -> AppResult<PlannedTransaction> {
    finsight_api::commands::planned_transactions::create_planned_transaction(&state.api, input)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn update_planned_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: PlannedTransactionPatch,
) -> AppResult<PlannedTransaction> {
    finsight_api::commands::planned_transactions::update_planned_transaction(
        &state.api, id, patch,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_planned_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    finsight_api::commands::planned_transactions::delete_planned_transaction(&state.api, id).await
}
