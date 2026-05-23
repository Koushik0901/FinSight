use crate::error::CoreResult;
use crate::models::{Account, AccountSummary, AccountType, NewAccount};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Insert a new account and seed today's `account_balances` row with the
/// opening balance. Must be called at most once per account-id; the seed
/// row's PK is (account_id, as_of_date), so a same-day repeat would fail.
pub fn insert(conn: &mut Connection, input: NewAccount) -> CoreResult<Account> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO accounts (id, owner, bank, type, name, last4, currency, color, source, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
    })
}

pub fn list_summaries(conn: &mut Connection) -> CoreResult<Vec<AccountSummary>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.owner, a.bank, a.type, a.name, a.currency, a.color, \
                COALESCE((SELECT balance_cents FROM account_balances b \
                          WHERE b.account_id = a.id ORDER BY as_of_date DESC LIMIT 1), 0) AS balance, \
                a.source \
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
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}
