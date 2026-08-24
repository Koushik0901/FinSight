use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{
    NewPlannedTransaction, PlannedTransaction, PlannedTransactionPatch, PlannedTxnFilter,
};
use finsight_core::repos::{planned_transactions, run};
use utoipa::ToSchema;

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ListPlannedTransactionsRequest {
    pub filter: PlannedTxnFilter,
}

#[utoipa::path(post, path = "/api/rpc/list_planned_transactions",
    request_body(content = ListPlannedTransactionsRequest), responses((status = 200, body = Vec<PlannedTransaction>)))]
pub async fn list_planned_transactions(
    state: &ApiState,
    filter: PlannedTxnFilter,
) -> AppResult<Vec<PlannedTransaction>> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::list(conn, filter))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct GetPlannedTransactionRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/get_planned_transaction",
    request_body(content = GetPlannedTransactionRequest), responses((status = 200, body = Option<PlannedTransaction>)))]
pub async fn get_planned_transaction(
    state: &ApiState,
    id: String,
) -> AppResult<Option<PlannedTransaction>> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::get(conn, &id))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CreatePlannedTransactionRequest {
    pub input: NewPlannedTransaction,
}

#[utoipa::path(post, path = "/api/rpc/create_planned_transaction",
    request_body(content = CreatePlannedTransactionRequest), responses((status = 200, body = PlannedTransaction)))]
pub async fn create_planned_transaction(
    state: &ApiState,
    input: NewPlannedTransaction,
) -> AppResult<PlannedTransaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::insert(conn, input))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct UpdatePlannedTransactionRequest {
    pub id: String,
    pub patch: PlannedTransactionPatch,
}

#[utoipa::path(post, path = "/api/rpc/update_planned_transaction", request_body(content = UpdatePlannedTransactionRequest), responses((status = 200, body = PlannedTransaction)))]
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

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct DeletePlannedTransactionRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/delete_planned_transaction",
    request_body(content = DeletePlannedTransactionRequest), responses((status = 200, description = "Success")))]
pub async fn delete_planned_transaction(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| planned_transactions::delete(conn, &id))
        .await
        .map_err(AppError::from)
}