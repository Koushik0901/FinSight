//! Per-category exemplar CRUD (issue #91, Slice 4a).
//!
//! Storage + CRUD only. Nothing reads these yet — issue #92 embeds the example
//! text into a prototype/centroid vector per category. The categorizer prompt
//! is deliberately unchanged by this slice.

use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::CategoryExample;
use finsight_core::repos::{category_examples, run};
use utoipa::ToSchema;

/// Attach an exemplar description to a category, keyed by the category's
/// stable id (so it rides through renames). Idempotent per (category, text).
///
/// `source_txn_id` is an optional provenance breadcrumb for an "add this
/// transaction as an example" affordance; the example survives that
/// transaction being deleted.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct AddCategoryExampleRequest {
    pub category_id: String,
    pub example_text: String,
    pub source_txn_id: Option<String>,
}

#[utoipa::path(post, path = "/api/rpc/add_category_example", request_body(content = AddCategoryExampleRequest), responses((status = 200, body = CategoryExample)))]
pub async fn add_category_example(
    state: &ApiState,
    category_id: String,
    example_text: String,
    source_txn_id: Option<String>,
) -> AppResult<CategoryExample> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        category_examples::add(conn, &category_id, &example_text, source_txn_id.as_deref())
    })
    .await
    .map_err(AppError::from)
}

/// Remove one exemplar by its own id. No-op if it's already gone.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct RemoveCategoryExampleRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/remove_category_example",
    request_body(content = RemoveCategoryExampleRequest), responses((status = 200, description = "Success")))]
pub async fn remove_category_example(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| category_examples::remove(conn, &id))
        .await
        .map_err(AppError::from)
}

/// Every exemplar for a category, oldest first.
///
/// Returns rows for ARCHIVED categories too, mirroring how `list_categories`
/// still returns `guidance` on an archived row — archiving hides examples from
/// active consumers, it does not delete them.
#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ListCategoryExamplesRequest {
    pub category_id: String,
}

#[utoipa::path(post, path = "/api/rpc/list_category_examples",
    request_body(content = ListCategoryExamplesRequest), responses((status = 200, body = Vec<CategoryExample>)))]
pub async fn list_category_examples(
    state: &ApiState,
    category_id: String,
) -> AppResult<Vec<CategoryExample>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        category_examples::list_for_category(conn, &category_id)
    })
    .await
    .map_err(AppError::from)
}