use finsight_core::repos::budgets::{available_funds, get_hold, set_hold, to_budget};
use finsight_core::testing::migrated_db;
use rusqlite::params;

fn setup() -> (tempfile::TempDir, finsight_core::Db) {
    let (dir, db) = migrated_db();
    {
        let conn = db.get().unwrap();
        // Seed account for income txns
        conn.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) \
             VALUES('acc1', 'Me', 'Bank', 'Checking', 'Checking', 'USD', '#fff', 'manual', '2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // Seed category for budget
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES('groceries', 'grp', 'Groceries', '#94A3B8', 0)",
            [],
        )
        .unwrap();
        // Income 100_00 (positive amount) in 2026-09
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at, settle_up) \
             VALUES('inc1', 'acc1', '2026-09-05T00:00:00Z', 10000, 'Paycheck', 'cleared', 0, 0, '2026-09-05T00:00:00Z', 0)",
            [],
        )
        .unwrap();
        // Budget 60_00 for 2026-09
        conn.execute(
            "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at) \
             VALUES('b1', 'groceries', '2026-09', 6000, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
    }
    (dir, db)
}

#[test]
fn hold_deducts_from_to_budget_and_appears_next_month() {
    let (_dir, db) = setup();
    // Use mutable conn for set_hold (takes &mut Connection per plan, but our impl takes &Connection)
    {
        let mut conn = db.get().unwrap();
        set_hold(&mut conn, "2026-09", 1500).unwrap();
    }
    {
        let conn = db.get().unwrap();
        let tb = to_budget(&conn, "2026-09").unwrap();
        assert_eq!(tb, 2500, "to_budget = income 10000 - budget 6000 - hold 1500 = 2500, got {tb}");
        let av = available_funds(&conn, "2026-10").unwrap();
        assert_eq!(
            av, 1500,
            "next month available includes prev hold as income-like, expected 1500 got {av}"
        );
    }
}

#[test]
fn get_hold_returns_none_initially_and_upserts() {
    let (_dir, db) = setup();
    {
        let conn = db.get().unwrap();
        assert_eq!(get_hold(&conn, "2026-09").unwrap(), None);
    }
    {
        let mut conn = db.get().unwrap();
        set_hold(&mut conn, "2026-09", 1000).unwrap();
    }
    {
        let conn = db.get().unwrap();
        assert_eq!(get_hold(&conn, "2026-09").unwrap(), Some(1000));
    }
    {
        let mut conn = db.get().unwrap();
        set_hold(&mut conn, "2026-09", 2000).unwrap();
    }
    {
        let conn = db.get().unwrap();
        assert_eq!(get_hold(&conn, "2026-09").unwrap(), Some(2000));
        // to_budget should now reflect updated hold
        let tb = to_budget(&conn, "2026-09").unwrap();
        assert_eq!(tb, 2000, "10000-6000-2000=2000");
    }
}

#[test]
fn to_budget_without_hold_equals_income_minus_budget() {
    let (_dir, db) = setup();
    let conn = db.get().unwrap();
    let tb = to_budget(&conn, "2026-09").unwrap();
    assert_eq!(tb, 4000);
}
