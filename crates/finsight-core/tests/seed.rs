use finsight_core::repos::{accounts, categories, transactions};

#[test]
fn seed_walks_skeleton_creates_expected_counts() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    finsight_core::seed::walking_skeleton(&db).unwrap();

    let mut conn = db.get().unwrap();
    assert_eq!(accounts::list_summaries(&mut conn).unwrap().len(), 1);
    assert_eq!(categories::list(&mut conn).unwrap().len(), 4);

    let txns = transactions::list(&mut conn, transactions::TxnFilter::default()).unwrap();
    assert_eq!(txns.len(), 3);
}

#[test]
fn seed_is_idempotent_no_duplicates() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    finsight_core::seed::walking_skeleton(&db).unwrap();
    finsight_core::seed::walking_skeleton(&db).unwrap();

    let mut conn = db.get().unwrap();
    assert_eq!(accounts::list_summaries(&mut conn).unwrap().len(), 1);
}

#[test]
fn seed_recovers_from_partial_reference_data() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    // Simulate "previous run wrote some reference data and crashed":
    // pre-populate one category group + one merchant with the same IDs the
    // seed uses, then run the seed.
    {
        let conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO category_groups (id, label, hint, sort_order) VALUES ('fixed', 'pre-existing', NULL, 1)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO merchants (id, canonical_name, color, initials) VALUES ('m_safeway', 'SF', '#000', 'SF')",
            [],
        ).unwrap();
    }

    finsight_core::seed::walking_skeleton(&db).unwrap();

    let mut conn = db.get().unwrap();
    assert_eq!(accounts::list_summaries(&mut conn).unwrap().len(), 1);
    // Category groups should still be 4 (the pre-existing 'fixed' kept its
    // original label since INSERT OR IGNORE preserves the prior row).
    let group_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM category_groups", [], |r| r.get(0))
        .unwrap();
    assert_eq!(group_count, 4);
}
