use crate::error::AppResult;
use crate::AppState;
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
