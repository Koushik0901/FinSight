use crate::error::CoreResult;
use crate::models::{
    NewTransaction, ProposedRule, Transaction, TransactionStatus, TxnActivity, TxnPatch,
};
use crate::repos::{accounts, categorizations};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn insert(conn: &mut Connection, input: NewTransaction) -> CoreResult<Transaction> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    // Trade/MoneyMovement rows are internal moves (cash↔security, account↔
    // account), never income or spending — flag them at insert time so every
    // `is_transfer = 0` metric filter excludes them from day one.
    let is_transfer = input
        .activity
        .as_ref()
        .map(|a| crate::categorize::activity_implies_transfer(&a.activity_type))
        .unwrap_or(false);
    let activity = input.activity.clone();
    conn.execute(
        "INSERT INTO transactions \
         (id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, notes, is_anomaly, created_at, imported_id, source, raw_synced_data, pending, external_tx_id, external_account_id, is_transfer, activity_type, activity_sub_type, symbol, security_name, quantity, unit_price) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
        params![
            &id,
            &input.account_id,
            input.posted_at.to_rfc3339(),
            input.amount_cents,
            &input.merchant_raw,
            &input.category_id,
            input.status.as_db(),
            &input.notes,
            now.to_rfc3339(),
            &input.imported_id,
            &input.source,
            &input.raw_synced_data,
            input.pending,
            &input.external_tx_id,
            &input.external_account_id,
            is_transfer,
            activity.as_ref().map(|a| a.activity_type.clone()),
            activity.as_ref().and_then(|a| a.activity_sub_type.clone()),
            activity.as_ref().and_then(|a| a.symbol.clone()),
            activity.as_ref().and_then(|a| a.security_name.clone()),
            activity.as_ref().and_then(|a| a.quantity),
            activity.as_ref().and_then(|a| a.unit_price),
        ],
    )?;
    let txn = Transaction {
        id,
        account_id: input.account_id,
        posted_at: input.posted_at,
        amount_cents: input.amount_cents,
        merchant_raw: input.merchant_raw,
        merchant_id: None,
        merchant_label: None,
        merchant_color: None,
        merchant_initials: None,
        category_id: input.category_id,
        category_label: None,
        category_color: None,
        status: input.status,
        notes: input.notes,
        ai_confidence: None,
        ai_explanation: None,
        is_anomaly: false,
        created_at: now,
        is_reimbursable: false,
        settle_up: false,
        is_split: false,
        is_transfer,
        transfer_peer_id: None,
        transfer_peer_account_name: None,
        owner_member_id: None,
        imported_id: input.imported_id,
        source: input.source,
        raw_synced_data: input.raw_synced_data,
        pending: input.pending,
        external_tx_id: input.external_tx_id,
        external_account_id: input.external_account_id,
        activity,
    };
    let account_id = txn.account_id.clone();
    // Keep SimpleFin-linked account balances in sync with the ledger.
    accounts::recompute_balance_if_linked(conn, &account_id)?;
    Ok(txn)
}

/// Hydrate the six V048 activity columns (selected contiguously starting at
/// `base`) into a nested `TxnActivity` — present only when `activity_type`
/// is non-NULL.
fn read_activity(r: &rusqlite::Row<'_>, base: usize) -> rusqlite::Result<Option<TxnActivity>> {
    let activity_type: Option<String> = r.get(base)?;
    Ok(match activity_type {
        None => None,
        Some(activity_type) => Some(TxnActivity {
            activity_type,
            activity_sub_type: r.get(base + 1)?,
            symbol: r.get(base + 2)?,
            security_name: r.get(base + 3)?,
            quantity: r.get(base + 4)?,
            unit_price: r.get(base + 5)?,
        }),
    })
}

pub struct TxnFilter {
    pub account_id: Option<String>,
    pub limit: i64,
    pub offset: i64,
    pub search: Option<String>,
    pub filter_preset: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

impl Default for TxnFilter {
    fn default() -> Self {
        Self {
            account_id: None,
            limit: 100,
            offset: 0,
            search: None,
            filter_preset: None,
            start_date: None,
            end_date: None,
        }
    }
}

pub fn list(conn: &mut Connection, filter: TxnFilter) -> CoreResult<Vec<Transaction>> {
    let mut sql = String::from(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at, \
                t.is_reimbursable, t.settle_up, t.is_split, t.imported_id, t.source, \
                t.raw_synced_data, t.pending, t.external_tx_id, t.external_account_id, t.is_transfer, \
                t.transfer_peer_id, pa.name, t.owner_member_id, \
                t.activity_type, t.activity_sub_type, t.symbol, t.security_name, t.quantity, t.unit_price \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         LEFT JOIN transactions pt ON pt.id = t.transfer_peer_id \
         LEFT JOIN accounts pa ON pa.id = pt.account_id ",
    );

    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut conditions: Vec<String> = Vec::new();

    if let Some(aid) = filter.account_id.as_ref() {
        conditions.push("t.account_id = ?".to_string());
        params.push(Box::new(aid.clone()));
    }
    if let Some(search) = filter.search.as_ref() {
        conditions.push(
            "(lower(t.merchant_raw) LIKE lower(?) OR lower(COALESCE(t.notes,'')) LIKE lower(?))"
                .to_string(),
        );
        let pattern = format!("%{}%", search);
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }
    if let Some(start_date) = filter.start_date.as_ref() {
        conditions.push("t.posted_at >= ?".to_string());
        params.push(Box::new(start_date.clone()));
    }
    if let Some(end_date) = filter.end_date.as_ref() {
        conditions.push("t.posted_at <= ?".to_string());
        params.push(Box::new(end_date.clone()));
    }
    match filter.filter_preset.as_deref() {
        Some("needs_review") => {
            conditions.push("t.ai_confidence IS NOT NULL AND t.ai_confidence < 0.6".to_string());
        }
        Some("anomalies") => {
            conditions.push("t.is_anomaly = 1".to_string());
        }
        Some("no_category") => {
            // Only rows the user can actually categorize: transfers and
            // investment-account activity are never categorized, so listing
            // them here would make the "needs categorizing" list unclearable.
            conditions.push(format!(
                "t.category_id IS NULL AND t.is_transfer = 0 AND {}",
                crate::metrics::non_investment_txn_predicate("t")
            ));
        }
        Some("transfer_review") => {
            conditions.push(crate::categorize::transfer_review_predicate("t"));
        }
        _ => {}
    }
    if !conditions.is_empty() {
        sql.push_str("WHERE ");
        sql.push_str(&conditions.join(" AND "));
        sql.push(' ');
    }
    sql.push_str("ORDER BY t.posted_at DESC LIMIT ? OFFSET ?");
    params.push(Box::new(filter.limit));
    params.push(Box::new(filter.offset));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())),
        |r| {
            let posted_at_s: String = r.get(2)?;
            let created_at_s: String = r.get(17)?;
            Ok(Transaction {
                id: r.get(0)?,
                account_id: r.get(1)?,
                posted_at: DateTime::parse_from_rfc3339(&posted_at_s)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc),
                amount_cents: r.get(3)?,
                merchant_raw: r.get(4)?,
                merchant_id: r.get(5)?,
                merchant_label: r.get(6)?,
                merchant_color: r.get(7)?,
                merchant_initials: r.get(8)?,
                category_id: r.get(9)?,
                category_label: r.get(10)?,
                category_color: r.get(11)?,
                status: TransactionStatus::from_db(&r.get::<_, String>(12)?),
                notes: r.get(13)?,
                ai_confidence: r.get(14)?,
                ai_explanation: r.get(15)?,
                is_anomaly: r.get::<_, i64>(16)? != 0,
                created_at: DateTime::parse_from_rfc3339(&created_at_s)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            17,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc),
                is_reimbursable: r.get::<_, i64>(18)? != 0,
                settle_up: r.get::<_, i64>(19)? != 0,
                is_split: r.get::<_, i64>(20)? != 0,
                is_transfer: r.get::<_, i64>(27)? != 0,
                transfer_peer_id: r.get(28)?,
                transfer_peer_account_name: r.get(29)?,
                owner_member_id: r.get(30)?,
                imported_id: r.get(21)?,
                source: r.get(22)?,
                raw_synced_data: r.get(23)?,
                pending: r.get::<_, i64>(24)? != 0,
                external_tx_id: r.get(25)?,
                external_account_id: r.get(26)?,
                activity: read_activity(r, 31)?,
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[derive(Debug, Clone, Default)]
pub struct SearchTxnQuery {
    pub merchant: Option<String>,
    pub account: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub min_amount_cents: Option<i64>,
    pub direction: Option<String>, // "expense" | "income" | None
}

pub struct SearchTxnRow {
    pub date: String,
    pub merchant: String,
    pub amount_cents: i64,
    pub account: String,
    pub category: String,
}

/// Shared query builder for both the `search_transactions` Copilot tool and
/// the Copilot "Export as CSV" command — one canonical filter implementation
/// instead of two SQL strings that could drift apart.
pub fn search(
    conn: &Connection,
    query: &SearchTxnQuery,
    limit: i64,
) -> CoreResult<Vec<SearchTxnRow>> {
    let mut sql = "SELECT t.merchant_raw, t.amount_cents, t.posted_at, COALESCE(c.label, 'Uncategorized'), COALESCE(a.name, 'Unknown account') \
         FROM transactions t \
         LEFT JOIN categories c ON c.id = t.category_id \
         LEFT JOIN accounts a ON a.id = t.account_id \
         WHERE 1=1".to_string();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(m) = &query.merchant {
        sql.push_str(" AND lower(t.merchant_raw) LIKE lower(?)");
        params.push(Box::new(format!("%{}%", m)));
    }
    if let Some(acct) = &query.account {
        sql.push_str(" AND lower(a.name) LIKE lower(?)");
        params.push(Box::new(format!("%{}%", acct)));
    }
    if let Some(s) = &query.start_date {
        sql.push_str(" AND t.posted_at >= ?");
        params.push(Box::new(s.clone()));
    }
    if let Some(e) = &query.end_date {
        sql.push_str(" AND t.posted_at <= ?");
        params.push(Box::new(format!("{}T23:59:59", e)));
    }
    if let Some(min) = query.min_amount_cents {
        sql.push_str(" AND ABS(t.amount_cents) >= ?");
        params.push(Box::new(min.abs()));
    }
    match query.direction.as_deref() {
        Some("expense") => sql.push_str(" AND t.amount_cents < 0"),
        Some("income") => sql.push_str(" AND t.amount_cents > 0"),
        _ => {}
    }
    sql.push_str(" ORDER BY t.posted_at DESC LIMIT ?");
    params.push(Box::new(limit));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())),
        |r| {
            Ok(SearchTxnRow {
                merchant: r.get(0)?,
                amount_cents: r.get(1)?,
                date: r.get(2)?,
                category: r.get(3)?,
                account: r.get(4)?,
            })
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update(
    conn: &mut Connection,
    id: &str,
    patch: TxnPatch,
) -> CoreResult<(Transaction, Option<ProposedRule>)> {
    if let Some(notes) = &patch.notes {
        conn.execute(
            "UPDATE transactions SET notes = ?1 WHERE id = ?2",
            params![notes, id],
        )?;
    }
    if let Some(amount) = patch.amount_cents {
        conn.execute(
            "UPDATE transactions SET amount_cents = ?1 WHERE id = ?2",
            params![amount, id],
        )?;
    }
    if let Some(merchant) = &patch.merchant_raw {
        conn.execute(
            "UPDATE transactions SET merchant_raw = ?1 WHERE id = ?2",
            params![merchant, id],
        )?;
    }
    if let Some(ai_confidence) = patch.ai_confidence {
        conn.execute(
            "UPDATE transactions SET ai_confidence = ?1 WHERE id = ?2",
            params![ai_confidence, id],
        )?;
    }

    let mut proposed_rule: Option<ProposedRule> = None;

    if let Some(cat) = &patch.category_id {
        // Append categorization audit row
        categorizations::insert(
            conn,
            crate::models::NewCategorization {
                txn_id: id.to_string(),
                category_id: cat.clone(),
                source: "user".to_string(),
                confidence: 1.0,
                model: None,
            },
        )?;
        // Update live columns
        conn.execute(
            "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
            params![cat, id],
        )?;
        // Check for rule proposal (only when setting a category, not clearing)
        if let Some(category_id) = cat {
            let merchant_raw: String = conn.query_row(
                "SELECT merchant_raw FROM transactions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )?;
            let category_label: String = conn
                .query_row(
                    "SELECT label FROM categories WHERE id = ?1",
                    params![category_id],
                    |r| r.get(0),
                )
                .unwrap_or_default();

            // Record what the agent has learned from this user correction.
            let merchant_key = merchant_raw.to_lowercase();
            let user_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM categorizations ca \
                 JOIN transactions t ON t.id = ca.txn_id \
                 WHERE ca.source = 'user' AND lower(t.merchant_raw) = ?1",
                params![merchant_key],
                |r| r.get(0),
            )?;
            let memo = format!(
                "{} → {} · you've set this {}×",
                merchant_raw, category_label, user_count
            );
            crate::repos::agent_memory::upsert_correction(conn, &merchant_key, &memo)?;

            // Propose a rule if none exists yet for this merchant.
            let rule_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM rules WHERE lower(pattern) = lower(?1) AND enabled = 1 LIMIT 1",
                    params![merchant_raw],
                    |_| Ok(true),
                )
                .or_else(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Ok(false),
                    other => Err(other),
                })?;
            if !rule_exists {
                // Generalize a person-to-person transfer descriptor to a
                // `%counterparty%` key so a rule created from e.g. a rent
                // e-transfer matches every future payment to that person (each
                // carries a unique reference number). Normal merchants unchanged.
                proposed_rule = Some(ProposedRule {
                    pattern: crate::categorize::suggested_rule_pattern(&merchant_raw),
                    category_id: category_id.clone(),
                    category_label,
                });
            }
        }
    }

    // Fetch and return updated transaction
    let txn = get_by_id(conn, id)?;
    accounts::recompute_balance_if_linked(conn, &txn.account_id)?;
    Ok((txn, proposed_rule))
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    let txn = get_by_id(conn, id)?;
    conn.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;
    accounts::recompute_balance_if_linked(conn, &txn.account_id)?;
    Ok(())
}

pub fn set_flags(
    conn: &mut Connection,
    id: &str,
    is_reimbursable: bool,
    is_split: bool,
) -> CoreResult<Transaction> {
    conn.execute(
        "UPDATE transactions SET is_reimbursable = ?1, is_split = ?2 WHERE id = ?3",
        params![is_reimbursable as i64, is_split as i64, id],
    )?;
    get_by_id(conn, id)
}

/// Record the user's verdict on whether a transaction is a transfer between
/// their own accounts. The verdict is sticky: `transfer_override` is respected
/// by both `apply_builtin_categorization` and `pair_transfers`, so it survives
/// re-imports and re-categorization runs.
///
/// Marking as a transfer clears the category (transfers are never categorized)
/// and the anomaly flag (moving your own money is not unusual spending).
/// Unmarking also unlinks a paired peer leg on both sides — the peer is then
/// re-evaluated on its own keyword merits, and the next pairing run may match
/// it elsewhere, but never back to this row.
pub fn set_transfer_override(
    conn: &mut Connection,
    id: &str,
    is_transfer: bool,
) -> CoreResult<Transaction> {
    let tx = conn.transaction()?;
    if is_transfer {
        tx.execute(
            "UPDATE transactions SET transfer_override = 1, is_transfer = 1, \
             category_id = NULL, ai_confidence = NULL, ai_explanation = NULL, \
             is_anomaly = 0 \
             WHERE id = ?1",
            params![id],
        )?;
    } else {
        let peer_id: Option<String> = tx.query_row(
            "SELECT transfer_peer_id FROM transactions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        if let Some(peer_id) = &peer_id {
            // Without this leg the peer is no longer pair-proven; keep it
            // flagged only if its own descriptor says transfer (same unilateral
            // logic the categorizer applies).
            let peer_merchant: String = tx.query_row(
                "SELECT merchant_raw FROM transactions WHERE id = ?1",
                params![peer_id],
                |r| r.get(0),
            )?;
            let ctx = crate::categorize::TransferContext::load(&tx)?;
            let peer_flag = crate::categorize::is_transfer(&peer_merchant)
                || ctx.is_self_transfer(&peer_merchant);
            tx.execute(
                "UPDATE transactions SET transfer_peer_id = NULL, is_transfer = ?2 \
                 WHERE id = ?1 AND transfer_override IS NULL",
                params![peer_id, peer_flag as i64],
            )?;
            // A peer the user already ruled on keeps its verdict; only the link goes.
            tx.execute(
                "UPDATE transactions SET transfer_peer_id = NULL \
                 WHERE id = ?1 AND transfer_override IS NOT NULL",
                params![peer_id],
            )?;
        }
        tx.execute(
            "UPDATE transactions SET transfer_override = 0, is_transfer = 0, \
             transfer_peer_id = NULL \
             WHERE id = ?1",
            params![id],
        )?;
    }
    tx.commit()?;
    get_by_id(conn, id)
}

/// The other UNDECIDED transactions that share this transaction's transfer
/// counterparty, so one verdict can be offered for all of them ("also mark the
/// other 11 e-transfers with swathi"). Returns `(like_pattern, count)` — or
/// `None` when the descriptor has no counterparty to generalize on (a bare
/// "INTERNET TRANSFER <ref>" is unique per row; bulk would be meaningless).
///
/// "Undecided" mirrors the transfer-review surface: no user verdict, not
/// paired, not flagged... and not categorized — a categorized sibling was
/// already ruled real spending by the user or a rule.
pub fn transfer_verdict_siblings(
    conn: &mut Connection,
    id: &str,
) -> CoreResult<Option<(String, i64)>> {
    let merchant: String = conn.query_row(
        "SELECT merchant_raw FROM transactions WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    let pattern = crate::categorize::suggested_rule_pattern(&merchant);
    // Only a GENERALIZED pattern (`%counterparty%`) identifies siblings; the
    // raw string only ever matches itself (unique reference numbers).
    if !pattern.starts_with('%') {
        return Ok(None);
    }
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions \
         WHERE id != ?1 AND lower(merchant_raw) LIKE ?2 \
           AND transfer_override IS NULL AND transfer_peer_id IS NULL \
           AND category_id IS NULL",
        params![id, pattern],
        |r| r.get(0),
    )?;
    if count == 0 {
        return Ok(None);
    }
    Ok(Some((pattern, count)))
}

/// The three treatments a user can rule a transfer-review counterparty as.
///
/// - `Transfer`: money moving between the user's own accounts — never
///   categorized, never an anomaly (delegates to [`set_transfer_override`]).
/// - `SettleUp`: real spending that gets netted against a person (e.g. rent
///   split, dinner IOU) — decided, but not a transfer; leaves the undecided
///   queue via `transfer_override = 0`.
/// - `Real`: decided, ordinary spending — not a transfer, not settled up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Transfer,
    SettleUp,
    Real,
}

/// Record the user's 3-way verdict on a transfer-review counterparty.
/// `Transfer` reuses [`set_transfer_override`]'s existing semantics.
/// `SettleUp` and `Real` both clear `transfer_override` to `0` (decided,
/// not-a-transfer) and any transfer peer link, so either way the row leaves
/// the undecided review queue; `SettleUp` additionally sets `settle_up = 1`
/// so the row nets against the counterparty instead of counting as an
/// ordinary transaction.
pub fn set_counterparty_verdict(
    conn: &mut Connection,
    id: &str,
    verdict: Verdict,
) -> CoreResult<Transaction> {
    match verdict {
        Verdict::Transfer => set_transfer_override(conn, id, true),
        Verdict::SettleUp => {
            conn.execute(
                "UPDATE transactions SET settle_up=1, transfer_override=0, is_transfer=0, \
                 transfer_peer_id=NULL, is_anomaly=0 WHERE id=?1",
                params![id],
            )?;
            get_by_id(conn, id)
        }
        Verdict::Real => {
            conn.execute(
                "UPDATE transactions SET settle_up=0, transfer_override=0, is_transfer=0, \
                 transfer_peer_id=NULL WHERE id=?1",
                params![id],
            )?;
            get_by_id(conn, id)
        }
    }
}

/// Apply one counterparty verdict to every undecided transaction matching a
/// pattern (from [`transfer_verdict_siblings`]). Each row goes through
/// [`set_counterparty_verdict`] so the full per-verdict semantics apply.
/// Returns how many rows were ruled.
pub fn apply_verdict_to_matching(
    conn: &mut Connection,
    pattern: &str,
    verdict: Verdict,
) -> CoreResult<u32> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM transactions \
             WHERE lower(merchant_raw) LIKE ?1 \
               AND transfer_override IS NULL AND transfer_peer_id IS NULL \
               AND category_id IS NULL",
        )?;
        let rows = stmt.query_map(params![pattern], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };
    let mut count = 0u32;
    for id in &ids {
        set_counterparty_verdict(conn, id, verdict)?;
        count += 1;
    }
    Ok(count)
}

/// Apply one transfer verdict to every undecided transaction matching a
/// counterparty pattern. Thin wrapper over [`apply_verdict_to_matching`] kept
/// for the existing binary transfer-review caller.
pub fn apply_transfer_override_to_matching(
    conn: &mut Connection,
    pattern: &str,
    is_transfer: bool,
) -> CoreResult<u32> {
    let verdict = if is_transfer {
        Verdict::Transfer
    } else {
        Verdict::Real
    };
    apply_verdict_to_matching(conn, pattern, verdict)
}

/// Fetch a single transaction by id (used internally).
fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Transaction> {
    conn.query_row(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at, \
                t.is_reimbursable, t.settle_up, t.is_split, t.imported_id, t.source, \
                t.raw_synced_data, t.pending, t.external_tx_id, t.external_account_id, t.is_transfer, \
                t.transfer_peer_id, pa.name, t.owner_member_id, \
                t.activity_type, t.activity_sub_type, t.symbol, t.security_name, t.quantity, t.unit_price \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         LEFT JOIN transactions pt ON pt.id = t.transfer_peer_id \
         LEFT JOIN accounts pa ON pa.id = pt.account_id \
         WHERE t.id = ?1",
        params![id],
        |r| {
            let posted_s: String = r.get(2)?;
            let created_s: String = r.get(17)?;
            Ok(Transaction {
                id: r.get(0)?,
                account_id: r.get(1)?,
                posted_at: DateTime::parse_from_rfc3339(&posted_s)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc),
                amount_cents: r.get(3)?,
                merchant_raw: r.get(4)?,
                merchant_id: r.get(5)?,
                merchant_label: r.get(6)?,
                merchant_color: r.get(7)?,
                merchant_initials: r.get(8)?,
                category_id: r.get(9)?,
                category_label: r.get(10)?,
                category_color: r.get(11)?,
                status: TransactionStatus::from_db(&r.get::<_, String>(12)?),
                notes: r.get(13)?,
                ai_confidence: r.get(14)?,
                ai_explanation: r.get(15)?,
                is_anomaly: r.get::<_, i64>(16)? != 0,
                created_at: DateTime::parse_from_rfc3339(&created_s)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            17,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc),
                is_reimbursable: r.get::<_, i64>(18)? != 0,
                settle_up: r.get::<_, i64>(19)? != 0,
                is_split: r.get::<_, i64>(20)? != 0,
                is_transfer: r.get::<_, i64>(27)? != 0,
                transfer_peer_id: r.get(28)?,
                transfer_peer_account_name: r.get(29)?,
                owner_member_id: r.get(30)?,
                imported_id: r.get(21)?,
                source: r.get(22)?,
                raw_synced_data: r.get(23)?,
                pending: r.get::<_, i64>(24)? != 0,
                external_tx_id: r.get(25)?,
                external_account_id: r.get(26)?,
                activity: read_activity(r, 31)?,
            })
        },
    )
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::run_migrations,
        keychain,
        models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
        repos::accounts,
        Db,
    };
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &mut rusqlite::Connection) -> (String, String) {
        // category
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        // account
        let acc = accounts::insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Ch".into(),
                last4: None,
                currency: "USD".into(),
                color: "#fff".into(),
                opening_balance_cents: 0,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "cash".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                limit_cents: None,
                original_balance_cents: None,
                started_at: None,
            },
        )
        .unwrap();
        // transaction
        let txn = insert(
            conn,
            NewTransaction {
                account_id: acc.id.clone(),
                posted_at: chrono::Utc::now(),
                amount_cents: 1000,
                merchant_raw: "AMAZON".to_string(),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
                activity: None,
            },
        )
        .unwrap();
        (acc.id, txn.id)
    }

    #[test]
    fn update_transaction_notes() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch {
            notes: Some(Some("edited".into())),
            ..Default::default()
        };
        let (updated, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert_eq!(updated.notes.as_deref(), Some("edited"));
        assert!(rule.is_none()); // no category change → no rule proposal
    }

    #[test]
    fn update_category_appends_categorization_and_proposes_rule() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch {
            category_id: Some(Some("cat1".into())),
            ..Default::default()
        };
        let (updated, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert_eq!(updated.category_id.as_deref(), Some("cat1"));
        // Rule proposed because no existing rule for "AMAZON"
        assert!(rule.is_some());
        let r = rule.unwrap();
        assert_eq!(r.pattern, "AMAZON");
        assert_eq!(r.category_id, "cat1");
    }

    #[test]
    fn update_category_no_rule_when_rule_exists() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        // Pre-create a matching rule
        conn.execute(
            "INSERT INTO rules(id,pattern,category_id,enabled,source,created_at) \
             VALUES('r1','AMAZON','cat1',1,'user','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        let patch = TxnPatch {
            category_id: Some(Some("cat1".into())),
            ..Default::default()
        };
        let (_, rule) = update(&mut conn, &txn_id, patch).unwrap();
        assert!(rule.is_none()); // rule already exists → no proposal
    }

    #[test]
    fn delete_transaction_removes_row() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        delete(&mut conn, &txn_id).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions WHERE id = ?1",
                rusqlite::params![txn_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn set_flags_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let t = set_flags(&mut conn, &txn_id, true, true).unwrap();
        assert!(t.is_reimbursable);
        assert!(t.is_split);
        let cleared = set_flags(&mut conn, &txn_id, false, true).unwrap();
        assert!(!cleared.is_reimbursable);
        assert!(cleared.is_split);
    }

    #[test]
    fn transfer_override_mark_clears_category_and_anomaly_and_survives_recategorization() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        // Simulate an ambiguous e-transfer the pipeline mis-treated as income:
        // categorized and anomaly-flagged. (Keyword pass says NOT a transfer.)
        conn.execute(
            "UPDATE transactions SET merchant_raw = 'INTERAC e-Transfer From: SATHVIK', \
             category_id = 'cat1', is_anomaly = 1 WHERE id = ?1",
            params![txn_id],
        )
        .unwrap();

        let t = set_transfer_override(&mut conn, &txn_id, true).unwrap();
        assert!(t.is_transfer, "user verdict flags the row");
        assert!(t.category_id.is_none(), "transfers are never categorized");
        assert!(!t.is_anomaly, "own money movement is not an anomaly");

        // A later categorizer re-run (e.g. after the next import) must not
        // overturn the user's verdict even though the keyword pass disagrees.
        crate::categorize::apply_builtin_categorization(&mut conn).unwrap();
        let (is_tf, cat): (i64, Option<String>) = conn
            .query_row(
                "SELECT is_transfer, category_id FROM transactions WHERE id = ?1",
                params![txn_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(is_tf, 1, "override survives recategorization");
        assert!(cat.is_none(), "override keeps the row uncategorized");
    }

    #[test]
    fn transfer_override_unmark_unlinks_peer_and_survives_reruns() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES\
             ('chk','You','CIBC','Checking','Chq','CAD','#111','manual',datetime('now')),\
             ('sav','You','CIBC','Savings','Sav','CAD','#222','manual',datetime('now'))",
            [],
        )
        .unwrap();
        // Two legs pair via their shared reference number...
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('a','chk','2026-05-01T12:00:00Z',-20000,'Internet Banking INTERNET TRANSFER 000000238417','cleared','2026-05-01T12:00:00Z'),\
             ('b','sav','2026-05-01T12:00:00Z', 20000,'Internet Banking INTERNET TRANSFER 000000238417','cleared','2026-05-01T12:00:00Z')",
            [],
        )
        .unwrap();
        assert_eq!(crate::categorize::pair_transfers(&mut conn).unwrap(), 1);

        // ...but the user says leg 'a' is real spending.
        let t = set_transfer_override(&mut conn, "a", false).unwrap();
        assert!(!t.is_transfer);
        assert!(t.transfer_peer_id.is_none(), "peer link removed on this side");
        let (peer_tf, peer_link): (i64, Option<String>) = conn
            .query_row(
                "SELECT is_transfer, transfer_peer_id FROM transactions WHERE id = 'b'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(peer_link.is_none(), "peer link removed on the other side too");
        assert_eq!(peer_tf, 0, "bare ref-only peer is not a transfer on its own merits");

        // Neither the pairing pass nor the categorizer may resurrect the pair.
        assert_eq!(
            crate::categorize::pair_transfers(&mut conn).unwrap(),
            0,
            "a user-declared non-transfer never re-pairs"
        );
        crate::categorize::apply_builtin_categorization(&mut conn).unwrap();
        let is_tf: i64 = conn
            .query_row("SELECT is_transfer FROM transactions WHERE id = 'a'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(is_tf, 0, "override survives the categorizer re-run");
    }

    #[test]
    fn transfer_override_not_transfer_beats_transfer_keywords() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        // A descriptor the keyword pass unilaterally flags…
        conn.execute(
            "UPDATE transactions SET merchant_raw = 'Internet Withdrawal to Tangerine' WHERE id = ?1",
            params![txn_id],
        )
        .unwrap();
        crate::categorize::apply_builtin_categorization(&mut conn).unwrap();
        let is_tf: i64 = conn
            .query_row("SELECT is_transfer FROM transactions WHERE id = ?1", params![txn_id], |r| r.get(0))
            .unwrap();
        assert_eq!(is_tf, 1, "precondition: keyword pass flags it");

        // …the user overrules, and the verdict sticks through a re-run.
        set_transfer_override(&mut conn, &txn_id, false).unwrap();
        crate::categorize::apply_builtin_categorization(&mut conn).unwrap();
        let is_tf: i64 = conn
            .query_row("SELECT is_transfer FROM transactions WHERE id = ?1", params![txn_id], |r| r.get(0))
            .unwrap();
        assert_eq!(is_tf, 0, "user's NOT-a-transfer verdict beats the keyword pass");
    }

    #[test]
    fn transfer_review_preset_lists_only_undecided_transfer_like_rows() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (acc_id, txn_id) = seed(&mut conn);
        // The seeded AMAZON row is not transfer-like. Add: an undecided bare
        // internet transfer, a person e-transfer, an already-flagged transfer,
        // and a user-ruled (override=0) e-transfer.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,is_transfer,transfer_override) VALUES\
             ('rv1',?1,'2026-05-01T12:00:00Z',-200000,'Internet Banking INTERNET TRANSFER 000000135957','cleared','2026-05-01T12:00:00Z',0,NULL),\
             ('rv2',?1,'2026-05-02T12:00:00Z', 150000,'INTERAC e-Transfer From: SATHVIK','cleared','2026-05-02T12:00:00Z',0,NULL),\
             ('rv3',?1,'2026-05-03T12:00:00Z',-50000,'Internet Withdrawal to Tangerine','cleared','2026-05-03T12:00:00Z',1,NULL),\
             ('rv4',?1,'2026-05-04T12:00:00Z', 90000,'INTERAC e-Transfer From: swathi','cleared','2026-05-04T12:00:00Z',0,0)",
            params![acc_id],
        )
        .unwrap();

        let rows = list(
            &mut conn,
            TxnFilter {
                filter_preset: Some("transfer_review".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let ids: Vec<&str> = rows.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"rv1"), "bare internet transfer needs review");
        assert!(ids.contains(&"rv2"), "person e-transfer counted as income needs review");
        assert!(!ids.contains(&"rv3"), "already-flagged transfers are decided");
        assert!(!ids.contains(&"rv4"), "user-ruled rows never reappear");
        assert!(!ids.contains(&txn_id.as_str()), "ordinary merchants are not suspects");
    }

    #[test]
    fn bulk_transfer_verdict_covers_a_counterparty_and_only_undecided_rows() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (acc_id, _) = seed(&mut conn);
        // Eleven months of rent-like e-transfers to the same person, each with
        // a unique reference number, plus one already-categorized and one
        // already-ruled — those two must be left alone. And an unrelated bare
        // internal transfer that must never ride a counterparty verdict.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,is_transfer,transfer_override,category_id) VALUES\
             ('s1',?1,'2026-01-01T12:00:00Z',-300000,'Internet Banking E-TRANSFER 105152493591 Swathi','cleared','2026-01-01T12:00:00Z',0,NULL,NULL),\
             ('s2',?1,'2026-02-01T12:00:00Z',-300000,'Internet Banking E-TRANSFER 105249142383 SWATHI','cleared','2026-02-01T12:00:00Z',0,NULL,NULL),\
             ('s3',?1,'2026-03-01T12:00:00Z', 300000,'Internet Banking E-TRANSFER 011654884429 swathi','cleared','2026-03-01T12:00:00Z',0,NULL,NULL),\
             ('s4',?1,'2026-04-01T12:00:00Z',-300000,'Internet Banking E-TRANSFER 105583684812 Swathi','cleared','2026-04-01T12:00:00Z',0,NULL,'cat1'),\
             ('s5',?1,'2026-05-01T12:00:00Z',-300000,'Internet Banking E-TRANSFER 105588077665 Swathi','cleared','2026-05-01T12:00:00Z',0,0,NULL),\
             ('u1',?1,'2026-05-02T12:00:00Z',-200000,'Internet Banking INTERNET TRANSFER 000000135957','cleared','2026-05-02T12:00:00Z',0,NULL,NULL)",
            params![acc_id],
        )
        .unwrap();

        // The offer: ruling s1 finds the two other undecided swathi rows.
        let siblings = transfer_verdict_siblings(&mut conn, "s1").unwrap();
        let (pattern, n) = siblings.expect("a person e-transfer generalizes");
        assert_eq!(pattern, "%swathi%");
        assert_eq!(n, 2, "s2+s3 are undecided; s4 categorized, s5 ruled — excluded");

        // A bare internal transfer has no counterparty — no bulk offer.
        assert!(transfer_verdict_siblings(&mut conn, "u1").unwrap().is_none());

        // Apply the verdict to the whole counterparty.
        let applied = apply_transfer_override_to_matching(&mut conn, &pattern, true).unwrap();
        assert_eq!(applied, 3, "s1, s2, s3 ruled in one decision");
        let (flags, overrides): (i64, i64) = conn
            .query_row(
                "SELECT SUM(is_transfer), SUM(transfer_override IS NOT NULL) \
                 FROM transactions WHERE id IN ('s1','s2','s3')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((flags, overrides), (3, 3), "all three flagged with a sticky verdict");
        let s4_touched: (i64, Option<String>) = conn
            .query_row(
                "SELECT is_transfer, category_id FROM transactions WHERE id = 's4'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            s4_touched,
            (0, Some("cat1".into())),
            "the categorized sibling keeps its category and stays real spending"
        );
        let s5_override: i64 = conn
            .query_row("SELECT transfer_override FROM transactions WHERE id = 's5'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(s5_override, 0, "an existing verdict is never overwritten by bulk");
    }

    #[test]
    fn user_category_change_records_agent_memory() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (_, txn_id) = seed(&mut conn);
        let patch = TxnPatch {
            category_id: Some(Some("cat1".into())),
            ..Default::default()
        };
        update(&mut conn, &txn_id, patch).unwrap();
        let mem = crate::repos::agent_memory::list(&mut conn).unwrap();
        assert_eq!(mem.len(), 1);
        assert_eq!(mem[0].kind, "correction");
        assert!(mem[0].description.contains("AMAZON"));
        assert!(mem[0].description.contains("Food"));
        // Pins the insert-before-count ordering: the just-inserted user
        // categorization must be included, so the tally reads 1×, not 0×.
        assert!(mem[0].description.contains("1×"));
    }

    #[test]
    fn settle_up_verdict_marks_and_leaves_the_undecided_queue() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (acc_id, _) = seed(&mut conn);
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,is_transfer,transfer_override) VALUES\
             ('su1',?1,'2026-05-01T12:00:00Z',-50000,'e-transfer joe','cleared','2026-05-01T12:00:00Z',0,NULL)",
            params![acc_id],
        )
        .unwrap();

        let t = set_counterparty_verdict(&mut conn, "su1", Verdict::SettleUp).unwrap();
        assert!(t.settle_up, "settle-up verdict marks the row settled");
        assert!(!t.is_transfer, "settle-up is real spending, netted — not a transfer");

        let still_undecided: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions \
                 WHERE id = 'su1' AND transfer_override IS NULL AND settle_up = 0 \
                   AND category_id IS NULL AND transfer_peer_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_undecided, 0, "settle-up leaves the undecided queue");
    }

    #[test]
    fn real_verdict_marks_decided_not_transfer() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (acc_id, _) = seed(&mut conn);
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,is_transfer,transfer_override) VALUES\
             ('rl1',?1,'2026-05-01T12:00:00Z',-50000,'e-transfer joe','cleared','2026-05-01T12:00:00Z',0,NULL)",
            params![acc_id],
        )
        .unwrap();

        let t = set_counterparty_verdict(&mut conn, "rl1", Verdict::Real).unwrap();
        assert!(!t.settle_up, "real verdict is not settled-up");
        assert!(!t.is_transfer, "real verdict is not a transfer");

        let override_val: i64 = conn
            .query_row(
                "SELECT transfer_override FROM transactions WHERE id = 'rl1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(override_val, 0, "real is a decided (non-transfer) verdict");

        let still_undecided: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions \
                 WHERE id = 'rl1' AND transfer_override IS NULL AND settle_up = 0 \
                   AND category_id IS NULL AND transfer_peer_id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_undecided, 0, "real leaves the undecided queue");
    }

    #[test]
    fn transfer_verdict_still_flags() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (acc_id, _) = seed(&mut conn);
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,is_transfer,transfer_override) VALUES\
             ('tf1',?1,'2026-05-01T12:00:00Z',-50000,'e-transfer joe','cleared','2026-05-01T12:00:00Z',0,NULL)",
            params![acc_id],
        )
        .unwrap();

        let t = set_counterparty_verdict(&mut conn, "tf1", Verdict::Transfer).unwrap();
        assert!(t.is_transfer, "transfer verdict delegates to the existing arm");
    }

    #[test]
    fn apply_verdict_to_matching_settle_up_covers_a_counterparty() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let (acc_id, _) = seed(&mut conn);
        // Two undecided "joe" e-transfers, plus one already categorized (decided).
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,is_transfer,transfer_override,category_id) VALUES\
             ('j1',?1,'2026-01-01T12:00:00Z',-50000,'e-transfer joe 001','cleared','2026-01-01T12:00:00Z',0,NULL,NULL),\
             ('j2',?1,'2026-02-01T12:00:00Z',-50000,'e-transfer joe 002','cleared','2026-02-01T12:00:00Z',0,NULL,NULL),\
             ('j3',?1,'2026-03-01T12:00:00Z',-50000,'e-transfer joe 003','cleared','2026-03-01T12:00:00Z',0,NULL,'cat1')",
            params![acc_id],
        )
        .unwrap();

        let applied =
            apply_verdict_to_matching(&mut conn, "%joe%", Verdict::SettleUp).unwrap();
        assert_eq!(applied, 2, "only the two undecided rows are ruled");

        let (j1_settled, j2_settled): (i64, i64) = conn
            .query_row(
                "SELECT (SELECT settle_up FROM transactions WHERE id='j1'), \
                        (SELECT settle_up FROM transactions WHERE id='j2')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((j1_settled, j2_settled), (1, 1), "both undecided rows settled");

        let j3_untouched: (i64, Option<String>) = conn
            .query_row(
                "SELECT settle_up, category_id FROM transactions WHERE id = 'j3'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            j3_untouched,
            (0, Some("cat1".into())),
            "the categorized sibling is left alone"
        );
    }
}
