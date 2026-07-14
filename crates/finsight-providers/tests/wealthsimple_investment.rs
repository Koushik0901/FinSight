//! End-to-end import of the Wealthsimple TFSA all-time statement fixture:
//! activity columns persist, Trade/MoneyMovement rows are transfer-flagged,
//! Dividend/Interest/Tax stay visible to income/expense, the verbatim row
//! survives as raw JSON, and re-import is fully idempotent.
//!
//! The fixture is a fully synthetic TFSA statement (fictional company names,
//! fictional account id) shaped like a real Wealthsimple export — it exists
//! purely to exercise the column layout and activity-type edge cases, not to
//! represent anyone's real portfolio.
mod common;

use finsight_providers::csv::CsvProvider;
use rusqlite::params;

/// Fixture ground truth (computed from the file):
/// 19 data rows = 7 Trade + 3 MoneyMovement + 4 Dividend + 3 Interest + 2 Tax,
/// plus a trailing "As of …" footer line that fails column-count validation
/// and must surface as exactly one row error.
const DATA_ROWS: u32 = 19;
const TRANSFER_ROWS: i64 = 7 + 3;
const INCOME_EXPENSE_ROWS: i64 = 4 + 3 + 2;

#[test]
fn wealthsimple_import_types_rows_and_dedupes_on_reimport() {
    let path = common::fixture("wealthsimple-tfsa.csv");
    let mapping = common::wealthsimple_mapping();
    let (db, _d, acct) = common::open_with_investment_account();

    let s = CsvProvider::import(
        &path,
        &acct,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s.rows_imported, DATA_ROWS);
    assert_eq!(s.rows_skipped_duplicates, 0);
    assert_eq!(s.errors.len(), 1, "the 'As of …' footer is a row error");

    let conn = db.get().unwrap();

    // Trade + MoneyMovement are internal moves; Dividend/Interest/Tax are real
    // income/expense. This is the "zero metric-SQL changes" contract: every
    // `is_transfer = 0` filter now sees exactly the right rows.
    let transfers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = ?1 AND is_transfer = 1",
            params![&acct],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(transfers, TRANSFER_ROWS);
    let visible: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE account_id = ?1 AND is_transfer = 0",
            params![&acct],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(visible, INCOME_EXPENSE_ROWS);

    // Activity typing matches the export verbatim.
    for (activity, expected) in [
        ("Trade", 7i64),
        ("MoneyMovement", 3),
        ("Dividend", 4),
        ("Interest", 3),
        ("Tax", 2),
    ] {
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions WHERE account_id = ?1 AND activity_type = ?2",
                params![&acct, activity],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, expected, "activity_type = {activity}");
    }

    // A trade row: synthesized merchant, symbol/quantity/unit_price columns,
    // and the full verbatim row (incl. unmapped commission) in raw JSON.
    let (merchant, symbol, qty, price, raw): (String, String, f64, f64, String) = conn
        .query_row(
            "SELECT merchant_raw, symbol, quantity, unit_price, raw_synced_data \
             FROM transactions \
             WHERE account_id = ?1 AND activity_type = 'Trade' AND symbol = 'ACME' \
             ORDER BY posted_at ASC LIMIT 1",
            params![&acct],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(merchant, "Buy ACME");
    assert_eq!(symbol, "ACME");
    assert!((qty - 10.3372).abs() < 1e-9);
    assert!((price - 48.3931).abs() < 1e-9);
    let raw: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(raw["commission"], "0");
    assert_eq!(raw["account_type"], "TFSA");

    // Non-trade rows synthesize deterministic merchants.
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions \
             WHERE account_id = ?1 AND merchant_raw = 'Withholding tax (NRT)'",
            params![&acct],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 2);

    // The zero-metric-changes acceptance check: through the untouched
    // `is_transfer = 0` filter, income is exactly dividends + interest
    // (10.00 + 0.06) and expense exactly the withholding tax (0.55) —
    // trades and contributions are invisible.
    let (income, expense) =
        finsight_core::metrics::income_expense_since(&conn, "1970-01-01").unwrap();
    assert_eq!(income, 1_000 + 6);
    assert_eq!(expense, 55);
    drop(conn);

    // Re-import: fully idempotent (deterministic merchant synthesis + the
    // batch matcher's K-for-K set matching absorb even same-day identical
    // Interest rows).
    let s2 = CsvProvider::import(
        &path,
        &acct,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();
    assert_eq!(s2.rows_imported, 0, "re-import must insert nothing");
    assert_eq!(
        s2.rows_skipped_duplicates + s2.rows_queued_for_review,
        DATA_ROWS,
        "every row is either a recognized duplicate or queued for review"
    );
    assert_eq!(
        s2.rows_queued_for_review, 0,
        "identical rows must auto-match, not queue for review"
    );
}

/// Ground truth for the fixture, computed independently from the file:
/// open positions GLOBEX 7 @ 110.00, INITECH 3 @ 200.00, UMBRA 1.0 @ 42.00
/// (ACME's BUY/SELL net to zero and is closed); cash = Σ net_cash = -199.49;
/// dividends 10.00, interest 0.06, withholding tax 0.55.
#[test]
fn positions_and_summary_derive_from_imported_trades() {
    let path = common::fixture("wealthsimple-tfsa.csv");
    let mapping = common::wealthsimple_mapping();
    let (db, _d, acct) = common::open_with_investment_account();
    CsvProvider::import(
        &path,
        &acct,
        &uuid::Uuid::new_v4().to_string(),
        &mapping,
        &db,
        |_| {},
    )
    .unwrap();

    let conn = db.get().unwrap();
    let positions = finsight_core::investments::positions_for_account(&conn, &acct).unwrap();
    let symbols: Vec<&str> = positions.iter().map(|p| p.symbol.as_str()).collect();
    assert_eq!(symbols, vec!["GLOBEX", "INITECH", "UMBRA"]);

    let globex = positions.iter().find(|p| p.symbol == "GLOBEX").unwrap();
    assert!((globex.quantity - 7.0).abs() < 1e-6);
    assert_eq!(globex.last_price, Some(110.0));
    assert_eq!(globex.market_value_cents, Some(77_000));
    assert_eq!(globex.invested_cents, 72_000);
    assert_eq!(globex.name.as_deref(), Some("Globex Corp"));

    let umbra = positions.iter().find(|p| p.symbol == "UMBRA").unwrap();
    assert!((umbra.quantity - 1.0).abs() < 1e-6);
    assert_eq!(umbra.market_value_cents, Some(4_200));

    let summary = finsight_core::investments::summary_for_account(&conn, &acct).unwrap();
    assert_eq!(summary.cash_cents, -19_949);
    assert_eq!(summary.positions_value_cents, 141_200);
    assert_eq!(summary.portfolio_estimate_cents, 121_251);
    assert_eq!(summary.dividend_income_cents, 1_000);
    assert_eq!(summary.interest_income_cents, 6);
    assert_eq!(summary.withholding_tax_cents, 55);
    assert_eq!(summary.open_positions, 3);
    assert!(!summary.has_negative_quantity);
}

/// A SELL without its earlier BUYs (partial-history import) must flag the
/// summary as unreliable instead of silently producing a nonsense estimate.
#[test]
fn sell_without_buy_flags_negative_quantity() {
    use finsight_core::models::{NewTransaction, TransactionStatus, TxnActivity};
    use finsight_core::repos::transactions;

    let (db, _d, acct) = common::open_with_investment_account();
    let mut conn = db.get().unwrap();
    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: acct.clone(),
            posted_at: chrono::Utc::now(),
            amount_cents: 40_993,
            merchant_raw: "Sell GLOBEX".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
            imported_id: None,
            source: Some("csv".into()),
            raw_synced_data: None,
            pending: false,
            external_tx_id: None,
            external_account_id: None,
            activity: Some(TxnActivity {
                activity_type: "Trade".into(),
                activity_sub_type: Some("SELL".into()),
                symbol: Some("GLOBEX".into()),
                security_name: None,
                quantity: Some(-9.1122),
                unit_price: Some(61.22),
            }),
        },
    )
    .unwrap();

    let summary = finsight_core::investments::summary_for_account(&conn, &acct).unwrap();
    assert!(summary.has_negative_quantity);
    let positions = finsight_core::investments::positions_for_account(&conn, &acct).unwrap();
    assert_eq!(positions.len(), 1);
    assert!(positions[0].quantity < 0.0);
}
