//! Codegen wrappers for the guided month-end close (#59).

use crate::error::AppResult;
use crate::AppState;

pub use finsight_api::commands::month_close::{
    CloseFlag, DriftLine, MonthCloseListItem, MonthCloseSnapshot, MonthCloseView, SaveMonthCloseInput,
};

#[tauri::command]
#[specta::specta]
pub async fn get_month_close(
    state: tauri::State<'_, AppState>,
    year: i32,
    month: i32,
) -> AppResult<MonthCloseView> {
    finsight_api::commands::month_close::get_month_close(&state.api, year, month).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_month_close(
    state: tauri::State<'_, AppState>,
    input: SaveMonthCloseInput,
) -> AppResult<MonthCloseView> {
    finsight_api::commands::month_close::save_month_close(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_month_closes(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<MonthCloseListItem>> {
    finsight_api::commands::month_close::list_month_closes(&state.api).await
}
