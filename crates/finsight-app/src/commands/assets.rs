use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{
    Liability, LiabilityPatch, ManualAsset, ManualAssetPatch, NetWorthPoint, NewLiability,
    NewManualAsset,
};
use finsight_core::repos::{liabilities, manual_assets, net_worth, run};

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
pub async fn list_liabilities(state: tauri::State<'_, AppState>) -> AppResult<Vec<Liability>> {
    let db = (*state.db).clone();
    run(&db, liabilities::list).await.map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_liability(
    state: tauri::State<'_, AppState>,
    input: NewLiability,
) -> AppResult<Liability> {
    let db = (*state.db).clone();
    run(&db, move |conn| liabilities::create(conn, input))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn update_liability(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: LiabilityPatch,
) -> AppResult<Liability> {
    let db = (*state.db).clone();
    run(&db, move |conn| liabilities::update(conn, &id, patch))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_liability(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| liabilities::delete(conn, &id))
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
