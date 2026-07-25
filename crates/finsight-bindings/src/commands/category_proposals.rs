use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::CategoryProposal;

pub use finsight_api::commands::transactions::UpdateTxnResult;

#[tauri::command]
#[specta::specta]
pub async fn list_category_proposals(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryProposal>> {
    finsight_api::commands::category_proposals::list_category_proposals(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn accept_category_proposal(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<UpdateTxnResult> {
    finsight_api::commands::category_proposals::accept_category_proposal(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn correct_category_proposal(
    state: tauri::State<'_, AppState>,
    id: String,
    category_id: String,
) -> AppResult<UpdateTxnResult> {
    finsight_api::commands::category_proposals::correct_category_proposal(
        &state.api,
        id,
        category_id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn reject_category_proposal(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    finsight_api::commands::category_proposals::reject_category_proposal(&state.api, id).await
}
