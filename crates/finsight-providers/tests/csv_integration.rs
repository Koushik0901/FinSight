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
fn amex_all_time_statement_imports_with_space_month_date() {
    let (_d, db, acct) = fresh_db();
    let mapping = CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![
            ColumnRole::Date,
            ColumnRole::Skip,
            ColumnRole::Merchant,
            ColumnRole::Amount,
        ],
        date_format: "%d %b %Y".to_string(),
        amount_convention: AmountConvention::PositiveIsOutflow,
        decimal_separator: '.',
        delimiter: Some(','),
    };
    let id = uuid::Uuid::new_v4().to_string();
    let s = CsvProvider::import(
        &fixture("amex-all-time-statement.csv"),
        &acct,
        &id,
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s.rows_imported, 5);
    assert_eq!(s.rows_skipped_duplicates, 0);
    assert!(s.errors.is_empty());

    // Verify sign convention: charges are positive in the file but stored as negative cents
    // (outflows), while the payment/credit is stored as positive cents (inflows).
    let conn = db.get().unwrap();
    let (payment, points_credit): (i64, i64) = conn
        .query_row(
            "SELECT \
                (SELECT amount_cents FROM transactions WHERE merchant_raw LIKE 'PAYMENT RECEIVED%' LIMIT 1), \
                (SELECT amount_cents FROM transactions WHERE merchant_raw LIKE 'Use Points%' LIMIT 1)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(payment, 298_614);
    assert_eq!(points_credit, 21_390);
    let charge: i64 = conn
        .query_row(
            "SELECT amount_cents FROM transactions WHERE merchant_raw LIKE 'TIM HORTONS%' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(charge, -943);
}

#[test]
fn repeated_identical_rows_in_one_statement_all_import() {
    // A single authoritative statement lists every posted charge once, so
    // identical lines are distinct real transactions (e.g. several same-day
    // pay-as-you-go API top-ups of the same amount). None of them may be
    // auto-skipped or queued as a "duplicate" of an earlier row in the SAME file.
    let (dir, db, acct) = fresh_db();
    let csv_path = dir.path().join("repeats.csv");
    std::fs::write(
        &csv_path,
        "date,merchant,amount\n\
         2026-04-18,OPENROUTER INC,-5.33\n\
         2026-04-18,OPENROUTER INC,-5.33\n\
         2026-04-18,OPENROUTER INC,-5.33\n\
         2026-04-18,OPENROUTER INC,-5.33\n\
         2026-04-19,OPENROUTER INC,-5.35\n\
         2026-04-20,OPENROUTER INC,-5.33\n",
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
    let s = CsvProvider::import(
        &csv_path,
        &acct,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s.rows_imported, 6, "every real charge must land");
    assert_eq!(s.rows_skipped_duplicates, 0, "no intra-file auto-skip");
    assert_eq!(s.rows_queued_for_review, 0, "no intra-file review noise");
    let count: i64 = db
        .get()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = ?1",
            [&acct],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 6);

    // Cross-import dedup must still fire: re-importing the same file inserts
    // nothing new — every row is caught as a duplicate of a prior-import row
    // (either auto-skipped or, when several identical priors tie, queued for
    // review), never blindly re-inserted.
    let s2 = CsvProvider::import(
        &csv_path,
        &acct,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s2.rows_imported, 0, "re-import must not blindly duplicate");
    assert_eq!(
        s2.rows_skipped_duplicates + s2.rows_queued_for_review,
        6,
        "re-import must reconcile every row against the prior import"
    );
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
