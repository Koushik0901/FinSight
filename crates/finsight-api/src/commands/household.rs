use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{AccountOwner, AssetOwner, HouseholdMember, OwnerShare};
use finsight_core::repos::{household, run};

pub async fn list_household_members(state: &ApiState) -> AppResult<Vec<HouseholdMember>> {
    let db = (*state.db).clone();
    run(&db, household::list_members)
        .await
        .map_err(AppError::from)
}

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

pub async fn delete_household_member(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| household::delete_member(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn list_account_owners(state: &ApiState) -> AppResult<Vec<AccountOwner>> {
    let db = (*state.db).clone();
    run(&db, household::list_account_owners)
        .await
        .map_err(AppError::from)
}

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

pub async fn list_asset_owners(state: &ApiState) -> AppResult<Vec<AssetOwner>> {
    let db = (*state.db).clone();
    run(&db, household::list_asset_owners)
        .await
        .map_err(AppError::from)
}

pub async fn set_asset_owners(
    state: &ApiState,
    asset_id: String,
    owners: Vec<OwnerShare>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| household::set_asset_owners(conn, &asset_id, &owners))
        .await
        .map_err(AppError::from)
}
