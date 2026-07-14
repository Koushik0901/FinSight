//! Read-only anticipatory fold: parse + reconcile a CSV into an ordered plan
//! WITHOUT any DB mutation. Shares reconcile logic with the write path so
//! decisions are identical by construction.
use crate::csv::encoding::decode_layered;
use crate::csv::mapping::CsvImportMapping;
use crate::csv::parse::{into_new_transaction, parse_row};
use crate::csv::{detect_delimiter, read_capped, CsvProvider, RowError};
use crate::error::{ProviderError, ProviderResult};
use crate::simplefin::matcher::{reconcile_excluding_batch, PotentialMatch, ReconciliationDecision};
use finsight_core::models::NewTransaction;
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum PreparedDecision {
    Insert {
        new_id: String,
        tx: NewTransaction,
    },
    Duplicate {
        existing_id: String,
    },
    Review {
        candidate: NewTransaction,
        matches: Vec<PotentialMatch>,
        confidence: i64,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct PreparedRow {
    pub row_number: u32,
    pub decision: PreparedDecision,
}

#[derive(Debug, Clone)]
pub struct PreparedImport {
    pub signature: String,
    pub delimiter: char,
    pub rows_imported: u32,
    pub rows_skipped_duplicates: u32,
    pub rows_queued_for_review: u32,
    pub errors: Vec<RowError>,
    pub rows: Vec<PreparedRow>,
}

/// Per-account ledger fingerprint: a cheap staleness signal for a prepared plan.
pub fn ledger_fingerprint(conn: &Connection, account_id: &str) -> ProviderResult<String> {
    let (count, max_created): (i64, Option<String>) = conn
        .query_row(
            "SELECT COUNT(*), MAX(created_at) FROM transactions WHERE account_id = ?1",
            [account_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| ProviderError::Internal(format!("fingerprint: {e}")))?;
    Ok(format!("{count}:{}", max_created.unwrap_or_default()))
}

/// Verbatim source row as JSON: an object keyed by header when the file has
/// one, else a positional array. Preserves columns without a mapped role
/// (commission, settlement_date, currency, …) for future features.
fn raw_row_json(headers: Option<&[String]>, fields: &[&str]) -> String {
    match headers {
        Some(h) if h.len() == fields.len() => {
            let obj: serde_json::Map<String, serde_json::Value> = h
                .iter()
                .zip(fields.iter())
                .map(|(k, v)| (k.clone(), serde_json::Value::String((*v).to_string())))
                .collect();
            serde_json::Value::Object(obj).to_string()
        }
        _ => serde_json::Value::Array(
            fields
                .iter()
                .map(|v| serde_json::Value::String((*v).to_string()))
                .collect(),
        )
        .to_string(),
    }
}

fn mapping_signature(m: &CsvImportMapping) -> String {
    format!(
        "{:?}|{}|{:?}|{}|{}|{:?}",
        m.columns, m.date_format, m.amount_convention, m.decimal_separator, m.skip_header_rows, m.delimiter
    )
}

impl CsvProvider {
    /// Read-only: parse + reconcile into an ordered plan. No writes at all.
    pub fn prepare(
        path: &Path,
        account_id: &str,
        mapping: &CsvImportMapping,
        conn: &Connection,
    ) -> ProviderResult<PreparedImport> {
        let bytes = read_capped(path)?;
        if bytes.is_empty() {
            return Err(ProviderError::EmptyFile);
        }
        let (text, _) = decode_layered(&bytes)?;
        let delimiter = mapping.delimiter.unwrap_or_else(|| detect_delimiter(&text));

        let meta = std::fs::metadata(path)?;
        let signature = format!(
            "{}|{}|{}|{}",
            account_id,
            mapping_signature(mapping),
            meta.len(),
            ledger_fingerprint(conn, account_id)?
        );

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter as u8)
            .flexible(true)
            .from_reader(text.as_bytes());

        let mut out = PreparedImport {
            signature,
            delimiter,
            rows_imported: 0,
            rows_skipped_duplicates: 0,
            rows_queued_for_review: 0,
            errors: Vec::new(),
            rows: Vec::new(),
        };
        let mut matched_existing_ids: HashSet<String> = HashSet::new();
        let mut self_import_ids: HashSet<String> = HashSet::new();
        // Header names from the first skipped row, used to key each row's
        // verbatim raw JSON (mirrors the SimpleFin raw_synced_data pattern).
        let mut headers: Option<Vec<String>> = None;

        for (idx, rec) in reader.records().enumerate() {
            let row_number = (idx + 1) as u32;
            let rec = match rec {
                Ok(r) => r,
                Err(e) => {
                    out.errors.push(RowError {
                        row_number,
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            if idx < mapping.skip_header_rows as usize {
                if idx == 0 {
                    headers = Some(rec.iter().map(|h| h.trim().to_string()).collect());
                }
                continue;
            }
            let fields: Vec<&str> = rec.iter().collect();
            let parsed = match parse_row(&fields, mapping) {
                Ok(p) => p,
                Err(e) => {
                    out.errors.push(RowError {
                        row_number,
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            let raw_json = raw_row_json(headers.as_deref(), &fields);
            let new_tx = into_new_transaction(parsed, account_id.to_string(), Some(raw_json));
            match reconcile_excluding_batch(
                conn,
                account_id,
                &new_tx,
                None,
                7,
                &matched_existing_ids,
                &self_import_ids,
            )? {
                ReconciliationDecision::AutoMatch(existing) => {
                    matched_existing_ids.insert(existing.id.clone());
                    out.rows_skipped_duplicates += 1;
                    out.rows.push(PreparedRow {
                        row_number,
                        decision: PreparedDecision::Duplicate {
                            existing_id: existing.id,
                        },
                    });
                }
                ReconciliationDecision::NeedsReview {
                    matches,
                    confidence,
                    reason,
                } => {
                    out.rows_queued_for_review += 1;
                    out.rows.push(PreparedRow {
                        row_number,
                        decision: PreparedDecision::Review {
                            candidate: new_tx,
                            matches,
                            confidence,
                            reason,
                        },
                    });
                }
                ReconciliationDecision::None => {
                    let new_id = Uuid::new_v4().to_string();
                    self_import_ids.insert(new_id.clone());
                    out.rows_imported += 1;
                    out.rows.push(PreparedRow {
                        row_number,
                        decision: PreparedDecision::Insert { new_id, tx: new_tx },
                    });
                }
            }
        }
        Ok(out)
    }
}
