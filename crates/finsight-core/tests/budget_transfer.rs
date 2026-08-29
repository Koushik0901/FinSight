use finsight_core::repos::budgets::{available, list_transfers, transfer};
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
        for (id, label) in [("groceries", "Groceries"), ("rent", "Rent")] {
            conn.execute(
                "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'grp', ?2, '#94A3B8', 0)",
                params![id, label],
            )
            .unwrap();
        }
        // groceries: budgeted 1000, spent 0 => remaining 1000
        conn.execute(
            "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at) VALUES('b1','groceries','2026-09',1000,'2026-09-01T00:00:00Z','2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // rent: budgeted 1000, spent 1500 => remaining -500
        conn.execute(
            "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at) VALUES('b2','rent','2026-09',1000,'2026-09-01T00:00:00Z','2026-09-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, status, is_anomaly, is_transfer, created_at) VALUES('e1','acc1','2026-09-10T00:00:00Z',-1500,'Rent Co','rent','cleared',0,0,'2026-09-10T00:00:00Z')",
            [],
        )
        .unwrap();
    }
    (dir, db)
}

#[test]
fn cover_is_atomic_and_auditable() {
    let (_dir, db) = setup();
    {
        let mut conn = db.get().unwrap();
        transfer(&mut conn, "groceries", "rent", 500, "2026-09", Some("cover")).unwrap();
    }
    {
        let conn = db.get().unwrap();
        assert_eq!(
            available(&conn, "groceries", "2026-09").unwrap(),
            500,
            "groceries should have 500 left after covering 500"
        );
        assert_eq!(
            available(&conn, "rent", "2026-09").unwrap(),
            0,
            "rent overspend -500 + cover 500 = 0"
        );
        let rows = list_transfers(&conn, "2026-09").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].from_category.as_deref(), Some("groceries"));
        assert_eq!(rows[0].to_category.as_deref(), Some("rent"));
        assert_eq!(rows[0].amount_cents, 500);
    }
}

#[test]
fn transfer_fails_when_insufficient_spare() {
    let (_dir, db) = setup();
    // groceries has 1000 spare, try to transfer 1500 should fail
    {
        let mut conn = db.get().unwrap();
        let err = transfer(&mut conn, "groceries", "rent", 1500, "2026-09", None).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("insufficient") || msg.contains("spare") || msg.contains("available"),
            "expected insufficient error, got {msg}"
        );
    }
    // No transfer should have been persisted
    {
        let conn = db.get().unwrap();
        let rows = list_transfers(&conn, "2026-09").unwrap();
        assert_eq!(rows.len(), 0);
        assert_eq!(available(&conn, "groceries", "2026-09").unwrap(), 1000);
        assert_eq!(available(&conn, "rent", "2026-09").unwrap(), -500);
    }
}

#[test]
fn list_transfers_filtered_by_month() {
    let (_dir, db) = setup();
    {
        let mut conn = db.get().unwrap();
        transfer(&mut conn, "groceries", "rent", 200, "2026-09", None).unwrap();
        transfer(&mut conn, "groceries", "rent", 300, "2026-10", None).unwrap();
    }
    {
        let conn = db.get().unwrap();
        let sep = list_transfers(&conn, "2026-09").unwrap();
        assert_eq!(sep.len(), 1);
        assert_eq!(sep[0].amount_cents, 200);
        let oct = list_transfers(&conn, "2026-10").unwrap();
        assert_eq!(oct.len(), 1);
        assert_eq!(oct[0].amount_cents, 300);
    }
}

#[test]
fn available_is_budgeted_plus_carryover_plus_transfers_minus_spent() {
    let (_dir, db) = setup();
    // Add a carryover scenario: budget in prior month
    {
        let mut conn = db.get().unwrap();
        // Prior month 2026-08 groceries budgeted 1000, no spend => carryover +1000 into Sep
        conn.execute(
            "INSERT INTO budgets(id, category_id, month, amount_cents, created_at, updated_at) VALUES('b0','groceries','2026-08',1000,'2026-08-01T00:00:00Z','2026-08-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // Add a future check: Sep groceries should be 1000(carry) +1000(budget) -0 + transfers
        // After transfer 500 to rent, groceries available should be 1500 (1000+1000-500)
        // But our setup groceries has 1000 carryover? Let's verify with function.
        // Actually we inserted carryover, so before transfer groceries available should be 2000.
    }
    {
        let conn = db.get().unwrap();
        let before = available(&conn, "groceries", "2026-09").unwrap();
        assert_eq!(before, 2000, "groceries: 1000 carryover + 1000 budgeted - 0 spent = 2000, got {before}");
    }
    {
        let mut conn = db.get().unwrap();
        transfer(&mut conn, "groceries", "rent", 500, "2026-09", None).unwrap();
    }
    {
        let conn = db.get().unwrap();
        assert_eq!(available(&conn, "groceries", "2026-09").unwrap(), 1500);
        // rent: budgeted 1000 - spent 1500 = -500, plus 500 transfer = 0 (no carryover)
        assert_eq!(available(&conn, "rent", "2026-09").unwrap(), 0);
    }
}
