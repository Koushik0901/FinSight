use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{
    NewPlannedTransaction, PlannedTransaction, PlannedTransactionPatch, PlannedTxnFilter,
};
use finsight_core::repos::{planned_transactions, run};

pub async fn list_planned_transactions(
    state: &ApiState,
    filter: PlannedTxnFilter,
) -> AppResult<Vec<PlannedTransaction>> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::list(conn, filter))
        .await
        .map_err(AppError::from)
}

pub async fn get_planned_transaction(
    state: &ApiState,
    id: String,
) -> AppResult<Option<PlannedTransaction>> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::get(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn create_planned_transaction(
    state: &ApiState,
    input: NewPlannedTransaction,
) -> AppResult<PlannedTransaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::insert(conn, input))
        .await
        .map_err(AppError::from)
}

pub async fn update_planned_transaction(
    state: &ApiState,
    id: String,
    patch: PlannedTransactionPatch,
) -> AppResult<PlannedTransaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        planned_transactions::update(conn, &id, patch)
    })
    .await
    .map_err(AppError::from)
}

pub async fn delete_planned_transaction(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::delete(conn, &id))
        .await
        .map_err(AppError::from)
}
