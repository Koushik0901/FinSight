use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{Category, CategoryGroup, Rule, NewTransaction, Transaction};
use finsight_core::repos::{run, transactions};
use serde::Deserialize;
use specta::Type;

#[derive(Debug, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct TxnFilterInput {
    pub account_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_transactions(
    state: tauri::State<'_, AppState>,
    filter: TxnFilterInput,
) -> AppResult<Vec<Transaction>> {
    let db = (*state.db).clone();
    let result = run(&db, move |conn| {
        transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: filter.account_id,
                limit: filter.limit.unwrap_or(100),
                offset: filter.offset.unwrap_or(0),
            },
        )
    })
    .await
    .map_err(AppError::from)?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn create_transaction(
    state: tauri::State<'_, AppState>,
    input: NewTransaction,
) -> AppResult<Transaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| transactions::insert(conn, input))
        .await
        .map_err(AppError::from)
}

// Stubs — implemented in a later task
#[tauri::command]
#[specta::specta]
pub async fn update_transaction(
    _state: tauri::State<'_, AppState>,
    _id: String,
    _input: serde_json::Value,
) -> AppResult<Transaction> {
    Err(crate::error::AppError::new("not_implemented", "update_transaction not yet implemented"))
}

#[tauri::command]
#[specta::specta]
pub async fn delete_transaction(
    _state: tauri::State<'_, AppState>,
    _id: String,
) -> AppResult<()> {
    Err(crate::error::AppError::new("not_implemented", "delete_transaction not yet implemented"))
}

#[tauri::command]
#[specta::specta]
pub async fn create_rule(
    _state: tauri::State<'_, AppState>,
    _input: serde_json::Value,
) -> AppResult<Rule> {
    Err(crate::error::AppError::new("not_implemented", "create_rule not yet implemented"))
}

#[derive(Debug, serde::Serialize, Type)]
pub struct CategoriesResponse {
    pub groups: Vec<CategoryGroup>,
    pub categories: Vec<Category>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_categories(
    _state: tauri::State<'_, AppState>,
) -> AppResult<CategoriesResponse> {
    Err(crate::error::AppError::new("not_implemented", "list_categories not yet implemented"))
}
