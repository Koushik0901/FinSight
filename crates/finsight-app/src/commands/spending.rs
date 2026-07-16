use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::spending::PathBackView;

#[tauri::command]
#[specta::specta]
pub async fn get_spending_path_back(
    state: tauri::State<'_, AppState>,
    period: Option<String>,
    target_monthly_cents: Option<i64>,
) -> AppResult<Option<PathBackView>> {
    finsight_api::commands::spending::get_spending_path_back(
        &state.api,
        period,
        target_monthly_cents,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn set_spending_annotation(
    state: tauri::State<'_, AppState>,
    merchant_key: String,
    verdict: String,
) -> AppResult<()> {
    finsight_api::commands::spending::set_spending_annotation(&state.api, merchant_key, verdict)
        .await
}
