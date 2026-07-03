use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::{repos::run, settings};
use tauri_plugin_dialog::DialogExt;

const CURRENCY_KEY: &str = "display_currency";
const AUTO_CATEGORIZE_ENABLED_KEY: &str = "agent.auto_categorize_enabled";

#[tauri::command]
#[specta::specta]
pub async fn get_currency(state: tauri::State<'_, AppState>) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let val: Option<String> = settings::get(conn, CURRENCY_KEY)?;
        Ok(val.unwrap_or_else(|| "USD".to_string()))
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_currency(state: tauri::State<'_, AppState>, currency: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, CURRENCY_KEY, &currency)
    })
    .await
    .map_err(AppError::from)
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

    let db = (*state.db).clone();
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

const NOTIFICATIONS_ENABLED_KEY: &str = "notifications.enabled";

#[tauri::command]
#[specta::specta]
pub async fn get_notifications_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let val: Option<bool> = settings::get(conn, NOTIFICATIONS_ENABLED_KEY)?;
        Ok(val.unwrap_or(true))
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_notifications_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, NOTIFICATIONS_ENABLED_KEY, &enabled)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_auto_categorize_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let val: Option<bool> = settings::get(conn, AUTO_CATEGORIZE_ENABLED_KEY)?;
        Ok(val.unwrap_or(true))
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_auto_categorize_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, AUTO_CATEGORIZE_ENABLED_KEY, &enabled)
    })
    .await
    .map_err(AppError::from)
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

    let db = (*state.db).clone();
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

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, repos::run, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("settings.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn auto_categorize_enabled_defaults_true() {
        let (_dir, db) = fresh_db();
        let val: bool = run(&db, |conn| {
            let v: Option<bool> = settings::get(conn, AUTO_CATEGORIZE_ENABLED_KEY)?;
            Ok(v.unwrap_or(true))
        })
        .await
        .unwrap();
        assert!(val);
    }

    #[tokio::test]
    async fn auto_categorize_enabled_round_trips() {
        let (_dir, db) = fresh_db();
        run(&db, |conn| settings::set(conn, AUTO_CATEGORIZE_ENABLED_KEY, &false))
            .await
            .unwrap();
        let val: bool = run(&db, |conn| {
            let v: Option<bool> = settings::get(conn, AUTO_CATEGORIZE_ENABLED_KEY)?;
            Ok(v.unwrap_or(true))
        })
        .await
        .unwrap();
        assert!(!val);
    }
}
