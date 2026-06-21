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
        "INSERT INTO accounts (id, owner, bank, type, name, last4, currency, color, source, created_at, simplefin_account_id, nickname) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
            now.to_rfc3339(),
            &input.simplefin_account_id,
            &input.nickname,
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
        created_at: now,
        simplefin_account_id: input.simplefin_account_id,
        last_synced_at: None,
        nickname: input.nickname,
    })
}

pub fn list_summaries(conn: &mut Connection) -> CoreResult<Vec<AccountSummary>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.owner, a.bank, a.type, a.name, a.currency, a.color, \
                COALESCE((SELECT balance_cents FROM account_balances b \
                          WHERE b.account_id = a.id ORDER BY as_of_date DESC LIMIT 1), 0) AS balance, \
                a.source, a.simplefin_account_id, a.last_synced_at, a.nickname \
         FROM accounts a \
         WHERE a.archived_at IS NULL \
         ORDER BY a.bank, a.name",
    )?;
    let rows = stmt.query_map([], |r| {
        let last_synced_s: Option<String> = r.get(10)?;
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
            simplefin_account_id: r.get(9)?,
            last_synced_at: last_synced_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            nickname: r.get(11)?,
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
    if let Some(nickname) = &patch.nickname {
        conn.execute(
            "UPDATE accounts SET nickname = ?1 WHERE id = ?2",
            params![nickname, id],
        )?;
    }
    // Return the updated account
    conn.query_row(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, created_at, \
                simplefin_account_id, last_synced_at, nickname \
         FROM accounts WHERE id = ?1",
        params![id],
        |r| {
            let archived_s: Option<String> = r.get(8)?;
            let created_s: String = r.get(9)?;
            let last_synced_s: Option<String> = r.get(11)?;
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
                created_at: DateTime::parse_from_rfc3339(&created_s)
                    .unwrap()
                    .with_timezone(&Utc),
                simplefin_account_id: r.get(10)?,
                last_synced_at: last_synced_s.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                nickname: r.get(12)?,
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

pub fn update_sync_metadata(
    conn: &mut Connection,
    id: &str,
    simplefin_account_id: Option<&str>,
    last_synced_at: Option<DateTime<Utc>>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE accounts SET simplefin_account_id = ?1, last_synced_at = ?2 WHERE id = ?3",
        params![
            simplefin_account_id,
            last_synced_at.map(|d| d.to_rfc3339()),
            id,
        ],
    )?;
    Ok(())
}

pub fn get_by_simplefin_id(conn: &mut Connection, simplefin_id: &str) -> CoreResult<Option<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, created_at, \
                simplefin_account_id, last_synced_at, nickname \
         FROM accounts WHERE simplefin_account_id = ?1 AND archived_at IS NULL",
    )?;
    let mut rows = stmt.query_map(params![simplefin_id], |r| {
        let archived_s: Option<String> = r.get(8)?;
        let created_s: String = r.get(9)?;
        let last_synced_s: Option<String> = r.get(11)?;
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
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .unwrap()
                .with_timezone(&Utc),
            simplefin_account_id: r.get(10)?,
            last_synced_at: last_synced_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            nickname: r.get(12)?,
        })
    })?;
    Ok(rows.next().transpose()?)
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
                simplefin_account_id: None,
                nickname: None,
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
