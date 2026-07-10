//! Staleness + edge-case coverage for `CsvProvider::prepare`.
//!
//! Ground truth (proven against the real amex sample, do not loosen):
//! - Empty ledger: prepare/import the amex sample → 1988 rows imported.
//! - Re-import against a ledger that already has the amex file imported once:
//!   rows_imported == 0, rows_skipped_duplicates == 1988, rows_queued_for_review == 0.
//!   Every incoming row is an exact twin of an existing row, so bipartite
//!   set-matching pairs them 1:1 and NOTHING queues for review. (The earlier
//!   1843/145 split predated the P1-4 fix, where ~7% of an identical re-import
//!   was wrongly queued because same-amount/same-day sibling charges — two Lyft
//!   rides, two McDonald's — made each row's exact twin look "ambiguous".)
mod common;
use finsight_providers::csv::CsvProvider;
use finsight_providers::error::ProviderError;
use finsight_providers::{AmountConvention, ColumnRole, CsvImportMapping};

/// 1. Staleness changes the signature, and a stale-then-refreshed prepare
/// re-reconciles against current DB state rather than reusing anything.
#[test]
fn stale_signature_changes_after_ledger_mutation() {
    let path = common::sample("amex-all-time-statement.csv");
    let mapping = common::amex_mapping();
    let (db, _dir, acct) = common::open_with_account();

    let sig_a = {
        let conn = db.get().unwrap();
        CsvProvider::prepare(&path, &acct, &mapping, &conn)
            .unwrap()
            .signature
    };

    // Mutate the ledger: import the file once (0 -> 1988 rows).
    let import_id = uuid::Uuid::new_v4().to_string();
    let summary = CsvProvider::import(&path, &acct, &import_id, &mapping, &db, |_| {}).unwrap();
    assert_eq!(summary.rows_imported, 1988, "ground truth: empty-ledger import count");

    let prepared_b = {
        let conn = db.get().unwrap();
        CsvProvider::prepare(&path, &acct, &mapping, &conn).unwrap()
    };

    assert_ne!(
        sig_a, prepared_b.signature,
        "ledger fingerprint moved from 0 to 1988 rows; signature must change"
    );
    assert_eq!(prepared_b.rows_imported, 0, "re-import shape: no new inserts");
    assert_eq!(
        prepared_b.rows_skipped_duplicates, 1988,
        "re-import shape: every row is an exact twin → all skip as duplicates"
    );
    assert_eq!(
        prepared_b.rows_queued_for_review, 0,
        "re-import shape: identical duplicates must NEVER queue for review"
    );
}

/// 2. Empty file must return `ProviderError::EmptyFile`, not panic.
#[test]
fn empty_file_returns_empty_file_error() {
    let (_db, _dir, acct) = common::open_with_account();
    let db = _db;
    let conn = db.get().unwrap();
    let mapping = common::amex_mapping();

    let empty = tempfile::NamedTempFile::new().unwrap();
    let result = CsvProvider::prepare(empty.path(), &acct, &mapping, &conn);

    assert!(
        matches!(result, Err(ProviderError::EmptyFile)),
        "expected EmptyFile error, got {result:?}"
    );
}

/// 3. Re-preparing the identical file against a ledger that already imported
/// it inserts nothing new (focused, standalone assertion).
#[test]
fn reimport_inserts_nothing_new() {
    let path = common::sample("amex-all-time-statement.csv");
    let mapping = common::amex_mapping();
    let (db, _dir, acct) = common::open_with_account();

    let import_id = uuid::Uuid::new_v4().to_string();
    CsvProvider::import(&path, &acct, &import_id, &mapping, &db, |_| {}).unwrap();

    let prepared = {
        let conn = db.get().unwrap();
        CsvProvider::prepare(&path, &acct, &mapping, &conn).unwrap()
    };

    assert_eq!(
        prepared.rows_imported, 0,
        "identical statement re-prepared should add no new charges"
    );
}

/// 4. Malformed rows are captured as errors, not fatal; well-formed rows in
/// the same file still import.
#[test]
fn malformed_rows_captured_good_rows_still_import() {
    let (db, dir, acct) = common::open_with_account();

    let csv_path = dir.path().join("mini.csv");
    let contents = "Date,Merchant,Amount\n\
                     2024-01-05,Coffee Shop,4.50\n\
                     NOT-A-DATE,Bad Row,9.99\n\
                     2024-01-07,Grocery Store,52.10\n";
    std::fs::write(&csv_path, contents).unwrap();

    let mapping = CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
        date_format: "%Y-%m-%d".to_string(),
        amount_convention: AmountConvention::NegativeIsOutflow,
        decimal_separator: '.',
        delimiter: None,
    };

    let conn = db.get().unwrap();
    let prepared = CsvProvider::prepare(&csv_path, &acct, &mapping, &conn).unwrap();

    assert_eq!(prepared.errors.len(), 1, "the unparseable-date row must be captured as an error");
    assert_eq!(prepared.rows_imported, 2, "the two well-formed rows still import");
}

/// 5. Flipping the amount convention changes the signature (the mapping is
/// part of the signature, not just the file).
#[test]
fn flipped_amount_convention_changes_signature() {
    let path = common::sample("amex-all-time-statement.csv");
    let (db, _dir, acct) = common::open_with_account();

    let mapping_pos = common::amex_mapping(); // PositiveIsOutflow (real amex mapping)
    let mut mapping_neg = mapping_pos.clone();
    mapping_neg.amount_convention = AmountConvention::NegativeIsOutflow;

    let conn = db.get().unwrap();
    let sig_pos = CsvProvider::prepare(&path, &acct, &mapping_pos, &conn)
        .unwrap()
        .signature;
    let sig_neg = CsvProvider::prepare(&path, &acct, &mapping_neg, &conn)
        .unwrap()
        .signature;

    assert_ne!(
        sig_pos, sig_neg,
        "amount convention is part of the mapping signature"
    );
}
