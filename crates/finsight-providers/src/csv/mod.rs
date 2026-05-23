//! CSV ingestion provider — preview, import with dedup, mapping persistence.

pub mod encoding;
pub mod mapping;
pub mod parse;

use crate::csv::encoding::{decode_layered, DetectedEncoding};
use crate::csv::mapping::CsvImportMapping;
use crate::csv::parse::{into_new_transaction, parse_row};
use crate::error::{ProviderError, ProviderResult};
use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::Path;
use uuid::Uuid;

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
        let bytes = read_capped(path)?;
        if bytes.is_empty() {
            return Err(ProviderError::EmptyFile);
        }
        let (text, _) = decode_layered(&bytes)?;
        let delimiter = mapping.delimiter.unwrap_or_else(|| detect_delimiter(&text));

        // First pass: count rows for progress.
        let total = {
            let mut r = csv::ReaderBuilder::new()
                .has_headers(false)
                .delimiter(delimiter as u8)
                .flexible(true)
                .from_reader(text.as_bytes());
            let mut n: u32 = 0;
            for (idx, rec) in r.records().enumerate() {
                rec?;
                if idx >= mapping.skip_header_rows as usize {
                    n = n.saturating_add(1);
                }
            }
            n
        };
        let emit_every = std::cmp::max(1, total / 20) as usize;

        let mut conn = db.get().map_err(ProviderError::Core)?;
        let filename = path.file_name().map(|s| s.to_string_lossy().into_owned());
        conn.execute(
            "INSERT INTO imports(id, source, filename, account_id, started_at) \
             VALUES(?1, 'csv', ?2, ?3, ?4)",
            params![import_id, filename, account_id, Utc::now().to_rfc3339()],
        )
        .map_err(|e| ProviderError::Internal(format!("imports insert: {e}")))?;

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter as u8)
            .flexible(true)
            .from_reader(text.as_bytes());

        let mut rows_imported: u32 = 0;
        let mut rows_skipped: u32 = 0;
        let mut errors: Vec<RowError> = Vec::new();
        let mut processed: u32 = 0;
        // in_batch tracks rows in the current open transaction so we can
        // commit every BATCH_SIZE rows without the monotonic `processed` check.
        let mut in_batch: usize = 0;

        let mut tx = conn
            .transaction()
            .map_err(|e| ProviderError::Internal(format!("begin: {e}")))?;

        for (idx, rec) in reader.records().enumerate() {
            let row_number = (idx + 1) as u32;
            let rec = match rec {
                Ok(r) => r,
                Err(e) => {
                    errors.push(RowError {
                        row_number,
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            if idx < mapping.skip_header_rows as usize {
                continue;
            }

            let fields: Vec<&str> = rec.iter().collect();
            let parsed = match parse_row(&fields, mapping) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(RowError {
                        row_number,
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            let new_tx = into_new_transaction(parsed, account_id.to_string());

            // Dedup check via V002 covering index.
            let exists: bool = tx.query_row(
                "SELECT 1 FROM transactions WHERE account_id = ?1 AND posted_at = ?2 \
                                                AND amount_cents = ?3 AND merchant_raw = ?4 LIMIT 1",
                params![&new_tx.account_id, new_tx.posted_at.to_rfc3339(),
                        new_tx.amount_cents, &new_tx.merchant_raw],
                |_| Ok(true),
            ).or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(false),
                other => Err(other),
            }).map_err(|e| ProviderError::Internal(format!("dedup: {e}")))?;

            if exists {
                rows_skipped += 1;
            } else {
                tx.execute(
                    "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                              status, notes, created_at) \
                     VALUES(?1, ?2, ?3, ?4, ?5, 'cleared', ?6, ?7)",
                    params![
                        Uuid::new_v4().to_string(),
                        &new_tx.account_id,
                        new_tx.posted_at.to_rfc3339(),
                        new_tx.amount_cents,
                        &new_tx.merchant_raw,
                        &new_tx.notes,
                        Utc::now().to_rfc3339(),
                    ],
                ).map_err(|e| ProviderError::Internal(format!("insert: {e}")))?;
                rows_imported += 1;
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

        on_progress(BatchProgress {
            rows_done: processed,
            rows_total: total,
        });

        Ok(ImportSummary {
            import_id: import_id.to_string(),
            rows_imported,
            rows_skipped_duplicates: rows_skipped,
            errors,
        })
    }
}

fn detect_delimiter(text: &str) -> char {
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

fn read_capped(path: &Path) -> ProviderResult<Vec<u8>> {
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
