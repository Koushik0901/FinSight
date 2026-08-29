use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::{repos::run, settings};

const CURRENCY_KEY: &str = "display_currency";
/// `pub` (not `pub(crate)`): `crates/finsight-bindings/src/lib.rs`'s startup cascade
/// and the finsight-bindings `settings` wrapper module both need this key across the
/// crate boundary now that the body lives here.
pub const AUTO_CATEGORIZE_ENABLED_KEY: &str = "agent.auto_categorize_enabled";

fn resolved_currency(conn: &rusqlite::Connection) -> finsight_core::CoreResult<String> {
    let val: Option<String> = settings::get(conn, CURRENCY_KEY)?;
    if let Some(currency) = val {
        return Ok(currency);
    }

    Ok(finsight_core::currency::currency_profile(conn)?
        .primary()
        .unwrap_or(finsight_core::currency::SCHEMA_DEFAULT_CURRENCY)
        .to_string())
}

#[utoipa::path(post, path = "/api/rpc/get_currency", responses((status = 200, content_type = "application/json", body = String)))]
pub async fn get_currency(state: &ApiState) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        // Before the user explicitly chooses a display/default currency, use
        // the currency their accounts actually establish. This keeps the
        // second account, manual assets, goals, and other currency-less plan
        // values aligned with a CAD/EUR/etc. household instead of silently
        // reverting to USD after the first account was entered correctly.
        resolved_currency(conn)
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetCurrencyRequest {
    pub currency: String,
}

#[utoipa::path(post, path = "/api/rpc/set_currency",
    request_body(content = SetCurrencyRequest), responses((status = 200, description = "Success")))]
pub async fn set_currency(state: &ApiState, currency: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, CURRENCY_KEY, &currency)
    })
    .await
    .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/delete_all_data", responses((status = 200, description = "Success")))]
pub async fn delete_all_data(state: &ApiState) -> AppResult<()> {
    let db = (*state.db).clone();
    // Begin a reset: advance the ledger epoch (so looping background writers
    // bail promptly) and take the exclusive barrier, which BLOCKS until every
    // in-flight writer lease (import cascade, categorizer commit) has drained.
    // Holding this guard across the wipe guarantees no operation started against
    // the previous epoch can commit after this returns success — a straggler
    // either already committed (and is wiped below) or will observe the advanced
    // epoch and abort.
    let _reset = db.reset_barrier().begin_reset().await;
    run(&db, finsight_core::repos::reset::delete_all_data)
        .await
        .map_err(AppError::from)
    // `_reset` drops here, after the wipe has committed.
}

const NOTIFICATIONS_ENABLED_KEY: &str = "notifications.enabled";

#[utoipa::path(post, path = "/api/rpc/get_notifications_enabled", responses((status = 200, content_type = "application/json", body = bool)))]
pub async fn get_notifications_enabled(state: &ApiState) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let val: Option<bool> = settings::get(conn, NOTIFICATIONS_ENABLED_KEY)?;
        Ok(val.unwrap_or(true))
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetNotificationsEnabledRequest {
    pub enabled: bool,
}

#[utoipa::path(post, path = "/api/rpc/set_notifications_enabled",
    request_body(content = SetNotificationsEnabledRequest), responses((status = 200, description = "Success")))]
pub async fn set_notifications_enabled(state: &ApiState, enabled: bool) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, NOTIFICATIONS_ENABLED_KEY, &enabled)
    })
    .await
    .map_err(AppError::from)
}

#[utoipa::path(post, path = "/api/rpc/get_auto_categorize_enabled", responses((status = 200, content_type = "application/json", body = bool)))]
pub async fn get_auto_categorize_enabled(state: &ApiState) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let val: Option<bool> = settings::get(conn, AUTO_CATEGORIZE_ENABLED_KEY)?;
        Ok(val.unwrap_or(true))
    })
    .await
    .map_err(AppError::from)
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SetAutoCategorizeEnabledRequest {
    pub enabled: bool,
}

#[utoipa::path(post, path = "/api/rpc/set_auto_categorize_enabled",
    request_body(content = SetAutoCategorizeEnabledRequest), responses((status = 200, description = "Success")))]
pub async fn set_auto_categorize_enabled(state: &ApiState, enabled: bool) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, AUTO_CATEGORIZE_ENABLED_KEY, &enabled)
    })
    .await
    .map_err(AppError::from)
}

/// Real implementation as of Phase 4 (previously 501'd — dialog-only).
#[utoipa::path(post, path = "/api/rpc/export_all_data_json", responses((status = 200, content_type = "application/json", body = String)))]
pub async fn export_all_data_json(state: &ApiState) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
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
    .map_err(AppError::from)
}

/// Real implementation as of Phase 4 (previously 501'd — dialog-only).
#[utoipa::path(post, path = "/api/rpc/export_all_data_csv", responses((status = 200, content_type = "application/json", body = String)))]
pub async fn export_all_data_csv(state: &ApiState) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
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
            let merchant = crate::csv::csv_escape(&t.merchant_raw);
            let category = crate::csv::csv_escape(t.category_label.as_deref().unwrap_or(""));
            let amount = format!("{:.2}", t.amount_cents as f64 / 100.0);
            let notes = crate::csv::csv_escape(t.notes.as_deref().unwrap_or(""));
            out.push_str(&format!("{date},{merchant},{category},{amount},{notes}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{repos::run, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let (dir, db) = finsight_core::testing::migrated_db();
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
        run(&db, |conn| {
            settings::set(conn, AUTO_CATEGORIZE_ENABLED_KEY, &false)
        })
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

    #[test]
    fn currency_defaults_to_live_account_currency_until_explicitly_set() {
        let (_dir, db) = fresh_db();
        let conn = db.get().unwrap();

        assert_eq!(resolved_currency(&conn).unwrap(), "USD");
        conn.execute(
            "INSERT INTO accounts (id, owner, bank, type, name, currency, color, created_at) \
             VALUES ('cad-1', 'joint', 'Bank', 'Checking', 'Chequing', 'CAD', '#000000', '2026-08-08T00:00:00Z')",
            [],
        )
        .unwrap();
        assert_eq!(resolved_currency(&conn).unwrap(), "CAD");

        settings::set(&conn, CURRENCY_KEY, &"EUR").unwrap();
        assert_eq!(resolved_currency(&conn).unwrap(), "EUR");
    }
}