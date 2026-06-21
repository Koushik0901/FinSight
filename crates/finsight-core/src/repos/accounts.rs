use crate::error::CoreResult;
use crate::models::{Account, AccountPatch, AccountSummary, AccountType, NewAccount};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Insert a new account and seed today's `account_balances` row with the
/// opening balance. Must be called at most once per account-id; the seed
/// row's PK is (account_id, as_of_date), so a same-day repeat would fail.
pub fn insert(conn: &mut Connection, input: NewAccount) -> CoreResult<Account> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO accounts (id, owner, bank, type, name, last4, currency, color, source, liquidity_type, emergency_fund_eligible, goal_earmark, apy_pct, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            &id,
            &input.owner,
            &input.bank,
            input.r#type.as_db(),
            &input.name,
            &input.last4,
            &input.currency,
            &input.color,
            &input.source,
            &input.liquidity_type,
            input.emergency_fund_eligible,
            &input.goal_earmark,
            &input.apy_pct,
            now.to_rfc3339(),
        ],
    )?;

    // Seed today's balance row.
    conn.execute(
        "INSERT INTO account_balances (account_id, as_of_date, balance_cents) VALUES (?1, ?2, ?3)",
        params![
            &id,
            now.date_naive().to_string(),
            input.opening_balance_cents
        ],
    )?;

    Ok(Account {
        id,
        owner: input.owner,
        bank: input.bank,
        r#type: input.r#type,
        name: input.name,
        last4: input.last4,
        currency: input.currency,
        color: input.color,
        archived_at: None,
        liquidity_type: input.liquidity_type,
        emergency_fund_eligible: input.emergency_fund_eligible,
        goal_earmark: input.goal_earmark,
        apy_pct: input.apy_pct,
        created_at: now,
    })
}

pub fn list_summaries(conn: &mut Connection) -> CoreResult<Vec<AccountSummary>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.owner, a.bank, a.type, a.name, a.currency, a.color, \
                COALESCE((SELECT balance_cents FROM account_balances b \
                          WHERE b.account_id = a.id ORDER BY as_of_date DESC LIMIT 1), 0) AS balance, \
                a.source, a.liquidity_type, a.emergency_fund_eligible, a.goal_earmark, a.apy_pct \
         FROM accounts a \
         WHERE a.archived_at IS NULL \
         ORDER BY a.bank, a.name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(AccountSummary {
            id: r.get(0)?,
            owner: r.get(1)?,
            bank: r.get(2)?,
            r#type: AccountType::from_db(&r.get::<_, String>(3)?),
            name: r.get(4)?,
            currency: r.get(5)?,
            color: r.get(6)?,
            balance_cents: r.get(7)?,
            source: r.get(8)?,
            liquidity_type: r.get(9)?,
            emergency_fund_eligible: r.get::<_, i64>(10)? != 0,
            goal_earmark: r.get(11)?,
            apy_pct: r.get(12)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn update(conn: &mut Connection, id: &str, patch: AccountPatch) -> CoreResult<Account> {
    if let Some(name) = &patch.name {
        conn.execute(
            "UPDATE accounts SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
    }
    if let Some(bank) = &patch.bank {
        conn.execute(
            "UPDATE accounts SET bank = ?1 WHERE id = ?2",
            params![bank, id],
        )?;
    }
    if let Some(at) = &patch.account_type {
        conn.execute(
            "UPDATE accounts SET type = ?1 WHERE id = ?2",
            params![at.as_db(), id],
        )?;
    }
    if let Some(color) = &patch.color {
        conn.execute(
            "UPDATE accounts SET color = ?1 WHERE id = ?2",
            params![color, id],
        )?;
    }
    if let Some(last4) = &patch.last4 {
        conn.execute(
            "UPDATE accounts SET last4 = ?1 WHERE id = ?2",
            params![last4, id],
        )?;
    }
    if let Some(currency) = &patch.currency {
        conn.execute(
            "UPDATE accounts SET currency = ?1 WHERE id = ?2",
            params![currency, id],
        )?;
    }
    if let Some(liquidity_type) = &patch.liquidity_type {
        conn.execute(
            "UPDATE accounts SET liquidity_type = ?1 WHERE id = ?2",
            params![liquidity_type, id],
        )?;
    }
    if let Some(eligible) = patch.emergency_fund_eligible {
        conn.execute(
            "UPDATE accounts SET emergency_fund_eligible = ?1 WHERE id = ?2",
            params![eligible, id],
        )?;
    }
    if let Some(goal_earmark) = &patch.goal_earmark {
        conn.execute(
            "UPDATE accounts SET goal_earmark = ?1 WHERE id = ?2",
            params![goal_earmark, id],
        )?;
    }
    if let Some(apy_pct) = &patch.apy_pct {
        conn.execute(
            "UPDATE accounts SET apy_pct = ?1 WHERE id = ?2",
            params![apy_pct, id],
        )?;
    }
    // Return the updated account
    conn.query_row(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, liquidity_type, emergency_fund_eligible, goal_earmark, apy_pct, created_at \
         FROM accounts WHERE id = ?1",
        params![id],
        |r| {
            let archived_s: Option<String> = r.get(8)?;
            let created_s: String = r.get(13)?;
            Ok(Account {
                id: r.get(0)?,
                owner: r.get(1)?,
                bank: r.get(2)?,
                r#type: AccountType::from_db(&r.get::<_, String>(3)?),
                name: r.get(4)?,
                last4: r.get(5)?,
                currency: r.get(6)?,
                color: r.get(7)?,
                archived_at: archived_s.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                liquidity_type: r.get(9)?,
                emergency_fund_eligible: r.get::<_, i64>(10)? != 0,
                goal_earmark: r.get(11)?,
                apy_pct: r.get(12)?,
                created_at: DateTime::parse_from_rfc3339(&created_s)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        },
    )
    .map_err(Into::into)
}

pub fn archive(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE accounts SET archived_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    // Clean up stale CSV import mappings for this account
    conn.execute(
        "DELETE FROM csv_import_mappings WHERE account_id = ?1",
        params![id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, models::AccountType, models::NewAccount, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("a.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn sample_account(conn: &mut rusqlite::Connection) -> Account {
        insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#fff".into(),
                opening_balance_cents: 0,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
            },
        )
        .unwrap()
    }

    #[test]
    fn update_account_name() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let patch = AccountPatch {
            name: Some("New Name".into()),
            ..Default::default()
        };
        let updated = update(&mut conn, &acc.id, patch).unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.bank, "Bank"); // unchanged
    }

    #[test]
    fn update_account_planning_metadata() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let updated = update(
            &mut conn,
            &acc.id,
            AccountPatch {
                liquidity_type: Some("restricted".into()),
                emergency_fund_eligible: Some(false),
                goal_earmark: Some(Some("car".into())),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(updated.liquidity_type, "restricted");
        assert!(!updated.emergency_fund_eligible);
        assert_eq!(updated.goal_earmark.as_deref(), Some("car"));
        let summaries = list_summaries(&mut conn).unwrap();
        assert_eq!(summaries[0].liquidity_type, "restricted");
        assert!(!summaries[0].emergency_fund_eligible);
    }

    #[test]
    fn update_account_apy_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let updated = update(
            &mut conn,
            &acc.id,
            AccountPatch {
                apy_pct: Some(Some(4.5)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.apy_pct, Some(4.5));
        let summaries = list_summaries(&mut conn).unwrap();
        assert_eq!(summaries[0].apy_pct, Some(4.5));
    }

    #[test]
    fn archive_account_sets_archived_at() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        archive(&mut conn, &acc.id).unwrap();
        let archived_at: Option<String> = conn
            .query_row(
                "SELECT archived_at FROM accounts WHERE id = ?1",
                rusqlite::params![acc.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(archived_at.is_some());
    }
}
