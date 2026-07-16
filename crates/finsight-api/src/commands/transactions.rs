use crate::error::{AppError, AppResult};
use crate::ApiState;
use chrono::{Datelike, Utc};
use finsight_core::models::{NewTransaction, Transaction, TxnPatch};
use finsight_core::repos::{rules, run, transactions};
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
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

pub async fn list_transactions(
    state: &ApiState,
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
                start_date: filter.start_date,
                end_date: filter.end_date,
            },
        )
    })
    .await
    .map_err(AppError::from)?;
    Ok(result)
}

pub async fn create_transaction(state: &ApiState, input: NewTransaction) -> AppResult<Transaction> {
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

pub async fn update_transaction(
    state: &ApiState,
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
        Ok(UpdateTxnResult {
            transaction: txn,
            proposed_rule,
        })
    })
    .await
    .map_err(AppError::from)
}

pub async fn delete_transaction(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| transactions::delete(conn, &id))
        .await
        .map_err(AppError::from)
}

pub async fn create_rule(
    state: &ApiState,
    pattern: String,
    category_id: String,
) -> AppResult<finsight_core::models::Rule> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let rule = rules::insert(
            conn,
            finsight_core::models::NewRule {
                pattern: pattern.clone(),
                category_id: category_id.clone(),
                source: "user".to_string(),
            },
        )?;
        // Apply immediately to existing uncategorized history so the user sees
        // the effect at once (e.g. "always Rent" back-fills past rent payments),
        // rather than waiting for the next import/scan.
        rules::apply_to_uncategorized(conn, &pattern, &category_id)?;
        Ok(rule)
    })
    .await
    .map_err(AppError::from)
}

pub async fn set_transaction_owner(
    state: &ApiState,
    transaction_id: String,
    member_id: Option<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        conn.execute(
            "UPDATE transactions SET owner_member_id = ?1 WHERE id = ?2",
            rusqlite::params![member_id, transaction_id],
        )?;
        Ok::<_, finsight_core::CoreError>(())
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
    pub spending_type: Option<String>,
}

pub async fn list_categories(state: &ApiState) -> AppResult<Vec<CategoryDto>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let mut stmt = conn.prepare(
            "SELECT c.id, c.label, c.color, c.group_id, COALESCE(g.label, ''), c.spending_type \
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
                spending_type: r.get(5)?,
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
    pub spending_type: Option<String>,
    /// Total outflow this calendar month (positive = money spent)
    pub this_month_cents: i64,
    /// Total outflow last calendar month
    pub last_month_cents: i64,
    /// Number of transactions categorised here this month
    pub txn_count: i64,
    pub year_total_cents: i64,
    /// Number of transactions categorised here so far this calendar year
    pub year_txn_count: i64,
    pub budget_cents: i64,
    /// Free-text categorizer/Copilot guidance the user attached.
    pub guidance: Option<String>,
}

pub async fn list_categories_with_spending(
    state: &ApiState,
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
            "WITH spending AS (
               SELECT t.category_id, t.posted_at, ABS(t.amount_cents) AS cents
               FROM transactions t
               WHERE t.amount_cents < 0
                 AND t.category_id IS NOT NULL
                 AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
               UNION ALL
               SELECT ts.category_id, t.posted_at, ts.amount_cents AS cents
               FROM transaction_splits ts
               JOIN transactions t ON t.id = ts.txn_id
               WHERE t.amount_cents < 0
                 AND ts.category_id IS NOT NULL
             )
             SELECT
               c.id, c.label, COALESCE(c.color,''), c.group_id, COALESCE(g.label,''), c.spending_type,
               COALESCE(SUM(CASE WHEN s.posted_at >= ?1 THEN s.cents ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN s.posted_at >= ?2 AND s.posted_at < ?1 THEN s.cents ELSE 0 END), 0),
               COUNT(CASE WHEN s.posted_at >= ?1 THEN 1 END),
               COALESCE(SUM(CASE WHEN s.posted_at >= ?3 THEN s.cents ELSE 0 END), 0),
               COUNT(CASE WHEN s.posted_at >= ?3 THEN 1 END),
               COALESCE(MAX(b.amount_cents), 0),
               c.guidance
             FROM categories c
             LEFT JOIN category_groups g ON g.id = c.group_id
             LEFT JOIN spending s ON s.category_id = c.id
             LEFT JOIN budgets b ON b.category_id = c.id AND b.month = ?4
             WHERE c.archived_at IS NULL
             GROUP BY c.id, c.label, c.color, c.group_id, g.label, c.spending_type, c.guidance
             ORDER BY 7 DESC, g.sort_order, c.sort_order",
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
                    spending_type: r.get(5)?,
                    this_month_cents: r.get(6)?,
                    last_month_cents: r.get(7)?,
                    txn_count: r.get(8)?,
                    year_total_cents: r.get(9)?,
                    year_txn_count: r.get(10)?,
                    budget_cents: r.get(11)?,
                    guidance: r.get(12)?,
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

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpendingBreakdown {
    pub fixed_cents: i64,
    pub investments_cents: i64,
    pub savings_cents: i64,
    pub guilt_free_cents: i64,
    pub untagged_cents: i64,
    pub total_income_cents: i64,
}

pub async fn set_category_spending_type(
    state: &ApiState,
    id: String,
    spending_type: Option<String>,
) -> AppResult<()> {
    if !matches!(
        spending_type.as_deref(),
        None | Some("fixed" | "investments" | "savings" | "guilt_free")
    ) {
        return Err(AppError::new(
            "validation",
            "Invalid spending type. Use fixed, investments, savings, guilt_free, or null.",
        ));
    }

    let db = (*state.db).clone();
    let updated_at = Utc::now().to_rfc3339();
    run(&db, move |conn| {
        conn.execute(
            "UPDATE categories SET spending_type = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![spending_type, updated_at, id],
        )?;
        Ok(())
    })
    .await
    .map_err(AppError::from)
}

pub async fn get_spending_breakdown(state: &ApiState) -> AppResult<SpendingBreakdown> {
    let db = (*state.db).clone();
    let this_month_start = Utc::now().format("%Y-%m-01").to_string();

    run(&db, move |conn| {
        let (fixed_cents, investments_cents, savings_cents, guilt_free_cents, untagged_cents): (
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = conn.query_row(
            "WITH spending AS (
                SELECT c.spending_type, ABS(t.amount_cents) AS cents
                FROM transactions t
                JOIN categories c ON c.id = t.category_id
                WHERE t.amount_cents < 0
                  AND t.category_id IS NOT NULL
                  AND t.posted_at >= ?1
                  AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
                UNION ALL
                SELECT c.spending_type, ts.amount_cents AS cents
                FROM transaction_splits ts
                JOIN transactions t ON t.id = ts.txn_id
                JOIN categories c ON c.id = ts.category_id
                WHERE t.amount_cents < 0
                  AND ts.category_id IS NOT NULL
                  AND t.posted_at >= ?1
             )
             SELECT
                COALESCE(SUM(CASE WHEN spending_type = 'fixed' THEN cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN spending_type = 'investments' THEN cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN spending_type = 'savings' THEN cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN spending_type = 'guilt_free' THEN cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN spending_type IS NULL THEN cents ELSE 0 END), 0)
             FROM spending",
            rusqlite::params![this_month_start],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;

        // Through the metrics layer: raw `amount_cents > 0` would count
        // transfers-in and brokerage contributions as "income", inflating the
        // conscious-spending denominator.
        let (total_income_cents, _) =
            finsight_core::metrics::income_expense_since(conn, &this_month_start)?;

        Ok(SpendingBreakdown {
            fixed_cents,
            investments_cents,
            savings_cents,
            guilt_free_cents,
            untagged_cents,
            total_income_cents,
        })
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

pub async fn list_rules_with_categories(state: &ApiState) -> AppResult<Vec<RuleWithCategory>> {
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

pub async fn toggle_rule(state: &ApiState, id: String, enabled: bool) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| rules::set_enabled(conn, &id, enabled))
        .await
        .map_err(AppError::from)
}

pub async fn get_transaction_count(state: &ApiState) -> AppResult<i64> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?)
    })
    .await
    .map_err(AppError::from)
}

pub async fn set_transaction_flags(
    state: &ApiState,
    id: String,
    is_reimbursable: bool,
    is_split: bool,
) -> AppResult<finsight_core::models::Transaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        transactions::set_flags(conn, &id, is_reimbursable, is_split)
    })
    .await
    .map_err(AppError::from)
}

/// Result of a transfer verdict: the updated transaction, plus how many other
/// UNDECIDED transactions share the same counterparty so the UI can offer to
/// apply the verdict to all of them in one click.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferVerdictResult {
    pub transaction: finsight_core::models::Transaction,
    /// LIKE pattern identifying the siblings (pass to
    /// `apply_transfer_verdict_to_similar`), e.g. `%swathi%`.
    pub similar_pattern: Option<String>,
    /// Human-readable counterparty ("swathi") for the offer text.
    pub similar_label: Option<String>,
    pub similar_count: i64,
}

pub async fn set_transaction_transfer(
    state: &ApiState,
    id: String,
    is_transfer: bool,
) -> AppResult<TransferVerdictResult> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        // Count siblings BEFORE ruling this row so the source row's own state
        // change can't affect the sibling query.
        let siblings = transactions::transfer_verdict_siblings(conn, &id)?;
        let transaction = transactions::set_transfer_override(conn, &id, is_transfer)?;
        let (similar_pattern, similar_count) = match siblings {
            Some((p, n)) => (Some(p), n),
            None => (None, 0),
        };
        let similar_label = similar_pattern
            .as_deref()
            .map(|p| p.trim_matches('%').to_string());
        Ok(TransferVerdictResult {
            transaction,
            similar_pattern,
            similar_label,
            similar_count,
        })
    })
    .await
    .map_err(AppError::from)
}

pub async fn apply_transfer_verdict_to_similar(
    state: &ApiState,
    pattern: String,
    is_transfer: bool,
) -> AppResult<u32> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        transactions::apply_transfer_override_to_matching(conn, &pattern, is_transfer)
    })
    .await
    .map_err(AppError::from)
}

// ── Split transaction commands ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSplitDto {
    pub id: String,
    pub txn_id: String,
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SplitInputDto {
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

pub async fn get_transaction_splits(
    state: &ApiState,
    transaction_id: String,
) -> AppResult<Vec<TransactionSplitDto>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        finsight_core::repos::splits::list(conn, &transaction_id).map(|v| {
            v.into_iter()
                .map(|s| TransactionSplitDto {
                    id: s.id,
                    txn_id: s.txn_id,
                    category_id: s.category_id,
                    amount_cents: s.amount_cents,
                })
                .collect()
        })
    })
    .await
    .map_err(AppError::from)
}

pub async fn set_transaction_splits(
    state: &ApiState,
    transaction_id: String,
    splits: Vec<SplitInputDto>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let inputs: Vec<finsight_core::repos::splits::SplitInput> = splits
            .into_iter()
            .map(|s| finsight_core::repos::splits::SplitInput {
                category_id: s.category_id,
                amount_cents: s.amount_cents,
            })
            .collect();
        finsight_core::repos::splits::set(conn, &transaction_id, &inputs)
    })
    .await
    .map_err(AppError::from)
}
