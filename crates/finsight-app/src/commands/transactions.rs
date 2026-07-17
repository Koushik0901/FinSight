use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{NewTransaction, Rule, Transaction, TxnPatch};
use finsight_core::repos::transactions;
use tauri_plugin_dialog::DialogExt;

pub use finsight_api::commands::transactions::{
    CategoryDto, CategoryWithSpending, CounterpartyVerdict, ProposedRuleDto, RuleWithCategory,
    SpendingBreakdown, SplitInputDto, TransactionSplitDto, TransferVerdictResult, TxnFilterInput,
    UnresolvedCounterpartyDto, UpdateTxnResult,
};

#[tauri::command]
#[specta::specta]
pub async fn list_transactions(
    state: tauri::State<'_, AppState>,
    filter: TxnFilterInput,
) -> AppResult<Vec<Transaction>> {
    finsight_api::commands::transactions::list_transactions(&state.api, filter).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_transaction(
    state: tauri::State<'_, AppState>,
    input: NewTransaction,
) -> AppResult<Transaction> {
    finsight_api::commands::transactions::create_transaction(&state.api, input).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: TxnPatch,
) -> AppResult<UpdateTxnResult> {
    finsight_api::commands::transactions::update_transaction(&state.api, id, patch).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_transaction(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::transactions::delete_transaction(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_rule(
    state: tauri::State<'_, AppState>,
    pattern: String,
    category_id: String,
) -> AppResult<Rule> {
    finsight_api::commands::transactions::create_rule(&state.api, pattern, category_id).await
}

/// Attribute a single transaction to one household member, overriding its
/// account's ownership shares for that row's cashflow — for a personal purchase
/// on a joint account. `member_id` None clears the override (revert to account
/// shares). Only flows are affected; balances are per-account.
#[tauri::command]
#[specta::specta]
pub async fn set_transaction_owner(
    state: tauri::State<'_, AppState>,
    transaction_id: String,
    member_id: Option<String>,
) -> AppResult<()> {
    finsight_api::commands::transactions::set_transaction_owner(
        &state.api,
        transaction_id,
        member_id,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_categories(state: tauri::State<'_, AppState>) -> AppResult<Vec<CategoryDto>> {
    finsight_api::commands::transactions::list_categories(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_category_spending_type(
    state: tauri::State<'_, AppState>,
    id: String,
    spending_type: Option<String>,
) -> AppResult<()> {
    finsight_api::commands::transactions::set_category_spending_type(
        &state.api,
        id,
        spending_type,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn get_spending_breakdown(
    state: tauri::State<'_, AppState>,
) -> AppResult<SpendingBreakdown> {
    finsight_api::commands::transactions::get_spending_breakdown(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_categories_with_spending(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryWithSpending>> {
    finsight_api::commands::transactions::list_categories_with_spending(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_rules_with_categories(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<RuleWithCategory>> {
    finsight_api::commands::transactions::list_rules_with_categories(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_rule(
    state: tauri::State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<()> {
    finsight_api::commands::transactions::toggle_rule(&state.api, id, enabled).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_transaction_count(state: tauri::State<'_, AppState>) -> AppResult<i64> {
    finsight_api::commands::transactions::get_transaction_count(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_transaction_flags(
    state: tauri::State<'_, AppState>,
    id: String,
    is_reimbursable: bool,
    is_split: bool,
) -> AppResult<Transaction> {
    finsight_api::commands::transactions::set_transaction_flags(
        &state.api,
        id,
        is_reimbursable,
        is_split,
    )
    .await
}

/// Record the user's verdict on whether a transaction is a transfer between
/// their own accounts. Sticky: survives re-imports and categorizer re-runs.
#[tauri::command]
#[specta::specta]
pub async fn set_transaction_transfer(
    state: tauri::State<'_, AppState>,
    id: String,
    is_transfer: bool,
) -> AppResult<TransferVerdictResult> {
    finsight_api::commands::transactions::set_transaction_transfer(&state.api, id, is_transfer)
        .await
}

/// Apply a transfer verdict to every undecided transaction matching the
/// counterparty pattern returned by `set_transaction_transfer`. One decision
/// clears a whole person's e-transfer history from the review list.
#[tauri::command]
#[specta::specta]
pub async fn apply_transfer_verdict_to_similar(
    state: tauri::State<'_, AppState>,
    pattern: String,
    is_transfer: bool,
) -> AppResult<u32> {
    finsight_api::commands::transactions::apply_transfer_verdict_to_similar(
        &state.api,
        pattern,
        is_transfer,
    )
    .await
}

/// Record the user's 3-way verdict (transfer / settle-up / real spending) on
/// a transfer-review counterparty transaction. Sticky: survives re-imports
/// and categorizer re-runs.
#[tauri::command]
#[specta::specta]
pub async fn set_counterparty_verdict(
    state: tauri::State<'_, AppState>,
    id: String,
    verdict: CounterpartyVerdict,
) -> AppResult<Transaction> {
    finsight_api::commands::transactions::set_counterparty_verdict(&state.api, id, verdict).await
}

/// Apply one counterparty verdict to every undecided transaction matching a
/// counterparty pattern (from [`UnresolvedCounterpartyDto::pattern`] or
/// `TransferVerdictResult::similar_pattern`). One decision clears a whole
/// person's e-transfer history from the review list.
#[tauri::command]
#[specta::specta]
pub async fn apply_counterparty_verdict_to_similar(
    state: tauri::State<'_, AppState>,
    pattern: String,
    verdict: CounterpartyVerdict,
) -> AppResult<u32> {
    finsight_api::commands::transactions::apply_counterparty_verdict_to_similar(
        &state.api, pattern, verdict,
    )
    .await
}

/// The undecided transfer-review queue, grouped by counterparty for a
/// bulk-decision surface.
#[tauri::command]
#[specta::specta]
pub async fn list_unresolved_counterparties(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<UnresolvedCounterpartyDto>> {
    finsight_api::commands::transactions::list_unresolved_counterparties(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_transaction_splits(
    state: tauri::State<'_, AppState>,
    transaction_id: String,
) -> AppResult<Vec<TransactionSplitDto>> {
    finsight_api::commands::transactions::get_transaction_splits(&state.api, transaction_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_transaction_splits(
    state: tauri::State<'_, AppState>,
    transaction_id: String,
    splits: Vec<SplitInputDto>,
) -> AppResult<()> {
    finsight_api::commands::transactions::set_transaction_splits(
        &state.api,
        transaction_id,
        splits,
    )
    .await
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
pub async fn export_transactions_csv(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    filter: TxnFilterInput,
) -> AppResult<String> {
    let maybe_path = app
        .dialog()
        .file()
        .set_file_name("transactions.csv")
        .blocking_save_file();

    let Some(file_path) = maybe_path else {
        return Ok(String::new());
    };
    let path = file_path
        .into_path()
        .map_err(|e| AppError::new("dialog", e.to_string()))?;

    let db = (*state.api.db).clone();
    let csv = finsight_core::repos::run(&db, move |conn| {
        let txns = transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: filter.account_id,
                limit: i64::MAX,
                offset: 0,
                search: filter.search,
                filter_preset: filter.filter_preset,
                start_date: filter.start_date,
                end_date: filter.end_date,
            },
        )?;
        let mut out = String::from("date,merchant,category,amount_dollars,notes\n");
        for t in txns {
            let date = t.posted_at.format("%Y-%m-%d").to_string();
            let merchant = csv_escape(&t.merchant_raw);
            let category = csv_escape(t.category_label.as_deref().unwrap_or(""));
            let amount = format!("{:.2}", t.amount_cents as f64 / 100.0);
            let notes = csv_escape(t.notes.as_deref().unwrap_or(""));
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

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchTxnQueryInput {
    pub merchant: Option<String>,
    pub account: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub min_amount_cents: Option<i64>,
    pub direction: Option<String>,
}

/// Re-run the Copilot `search_transactions` query and export the matching
/// rows as CSV via a native save dialog. Shares `transactions::search` with the
/// Copilot tool so the exported rows match exactly what the card displayed.
#[tauri::command]
#[specta::specta]
pub async fn export_search_transactions_csv(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
    query: SearchTxnQueryInput,
) -> AppResult<String> {
    let maybe_path = app
        .dialog()
        .file()
        .set_file_name("transactions.csv")
        .blocking_save_file();

    let Some(file_path) = maybe_path else {
        return Ok(String::new());
    };
    let path = file_path
        .into_path()
        .map_err(|e| AppError::new("dialog", e.to_string()))?;

    let db = (*state.api.db).clone();
    let csv = finsight_core::repos::run(&db, move |conn| {
        let rows = finsight_core::repos::transactions::search(
            conn,
            &finsight_core::repos::transactions::SearchTxnQuery {
                merchant: query.merchant,
                account: query.account,
                start_date: query.start_date,
                end_date: query.end_date,
                min_amount_cents: query.min_amount_cents,
                direction: query.direction,
            },
            i64::MAX,
        )?;
        let mut out = String::from("date,merchant,category,amount_dollars,account\n");
        for r in rows {
            let date = &r.date[..10.min(r.date.len())];
            let merchant = csv_escape(&r.merchant);
            let category = csv_escape(&r.category);
            let amount = format!("{:.2}", r.amount_cents as f64 / 100.0);
            let account = csv_escape(&r.account);
            out.push_str(&format!("{date},{merchant},{category},{amount},{account}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)?;

    let path_str = path.to_string_lossy().to_string();
    std::fs::write(&path, csv).map_err(|e| AppError::new("io", e.to_string()))?;
    Ok(path_str)
}
