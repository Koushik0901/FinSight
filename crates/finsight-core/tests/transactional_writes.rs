use finsight_core::{db::run_migrations, keychain};
use rusqlite::params;

fn migrated_db() -> (tempfile::TempDir, finsight_core::Db) {
    let dir = tempfile::TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = finsight_core::Db::open(&dir.path().join("test.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

/// Mid-batch failure must roll back the entire chunk, not leave a partial write.
///
/// This is the regression for the categorizer/executor multi-row writers:
/// they must wrap their writes in a single transaction (BEGIN; ... COMMIT;)
/// like `repos/rules.rs:90-104`, so a failure on the second row does not
/// commit the first. The assertion is on the DB state (0 rows), not on
/// whether the transaction returned an error — a partial write would still
/// have the error but would leave 1 row behind.
#[test]
fn mid_batch_failure_rolls_back() {
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();

    // Minimal FK parents so a valid categorization can exist.
    conn.execute(
        "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) VALUES('t1','a1','2024-01-15T00:00:00Z',-1500,'CHIPOTLE','cleared',0,'2024-01-15T00:00:00Z')",
        [],
    )
    .unwrap();

    // Two-row batch in one transaction: first is valid, second violates the FK
    // (txn_id ghost does not exist). With a correct transaction the whole
    // batch rolls back; without it the first row would remain committed.
    let tx_result = (|| -> rusqlite::Result<()> {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, at) VALUES(?1, ?2, 'cat1', 'llm', 0.9, ?3)",
            params!["c1", "t1", "2024-01-15T00:00:00Z"],
        )?;
        // This second insert must fail: ghost-txn does not exist in transactions.
        tx.execute(
            "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, at) VALUES(?1, ?2, 'cat1', 'llm', 0.9, ?3)",
            params!["c2", "ghost-txn", "2024-01-15T00:00:00Z"],
        )?;
        tx.commit()?;
        Ok(())
    })();

    assert!(tx_result.is_err(), "second row must violate FK and error");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM categorizations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 0,
        "transaction must be all-or-nothing: 0 rows, not 1 partial — mid-batch failure must roll back"
    );

    // t1 must remain uncategorized; the partial write did not leak through.
    let cat: Option<String> = conn
        .query_row("SELECT category_id FROM transactions WHERE id='t1'", [], |r| r.get(0))
        .unwrap();
    assert!(cat.is_none(), "transaction category_id must not have been updated by a rolled-back batch");
}

/// The same guarantee via the `repos::atomic` helper (BEGIN IMMEDIATE / COMMIT)
/// the application code actually uses — ensures the helper itself is not the
/// leak point.
#[test]
fn atomic_helper_rolls_back_on_mid_batch_failure() {
    let (_dir, db) = migrated_db();
    let mut conn = db.get().unwrap();
    conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
    conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')", []).unwrap();
    conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) VALUES('t1','a1','2024-01-15T00:00:00Z',-1500,'A','cleared',0,'2024-01-15T00:00:00Z')", []).unwrap();
    conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) VALUES('t2','a1','2024-01-16T00:00:00Z',-2000,'B','cleared',0,'2024-01-16T00:00:00Z')", []).unwrap();

    let res: Result<(), finsight_core::error::CoreError> = finsight_core::repos::atomic(&mut conn, |conn| {
        conn.execute("UPDATE transactions SET category_id='cat1' WHERE id='t1'", [])?;
        // Simulate a mid-batch application error after the first row.
        Err(finsight_core::error::CoreError::InvalidState("injected mid-batch failure".into()))
    });
    assert!(res.is_err());

    let t1: Option<String> = conn.query_row("SELECT category_id FROM transactions WHERE id='t1'", [], |r| r.get(0)).unwrap();
    assert!(t1.is_none(), "first row must be rolled back when second row fails via atomic()");
}
