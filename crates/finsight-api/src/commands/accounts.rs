use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{
    Account, AccountBalancePoint, AccountPatch, AccountSparkline, AccountSummary, NewAccount,
};
use finsight_core::repos::{accounts, run};

pub async fn list_accounts(state: &ApiState) -> AppResult<Vec<AccountSummary>> {
    // `state.db` is `Arc<Db>`; deref + clone gives us an owned `Db` (cheap — it's
    // an Arc-wrapped pool internally) that we can move into the blocking closure.
    let db = (*state.db).clone();
    let result = run(&db, accounts::list_summaries)
        .await
        .map_err(AppError::from)?;
    Ok(result)
}

pub async fn create_account(state: &ApiState, mut input: NewAccount) -> AppResult<Account> {
    // Always force source to "manual" — the frontend cannot create sample accounts.
    // Without this, a caller could mislabel user-created accounts as imported data.
    input.source = "manual".to_string();
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::insert(conn, input))
        .await
        .map_err(AppError::from)
}

pub async fn update_account(
    state: &ApiState,
    id: String,
    patch: AccountPatch,
) -> AppResult<Account> {
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::update(conn, &id, patch))
        .await
        .map_err(AppError::from)
}

pub async fn archive_account(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::archive(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn set_account_balance(
    state: &ApiState,
    id: String,
    balance_cents: i64,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        accounts::set_current_balance(conn, &id, balance_cents)
    })
    .await
    .map_err(AppError::from)
}

pub async fn list_account_balance_history(
    state: &ApiState,
    account_id: String,
    days: u32,
) -> AppResult<Vec<AccountBalancePoint>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        accounts::list_balance_history(conn, &account_id, days)
    })
    .await
    .map_err(AppError::from)
}

pub async fn list_account_balance_sparklines(
    state: &ApiState,
    days: u32,
) -> AppResult<Vec<AccountSparkline>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        accounts::list_all_balance_sparklines(conn, days)
    })
    .await
    .map_err(AppError::from)
}
