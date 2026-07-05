//! CSV ingestion provider — preview, import with dedup, mapping persistence.

pub mod encoding;
pub mod mapping;
pub mod parse;
pub mod prepare;

use crate::csv::encoding::{decode_layered, DetectedEncoding};
use crate::csv::mapping::CsvImportMapping;
use crate::error::{ProviderError, ProviderResult};
use chrono::Utc;
use finsight_core::models::{NewImportCandidate, NewImportCandidateMatch};
use finsight_core::repos::{accounts, import_candidates};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;

const MAX_BYTES: u64 = 50 * 1024 * 1024;
const PREVIEW_ROWS: usize = 10;
const PREVIEW_COUNT_CAP: u32 = 10_000;
const BATCH_SIZE: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CsvPreview {
    pub headers: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
    pub detected_delimiter: char,
    pub total_rows: u32,
    pub encoding_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ImportSummary {
    pub import_id: String,
    pub rows_imported: u32,
    pub rows_skipped_duplicates: u32,
    pub rows_queued_for_review: u32,
    pub errors: Vec<RowError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RowError {
    pub row_number: u32,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct BatchProgress {
    pub rows_done: u32,
    pub rows_total: u32,
}

pub struct CsvProvider;

impl CsvProvider {
    pub fn preview(path: &Path, skip_header_rows: u32) -> ProviderResult<CsvPreview> {
        let bytes = read_capped(path)?;
        if bytes.is_empty() {
            return Err(ProviderError::EmptyFile);
        }
        let (text, encoding) = decode_layered(&bytes)?;
        let delimiter = detect_delimiter(&text);
        let encoding_note = match encoding {
            DetectedEncoding::Windows1252 => {
                Some("Decoded as Windows-1252 (no UTF-8 BOM detected)".into())
            }
            _ => None,
        };

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter as u8)
            .flexible(true)
            .from_reader(text.as_bytes());

        let mut headers: Option<Vec<String>> = None;
        let mut rows: Vec<Vec<String>> = Vec::with_capacity(PREVIEW_ROWS);
        let mut total: u32 = 0;
        let skip = skip_header_rows as usize;

        for (idx, rec) in reader.records().enumerate() {
            let rec = rec?;
            if idx == 0 && skip > 0 {
                headers = Some(rec.iter().map(str::to_owned).collect());
            }
            if idx >= skip {
                total = total.saturating_add(1);
                if rows.len() < PREVIEW_ROWS {
                    rows.push(rec.iter().map(str::to_owned).collect());
                }
                if total >= PREVIEW_COUNT_CAP {
                    break;
                }
            }
        }

        Ok(CsvPreview {
            headers,
            rows,
            detected_delimiter: delimiter,
            total_rows: total,
            encoding_note,
        })
    }

    /// `import_id` must be a pre-generated UUID supplied by the caller so that
    /// progress callbacks can include it in events before the summary is returned.
    pub fn import(
        path: &Path,
        account_id: &str,
        import_id: &str,
        mapping: &CsvImportMapping,
        db: &finsight_core::Db,
        mut on_progress: impl FnMut(BatchProgress),
    ) -> ProviderResult<ImportSummary> {
        let mut conn = db.get().map_err(ProviderError::Core)?;
        let filename = path.file_name().map(|s| s.to_string_lossy().into_owned());
        conn.execute(
            "INSERT INTO imports(id, source, filename, account_id, started_at) \
             VALUES(?1, 'csv', ?2, ?3, ?4)",
            params![import_id, filename, account_id, Utc::now().to_rfc3339()],
        )
        .map_err(|e| ProviderError::Internal(format!("imports insert: {e}")))?;

        // Single read+parse+reconcile pass, producing an ordered decision plan.
        let prepared = Self::prepare(path, account_id, mapping, &conn)?;

        let total = (prepared.rows.len() + prepared.errors.len()) as u32;
        let emit_every = std::cmp::max(1, total / 20) as usize;

        let mut rows_imported: u32 = 0;
        let mut rows_skipped: u32 = 0;
        let mut rows_queued: u32 = 0;
        let mut processed: u32 = 0;
        // in_batch tracks rows in the current open transaction so we can
        // commit every BATCH_SIZE rows without the monotonic `processed` check.
        let mut in_batch: usize = 0;

        let mut tx = conn
            .transaction()
            .map_err(|e| ProviderError::Internal(format!("begin: {e}")))?;

        for row in prepared.rows {
            match row.decision {
                PreparedDecision::Duplicate { .. } => {
                    rows_skipped += 1;
                }
                PreparedDecision::Review {
                    candidate,
                    matches,
                    confidence,
                    reason,
                } => {
                    import_candidates::create(
                        &mut tx,
                        NewImportCandidate {
                            source: "csv".to_string(),
                            import_id: Some(import_id.to_string()),
                            sync_run_id: None,
                            account_id: account_id.to_string(),
                            candidate_json: serde_json::to_string(&candidate).map_err(|e| {
                                ProviderError::Internal(format!("serialize candidate: {e}"))
                            })?,
                            raw_payload_json: None,
                            imported_id: candidate.imported_id.clone(),
                            external_tx_id: candidate.external_tx_id.clone(),
                            external_account_id: candidate.external_account_id.clone(),
                            posted_at: candidate.posted_at,
                            amount_cents: candidate.amount_cents,
                            merchant_raw: candidate.merchant_raw.clone(),
                            confidence,
                            reason,
                        },
                        matches
                            .into_iter()
                            .map(|m| NewImportCandidateMatch {
                                transaction_id: m.transaction.id,
                                match_kind: m.match_kind,
                                score: m.score,
                                is_recommended: m.is_recommended,
                                explanation_json: m.explanation_json,
                            })
                            .collect(),
                    )
                    .map_err(ProviderError::Core)?;
                    rows_queued += 1;
                }
                PreparedDecision::Insert { new_id, tx: new_tx } => {
                    tx.execute(
                        "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                                  category_id, status, notes, created_at, imported_id, source, \
                                                  raw_synced_data, pending, external_tx_id, external_account_id) \
                         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                        params![
                            new_id,
                            &new_tx.account_id,
                            new_tx.posted_at.to_rfc3339(),
                            new_tx.amount_cents,
                            &new_tx.merchant_raw,
                            &new_tx.category_id,
                            new_tx.status.as_db(),
                            &new_tx.notes,
                            Utc::now().to_rfc3339(),
                            &new_tx.imported_id,
                            &new_tx.source,
                            &new_tx.raw_synced_data,
                            new_tx.pending,
                            &new_tx.external_tx_id,
                            &new_tx.external_account_id,
                        ],
                    )
                    .map_err(|e| ProviderError::Internal(format!("insert: {e}")))?;
                    rows_imported += 1;
                }
            }

            processed += 1;
            in_batch += 1;

            let should_emit = emit_every > 0 && (processed as usize).is_multiple_of(emit_every);
            if in_batch >= BATCH_SIZE || should_emit {
                tx.commit()
                    .map_err(|e| ProviderError::Internal(format!("commit batch: {e}")))?;
                on_progress(BatchProgress {
                    rows_done: processed,
                    rows_total: total,
                });
                tx = conn
                    .transaction()
                    .map_err(|e| ProviderError::Internal(format!("begin: {e}")))?;
                in_batch = 0;
            }
        }

        debug_assert_eq!(rows_imported, prepared.rows_imported);
        debug_assert_eq!(rows_skipped, prepared.rows_skipped_duplicates);
        debug_assert_eq!(rows_queued, prepared.rows_queued_for_review);

        // Persist mapping + finalize imports row.
        mapping::save(&tx, account_id, mapping)?;
        tx.execute(
            "UPDATE imports SET finished_at = ?1, rows_imported = ?2, \
                                rows_skipped_duplicates = ?3 WHERE id = ?4",
            params![
                Utc::now().to_rfc3339(),
                rows_imported as i64,
                rows_skipped as i64,
                import_id
            ],
        )
        .map_err(|e| ProviderError::Internal(format!("imports finish: {e}")))?;
        tx.commit()
            .map_err(|e| ProviderError::Internal(format!("commit final: {e}")))?;
        accounts::recompute_balance_if_linked(&mut conn, account_id)
            .map_err(ProviderError::Core)?;

        // Final emit fires after the commit + balance recompute, so import is
        // fully done: report rows_done == total so the bar always reaches 100%,
        // even when some rows errored (errors are counted in `total` but never
        // in `processed`, which would otherwise leave the bar short of 100%).
        on_progress(BatchProgress {
            rows_done: total,
            rows_total: total,
        });

        Ok(ImportSummary {
            import_id: import_id.to_string(),
            rows_imported,
            rows_skipped_duplicates: rows_skipped,
            rows_queued_for_review: rows_queued,
            errors: prepared.errors,
        })
    }
}

pub(crate) fn detect_delimiter(text: &str) -> char {
    let first_line = text.lines().next().unwrap_or("");
    let commas = first_line.matches(',').count();
    let semis = first_line.matches(';').count();
    let tabs = first_line.matches('\t').count();
    if tabs >= commas && tabs >= semis && tabs > 0 {
        '\t'
    } else if semis > commas {
        ';'
    } else {
        ','
    }
}

pub(crate) fn read_capped(path: &Path) -> ProviderResult<Vec<u8>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_BYTES {
        return Err(ProviderError::FileTooLarge {
            bytes: meta.len(),
            cap: MAX_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    use std::io::Read;
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub use mapping::{AmountConvention, ColumnRole};
pub use prepare::{PreparedDecision, PreparedImport, PreparedRow};
