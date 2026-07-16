use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::run;
use tauri_plugin_dialog::DialogExt;

/// Re-exported so `crate::commands::settings::AUTO_CATEGORIZE_ENABLED_KEY`
/// (used by `lib.rs`'s startup resume-categorization check) keeps resolving
/// now that the constant + its owning commands live in finsight-api.
pub use finsight_api::commands::settings::AUTO_CATEGORIZE_ENABLED_KEY;

#[tauri::command]
#[specta::specta]
pub async fn get_currency(state: tauri::State<'_, AppState>) -> AppResult<String> {
    finsight_api::commands::settings::get_currency(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_currency(state: tauri::State<'_, AppState>, currency: String) -> AppResult<()> {
    finsight_api::commands::settings::set_currency(&state.api, currency).await
}

/// Factory-reset: wipes every local financial/user-data table (accounts,
/// transactions, budgets, goals, categories, reports/insight caches,
/// scenarios, recipes, agent memory/context, review queues, etc.) while
/// preserving `settings` (provider selection, currency, toggles) and the OS
/// keychain (API keys, DB encryption key) untouched. The frontend is
/// responsible for the double-confirmation UX before calling this.
#[tauri::command]
#[specta::specta]
pub async fn delete_all_data(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::settings::delete_all_data(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_all_data_json(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let maybe_path = app
        .dialog()
        .file()
        .set_file_name("finsight-export.json")
        .blocking_save_file();

    let Some(file_path) = maybe_path else {
        return Ok(());
    };
    let path = file_path
        .into_path()
        .map_err(|e| AppError::new("dialog", e.to_string()))?;

    let db = (*state.api.db).clone();
    let json = run(&db, move |conn| {
        use chrono::Utc;
        use finsight_core::repos::{accounts, goals, rules, transactions};

        let accs = accounts::list_summaries(conn)?;
        let txns = transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: None,
                limit: i64::MAX,
                offset: 0,
                search: None,
                filter_preset: None,
                start_date: None,
                end_date: None,
            },
        )?;
        let gs: Vec<serde_json::Value> = goals::list(conn)?
            .into_iter()
            .map(|g| {
                serde_json::json!({
                    "id": g.id,
                    "name": g.name,
                    "goalType": g.goal_type,
                    "targetCents": g.target_cents,
                    "currentCents": g.current_cents,
                    "monthlyCents": g.monthly_cents,
                    "targetDate": g.target_date,
                    "color": g.color,
                    "notes": g.notes,
                    "sortOrder": g.sort_order,
                    "createdAt": g.created_at,
                })
            })
            .collect();
        let rs = rules::list_active(conn)?;

        let out = serde_json::json!({
            "exportedAt": Utc::now().to_rfc3339(),
            "accounts": accs,
            "transactions": txns,
            "goals": gs,
            "rules": rs,
        });
        serde_json::to_string_pretty(&out)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)?;

    std::fs::write(&path, json).map_err(|e| AppError::new("io", e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn get_notifications_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    finsight_api::commands::settings::get_notifications_enabled(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_notifications_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    finsight_api::commands::settings::set_notifications_enabled(&state.api, enabled).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_auto_categorize_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    finsight_api::commands::settings::get_auto_categorize_enabled(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_auto_categorize_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    finsight_api::commands::settings::set_auto_categorize_enabled(&state.api, enabled).await
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
pub async fn export_all_data_csv(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    let maybe_path = app
        .dialog()
        .file()
        .set_file_name("finsight-transactions.csv")
        .blocking_save_file();

    let Some(file_path) = maybe_path else {
        return Ok(());
    };
    let path = file_path
        .into_path()
        .map_err(|e| AppError::new("dialog", e.to_string()))?;

    let db = (*state.api.db).clone();
    let csv = run(&db, move |conn| {
        use finsight_core::repos::transactions;
        let txns = transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: None,
                limit: i64::MAX,
                offset: 0,
                search: None,
                filter_preset: None,
                start_date: None,
                end_date: None,
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

    std::fs::write(&path, csv).map_err(|e| AppError::new("io", e.to_string()))
}
