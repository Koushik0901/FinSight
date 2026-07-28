// We can't wire tauri::State in tests, so we test the underlying finsight_core pieces directly.
use finsight_core::{sample::seed_household, settings};

#[test]
fn fresh_db_reports_zero_then_sample_increments() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    {
        let conn = db.get().unwrap();
        let zero: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(zero, 0);
        let marked: Option<bool> = settings::get(&conn, "onboarding_completion_marked").unwrap();
        assert_eq!(marked, None);
    }

    let summary = seed_household(&db).unwrap();
    assert_eq!(summary.accounts_created, 6);

    {
        let conn = db.get().unwrap();
        let six: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(six, 6);
    }
}
