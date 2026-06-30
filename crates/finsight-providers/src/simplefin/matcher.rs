use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};
use std::collections::HashSet;

use crate::error::{ProviderError, ProviderResult};
use finsight_core::error::CoreResult;
use finsight_core::models::{NewTransaction, Transaction, TransactionStatus};

const AMOUNT_TOLERANCE_CENTS: i64 = 100;
const AUTO_MATCH_SCORE: i64 = 85;
const REVIEW_MATCH_SCORE: i64 = 35;

/// Result of matching an imported transaction against the local ledger.
pub enum MatchResult {
    /// Exact match by imported_id.
    Exact(Transaction),
    /// Fuzzy match by amount + date window + merchant.
    Fuzzy(Transaction),
    /// No match found.
    None,
}

#[derive(Debug, Clone)]
pub struct PotentialMatch {
    pub transaction: Transaction,
    pub match_kind: String,
    pub score: i64,
    pub is_recommended: bool,
    pub explanation_json: Option<String>,
}

pub enum ReconciliationDecision {
    AutoMatch(Transaction),
    NeedsReview {
        matches: Vec<PotentialMatch>,
        confidence: i64,
        reason: String,
    },
    None,
}

/// Find the best existing transaction for a candidate imported transaction.
///
/// Matching hierarchy:
/// 1. Exact `imported_id` match (highest fidelity).
/// 2. Fuzzy match on (account_id, amount_cents, posted_at within ±window_days, merchant_raw).
/// 3. No match → insert as new.
pub fn find_match(
    conn: &Connection,
    account_id: &str,
    candidate: &NewTransaction,
    imported_id: Option<&str>,
    window_days: i64,
) -> ProviderResult<MatchResult> {
    find_match_excluding(
        conn,
        account_id,
        candidate,
        imported_id,
        window_days,
        &HashSet::new(),
    )
}

pub fn find_match_excluding(
    conn: &Connection,
    account_id: &str,
    candidate: &NewTransaction,
    imported_id: Option<&str>,
    window_days: i64,
    excluded_fuzzy_ids: &HashSet<String>,
) -> ProviderResult<MatchResult> {
    // 1. Exact imported_id match.
    if let Some(id) = imported_id {
        if let Some(txn) = find_by_imported_id(conn, account_id, id).map_err(ProviderError::Core)? {
            return Ok(MatchResult::Exact(txn));
        }
    }

    // 2. Fuzzy match.
    if let Some(txn) = find_fuzzy_match(
        conn,
        account_id,
        candidate.amount_cents,
        candidate.posted_at,
        &candidate.merchant_raw,
        window_days,
        excluded_fuzzy_ids,
    )
    .map_err(ProviderError::Core)?
    {
        return Ok(MatchResult::Fuzzy(txn));
    }

    Ok(MatchResult::None)
}

pub fn reconcile_excluding(
    conn: &Connection,
    account_id: &str,
    candidate: &NewTransaction,
    imported_id: Option<&str>,
    window_days: i64,
    excluded_fuzzy_ids: &HashSet<String>,
) -> ProviderResult<ReconciliationDecision> {
    if let Some(id) = imported_id {
        if let Some(txn) = find_by_imported_id(conn, account_id, id).map_err(ProviderError::Core)? {
            return Ok(ReconciliationDecision::AutoMatch(txn));
        }
    }

    if candidate.pending {
        // Incoming pending transactions can only safely enrich exact ids. They
        // should not fuzzy-match posted user ledger rows automatically.
        return Ok(ReconciliationDecision::None);
    }

    if let Some(external_tx_id) = candidate.external_tx_id.as_deref() {
        if let Some(txn) = find_pending_provider_match(conn, account_id, external_tx_id)
            .map_err(ProviderError::Core)?
        {
            return Ok(ReconciliationDecision::AutoMatch(txn));
        }
    }

    let mut matches = find_fuzzy_candidates(
        conn,
        account_id,
        candidate.amount_cents,
        candidate.posted_at,
        &candidate.merchant_raw,
        window_days,
    )
    .map_err(ProviderError::Core)?;

    if matches.is_empty() {
        return Ok(ReconciliationDecision::None);
    }

    matches.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then(a.transaction.id.cmp(&b.transaction.id))
    });
    for (idx, m) in matches.iter_mut().enumerate() {
        m.is_recommended = idx == 0;
    }

    let best = matches[0].clone();
    if excluded_fuzzy_ids.contains(&best.transaction.id) {
        return Ok(ReconciliationDecision::NeedsReview {
            confidence: best.score,
            matches,
            reason: "Possible duplicate collides with another row in this batch".to_string(),
        });
    }

    let ambiguous = matches
        .iter()
        .skip(1)
        .any(|m| best.score.saturating_sub(m.score) <= 10);

    if best.score >= AUTO_MATCH_SCORE && !ambiguous {
        Ok(ReconciliationDecision::AutoMatch(best.transaction))
    } else if best.score >= REVIEW_MATCH_SCORE {
        Ok(ReconciliationDecision::NeedsReview {
            confidence: best.score,
            matches,
            reason: if ambiguous {
                "Multiple plausible existing transactions need review".to_string()
            } else {
                "Possible match is below automatic confidence threshold".to_string()
            },
        })
    } else {
        Ok(ReconciliationDecision::None)
    }
}

fn find_by_imported_id(
    conn: &Connection,
    account_id: &str,
    imported_id: &str,
) -> CoreResult<Option<Transaction>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at, \
                t.is_reimbursable, t.is_split, t.imported_id, t.source, \
                t.raw_synced_data, t.pending, t.external_tx_id, t.external_account_id \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE t.account_id = ?1 AND t.imported_id = ?2 \
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![account_id, imported_id], map_transaction_row)?;
    Ok(rows.next().transpose()?)
}

fn find_fuzzy_match(
    conn: &Connection,
    account_id: &str,
    amount_cents: i64,
    posted_at: DateTime<Utc>,
    merchant_raw: &str,
    window_days: i64,
    excluded_ids: &HashSet<String>,
) -> CoreResult<Option<Transaction>> {
    let matches = find_fuzzy_candidates(
        conn,
        account_id,
        amount_cents,
        posted_at,
        merchant_raw,
        window_days,
    )?;
    Ok(matches
        .into_iter()
        .filter(|m| !excluded_ids.contains(&m.transaction.id))
        .max_by_key(|m| m.score)
        .filter(|m| m.score > 0)
        .map(|m| m.transaction))
}

fn find_fuzzy_candidates(
    conn: &Connection,
    account_id: &str,
    amount_cents: i64,
    posted_at: DateTime<Utc>,
    merchant_raw: &str,
    window_days: i64,
) -> CoreResult<Vec<PotentialMatch>> {
    let start = (posted_at - Duration::days(window_days)).to_rfc3339();
    let end = (posted_at + Duration::days(window_days)).to_rfc3339();
    let merchant_lower = merchant_raw.to_lowercase();
    let min_amount = amount_cents - AMOUNT_TOLERANCE_CENTS;
    let max_amount = amount_cents + AMOUNT_TOLERANCE_CENTS;

    let mut stmt = conn.prepare(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at, \
                t.is_reimbursable, t.is_split, t.imported_id, t.source, \
                t.raw_synced_data, t.pending, t.external_tx_id, t.external_account_id \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE t.account_id = ?1 \
           AND t.amount_cents BETWEEN ?2 AND ?3 \
           AND t.posted_at >= ?4 AND t.posted_at <= ?5 \
         ORDER BY t.posted_at DESC",
    )?;
    let rows = stmt.query_map(
        params![account_id, min_amount, max_amount, start, end],
        map_transaction_row,
    )?;

    let mut out = Vec::new();
    for row in rows {
        let txn = row?;
        let score = fuzzy_score_amount(&txn, amount_cents, posted_at, &merchant_lower);
        if score > 0 {
            out.push(PotentialMatch {
                transaction: txn.clone(),
                match_kind: if txn.amount_cents == amount_cents {
                    "fuzzy".to_string()
                } else {
                    "amount_tolerance".to_string()
                },
                score,
                is_recommended: false,
                explanation_json: Some(format!(
                    r#"{{"amount_delta_cents":{},"posted_days_delta":{}}}"#,
                    txn.amount_cents - amount_cents,
                    (txn.posted_at - posted_at).num_days()
                )),
            });
        }
    }
    Ok(out)
}

fn find_pending_provider_match(
    conn: &Connection,
    account_id: &str,
    external_tx_id: &str,
) -> CoreResult<Option<Transaction>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.account_id, t.posted_at, t.amount_cents, t.merchant_raw, \
                t.merchant_id, m.canonical_name, m.color, m.initials, \
                t.category_id, c.label, c.color, t.status, t.notes, \
                t.ai_confidence, t.ai_explanation, t.is_anomaly, t.created_at, \
                t.is_reimbursable, t.is_split, t.imported_id, t.source, \
                t.raw_synced_data, t.pending, t.external_tx_id, t.external_account_id \
         FROM transactions t \
         LEFT JOIN merchants m ON m.id = t.merchant_id \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE t.account_id = ?1 AND t.external_tx_id = ?2 AND t.pending = 1 \
         LIMIT 1",
    )?;
    let mut rows = stmt.query_map(params![account_id, external_tx_id], map_transaction_row)?;
    Ok(rows.next().transpose()?)
}

fn fuzzy_score_amount(
    txn: &Transaction,
    candidate_amount_cents: i64,
    posted_at: DateTime<Utc>,
    merchant_lower: &str,
) -> i64 {
    let mut score = 0i64;

    let amount_delta = (txn.amount_cents - candidate_amount_cents).abs();
    if amount_delta == 0 {
        score += 35;
    } else if amount_delta <= AMOUNT_TOLERANCE_CENTS {
        score += 20 - (amount_delta / 10).min(20);
    } else {
        return 0;
    }

    // Date proximity: within 1 day is best, within window gets partial.
    let days_diff = (txn.posted_at - posted_at).num_days().abs();
    score += (7 - days_diff.min(7)) * 7;

    // Merchant similarity.
    let txn_merchant = txn.merchant_raw.to_lowercase();
    if txn_merchant == merchant_lower {
        score += 35;
    } else if txn_merchant.contains(merchant_lower) || merchant_lower.contains(&txn_merchant) {
        score += 25;
    } else {
        // Jaccard-ish word overlap.
        let words1: std::collections::HashSet<&str> = merchant_lower.split_whitespace().collect();
        let words2: std::collections::HashSet<&str> = txn_merchant.split_whitespace().collect();
        if !words1.is_empty() {
            let overlap = words1.intersection(&words2).count();
            score += (overlap * 8) as i64;
        }
    }

    // Prefer matching transactions that are not already reconciled/locked.
    if txn.status != TransactionStatus::Manual {
        score += 5;
    }

    score
}

fn map_transaction_row(r: &rusqlite::Row) -> rusqlite::Result<Transaction> {
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
        is_split: r.get::<_, i64>(19)? != 0,
        imported_id: r.get(20)?,
        source: r.get(21)?,
        raw_synced_data: r.get(22)?,
        pending: r.get::<_, i64>(23)? != 0,
        external_tx_id: r.get(24)?,
        external_account_id: r.get(25)?,
    })
}
