use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{AccountOwner, AssetOwner, HouseholdMember, OwnerShare};
use finsight_core::repos::{household, run};

#[utoipa::path(post, path = "/api/rpc/list_household_members", responses((status = 200, body = Vec<HouseholdMember>)))]
pub async fn list_household_members(state: &ApiState) -> AppResult<Vec<HouseholdMember>> {
    let db = (*state.db).clone();
    run(&db, household::list_members)
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CreateHouseholdMemberRequest {
    pub name: String,
    pub color: Option<String>,
}

#[utoipa::path(post, path = "/api/rpc/create_household_member", request_body(content = CreateHouseholdMemberRequest), responses((status = 200, body = HouseholdMember)))]
pub async fn create_household_member(
    state: &ApiState,
    name: String,
    color: Option<String>,
) -> AppResult<HouseholdMember> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::create_member(conn, &name, color.as_deref())
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetSelfMemberRequest {
    pub member_id: String,
}

#[utoipa::path(post, path = "/api/rpc/set_self_member",
    request_body(content = SetSelfMemberRequest), responses((status = 200, description = "Success")))]
pub async fn set_self_member(state: &ApiState, member_id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::set_self_member(conn, &member_id)?;
        finsight_core::categorize::apply_builtin_categorization(conn)?;
        finsight_core::categorize::pair_transfers(conn)?;
        finsight_core::anomaly::recompute_anomalies(conn)?;
        Ok::<_, finsight_core::CoreError>(())
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct DeleteHouseholdMemberRequest {
    pub id: String,
}

#[utoipa::path(post, path = "/api/rpc/delete_household_member",
    request_body(content = DeleteHouseholdMemberRequest), responses((status = 200, description = "Success")))]
pub async fn delete_household_member(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| household::delete_member(conn, &id))
        .await
        .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/list_account_owners", responses((status = 200, body = Vec<AccountOwner>)))]
pub async fn list_account_owners(state: &ApiState) -> AppResult<Vec<AccountOwner>> {
    let db = (*state.db).clone();
    run(&db, household::list_account_owners)
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetAccountOwnersRequest {
    pub account_id: String,
    pub member_ids: Vec<String>,
}

#[utoipa::path(post, path = "/api/rpc/set_account_owners", request_body(content = SetAccountOwnersRequest), responses((status = 200, description = "Success")))]
pub async fn set_account_owners(
    state: &ApiState,
    account_id: String,
    member_ids: Vec<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::set_account_owners(conn, &account_id, &member_ids)
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetAccountOwnerSharesRequest {
    pub account_id: String,
    pub owners: Vec<OwnerShare>,
}

#[utoipa::path(post, path = "/api/rpc/set_account_owner_shares", request_body(content = SetAccountOwnerSharesRequest), responses((status = 200, description = "Success")))]
pub async fn set_account_owner_shares(
    state: &ApiState,
    account_id: String,
    owners: Vec<OwnerShare>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::set_account_owner_shares(conn, &account_id, &owners)
    })
    .await
    .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/list_asset_owners", responses((status = 200, body = Vec<AssetOwner>)))]
pub async fn list_asset_owners(state: &ApiState) -> AppResult<Vec<AssetOwner>> {
    let db = (*state.db).clone();
    run(&db, household::list_asset_owners)
        .await
        .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetAssetOwnersRequest {
    pub asset_id: String,
    pub owners: Vec<OwnerShare>,
}

#[utoipa::path(post, path = "/api/rpc/set_asset_owners", request_body(content = SetAssetOwnersRequest), responses((status = 200, description = "Success")))]
pub async fn set_asset_owners(
    state: &ApiState,
    asset_id: String,
    owners: Vec<OwnerShare>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        household::set_asset_owners(conn, &asset_id, &owners)
    })
    .await
    .map_err(AppError::from)
}