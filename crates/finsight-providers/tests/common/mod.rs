//! Shared test helpers for provider integration tests.
use finsight_core::models::{AccountType, NewAccount};
use finsight_core::repos::accounts;
use finsight_core::Db;
use finsight_providers::{AmountConvention, ColumnRole, CsvImportMapping};
use std::path::PathBuf;

pub fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name)
}

/// Committed fixture under tests/fixtures/csv (unlike `sample`, which points
/// at the untracked repo-root samples/ directory).
#[allow(dead_code)]
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/csv")
        .join(name)
}

pub fn amex_mapping() -> CsvImportMapping {
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

/// Column mapping for the Wealthsimple TFSA all-time statement export
/// (fixture `wealthsimple-tfsa.csv`): transaction_date, settlement_date,
/// account_id, account_type, activity_type, activity_sub_type, direction,
/// symbol, name, currency, quantity, unit_price, commission, net_cash_amount.
pub fn wealthsimple_mapping() -> CsvImportMapping {
    CsvImportMapping {
        skip_header_rows: 1,
        columns: vec![
            ColumnRole::Date,
            ColumnRole::Skip, // settlement_date
            ColumnRole::Skip, // account_id
            ColumnRole::Skip, // account_type
            ColumnRole::ActivityType,
            ColumnRole::ActivitySubType,
            ColumnRole::Skip, // direction
            ColumnRole::Symbol,
            ColumnRole::SecurityName,
            ColumnRole::Skip, // currency
            ColumnRole::Quantity,
            ColumnRole::UnitPrice,
            ColumnRole::Skip, // commission
            ColumnRole::Amount,
        ],
        date_format: "%Y-%m-%d".to_string(),
        amount_convention: AmountConvention::NegativeIsOutflow,
        decimal_separator: '.',
        delimiter: Some(','),
    }
}

/// Fresh migrated DB + one Credit account; returns (db, tempdir, account_id).
pub fn open_with_account() -> (Db, tempfile::TempDir, String) {
    open_with_typed_account(AccountType::Credit, "Amex", "Amex Card", "credit")
}

/// Fresh migrated DB + one Investment account (Wealthsimple TFSA shape).
#[allow(dead_code)]
pub fn open_with_investment_account() -> (Db, tempfile::TempDir, String) {
    open_with_typed_account(AccountType::Investment, "Wealthsimple", "TFSA", "investments")
}

fn open_with_typed_account(
    r#type: AccountType,
    bank: &str,
    name: &str,
    account_group: &str,
) -> (Db, tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.sqlcipher"), &"cd".repeat(32)).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();
    let id = {
        let mut conn = db.get().unwrap();
        accounts::insert(
            &mut conn,
            NewAccount {
                owner: "joint".into(),
                bank: bank.into(),
                r#type,
                name: name.into(),
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
                account_group: account_group.into(),
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
    (db, dir, id)
}
