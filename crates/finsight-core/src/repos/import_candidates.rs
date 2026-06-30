use crate::error::CoreResult;
use crate::models::{
    ImportCandidate, ImportCandidateMatch, ImportCandidateWithMatches, NewImportCandidate,
    NewImportCandidateMatch, NewTransaction,
};
use crate::repos::transactions;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn create(
    conn: &Connection,
    input: NewImportCandidate,
    matches: Vec<NewImportCandidateMatch>,
) -> CoreResult<ImportCandidate> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO import_candidates \
         (id, source, import_id, sync_run_id, account_id, candidate_json, raw_payload_json, \
          imported_id, external_tx_id, external_account_id, posted_at, amount_cents, merchant_raw, \
          confidence, reason, status, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 'pending', ?16)",
        params![
            &id,
            &input.source,
            &input.import_id,
            &input.sync_run_id,
            &input.account_id,
            &input.candidate_json,
            &input.raw_payload_json,
            &input.imported_id,
            &input.external_tx_id,
            &input.external_account_id,
            input.posted_at.to_rfc3339(),
            input.amount_cents,
            &input.merchant_raw,
            input.confidence,
            &input.reason,
            now.to_rfc3339(),
        ],
    )?;

    for m in matches {
        conn.execute(
            "INSERT INTO import_candidate_matches \
             (id, candidate_id, transaction_id, match_kind, score, is_recommended, explanation_json, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Uuid::new_v4().to_string(),
                &id,
                &m.transaction_id,
                &m.match_kind,
                m.score,
                m.is_recommended,
                &m.explanation_json,
                now.to_rfc3339(),
            ],
        )?;
    }

    get(conn, &id)
}

pub fn list_pending(conn: &mut Connection) -> CoreResult<Vec<ImportCandidateWithMatches>> {
    let ids = {
        let mut stmt = conn.prepare(
            "SELECT id FROM import_candidates WHERE status = 'pending' ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(ImportCandidateWithMatches {
            candidate: get(conn, &id)?,
            matches: list_matches(conn, &id)?,
        });
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> CoreResult<ImportCandidate> {
    conn.query_row(
        "SELECT id, source, import_id, sync_run_id, account_id, candidate_json, raw_payload_json, \
                imported_id, external_tx_id, external_account_id, posted_at, amount_cents, merchant_raw, \
                confidence, reason, status, resolution, resolved_transaction_id, created_at, resolved_at \
         FROM import_candidates WHERE id = ?1",
        params![id],
        import_candidate_from_row,
    )
    .map_err(Into::into)
}

pub fn list_matches(
    conn: &Connection,
    candidate_id: &str,
) -> CoreResult<Vec<ImportCandidateMatch>> {
    let mut stmt = conn.prepare(
        "SELECT id, candidate_id, transaction_id, match_kind, score, is_recommended, explanation_json, created_at \
         FROM import_candidate_matches WHERE candidate_id = ?1 ORDER BY score DESC, created_at ASC",
    )?;
    let rows = stmt.query_map(params![candidate_id], import_candidate_match_from_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn resolve_with_match(
    conn: &mut Connection,
    candidate_id: &str,
    transaction_id: &str,
) -> CoreResult<()> {
    let candidate = get(conn, candidate_id)?;
    let new_tx: NewTransaction = serde_json::from_str(&candidate.candidate_json)
        .map_err(|e| crate::CoreError::InvalidState(format!("candidate_json parse: {e}")))?;
    apply_candidate_metadata(
        conn,
        transaction_id,
        &new_tx,
        candidate.raw_payload_json.as_deref(),
    )?;
    mark_resolved(conn, candidate_id, "matched", Some(transaction_id))
}

pub fn resolve_create_new(conn: &mut Connection, candidate_id: &str) -> CoreResult<String> {
    let candidate = get(conn, candidate_id)?;
    let mut new_tx: NewTransaction = serde_json::from_str(&candidate.candidate_json)
        .map_err(|e| crate::CoreError::InvalidState(format!("candidate_json parse: {e}")))?;
    if new_tx.raw_synced_data.is_none() {
        new_tx.raw_synced_data = candidate.raw_payload_json.clone();
    }
    let txn = transactions::insert(conn, new_tx)?;
    mark_resolved(conn, candidate_id, "created", Some(&txn.id))?;
    Ok(txn.id)
}

pub fn dismiss(conn: &mut Connection, candidate_id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE import_candidates SET status = 'dismissed', resolution = 'dismissed', resolved_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), candidate_id],
    )?;
    Ok(())
}

pub fn mark_resolved(
    conn: &mut Connection,
    candidate_id: &str,
    resolution: &str,
    transaction_id: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE import_candidates \
         SET status = 'resolved', resolution = ?1, resolved_transaction_id = ?2, resolved_at = ?3 \
         WHERE id = ?4",
        params![
            resolution,
            transaction_id,
            Utc::now().to_rfc3339(),
            candidate_id
        ],
    )?;
    Ok(())
}

pub fn apply_candidate_metadata(
    conn: &mut Connection,
    transaction_id: &str,
    new_tx: &NewTransaction,
    raw_payload_json: Option<&str>,
) -> CoreResult<()> {
    let raw = raw_payload_json
        .map(str::to_string)
        .or_else(|| new_tx.raw_synced_data.clone());
    conn.execute(
        "UPDATE transactions SET \
            posted_at = ?1, \
            amount_cents = ?2, \
            merchant_raw = ?3, \
            status = ?4, \
            imported_id = COALESCE(?5, imported_id), \
            source = COALESCE(?6, source), \
            raw_synced_data = COALESCE(?7, raw_synced_data), \
            pending = ?8, \
            external_tx_id = COALESCE(?9, external_tx_id), \
            external_account_id = COALESCE(?10, external_account_id) \
         WHERE id = ?11",
        params![
            new_tx.posted_at.to_rfc3339(),
            new_tx.amount_cents,
            &new_tx.merchant_raw,
            new_tx.status.as_db(),
            &new_tx.imported_id,
            &new_tx.source,
            &raw,
            new_tx.pending,
            &new_tx.external_tx_id,
            &new_tx.external_account_id,
            transaction_id,
        ],
    )?;
    Ok(())
}

fn import_candidate_from_row(r: &rusqlite::Row) -> rusqlite::Result<ImportCandidate> {
    let posted_at: String = r.get(10)?;
    let created_at: String = r.get(18)?;
    let resolved_at: Option<String> = r.get(19)?;
    Ok(ImportCandidate {
        id: r.get(0)?,
        source: r.get(1)?,
        import_id: r.get(2)?,
        sync_run_id: r.get(3)?,
        account_id: r.get(4)?,
        candidate_json: r.get(5)?,
        raw_payload_json: r.get(6)?,
        imported_id: r.get(7)?,
        external_tx_id: r.get(8)?,
        external_account_id: r.get(9)?,
        posted_at: parse_dt(&posted_at, 10)?,
        amount_cents: r.get(11)?,
        merchant_raw: r.get(12)?,
        confidence: r.get(13)?,
        reason: r.get(14)?,
        status: r.get(15)?,
        resolution: r.get(16)?,
        resolved_transaction_id: r.get(17)?,
        created_at: parse_dt(&created_at, 18)?,
        resolved_at: resolved_at
            .as_deref()
            .map(|s| parse_dt(s, 19))
            .transpose()?,
    })
}

fn import_candidate_match_from_row(r: &rusqlite::Row) -> rusqlite::Result<ImportCandidateMatch> {
    let created_at: String = r.get(7)?;
    Ok(ImportCandidateMatch {
        id: r.get(0)?,
        candidate_id: r.get(1)?,
        transaction_id: r.get(2)?,
        match_kind: r.get(3)?,
        score: r.get(4)?,
        is_recommended: r.get::<_, i64>(5)? != 0,
        explanation_json: r.get(6)?,
        created_at: parse_dt(&created_at, 7)?,
    })
}

fn parse_dt(s: &str, idx: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
        })
}
