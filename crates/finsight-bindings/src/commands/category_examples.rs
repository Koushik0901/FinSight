use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::CategoryExample;

#[tauri::command]
#[specta::specta]
pub async fn add_category_example(
    state: tauri::State<'_, AppState>,
    category_id: String,
    example_text: String,
    source_txn_id: Option<String>,
) -> AppResult<CategoryExample> {
    finsight_api::commands::category_examples::add_category_example(
        &state.api,
        category_id,
        example_text,
        source_txn_id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_category_example(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    finsight_api::commands::category_examples::remove_category_example(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_category_examples(
    state: tauri::State<'_, AppState>,
    category_id: String,
) -> AppResult<Vec<CategoryExample>> {
    finsight_api::commands::category_examples::list_category_examples(&state.api, category_id).await
}
