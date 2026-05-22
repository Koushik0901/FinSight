use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::Transaction;
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
