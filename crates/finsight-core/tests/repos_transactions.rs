use chrono::Utc;
use finsight_core::models::{
    AccountType, NewAccount, NewTransaction, TransactionStatus, TxnActivity,
};
use finsight_core::repos::{accounts, transactions};
use finsight_core::Db;
use tempfile::tempdir;

fn open() -> (Db, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let path = dir.path().join("t.sqlcipher");
    let key = "cd".repeat(32);
    let db = Db::open(&path, &key).unwrap();
    finsight_core::db::run_migrations(&db).unwrap();
    (db, dir)
}

#[test]
fn insert_and_list_returns_descending_by_posted_at() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let acct = accounts::insert(
        &mut conn,
        NewAccount {
            promo_apr_expires_on: None,
            post_promo_apr_pct: None,
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "X".into(),
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
            apr_pct: None,
            min_payment_cents: None,
            payoff_date: None,
            limit_cents: None,
            original_balance_cents: None,
            started_at: None,
        },
    )
    .unwrap();

    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: acct.id.clone(),
            posted_at: Utc::now() - chrono::Duration::days(2),
            amount_cents: -4200,
            merchant_raw: "Older".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
            imported_id: None,
            source: None,
            raw_synced_data: None,
            pending: false,
            external_tx_id: None,
            external_account_id: None,
            activity: None,
        },
    )
    .unwrap();
    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: acct.id,
            posted_at: Utc::now(),
            amount_cents: -1234,
            merchant_raw: "Newer".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
            imported_id: None,
            source: None,
            raw_synced_data: None,
            pending: false,
            external_tx_id: None,
            external_account_id: None,
            activity: None,
        },
    )
    .unwrap();

    let list = transactions::list(&mut conn, transactions::TxnFilter::default()).unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].merchant_raw, "Newer");
    assert_eq!(list[1].merchant_raw, "Older");
}

#[test]
fn list_filtered_by_account_id_only_returns_that_account_txns() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let a = accounts::insert(
        &mut conn,
        NewAccount {
            promo_apr_expires_on: None,
            post_promo_apr_pct: None,
            owner: "mira".into(),
            bank: "Schwab".into(),
            r#type: AccountType::Checking,
            name: "A".into(),
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
            apr_pct: None,
            min_payment_cents: None,
            payoff_date: None,
            limit_cents: None,
            original_balance_cents: None,
            started_at: None,
        },
    )
    .unwrap();
    let b = accounts::insert(
        &mut conn,
        NewAccount {
            promo_apr_expires_on: None,
            post_promo_apr_pct: None,
            owner: "adam".into(),
            bank: "Chase".into(),
            r#type: AccountType::Checking,
            name: "B".into(),
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
            apr_pct: None,
            min_payment_cents: None,
            payoff_date: None,
            limit_cents: None,
            original_balance_cents: None,
            started_at: None,
        },
    )
    .unwrap();

    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: a.id.clone(),
            posted_at: Utc::now(),
            amount_cents: -100,
            merchant_raw: "OnlyA".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
            imported_id: None,
            source: None,
            raw_synced_data: None,
            pending: false,
            external_tx_id: None,
            external_account_id: None,
            activity: None,
        },
    )
    .unwrap();
    transactions::insert(
        &mut conn,
        NewTransaction {
            account_id: b.id.clone(),
            posted_at: Utc::now(),
            amount_cents: -200,
            merchant_raw: "OnlyB".into(),
            category_id: None,
            notes: None,
            status: TransactionStatus::Cleared,
            imported_id: None,
            source: None,
            raw_synced_data: None,
            pending: false,
            external_tx_id: None,
            external_account_id: None,
            activity: None,
        },
    )
    .unwrap();

    let filtered = transactions::list(
        &mut conn,
        transactions::TxnFilter {
            account_id: Some(a.id.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].merchant_raw, "OnlyA");
}

#[test]
fn list_respects_limit() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let acct = accounts::insert(
        &mut conn,
        NewAccount {
            promo_apr_expires_on: None,
            post_promo_apr_pct: None,
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "X".into(),
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
            apr_pct: None,
            min_payment_cents: None,
            payoff_date: None,
            limit_cents: None,
            original_balance_cents: None,
            started_at: None,
        },
    )
    .unwrap();
    for i in 0..5 {
        transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acct.id.clone(),
                posted_at: Utc::now() - chrono::Duration::seconds(i),
                amount_cents: -1,
                merchant_raw: format!("Txn{i}"),
                category_id: None,
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: None,
                raw_synced_data: None,
                pending: false,
                external_tx_id: None,
                external_account_id: None,
                activity: None,
            },
        )
        .unwrap();
    }
    let limited = transactions::list(
        &mut conn,
        transactions::TxnFilter {
            account_id: None,
            limit: 2,
            offset: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(limited.len(), 2);
}

fn seed_investment_account(conn: &mut rusqlite::Connection) -> String {
    accounts::insert(
        conn,
        NewAccount {
            promo_apr_expires_on: None,
            post_promo_apr_pct: None,
            owner: "joint".into(),
            bank: "Wealthsimple".into(),
            r#type: AccountType::Investment,
            name: "TFSA".into(),
            last4: None,
            currency: "CAD".into(),
            color: "#fff".into(),
            opening_balance_cents: 0,
            source: "manual".into(),
            liquidity_type: "invested".into(),
            emergency_fund_eligible: false,
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
            account_group: "investments".into(),
            available_balance_cents: None,
            balance_date: None,
            extra_json: None,
            raw_json: None,
            import_pending: false,
            apr_pct: None,
            min_payment_cents: None,
            payoff_date: None,
            limit_cents: None,
            original_balance_cents: None,
            started_at: None,
        },
    )
    .unwrap()
    .id
}

fn mk_activity_txn(account_id: &str, activity: Option<TxnActivity>) -> NewTransaction {
    NewTransaction {
        account_id: account_id.to_string(),
        posted_at: Utc::now(),
        amount_cents: -12_260,
        merchant_raw: "Buy ACME".into(),
        category_id: None,
        notes: None,
        status: TransactionStatus::Cleared,
        imported_id: None,
        source: Some("csv".into()),
        raw_synced_data: None,
        pending: false,
        external_tx_id: None,
        external_account_id: None,
        activity,
    }
}

#[test]
fn insert_with_activity_persists_and_hydrates_and_flags_transfer() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let acct = seed_investment_account(&mut conn);

    let trade = TxnActivity {
        activity_type: "Trade".into(),
        activity_sub_type: Some("BUY".into()),
        symbol: Some("ACME".into()),
        security_name: Some("Acme Corp".into()),
        quantity: Some(8.1234),
        unit_price: Some(15.0876),
    };
    let inserted =
        transactions::insert(&mut conn, mk_activity_txn(&acct, Some(trade.clone()))).unwrap();
    // Trade rows are internal cash↔security moves: flagged at insert time.
    assert!(inserted.is_transfer);
    assert_eq!(inserted.activity, Some(trade.clone()));

    let list = transactions::list(&mut conn, transactions::TxnFilter::default()).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].is_transfer);
    assert_eq!(list[0].activity, Some(trade));
}

#[test]
fn insert_dividend_activity_is_not_a_transfer() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let acct = seed_investment_account(&mut conn);

    let dividend = TxnActivity {
        activity_type: "Dividend".into(),
        activity_sub_type: None,
        symbol: Some("GLOBEX".into()),
        security_name: Some("Globex Corp".into()),
        quantity: None,
        unit_price: None,
    };
    let inserted =
        transactions::insert(&mut conn, mk_activity_txn(&acct, Some(dividend))).unwrap();
    // Dividends are real income — must stay visible to income/expense metrics.
    assert!(!inserted.is_transfer);
}

#[test]
fn insert_without_activity_is_unchanged() {
    let (db, _dir) = open();
    let mut conn = db.get().unwrap();
    let acct = seed_investment_account(&mut conn);

    let inserted = transactions::insert(&mut conn, mk_activity_txn(&acct, None)).unwrap();
    assert!(!inserted.is_transfer);
    assert_eq!(inserted.activity, None);

    let list = transactions::list(&mut conn, transactions::TxnFilter::default()).unwrap();
    assert_eq!(list[0].activity, None);
}
