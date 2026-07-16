use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{
    Account, AccountBalancePoint, AccountPatch, AccountSparkline, AccountSummary, NewAccount,
};
use finsight_core::repos::run;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
#[specta::specta]
pub async fn list_accounts(state: tauri::State<'_, AppState>) -> AppResult<Vec<AccountSummary>> {
    finsight_api::commands::accounts::list_accounts(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_account(
    state: tauri::State<'_, AppState>,
    input: NewAccount,
) -> AppResult<Account> {
    finsight_api::commands::accounts::create_account(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_account(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: AccountPatch,
) -> AppResult<Account> {
    finsight_api::commands::accounts::update_account(&state.api, id, patch).await
}

#[tauri::command]
#[specta::specta]
pub async fn archive_account(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::accounts::archive_account(&state.api, id).await
}

/// User-confirmed "this is my real balance right now" entry point — e.g. after
/// importing CSV history that carries no balance field. Back-solves the account
/// opening so the balance model reproduces the entered value AND keeps tracking
/// as transactions change, instead of freezing a fixed snapshot that goes stale
/// (see [`accounts::set_current_balance`]).
#[tauri::command]
#[specta::specta]
pub async fn set_account_balance(
    state: tauri::State<'_, AppState>,
    id: String,
    balance_cents: i64,
) -> AppResult<()> {
    finsight_api::commands::accounts::set_account_balance(&state.api, id, balance_cents).await
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[tauri::command]
#[specta::specta]
pub async fn export_account_csv(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    account_id: String,
) -> AppResult<String> {
    // Get account name for filename
    let db = (*state.api.db).clone();
    let account_name = {
        let db2 = db.clone();
        let aid = account_id.clone();
        run(&db2, move |conn| {
            conn.query_row(
                "SELECT COALESCE(name, 'account') FROM accounts WHERE id = ?1",
                rusqlite::params![aid],
                |r| r.get::<_, String>(0),
            )
            .map_err(finsight_core::CoreError::from)
        })
        .await
        .unwrap_or_else(|_| "account".to_string())
    };

    let safe_name: String = account_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let maybe_path = app
        .dialog()
        .file()
        .set_file_name(format!("{safe_name}-transactions.csv"))
        .blocking_save_file();

    let Some(file_path) = maybe_path else {
        return Ok(String::new());
    };
    let path = file_path
        .into_path()
        .map_err(|e| AppError::new("dialog", e.to_string()))?;

    let csv = run(&db, move |conn| {
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
            let date = &posted_at[..10]; // "YYYY-MM-DD"
            let merchant = csv_escape(&merchant);
            let category = csv_escape(&category);
            let amount = format!("{:.2}", amount_cents as f64 / 100.0);
            let notes = csv_escape(&notes);
            out.push_str(&format!("{date},{merchant},{category},{amount},{notes}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)?;

    let path_str = path.to_string_lossy().to_string();
    std::fs::write(&path, csv).map_err(|e| AppError::new("io", e.to_string()))?;
    Ok(path_str)
}

#[tauri::command]
#[specta::specta]
pub async fn list_account_balance_history(
    state: tauri::State<'_, AppState>,
    account_id: String,
    days: u32,
) -> AppResult<Vec<AccountBalancePoint>> {
    finsight_api::commands::accounts::list_account_balance_history(&state.api, account_id, days)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_account_balance_sparklines(
    state: tauri::State<'_, AppState>,
    days: u32,
) -> AppResult<Vec<AccountSparkline>> {
    finsight_api::commands::accounts::list_account_balance_sparklines(&state.api, days).await
}
