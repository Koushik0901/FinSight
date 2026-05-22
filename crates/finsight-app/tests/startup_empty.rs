//! After Task 5, a fresh DB on app startup MUST stay empty — the walking-skeleton
//! seed call was removed. This test reproduces the startup chain (open + migrate)
//! and asserts the accounts table is empty.

use finsight_core::{db::run_migrations, keychain, Db};
use tempfile::TempDir;

#[test]
fn fresh_db_after_startup_has_no_accounts() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("startup.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    // NOTE: We deliberately do NOT call seed::walking_skeleton here; this mirrors
    // what configure_app() does in production after Task 5.

    let count: i64 = db.get().unwrap()
        .query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0, "expected 0 accounts after fresh migration (no auto-seed)");
}
