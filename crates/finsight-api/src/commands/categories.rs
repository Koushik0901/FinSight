use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{Category, CategoryGroup};
use finsight_core::repos::{categories, run};

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct UpdateCategoryColorRequest {
    pub id: String,
    pub color: String,
}

#[utoipa::path(post, path = "/api/rpc/update_category_color", request_body(content = UpdateCategoryColorRequest), responses((status = 200, description = "Success")))]
pub async fn update_category_color(state: &ApiState, id: String, color: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| categories::update_color(conn, &id, &color))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CreateCategoryRequest {
    pub label: String,
    pub group_id: Option<String>,
    pub color: String,
}

#[utoipa::path(post, path = "/api/rpc/create_category", request_body(content = CreateCategoryRequest), responses((status = 200, body = Category)))]
pub async fn create_category(
    state: &ApiState,
    label: String,
    group_id: Option<String>,
    color: String,
) -> AppResult<Category> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::create(conn, &label, group_id.as_deref(), &color)
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct RenameCategoryRequest {
    pub id: String,
    pub label: String,
}

#[utoipa::path(post, path = "/api/rpc/rename_category", request_body(content = RenameCategoryRequest), responses((status = 200, description = "Success")))]
pub async fn rename_category(state: &ApiState, id: String, label: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| categories::rename(conn, &id, &label))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ArchiveCategoryRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/archive_category",
    request_body(content = ArchiveCategoryRequest), responses((status = 200, description = "Success")))]
pub async fn archive_category(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| categories::archive(conn, &id))
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetCategoryGuidanceRequest {
    pub id: String,
    pub guidance: Option<String>,
}

#[utoipa::path(post, path = "/api/rpc/set_category_guidance", request_body(content = SetCategoryGuidanceRequest), responses((status = 200, description = "Success")))]
pub async fn set_category_guidance(
    state: &ApiState,
    id: String,
    guidance: Option<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::set_guidance(conn, &id, guidance.as_deref())
    })
    .await
    .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/list_category_groups", responses((status = 200, body = Vec<CategoryGroup>)))]
pub async fn list_category_groups(state: &ApiState) -> AppResult<Vec<CategoryGroup>> {
    let db = (*state.db).clone();
    run(&db, categories::list_groups)
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CreateCategoryGroupRequest {
    pub label: String,
    pub hint: Option<String>,
}

#[utoipa::path(post, path = "/api/rpc/create_category_group", request_body(content = CreateCategoryGroupRequest), responses((status = 200, body = CategoryGroup)))]
pub async fn create_category_group(
    state: &ApiState,
    label: String,
    hint: Option<String>,
) -> AppResult<CategoryGroup> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::create_group(conn, &label, hint.as_deref())
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetCategoryGroupRequest {
    pub category_id: String,
    pub group_id: String,
}

#[utoipa::path(post, path = "/api/rpc/set_category_group", request_body(content = SetCategoryGroupRequest), responses((status = 200, description = "Success")))]
pub async fn set_category_group(
    state: &ApiState,
    category_id: String,
    group_id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::set_group(conn, &category_id, &group_id)
    })
    .await
    .map_err(AppError::from)
}
