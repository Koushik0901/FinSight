use finsight_core::{
    db::run_migrations,
    keychain,
    models::{AccountType, NewAccount},
    repos::accounts,
    Db,
};
use finsight_providers::{AmountConvention, ColumnRole, CsvImportMapping};
use std::path::PathBuf;
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("prep.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

fn amex_mapping() -> CsvImportMapping {
    CsvImportMapping {
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
        delimiter: None,
    }
}

#[tokio::test]
async fn build_preview_reports_bounded_outcome_for_amex_sample() {
    let (_d, db) = fresh_db();
    let account_id = {
        let mut conn = db.get().unwrap();
        accounts::insert(
            &mut conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                owner: "Me".into(),
                bank: "Amex".into(),
                r#type: AccountType::Credit,
                name: "Amex Card".into(),
                last4: None,
                currency: "USD".into(),
                color: "#000".into(),
                opening_balance_cents: 0,
                source: "manual".into(),
                liquidity_type: "illiquid".into(),
                emergency_fund_eligible: false,
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
                account_group: "credit".into(),
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
        .unwrap()
        .id
    };

    let mapping = amex_mapping();
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../samples/amex-all-time-statement.csv");

    let preview = finsight_app::commands::import::build_preview(&db, &path, &account_id, &mapping)
        .expect("build_preview should succeed");

    assert_eq!(preview.rows_imported, 1988);
    assert_eq!(preview.rows_skipped_duplicates, 0);
    assert_eq!(preview.rows_queued_for_review, 0);
    assert_eq!(preview.rows_total, 1988);
    assert!(!preview.signature.is_empty());
}
