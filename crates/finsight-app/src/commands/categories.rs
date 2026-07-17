use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{Category, CategoryGroup};

#[tauri::command]
#[specta::specta]
pub async fn update_category_color(
    state: tauri::State<'_, AppState>,
    id: String,
    color: String,
) -> AppResult<()> {
    finsight_api::commands::categories::update_category_color(&state.api, id, color).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_category(
    state: tauri::State<'_, AppState>,
    label: String,
    group_id: Option<String>,
    color: String,
) -> AppResult<Category> {
    finsight_api::commands::categories::create_category(&state.api, label, group_id, color).await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_category(
    state: tauri::State<'_, AppState>,
    id: String,
    label: String,
) -> AppResult<()> {
    finsight_api::commands::categories::rename_category(&state.api, id, label).await
}

#[tauri::command]
#[specta::specta]
pub async fn archive_category(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::categories::archive_category(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_category_guidance(
    state: tauri::State<'_, AppState>,
    id: String,
    guidance: Option<String>,
) -> AppResult<()> {
    finsight_api::commands::categories::set_category_guidance(&state.api, id, guidance).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_category_groups(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryGroup>> {
    finsight_api::commands::categories::list_category_groups(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_category_group(
    state: tauri::State<'_, AppState>,
    label: String,
    hint: Option<String>,
) -> AppResult<CategoryGroup> {
    finsight_api::commands::categories::create_category_group(&state.api, label, hint).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_category_group(
    state: tauri::State<'_, AppState>,
    category_id: String,
    group_id: String,
) -> AppResult<()> {
    finsight_api::commands::categories::set_category_group(&state.api, category_id, group_id)
        .await
}
