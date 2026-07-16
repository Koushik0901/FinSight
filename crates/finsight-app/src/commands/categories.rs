use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::{Category, CategoryGroup};
use finsight_core::repos::{categories, run};

#[tauri::command]
#[specta::specta]
pub async fn update_category_color(
    state: tauri::State<'_, AppState>,
    id: String,
    color: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| categories::update_color(conn, &id, &color))
        .await
        .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_category(
    state: tauri::State<'_, AppState>,
    label: String,
    group_id: Option<String>,
    color: String,
) -> AppResult<Category> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::create(conn, &label, group_id.as_deref(), &color)
    })
    .await
    .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn rename_category(
    state: tauri::State<'_, AppState>,
    id: String,
    label: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| categories::rename(conn, &id, &label))
        .await
        .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn archive_category(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| categories::archive(conn, &id))
        .await
        .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_category_guidance(
    state: tauri::State<'_, AppState>,
    id: String,
    guidance: Option<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::set_guidance(conn, &id, guidance.as_deref())
    })
    .await
    .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_category_groups(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryGroup>> {
    let db = (*state.db).clone();
    run(&db, |conn| categories::list_groups(conn))
        .await
        .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_category_group(
    state: tauri::State<'_, AppState>,
    label: String,
    hint: Option<String>,
) -> AppResult<CategoryGroup> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::create_group(conn, &label, hint.as_deref())
    })
    .await
    .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_category_group(
    state: tauri::State<'_, AppState>,
    category_id: String,
    group_id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::set_group(conn, &category_id, &group_id)
    })
    .await
    .map_err(crate::error::AppError::from)
}
