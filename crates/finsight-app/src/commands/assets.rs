use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{ManualAsset, ManualAssetPatch, NetWorthPoint, NewManualAsset};

pub use finsight_api::commands::assets::{
    DebtPayoffMonth, DebtPayoffResult, DebtPayoffSummary,
};

#[tauri::command]
#[specta::specta]
pub async fn list_manual_assets(state: tauri::State<'_, AppState>) -> AppResult<Vec<ManualAsset>> {
    finsight_api::commands::assets::list_manual_assets(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_manual_asset(
    state: tauri::State<'_, AppState>,
    input: NewManualAsset,
) -> AppResult<ManualAsset> {
    finsight_api::commands::assets::create_manual_asset(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_manual_asset(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: ManualAssetPatch,
) -> AppResult<ManualAsset> {
    finsight_api::commands::assets::update_manual_asset(&state.api, id, patch).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_manual_asset(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::assets::delete_manual_asset(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn record_net_worth_snapshot(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::assets::record_net_worth_snapshot(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_net_worth_history(
    state: tauri::State<'_, AppState>,
    days: u32,
) -> AppResult<Vec<NetWorthPoint>> {
    finsight_api::commands::assets::list_net_worth_history(&state.api, days).await
}

#[tauri::command]
#[specta::specta]
pub async fn compute_debt_payoff(
    state: tauri::State<'_, AppState>,
    extra_monthly_cents: i64,
) -> AppResult<Vec<DebtPayoffResult>> {
    finsight_api::commands::assets::compute_debt_payoff(&state.api, extra_monthly_cents).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_uncelebrated_milestones(state: tauri::State<'_, AppState>) -> AppResult<Vec<i64>> {
    finsight_api::commands::assets::get_uncelebrated_milestones(&state.api).await
}
