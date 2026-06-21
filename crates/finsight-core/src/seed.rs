use crate::error::CoreResult;
use crate::models::{AccountType, NewAccount, NewTransaction, TransactionStatus};
use crate::repos::{accounts, transactions};
use crate::Db;
use chrono::{Duration, Utc};
use rusqlite::params;

/// Seed the walking-skeleton fixture: 4 category groups + 4 categories,
/// 3 merchants, 1 account, 3 transactions.
///
/// Idempotent: if any account already exists, returns immediately. Otherwise
/// uses `INSERT OR IGNORE` on the reference data (category groups, categories,
/// merchants) so partial state from an earlier interrupted run does not block
/// recovery — and then writes one account + three transactions.
pub fn walking_skeleton(db: &Db) -> CoreResult<()> {
    let mut conn = db.get()?;

    // Check if already seeded (any account exists)
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))?;
    if count > 0 {
        return Ok(());
    }

    let tx = conn.transaction()?;

    // Category groups
    for (id, label, hint, sort) in [
        (
            "fixed",
            "Fixed costs",
            Some("predictable, mostly recurring"),
            1,
        ),
        (
            "daily",
            "Daily life",
            Some("groceries, fuel, eating out"),
            2,
        ),
        ("lifestyle", "Lifestyle", Some("things you choose"), 3),
        ("wellbeing", "Wellbeing", Some("health, body, mind"), 4),
    ] {
        tx.execute(
            "INSERT OR IGNORE INTO category_groups (id, label, hint, sort_order) VALUES (?1, ?2, ?3, ?4)",
            params![id, label, hint, sort],
        )?;
    }

    // Categories
    for (id, group, label, color, icon) in [
        ("groceries", "daily", "Groceries", "#34D399", Some("🛒")),
        ("dining", "daily", "Dining", "#FB923C", Some("🍽")),
        ("transport", "daily", "Transport", "#60A5FA", Some("🚗")),
        ("subs", "fixed", "Subscriptions", "#F472B6", Some("📦")),
    ] {
        tx.execute(
            "INSERT OR IGNORE INTO categories (id, group_id, label, color, icon, sort_order) VALUES (?1,?2,?3,?4,?5,0)",
            params![id, group, label, color, icon],
        )?;
    }

    // Merchants
    for (id, name, color, initials) in [
        ("m_safeway", "Safeway", "#34D399", "SF"),
        ("m_mosswood", "Mosswood Café", "#FB923C", "MC"),
        ("m_netflix", "Netflix", "#F472B6", "NF"),
    ] {
        tx.execute(
            "INSERT OR IGNORE INTO merchants (id, canonical_name, color, initials) VALUES (?1,?2,?3,?4)",
            params![id, name, color, initials],
        )?;
    }

    tx.commit()?;

    // Account
    let acct = accounts::insert(
        &mut conn,
        NewAccount {
            owner: "joint".into(),
            bank: "Mercury".into(),
            r#type: AccountType::Checking,
            name: "Joint Checking".into(),
            last4: Some("4421".into()),
            currency: "USD".into(),
            color: "#C9F950".into(),
            opening_balance_cents: 1_482_042,
            source: "manual".into(),
            simplefin_account_id: None,
            nickname: None,
        },
    )?;

    // Transactions
    let now = Utc::now();
    for (offset_days, amount_cents, merchant, merchant_id, category) in [
        (1, -842_i64, "Safeway", "m_safeway", "groceries"),
        (2, -1420_i64, "Mosswood Café", "m_mosswood", "dining"),
        (3, -1599_i64, "Netflix", "m_netflix", "subs"),
    ] {
        let t = transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: acct.id.clone(),
                posted_at: now - Duration::days(offset_days),
                amount_cents,
                merchant_raw: merchant.into(),
                category_id: Some(category.into()),
                notes: None,
                status: TransactionStatus::Cleared,
                imported_id: None,
                source: Some("sample".to_string()),
            },
        )?;
        // Link merchant
        conn.execute(
            "UPDATE transactions SET merchant_id = ?1 WHERE id = ?2",
            params![merchant_id, t.id],
        )?;
    }

    Ok(())
}
