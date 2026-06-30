use finsight_core::{
    db::run_migrations,
    keychain,
    models::{AccountType, NewAccount},
    repos::{accounts, run},
    Db,
};
use tempfile::TempDir;

fn fresh_db() -> (TempDir, Db) {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("ea.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();
    (dir, db)
}

#[tokio::test]
async fn update_account_name_and_color() {
    let (_d, db) = fresh_db();
    let account_id = {
        let mut conn = db.get().unwrap();
        accounts::insert(
            &mut conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Chase".into(),
                r#type: AccountType::Checking,
                name: "Old".into(),
                last4: None,
                currency: "USD".into(),
                color: "#000".into(),
                opening_balance_cents: 0,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "cash".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
            },
        )
        .unwrap()
        .id
    };
    let patch = finsight_core::models::AccountPatch {
        name: Some("New Name".into()),
        color: Some("#ff0000".into()),
        ..Default::default()
    };
    let updated = run(&db, move |conn| accounts::update(conn, &account_id, patch))
        .await
        .unwrap();
    assert_eq!(updated.name, "New Name");
    assert_eq!(updated.color, "#ff0000");
}

#[tokio::test]
async fn archive_account_cleans_up_mappings() {
    let (_d, db) = fresh_db();
    let account_id = {
        let mut conn = db.get().unwrap();
        let acc = accounts::insert(
            &mut conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Chase".into(),
                r#type: AccountType::Checking,
                name: "Acc".into(),
                last4: None,
                currency: "USD".into(),
                color: "#fff".into(),
                opening_balance_cents: 0,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "cash".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
            },
        )
        .unwrap();
        // Seed a fake csv_import_mappings row (last_used_at is NOT NULL)
        conn.execute(
            "INSERT INTO csv_import_mappings(account_id, mapping_json, last_used_at) VALUES(?1, '{}', '2024-01-01T00:00:00Z')",
            rusqlite::params![acc.id],
        ).unwrap();
        acc.id
    };
    run(&db, {
        let aid = account_id.clone();
        move |conn| accounts::archive(conn, &aid)
    })
    .await
    .unwrap();
    let conn = db.get().unwrap();
    let archived_at: Option<String> = conn
        .query_row(
            "SELECT archived_at FROM accounts WHERE id = ?1",
            rusqlite::params![account_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(archived_at.is_some());
    let mapping_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM csv_import_mappings WHERE account_id = ?1",
            rusqlite::params![account_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mapping_count, 0);
}
