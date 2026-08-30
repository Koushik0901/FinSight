use finsight_core::repos::budgets::{
    apply_templates, create_funding_template, delete_funding_template, list_funding_templates,
};
use finsight_core::testing::migrated_db;
use rusqlite::params;

fn setup() -> (tempfile::TempDir, finsight_core::Db) {
    let (dir, db) = migrated_db();
    {
        let conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) \
              VALUES('acc1', 'Me', 'Bank', 'Checking', 'Checking', 'USD', '#fff', 'manual', '2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        for (id, label) in [("groceries", "Groceries"), ("rent", "Rent"), ("savings", "Savings")] {
            conn.execute(
                "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'grp', ?2, '#94A3B8', 0)",
                params![id, label],
            )
            .unwrap();
        }
        // Income 20000 for 2026-09 so to_budget is ample.
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, is_anomaly, is_transfer, created_at, settle_up) \
              VALUES('inc1', 'acc1', '2026-09-05T00:00:00Z', 20000, 'Paycheck', 'cleared', 0, 0, '2026-09-05T00:00:00Z', 0)",
            [],
        )
        .unwrap();
    }
    (dir, db)
}

#[test]
fn template_fixed_up_to_and_remainder() {
    let (_dir, db) = setup();
    {
        let mut conn = db.get().unwrap();
        // Fixed groceries 7299 priority 0
        create_funding_template(&mut conn, "groceries", "fixed", r#"{"amount":7299}"#, 0).unwrap();
        // UpTo rent cap 30000 priority 1 – balance 0 => need 30000 but capped by remaining available
        create_funding_template(&mut conn, "rent", "up_to", r#"{"cap":30000}"#, 1).unwrap();
        // Remainder savings priority 2 – takes whatever is left
        create_funding_template(&mut conn, "savings", "remainder", r#"{}"#, 2).unwrap();
    }
    let mut conn = db.get().unwrap();
    let changes = apply_templates(&mut conn, "2026-09").unwrap();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].category_id, "groceries");
    assert_eq!(changes[0].amount_cents, 7299, "fixed should fund 7299");
    // available = 20000, after groceries 12701 left, rent needs 30000 => takes 12701, remainder 0
    // But if our remainder is last, it would take 0. That's expected if remaining after rent is 0.
    // To make remainder meaningful, we expect rent takes min(need, available) then remainder takes remainder.
    // With income 20000, groceries 7299 -> 12701 remaining, rent needs 30000 => takes 12701, remainder 0
    // So check that rent got 12701
    assert_eq!(changes[1].category_id, "rent");
    assert_eq!(changes[1].amount_cents, 12701);
    assert_eq!(changes[2].category_id, "savings");
    assert_eq!(changes[2].amount_cents, 0);
}

#[test]
fn up_to_respects_existing_budget_and_spend() {
    let (_dir, db) = setup();
    {
        let mut conn = db.get().unwrap();
        // Pre-budget groceries 5000 and spend 2000 => balance = 5000 + carryover(0) - 2000 = 3000 ? Actually budgeted 5000, spent 2000 => available 3000
        // But apply_templates UpTo cap 10000 should need 7000 more (10000-3000)
        conn.execute(
            "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at) VALUES('b1','groceries','2026-09',5000,'2026-09-01T00:00:00Z','2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, created_at) VALUES('e1','acc1','2026-09-10T00:00:00Z',-2000,'Store','groceries','cleared',0,0,'2026-09-10T00:00:00Z')",
            [],
        )
        .unwrap();
        create_funding_template(&mut conn, "groceries", "up_to", r#"{"cap":10000}"#, 0).unwrap();
    }
    let mut conn = db.get().unwrap();
    let changes = apply_templates(&mut conn, "2026-09").unwrap();
    assert_eq!(changes.len(), 1);
    // existing budgeted 5000, spent 2000 => balance 3000, cap 10000 => need 7000, available = income 20000 - budgeted 5000 =15000 => need fully fundable => 7000
    assert_eq!(changes[0].amount_cents, 7000);
}

#[test]
fn percent_and_remainder_split() {
    let (_dir, db) = setup();
    {
        let mut conn = db.get().unwrap();
        // Percent 50% then remainder
        create_funding_template(&mut conn, "groceries", "percent", r#"{"pct":0.5}"#, 0).unwrap();
        create_funding_template(&mut conn, "rent", "remainder", r#"{}"#, 1).unwrap();
    }
    let mut conn = db.get().unwrap();
    let changes = apply_templates(&mut conn, "2026-09").unwrap();
    assert_eq!(changes.len(), 2);
    // available 20000, percent takes 10000, remainder takes remaining 10000
    assert_eq!(changes[0].amount_cents, 10000);
    assert_eq!(changes[1].amount_cents, 10000);
}

#[test]
fn crud_list_and_delete() {
    let (_dir, db) = setup();
    {
        let mut conn = db.get().unwrap();
        let t1 = create_funding_template(&mut conn, "groceries", "fixed", r#"{"amount":1000}"#, 5).unwrap();
        let t2 = create_funding_template(&mut conn, "rent", "fixed", r#"{"amount":2000}"#, 1).unwrap();
        let list = list_funding_templates(&conn).unwrap();
        assert_eq!(list.len(), 2);
        // ordered by priority ASC then id ASC: rent priority 1 before groceries priority 5
        assert_eq!(list[0].id, t2.id);
        assert_eq!(list[1].id, t1.id);
        delete_funding_template(&mut conn, &t1.id).unwrap();
        let list2 = list_funding_templates(&conn).unwrap();
        assert_eq!(list2.len(), 1);
        assert_eq!(list2[0].id, t2.id);
    }
}
