use chrono::{Duration, Utc};
use finsight_core::models::custom_report::{CustomReportParams, Period, SplitBy};
use finsight_core::repos::budgets::custom_report;
use finsight_core::testing::migrated_db;
use rusqlite::params;
use uuid::Uuid;

fn setup() -> (tempfile::TempDir, finsight_core::Db) {
    let (dir, db) = migrated_db();
    {
        let conn = db.get().unwrap();
        // Seed account
        conn.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) \
             VALUES('acc1', 'Me', 'Bank', 'Checking', 'Checking', 'USD', '#fff', 'manual', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // Seed categories / groups for completeness (not strictly needed for payee split)
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) VALUES('cat1', 'grp', 'Groceries Cat', '#fff', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) VALUES('cat2', 'grp', 'Rent Cat', '#fff', 1)",
            [],
        )
        .unwrap();

        let now = Utc::now();
        // 3 months spread: 10 days ago, 40 days ago, 70 days ago — all within Last6Months (180 days)
        let dates = [
            (now - Duration::days(10)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            (now - Duration::days(40)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            (now - Duration::days(70)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        ];
        // Two payees: Groceries and Rent
        let txns = [
            ("Groceries", "cat1", dates[0].as_str(), 5000),
            ("Groceries", "cat1", dates[1].as_str(), 3000),
            ("Rent", "cat2", dates[2].as_str(), 10000),
        ];
        for (merchant, cat, posted, cents) in txns {
            conn.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, created_at, settle_up) \
                 VALUES(?1, 'acc1', ?2, ?3, ?4, ?5, 'cleared', 0, 0, ?2, 0)",
                params![Uuid::new_v4().to_string(), posted, -cents, merchant, cat],
            )
            .unwrap();
        }
        // Add a transfer that must be excluded when include_transfers = false
        let transfer_date = (now - Duration::days(5)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, created_at, settle_up) \
             VALUES(?1, 'acc1', ?2, -9999, 'Transfer Payee', 'cat1', 'cleared', 0, 1, ?2, 0)",
            params![Uuid::new_v4().to_string(), transfer_date],
        )
        .unwrap();
    }
    (dir, db)
}

#[test]
fn custom_report_splits_by_payee_and_month() {
    let (_dir, db) = setup();
    let conn = db.get().unwrap();
    let r = custom_report(
        &conn,
        CustomReportParams {
            split_by: SplitBy::Payee,
            period: Period::Last6Months,
            include_transfers: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.rows.len(), 2, "expected 2 payee rows, got {:?}", r.rows);
    assert!(r.rows.iter().any(|row| row.label == "Groceries"));
}

#[test]
fn custom_report_excludes_transfers_when_flag_false() {
    let (_dir, db) = setup();
    let conn = db.get().unwrap();
    // Without transfers, total should be 5000+3000+10000 = 18000
    let r = custom_report(
        &conn,
        CustomReportParams {
            split_by: SplitBy::Payee,
            period: Period::Last6Months,
            include_transfers: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.total_cents, 18000);

    // With transfers, total should be 18000+9999 = 27999
    let r2 = custom_report(
        &conn,
        CustomReportParams {
            split_by: SplitBy::Payee,
            period: Period::Last6Months,
            include_transfers: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r2.total_cents, 27999);
}

#[test]
fn custom_report_splits_by_category() {
    let (_dir, db) = setup();
    let conn = db.get().unwrap();
    let r = custom_report(
        &conn,
        CustomReportParams {
            split_by: SplitBy::Category,
            period: Period::Last6Months,
            include_transfers: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.rows.len(), 2, "should group into 2 categories");
}
