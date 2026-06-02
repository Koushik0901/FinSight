use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::models::{NewTransaction, Transaction, TxnPatch};
use finsight_core::repos::{rules, run, transactions};
use chrono::{Datelike, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct TxnFilterInput {
    pub account_id: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub search: Option<String>,
    pub filter_preset: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_transactions(
    state: tauri::State<'_, AppState>,
    filter: TxnFilterInput,
) -> AppResult<Vec<Transaction>> {
    let db = (*state.db).clone();
    let result = run(&db, move |conn| {
        transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: filter.account_id,
                limit: filter.limit.unwrap_or(100),
                offset: filter.offset.unwrap_or(0),
                search: filter.search,
                filter_preset: filter.filter_preset,
            },
        )
    })
    .await
    .map_err(AppError::from)?;
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn create_transaction(
    state: tauri::State<'_, AppState>,
    input: NewTransaction,
) -> AppResult<Transaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| transactions::insert(conn, input))
        .await
        .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, Type)]
pub struct ProposedRuleDto {
    pub pattern: String,
    pub category_id: String,
    pub category_label: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct UpdateTxnResult {
    pub transaction: Transaction,
    pub proposed_rule: Option<ProposedRuleDto>,
}

#[tauri::command]
#[specta::specta]
pub async fn update_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
    patch: TxnPatch,
) -> AppResult<UpdateTxnResult> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let (txn, rule) = transactions::update(conn, &id, patch)?;
        let proposed_rule = rule.map(|r| ProposedRuleDto {
            pattern: r.pattern,
            category_id: r.category_id,
            category_label: r.category_label,
        });
        Ok(UpdateTxnResult { transaction: txn, proposed_rule })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_transaction(
    state: tauri::State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| transactions::delete(conn, &id))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_rule(
    state: tauri::State<'_, AppState>,
    pattern: String,
    category_id: String,
) -> AppResult<finsight_core::models::Rule> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        rules::insert(conn, finsight_core::models::NewRule {
            pattern,
            category_id,
            source: "user".to_string(),
        })
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CategoryDto {
    pub id: String,
    pub label: String,
    pub color: String,
    pub group_id: String,
    pub group_label: String,
}

#[tauri::command]
#[specta::specta]
pub async fn list_categories(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryDto>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let mut stmt = conn.prepare(
            "SELECT c.id, c.label, c.color, c.group_id, COALESCE(g.label, '') \
             FROM categories c \
             LEFT JOIN category_groups g ON g.id = c.group_id \
             WHERE c.archived_at IS NULL \
             ORDER BY g.sort_order, c.sort_order",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(CategoryDto {
                id: r.get(0)?,
                label: r.get(1)?,
                color: r.get(2)?,
                group_id: r.get(3)?,
                group_label: r.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

/// Category with real spending aggregated from transactions.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryWithSpending {
    pub id: String,
    pub label: String,
    pub color: String,
    pub group_id: String,
    pub group_label: String,
    /// Total outflow this calendar month (positive = money spent)
    pub this_month_cents: i64,
    /// Total outflow last calendar month
    pub last_month_cents: i64,
    /// Number of transactions categorised here this month
    pub txn_count: i64,
    pub year_total_cents: i64,
    pub budget_cents: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn list_categories_with_spending(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryWithSpending>> {
    let db = (*state.db).clone();
    let now = Utc::now();
    let this_month_start = now.format("%Y-%m-01").to_string();
    let last_month_start = {
        let m = now.month0();
        if m == 0 {
            format!("{}-12-01", now.year() - 1)
        } else {
            format!("{}-{:02}-01", now.year(), m)
        }
    };
    let year_start = format!("{}-01-01", now.year());
    let current_month = now.format("%Y-%m").to_string();

    run(&db, move |conn| {
        let mut stmt = conn.prepare(
            "SELECT \
               c.id, c.label, COALESCE(c.color,''), c.group_id, COALESCE(g.label,''), \
               COALESCE(SUM(CASE WHEN t.posted_at >= ?1 AND t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0), \
               COALESCE(SUM(CASE WHEN t.posted_at >= ?2 AND t.posted_at < ?1 AND t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0), \
               COUNT(CASE WHEN t.posted_at >= ?1 THEN 1 END), \
               COALESCE(SUM(CASE WHEN t.posted_at >= ?3 AND t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END),0), \
               COALESCE(MAX(b.amount_cents), 0) \
             FROM categories c \
             LEFT JOIN category_groups g ON g.id = c.group_id \
             LEFT JOIN transactions t ON t.category_id = c.id \
             LEFT JOIN budgets b ON b.category_id = c.id AND b.month = ?4 \
             WHERE c.archived_at IS NULL \
             GROUP BY c.id, c.label, c.color, c.group_id, g.label \
             ORDER BY 6 DESC, g.sort_order, c.sort_order",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![this_month_start, last_month_start, year_start, current_month],
            |r| {
                Ok(CategoryWithSpending {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    color: r.get(2)?,
                    group_id: r.get(3)?,
                    group_label: r.get(4)?,
                    this_month_cents: r.get(5)?,
                    last_month_cents: r.get(6)?,
                    txn_count: r.get(7)?,
                    year_total_cents: r.get(8)?,
                    budget_cents: r.get(9)?,
                })
            },
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

/// Rule with resolved category label and color.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleWithCategory {
    pub id: String,
    pub pattern: String,
    pub category_id: String,
    pub category_label: String,
    pub category_color: String,
    pub enabled: bool,
    pub source: String,
    pub created_at: String,
}

#[tauri::command]
#[specta::specta]
pub async fn list_rules_with_categories(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<RuleWithCategory>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.pattern, r.category_id, \
                    COALESCE(c.label,''), COALESCE(c.color,''), \
                    r.enabled, r.source, r.created_at \
             FROM rules r \
             LEFT JOIN categories c ON c.id = r.category_id \
             ORDER BY r.created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(RuleWithCategory {
                id: r.get(0)?,
                pattern: r.get(1)?,
                category_id: r.get(2)?,
                category_label: r.get(3)?,
                category_color: r.get(4)?,
                enabled: r.get::<_, i64>(5)? != 0,
                source: r.get(6)?,
                created_at: r.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_rule(
    state: tauri::State<'_, AppState>,
    id: String,
    enabled: bool,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| rules::set_enabled(conn, &id, enabled))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn get_transaction_count(
    state: tauri::State<'_, AppState>,
) -> AppResult<i64> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?)
    })
    .await
    .map_err(AppError::from)
}
