use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::{Datelike, Utc};
use finsight_core::models::{NewTransaction, Transaction, TxnPatch};
use finsight_core::repos::{rules, run, transactions};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_plugin_dialog::DialogExt;

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
                start_date: filter.start_date,
                end_date: filter.end_date,
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
        Ok(UpdateTxnResult {
            transaction: txn,
            proposed_rule,
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_transaction(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
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
        let rule = rules::insert(
            conn,
            finsight_core::models::NewRule {
                pattern: pattern.clone(),
                category_id: category_id.clone(),
                source: "user".to_string(),
                treatment: "categorize".to_string(),
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

#[tauri::command]
#[specta::specta]
pub async fn list_categories(state: tauri::State<'_, AppState>) -> AppResult<Vec<CategoryDto>> {
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

/// Query logic behind [`list_categories_with_spending`], extracted so it is
/// directly unit-testable against a raw connection without a Tauri
/// `AppState`.
fn categories_with_spending(
    conn: &rusqlite::Connection,
    this_month_start: &str,
    last_month_start: &str,
    year_start: &str,
    current_month: &str,
) -> finsight_core::CoreResult<Vec<CategoryWithSpending>> {
    let mut stmt = conn.prepare(
        "WITH spending AS (
           SELECT t.category_id, t.posted_at,
                  CASE WHEN t.settle_up = 1 THEN -t.amount_cents ELSE ABS(t.amount_cents) END AS cents
           FROM transactions t
           WHERE (t.amount_cents < 0 OR t.settle_up = 1)
             AND t.category_id IS NOT NULL
             AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
           UNION ALL
           SELECT ts.category_id, t.posted_at,
                  CASE WHEN t.settle_up = 1 AND t.amount_cents < 0 THEN ts.amount_cents
                       WHEN t.settle_up = 1 THEN -ts.amount_cents
                       ELSE ts.amount_cents END AS cents
           FROM transaction_splits ts
           JOIN transactions t ON t.id = ts.txn_id
           WHERE (t.amount_cents < 0 OR t.settle_up = 1)
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
        categories_with_spending(
            conn,
            &this_month_start,
            &last_month_start,
            &year_start,
            &current_month,
        )
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

#[tauri::command]
#[specta::specta]
pub async fn set_category_spending_type(
    state: tauri::State<'_, AppState>,
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

#[tauri::command]
#[specta::specta]
pub async fn get_spending_breakdown(
    state: tauri::State<'_, AppState>,
) -> AppResult<SpendingBreakdown> {
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
                SELECT c.spending_type,
                       CASE WHEN t.settle_up = 1 THEN -t.amount_cents ELSE ABS(t.amount_cents) END AS cents
                FROM transactions t
                JOIN categories c ON c.id = t.category_id
                WHERE (t.amount_cents < 0 OR t.settle_up = 1)
                  AND t.category_id IS NOT NULL
                  AND t.posted_at >= ?1
                  AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
                UNION ALL
                SELECT c.spending_type,
                       CASE WHEN t.settle_up = 1 AND t.amount_cents < 0 THEN ts.amount_cents
                            WHEN t.settle_up = 1 THEN -ts.amount_cents
                            ELSE ts.amount_cents END AS cents
                FROM transaction_splits ts
                JOIN transactions t ON t.id = ts.txn_id
                JOIN categories c ON c.id = ts.category_id
                WHERE (t.amount_cents < 0 OR t.settle_up = 1)
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
pub async fn get_transaction_count(state: tauri::State<'_, AppState>) -> AppResult<i64> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM transactions", [], |r| r.get(0))?)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_transaction_flags(
    state: tauri::State<'_, AppState>,
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

/// Record the user's verdict on whether a transaction is a transfer between
/// their own accounts. Sticky: survives re-imports and categorizer re-runs.
#[tauri::command]
#[specta::specta]
pub async fn set_transaction_transfer(
    state: tauri::State<'_, AppState>,
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
    let db = (*state.db).clone();
    run(&db, move |conn| {
        transactions::apply_transfer_override_to_matching(conn, &pattern, is_transfer)
    })
    .await
    .map_err(AppError::from)
}

/// Serializable mirror of `finsight_core::repos::transactions::Verdict` —
/// the core enum has no serde/specta derives by design (finsight-core has no
/// Tauri/specta dependency), so this DTO crosses the specta boundary instead.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CounterpartyVerdict {
    Transfer,
    SettleUp,
    Real,
}

impl From<CounterpartyVerdict> for finsight_core::repos::transactions::Verdict {
    fn from(v: CounterpartyVerdict) -> Self {
        use finsight_core::repos::transactions::Verdict;
        match v {
            CounterpartyVerdict::Transfer => Verdict::Transfer,
            CounterpartyVerdict::SettleUp => Verdict::SettleUp,
            CounterpartyVerdict::Real => Verdict::Real,
        }
    }
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
) -> AppResult<finsight_core::models::Transaction> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        transactions::set_counterparty_verdict(conn, &id, verdict.into())
    })
    .await
    .map_err(AppError::from)
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
    let db = (*state.db).clone();
    run(&db, move |conn| {
        transactions::apply_verdict_to_matching(conn, &pattern, verdict.into())
    })
    .await
    .map_err(AppError::from)
}

/// One counterparty's undecided transfer-like rows, netted for the grouped
/// review surface. Mirrors `finsight_core::repos::transactions::UnresolvedCounterparty`.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedCounterpartyDto {
    pub pattern: Option<String>,
    pub label: String,
    pub txn_count: i64,
    pub inflow_cents: i64,
    pub outflow_cents: i64,
}

impl From<transactions::UnresolvedCounterparty> for UnresolvedCounterpartyDto {
    fn from(u: transactions::UnresolvedCounterparty) -> Self {
        UnresolvedCounterpartyDto {
            pattern: u.pattern,
            label: u.label,
            txn_count: u.txn_count,
            inflow_cents: u.inflow_cents,
            outflow_cents: u.outflow_cents,
        }
    }
}

/// The undecided transfer-review queue, grouped by counterparty for a
/// bulk-decision surface.
#[tauri::command]
#[specta::specta]
pub async fn list_unresolved_counterparties(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<UnresolvedCounterpartyDto>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        transactions::list_unresolved_counterparties(conn)
    })
    .await
    .map_err(AppError::from)
    .map(|v| v.into_iter().map(UnresolvedCounterpartyDto::from).collect())
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

#[tauri::command]
#[specta::specta]
pub async fn get_transaction_splits(
    state: tauri::State<'_, AppState>,
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

#[tauri::command]
#[specta::specta]
pub async fn set_transaction_splits(
    state: tauri::State<'_, AppState>,
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

    let db = (*state.db).clone();
    let csv = run(&db, move |conn| {
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

    let db = (*state.db).clone();
    let csv = run(&db, move |conn| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("txn_cmd.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_account(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('a1','Me','Bank','Checking','Checking','USD','#fff',datetime('now'))",
            [],
        )
        .unwrap();
    }

    fn seed_category(conn: &rusqlite::Connection, id: &str, label: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'grp', ?2, '#94A3B8', 0)",
            rusqlite::params![id, label],
        )
        .unwrap();
    }

    #[test]
    fn split_settle_up_outflow_nets_as_positive_expense() {
        // A settle_up=1 outflow parent (you paid $60 on a friend's behalf,
        // to be paid back) split into two categories must show up as
        // POSITIVE expense in each category — the split-union arm must not
        // flip sign to a negative (income-looking) number just because the
        // parent happens to be settle_up. Only a settle-up INFLOW parent
        // (a reimbursement) should net negative.
        let (_dir, db) = fresh_db();
        let conn = db.get().unwrap();
        seed_account(&conn);
        seed_category(&conn, "dining", "Dining");
        seed_category(&conn, "groceries", "Groceries");

        let now = chrono::Utc::now();
        let posted_at = now.to_rfc3339();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,is_transfer,created_at,settle_up,is_split) \
             VALUES('p1','a1',?1,-6000,'DINNER OUT','cleared',0,0,?1,1,1)",
            rusqlite::params![posted_at],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transaction_splits(id,txn_id,category_id,amount_cents) VALUES \
             ('sp1','p1','dining',4000),('sp2','p1','groceries',2000)",
            [],
        )
        .unwrap();

        let this_month_start = now.format("%Y-%m-01").to_string();
        let current_month = now.format("%Y-%m").to_string();
        let categories = categories_with_spending(
            &conn,
            &this_month_start,
            "1900-01-01",
            "1900-01-01",
            &current_month,
        )
        .unwrap();

        let dining = categories
            .iter()
            .find(|c| c.id == "dining")
            .expect("dining category present");
        let groceries = categories
            .iter()
            .find(|c| c.id == "groceries")
            .expect("groceries category present");
        assert_eq!(
            dining.this_month_cents, 4000,
            "settle-up outflow split child nets as positive expense, not negative"
        );
        assert_eq!(
            groceries.this_month_cents, 2000,
            "settle-up outflow split child nets as positive expense, not negative"
        );
    }
}
