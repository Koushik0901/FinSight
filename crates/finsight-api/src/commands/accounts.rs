use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{
    Account, AccountBalancePoint, AccountBalanceTimeline, AccountPatch, AccountSparkline,
    AccountSummary, NewAccount,
};
use finsight_core::repos::{accounts, run};
use utoipa::ToSchema;

#[utoipa::path(post, path = "/api/rpc/list_accounts", responses((status = 200, body = Vec<AccountSummary>)))]
pub async fn list_accounts(state: &ApiState) -> AppResult<Vec<AccountSummary>> {
    // `state.db` is `Arc<Db>`; deref + clone gives us an owned `Db` (cheap — it's
    // an Arc-wrapped pool internally) that we can move into the blocking closure.
    let db = (*state.db).clone();
    let result = run(&db, accounts::list_summaries)
        .await
        .map_err(AppError::from)?;
    Ok(result)
}

#[utoipa::path(post, path = "/api/rpc/create_account",
    request_body(content = NewAccount), responses((status = 200, body = Account)))]
pub async fn create_account(state: &ApiState, mut input: NewAccount) -> AppResult<Account> {
    // Always force source to "manual" — the frontend cannot create sample accounts.
    // Without this, a caller could mislabel user-created accounts as imported data.
    input.source = "manual".to_string();
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::insert(conn, input))
        .await
        .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/update_account", responses((status = 200, body = Account)))]
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

#[utoipa::path(post, path = "/api/rpc/archive_account",
    request_body(content = String), responses((status = 200, description = "Success")))]
pub async fn archive_account(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| accounts::archive(conn, &id))
        .await
        .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/set_account_balance", responses((status = 200, description = "Success")))]
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

#[utoipa::path(post, path = "/api/rpc/list_account_balance_history", responses((status = 200, body = Vec<AccountBalancePoint>)))]
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

/// Reconstruct an account's balance curve from its ledger, with the peak and
/// trough over the requested window. `since` is an ISO `YYYY-MM-DD` date; omit
/// it for all-time.
///
/// Unlike [`list_account_balance_history`], which reads the sparse stored
/// snapshots, this derives every point — so it can answer "when was this account
/// at its highest" rather than "which recorded day was highest".
#[utoipa::path(post, path = "/api/rpc/get_account_balance_timeline", responses((status = 200, body = AccountBalanceTimeline)))]
pub async fn get_account_balance_timeline(
    state: &ApiState,
    account_id: String,
    since: Option<String>,
) -> AppResult<AccountBalanceTimeline> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        accounts::balance_timeline(conn, &account_id, since.as_deref())
    })
    .await
    .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/list_account_balance_sparklines",
    request_body(content = u32), responses((status = 200, body = Vec<AccountSparkline>)))]
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

/// Returns the CSV content for one account's transactions (caller downloads
/// it client-side — no server-side file I/O). Real implementation as of
/// Phase 4; previously 501'd behind a native-dialog-only Tauri command.
#[utoipa::path(post, path = "/api/rpc/export_account_csv",
    request_body(content = String), responses((status = 200, body = String)))]
pub async fn export_account_csv(state: &ApiState, account_id: String) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let mut stmt = conn.prepare(
            "SELECT t.posted_at, t.merchant_raw, COALESCE(c.label,''), t.amount_cents, COALESCE(t.notes,'')
             FROM transactions t
             LEFT JOIN categories c ON c.id = t.category_id
             WHERE t.account_id = ?1
             ORDER BY t.posted_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![account_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = String::from("date,merchant,category,amount_dollars,notes\n");
        for row in rows {
            let (posted_at, merchant, category, amount_cents, notes) = row?;
            let date = &posted_at[..10.min(posted_at.len())];
            let merchant = crate::csv::csv_escape(&merchant);
            let category = crate::csv::csv_escape(&category);
            let amount = format!("{:.2}", amount_cents as f64 / 100.0);
            let notes = crate::csv::csv_escape(&notes);
            out.push_str(&format!("{date},{merchant},{category},{amount},{notes}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}