use chrono::Utc;
use finsight_core::models::{NewTransaction, TransactionStatus};
use finsight_core::{db::run_migrations, keychain, repos::transactions, Db};
use finsight_providers::{AmountConvention, ColumnRole, CsvImportMapping, CsvProvider};
use rusqlite::params;
use std::path::PathBuf;
use tempfile::TempDir;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/csv")
        .join(name)
}

fn fresh_db() -> (TempDir, Db, String) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("ci.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    let acct = uuid::Uuid::new_v4().to_string();
    db.get().unwrap().execute(
        "INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at, source) \
         VALUES(?1, 'joint', 'Chase', 'Checking', 'Test', 'USD', '#000', ?2, 'manual')",
        params![&acct, Utc::now().to_rfc3339()],
    ).unwrap();
    (dir, db, acct)
}

#[test]
fn chase_csv_imports_then_dedupes_on_reimport() {
    let (_d, db, acct) = fresh_db();
    let mapping = CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![
            ColumnRole::Skip,
            ColumnRole::Date,
            ColumnRole::Merchant,
            ColumnRole::Amount,
            ColumnRole::Skip,
            ColumnRole::Skip,
            ColumnRole::Skip,
        ],
        date_format: "%m/%d/%Y".to_string(),
        amount_convention: AmountConvention::NegativeIsOutflow,
        decimal_separator: '.',
        delimiter: Some(','),
    };

    let id1 = uuid::Uuid::new_v4().to_string();
    let s = CsvProvider::import(
        &fixture("chase-checking.csv"),
        &acct,
        &id1,
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s.rows_imported, 3);
    assert_eq!(s.rows_skipped_duplicates, 0);
    assert!(s.errors.is_empty());

    let id2 = uuid::Uuid::new_v4().to_string();
    let s2 = CsvProvider::import(
        &fixture("chase-checking.csv"),
        &acct,
        &id2,
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s2.rows_imported, 0);
    assert_eq!(s2.rows_skipped_duplicates, 3);
}

#[test]
fn semicolon_german_csv_parses_with_comma_decimal() {
    let (_d, db, acct) = fresh_db();
    let mapping = CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
        date_format: "%d.%m.%Y".to_string(),
        amount_convention: AmountConvention::NegativeIsOutflow,
        decimal_separator: ',',
        delimiter: Some(';'),
    };
    let id = uuid::Uuid::new_v4().to_string();
    let s = CsvProvider::import(
        &fixture("simple-semicolon.csv"),
        &acct,
        &id,
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s.rows_imported, 2);
}

#[test]
fn preview_returns_correct_row_count_and_first_rows() {
    let p = CsvProvider::preview(&fixture("amex-card.csv"), 1).unwrap();
    assert_eq!(p.total_rows, 3);
    assert_eq!(p.rows.len(), 3);
    assert_eq!(p.detected_delimiter, ',');
}

#[test]
fn csv_import_skips_matching_simplefin_transaction() {
    let (dir, db, acct) = fresh_db();
    let posted_at = chrono::NaiveDate::from_ymd_opt(2026, 5, 19)
        .unwrap()
        .and_hms_opt(12, 0, 0)
        .unwrap()
        .and_utc();
    {
        let mut conn = db.get().unwrap();
        transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acct.clone(),
                posted_at,
                amount_cents: -842,
                merchant_raw: "Safeway".into(),
                category_id: None,
                notes: Some("from bank sync".into()),
                status: TransactionStatus::Cleared,
                imported_id: Some("sf-1".into()),
                source: Some("simplefin".into()),
                raw_synced_data: Some("{}".into()),
                pending: false,
                external_tx_id: Some("sf-1".into()),
                external_account_id: Some("sf-acct".into()),
            },
        )
        .unwrap();
    }

    let csv_path = dir.path().join("overlap.csv");
    std::fs::write(
        &csv_path,
        "date,merchant,amount\n2026-05-19,Safeway,-8.42\n",
    )
    .unwrap();
    let mapping = CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
        date_format: "%Y-%m-%d".to_string(),
        amount_convention: AmountConvention::NegativeIsOutflow,
        decimal_separator: '.',
        delimiter: Some(','),
    };

    let summary = CsvProvider::import(
        &csv_path,
        &acct,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();

    assert_eq!(summary.rows_imported, 0);
    assert_eq!(summary.rows_skipped_duplicates, 1);
    let count: i64 = db
        .get()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
            [&acct],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
