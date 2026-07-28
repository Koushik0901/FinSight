use finsight_core::{db::run_migrations, keychain, Db};
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let (dir, db) = finsight_core::testing::migrated_db();
    (dir, db)
}

#[tokio::test]
async fn get_needs_review_count_returns_zero_when_no_low_confidence() {
    let (_d, db) = fresh_db();
    // No transactions → count is 0
    let conn = db.get().unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions \
         WHERE ai_confidence < 0.6 \
           AND (SELECT source FROM categorizations c \
                WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}
