//! prepare() decisions must match import() outcomes exactly, on real samples.
mod common;
use finsight_providers::csv::CsvProvider;

/// Case A: against an EMPTY ledger, every amex row is a fresh insert.
#[test]
fn prepare_counts_match_import_on_empty_ledger() {
    let path = common::sample("amex-all-time-statement.csv");
    let mapping = common::amex_mapping();

    let (db, _d, acct) = common::open_with_account();
    let prepared = {
        let conn = db.get().unwrap();
        CsvProvider::prepare(&path, &acct, &mapping, &conn).unwrap()
    };

    let (db2, _d2, acct2) = common::open_with_account();
    let id = uuid::Uuid::new_v4().to_string();
    let summary = CsvProvider::import(&path, &acct2, &id, &mapping, &db2, |_| {}).unwrap();

    assert_eq!(prepared.rows_imported, summary.rows_imported);
    assert_eq!(
        prepared.rows_skipped_duplicates,
        summary.rows_skipped_duplicates
    );
    assert_eq!(
        prepared.rows_queued_for_review,
        summary.rows_queued_for_review
    );
    assert_eq!(prepared.errors.len(), summary.errors.len());
}

/// Case B (the real parity risk): against a ledger that ALREADY imported the
/// same file, re-preparing vs re-importing must agree — exercising the
/// AutoMatch/duplicate + review branches and the accumulator fold. Both DBs are
/// pre-populated by importing amex once (deterministic decisions; only row ids
/// differ), so prepare-on-DB1 and import-on-DB2 reconcile against equivalent
/// ledgers.
#[test]
fn prepare_matches_import_on_reimport() {
    let path = common::sample("amex-all-time-statement.csv");
    let mapping = common::amex_mapping();

    // DB1: import once, then PREPARE the same file (read-only).
    let (db1, _d1, acct1) = common::open_with_account();
    CsvProvider::import(
        &path,
        &acct1,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db1,
        |_| {},
    )
    .unwrap();
    let prepared = {
        let conn = db1.get().unwrap();
        CsvProvider::prepare(&path, &acct1, &mapping, &conn).unwrap()
    };

    // DB2: import once, then IMPORT the same file again.
    let (db2, _d2, acct2) = common::open_with_account();
    CsvProvider::import(
        &path,
        &acct2,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db2,
        |_| {},
    )
    .unwrap();
    let summary = CsvProvider::import(
        &path,
        &acct2,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db2,
        |_| {},
    )
    .unwrap();

    assert_eq!(
        prepared.rows_imported, summary.rows_imported,
        "insert count parity"
    );
    assert_eq!(
        prepared.rows_skipped_duplicates, summary.rows_skipped_duplicates,
        "duplicate count parity"
    );
    assert_eq!(
        prepared.rows_queued_for_review, summary.rows_queued_for_review,
        "review count parity"
    );
}
