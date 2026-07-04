use crate::error::CoreResult;
use crate::models::{
    Account, AccountBalancePoint, AccountPatch, AccountSparkline, AccountSummary, AccountType,
    NewAccount,
};
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
        "INSERT INTO accounts (id, owner, bank, type, name, last4, currency, color, source, liquidity_type, emergency_fund_eligible, goal_earmark, apy_pct, created_at, simplefin_account_id, nickname) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
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
            &input.simplefin_account_id,
            &input.nickname,
        ],
    )?;

    // Seed today's balance row with source='seed' so it is distinguishable from
    // a user-confirmed, synced, or recompute-derived balance. An untouched seed
    // is NOT treated as a trustworthy current balance once the account also has
    // transaction history it doesn't account for (see `list_summaries` /
    // `recompute_balance_if_linked`).
    conn.execute(
        "INSERT INTO account_balances (account_id, as_of_date, balance_cents, source) \
         VALUES (?1, ?2, ?3, 'seed')",
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
        simplefin_account_id: input.simplefin_account_id,
        last_synced_at: None,
        nickname: input.nickname,
        connection_id: input.connection_id,
        institution_id: input.institution_id,
        external_account_id: input.external_account_id,
        official_name: input.official_name,
        mask: input.mask,
        subtype: input.subtype,
        account_group: input.account_group,
        available_balance_cents: input.available_balance_cents,
        balance_date: input.balance_date,
        extra_json: input.extra_json,
        raw_json: input.raw_json,
        import_pending: input.import_pending,
    })
}

pub fn list_summaries(conn: &mut Connection) -> CoreResult<Vec<AccountSummary>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.owner, a.bank, a.type, a.name, a.currency, a.color, \
                COALESCE((SELECT balance_cents FROM account_balances b \
                          WHERE b.account_id = a.id \
                          ORDER BY b.as_of_date DESC, \
                            CASE b.source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END \
                          LIMIT 1), 0) AS balance, \
                a.source, a.liquidity_type, a.emergency_fund_eligible, a.goal_earmark, a.apy_pct, \
                a.simplefin_account_id, a.last_synced_at, a.nickname, \
                a.connection_id, a.institution_id, a.external_account_id, a.official_name, a.mask, \
                a.subtype, a.account_group, a.available_balance_cents, a.balance_date, a.extra_json, \
                a.raw_json, a.import_pending, \
                CASE \
                  WHEN EXISTS (SELECT 1 FROM account_balances b \
                               WHERE b.account_id = a.id AND b.source <> 'seed') THEN 1 \
                  WHEN NOT EXISTS (SELECT 1 FROM transactions t WHERE t.account_id = a.id) THEN 1 \
                  ELSE 0 \
                END AS balance_known \
         FROM accounts a \
         WHERE a.archived_at IS NULL \
         ORDER BY a.bank, a.name",
    )?;
    let rows = stmt.query_map([], |r| {
        let last_synced_s: Option<String> = r.get(14)?;
        let balance_date_s: Option<String> = r.get(24)?;
        Ok(AccountSummary {
            id: r.get(0)?,
            owner: r.get(1)?,
            bank: r.get(2)?,
            r#type: AccountType::from_db(&r.get::<_, String>(3)?),
            name: r.get(4)?,
            currency: r.get(5)?,
            color: r.get(6)?,
            balance_cents: r.get(7)?,
            balance_known: r.get::<_, i64>(28)? != 0,
            source: r.get(8)?,
            liquidity_type: r.get(9)?,
            emergency_fund_eligible: r.get::<_, i64>(10)? != 0,
            goal_earmark: r.get(11)?,
            apy_pct: r.get(12)?,
            simplefin_account_id: r.get(13)?,
            last_synced_at: last_synced_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            nickname: r.get(15)?,
            connection_id: r.get(16)?,
            institution_id: r.get(17)?,
            external_account_id: r.get(18)?,
            official_name: r.get(19)?,
            mask: r.get(20)?,
            subtype: r.get(21)?,
            account_group: r.get(22)?,
            available_balance_cents: r.get(23)?,
            balance_date: balance_date_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            extra_json: r.get(25)?,
            raw_json: r.get(26)?,
            import_pending: r.get::<_, i64>(27)? != 0,
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
    if let Some(nickname) = &patch.nickname {
        conn.execute(
            "UPDATE accounts SET nickname = ?1 WHERE id = ?2",
            params![nickname, id],
        )?;
    }
    get_by_id(conn, id)
}

pub fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Account> {
    conn.query_row(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, \
                liquidity_type, emergency_fund_eligible, goal_earmark, apy_pct, created_at, \
                simplefin_account_id, last_synced_at, nickname, connection_id, institution_id, \
                external_account_id, official_name, mask, subtype, account_group, \
                available_balance_cents, balance_date, extra_json, raw_json, import_pending \
         FROM accounts WHERE id = ?1",
        params![id],
        |r| {
            let archived_s: Option<String> = r.get(8)?;
            let created_s: String = r.get(13)?;
            let last_synced_s: Option<String> = r.get(15)?;
            let balance_date_s: Option<String> = r.get(25)?;
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
                simplefin_account_id: r.get(14)?,
                last_synced_at: last_synced_s.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                nickname: r.get(16)?,
                connection_id: r.get(17)?,
                institution_id: r.get(18)?,
                external_account_id: r.get(19)?,
                official_name: r.get(20)?,
                mask: r.get(21)?,
                subtype: r.get(22)?,
                account_group: r.get(23)?,
                available_balance_cents: r.get(24)?,
                balance_date: balance_date_s.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                extra_json: r.get(26)?,
                raw_json: r.get(27)?,
                import_pending: r.get::<_, i64>(28)? != 0,
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

/// Recompute today's balance snapshot for manual accounts after a transaction
/// change. Linked (SimpleFin) accounts are skipped because their balance is
/// bank-reported during sync.
pub fn recompute_balance_if_linked(conn: &mut Connection, account_id: &str) -> CoreResult<()> {
    let is_linked: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND simplefin_account_id IS NOT NULL)",
        params![account_id],
        |r| r.get(0),
    )?;
    if is_linked {
        return Ok(());
    }

    // Respect an explicit balance the user set themselves — never overwrite it
    // with a derived estimate. (Seed and our own 'derived' snapshots don't count.)
    let user_set_balance: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM account_balances \
         WHERE account_id = ?1 AND source NOT IN ('seed', 'derived'))",
        params![account_id],
        |r| r.get(0),
    )?;
    if user_set_balance {
        return Ok(());
    }

    // The earliest balance snapshot is the "opening" baseline for this account.
    let (opening_balance, opening_date, opening_source): (i64, String, Option<String>) = conn
        .query_row(
            "SELECT COALESCE(balance_cents, 0), as_of_date, source FROM account_balances \
             WHERE account_id = ?1 ORDER BY as_of_date ASC, rowid ASC LIMIT 1",
            params![account_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, "1970-01-01".to_string(), None));

    // With no activity, the seed value is itself the balance — leave it alone.
    let txn_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE account_id = ?1 AND pending = 0",
        params![account_id],
        |r| r.get(0),
    )?;
    if txn_count == 0 {
        return Ok(());
    }

    // Derive the current balance (YNAB/Actual model): the entered opening balance
    // is the anchor *before* the account's history. When the only baseline is the
    // account-creation seed (dated at creation, typically *after* an imported
    // back-history), treat the opening as pre-history and fold in ALL activity.
    // When there's a real prior baseline, only add activity dated after it so we
    // don't double-count. Written as a distinct 'derived' snapshot so it reads as
    // known but a later user-set balance still wins.
    let only_seed_baseline = opening_source.as_deref() == Some("seed");
    let txn_sum: i64 = if only_seed_baseline {
        conn.query_row(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions \
             WHERE account_id = ?1 AND pending = 0",
            params![account_id],
            |r| r.get(0),
        )?
    } else {
        conn.query_row(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions \
             WHERE account_id = ?1 AND pending = 0 AND date(posted_at) > ?2",
            params![account_id, opening_date],
            |r| r.get(0),
        )?
    };

    let today = Utc::now().date_naive().to_string();
    upsert_balance_snapshot(
        conn,
        account_id,
        &today,
        opening_balance + txn_sum,
        None,
        Some("derived"),
    )?;
    Ok(())
}

pub fn list_by_connection_id(
    conn: &mut Connection,
    connection_id: &str,
) -> CoreResult<Vec<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, liquidity_type, \
                emergency_fund_eligible, goal_earmark, apy_pct, created_at, simplefin_account_id, \
                last_synced_at, nickname, connection_id, institution_id, external_account_id, \
                official_name, mask, subtype, account_group, available_balance_cents, balance_date, \
                extra_json, raw_json, import_pending \
         FROM accounts WHERE connection_id = ?1 AND archived_at IS NULL",
    )?;
    let rows = stmt.query_map(params![connection_id], |r| {
        let archived_s: Option<String> = r.get(8)?;
        let created_s: String = r.get(13)?;
        let last_synced_s: Option<String> = r.get(15)?;
        let balance_date_s: Option<String> = r.get(25)?;
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
            simplefin_account_id: r.get(14)?,
            last_synced_at: last_synced_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            nickname: r.get(16)?,
            connection_id: r.get(17)?,
            institution_id: r.get(18)?,
            external_account_id: r.get(19)?,
            official_name: r.get(20)?,
            mask: r.get(21)?,
            subtype: r.get(22)?,
            account_group: r.get(23)?,
            available_balance_cents: r.get(24)?,
            balance_date: balance_date_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            extra_json: r.get(26)?,
            raw_json: r.get(27)?,
            import_pending: r.get::<_, i64>(28)? != 0,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Upsert an account by its SimpleFin ID. If an account with the same
/// `simplefin_account_id` already exists, update metadata while preserving
/// the user-set nickname; otherwise insert a new account.
pub fn upsert_simplefin_account(conn: &mut Connection, input: NewAccount) -> CoreResult<Account> {
    let simplefin_id = input.simplefin_account_id.clone().ok_or_else(|| {
        crate::CoreError::InvalidState("simplefin_account_id is required for upsert".into())
    })?;

    if let Some(existing) = get_by_simplefin_id(conn, &simplefin_id)? {
        let patch = AccountPatch {
            name: Some(input.name),
            bank: Some(input.bank),
            account_type: Some(input.r#type),
            color: Some(input.color),
            last4: Some(input.last4),
            currency: Some(input.currency),
            liquidity_type: Some(input.liquidity_type),
            emergency_fund_eligible: Some(input.emergency_fund_eligible),
            goal_earmark: Some(input.goal_earmark),
            apy_pct: Some(input.apy_pct),
            nickname: Some(input.nickname.or(existing.nickname)),
            official_name: Some(input.official_name),
            subtype: Some(input.subtype),
            account_group: Some(input.account_group),
            import_pending: Some(input.import_pending),
        };
        update(conn, &existing.id, patch)
    } else {
        insert(conn, input)
    }
}

pub fn get_by_simplefin_id(
    conn: &mut Connection,
    simplefin_id: &str,
) -> CoreResult<Option<Account>> {
    let mut stmt = conn.prepare(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, liquidity_type, \
                emergency_fund_eligible, goal_earmark, apy_pct, created_at, simplefin_account_id, \
                last_synced_at, nickname, connection_id, institution_id, external_account_id, \
                official_name, mask, subtype, account_group, available_balance_cents, balance_date, \
                extra_json, raw_json, import_pending \
         FROM accounts WHERE simplefin_account_id = ?1 AND archived_at IS NULL",
    )?;
    let mut rows = stmt.query_map(params![simplefin_id], |r| {
        let archived_s: Option<String> = r.get(8)?;
        let created_s: String = r.get(13)?;
        let last_synced_s: Option<String> = r.get(15)?;
        let balance_date_s: Option<String> = r.get(25)?;
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
            simplefin_account_id: r.get(14)?,
            last_synced_at: last_synced_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            nickname: r.get(16)?,
            connection_id: r.get(17)?,
            institution_id: r.get(18)?,
            external_account_id: r.get(19)?,
            official_name: r.get(20)?,
            mask: r.get(21)?,
            subtype: r.get(22)?,
            account_group: r.get(23)?,
            available_balance_cents: r.get(24)?,
            balance_date: balance_date_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            extra_json: r.get(26)?,
            raw_json: r.get(27)?,
            import_pending: r.get::<_, i64>(28)? != 0,
        })
    })?;
    Ok(rows.next().transpose()?)
}

/// Insert or update a daily balance snapshot for an account.
/// The PK is (account_id, as_of_date, source), so multiple sources per day
/// are allowed; readers should prefer `simplefin` over `manual`.
pub fn upsert_balance_snapshot(
    conn: &mut Connection,
    account_id: &str,
    as_of_date: &str,
    balance_cents: i64,
    available_balance_cents: Option<i64>,
    source: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO account_balances (account_id, as_of_date, balance_cents, available_balance_cents, source) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(account_id, as_of_date, source) DO UPDATE SET \
             balance_cents = excluded.balance_cents,
             available_balance_cents = excluded.available_balance_cents",
        params![
            account_id,
            as_of_date,
            balance_cents,
            available_balance_cents,
            source.unwrap_or("manual"),
        ],
    )?;
    Ok(())
}

/// Return the last `days` of balance history for one account. When multiple
/// sources exist for the same date, `simplefin` is preferred over `manual`.
pub fn list_balance_history(
    conn: &mut Connection,
    account_id: &str,
    days: u32,
) -> CoreResult<Vec<AccountBalancePoint>> {
    let cutoff = format!("-{} days", days);
    let mut stmt = conn.prepare(
        "SELECT as_of_date, balance_cents, source FROM account_balances \
         WHERE account_id = ?1 AND as_of_date >= date('now', ?2) \
         ORDER BY as_of_date ASC, CASE source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END",
    )?;
    let rows = stmt.query_map(params![account_id, cutoff], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;

    let mut out = Vec::new();
    let mut last_date: Option<String> = None;
    for row in rows {
        let (date, balance_cents) = row?;
        if last_date.as_ref() != Some(&date) {
            out.push(AccountBalancePoint {
                date,
                balance_cents,
            });
            last_date = out.last().map(|p: &AccountBalancePoint| p.date.clone());
        }
    }
    Ok(out)
}

/// Return a sparkline series for every non-archived account.
pub fn list_all_balance_sparklines(
    conn: &mut Connection,
    days: u32,
) -> CoreResult<Vec<AccountSparkline>> {
    let cutoff = format!("-{} days", days);
    let mut stmt = conn.prepare(
        "SELECT a.id, b.as_of_date, b.balance_cents, b.source \
         FROM accounts a \
         LEFT JOIN account_balances b \
            ON b.account_id = a.id AND b.as_of_date >= date('now', ?1) \
         WHERE a.archived_at IS NULL \
         ORDER BY a.id, b.as_of_date ASC, CASE b.source WHEN 'simplefin' THEN 0 ELSE 1 END",
    )?;
    let rows = stmt.query_map(params![cutoff], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, Option<i64>>(2)?,
        ))
    })?;

    let mut out: Vec<AccountSparkline> = Vec::new();
    for row in rows {
        let (account_id, maybe_date, maybe_balance) = row?;
        if let (Some(date), Some(balance_cents)) = (maybe_date, maybe_balance) {
            if let Some(series) = out
                .last_mut()
                .filter(|s: &&mut AccountSparkline| s.account_id == account_id)
            {
                if series
                    .points
                    .last()
                    .map(|p: &AccountBalancePoint| p.date.clone())
                    != Some(date.clone())
                {
                    series.points.push(AccountBalancePoint {
                        date,
                        balance_cents,
                    });
                }
            } else {
                out.push(AccountSparkline {
                    account_id,
                    points: vec![AccountBalancePoint {
                        date,
                        balance_cents,
                    }],
                });
            }
        } else {
            // Account with no balance history in window: still include it with empty points
            // so callers know the account exists.
            if out.last().map(|s: &AccountSparkline| s.account_id.clone())
                != Some(account_id.clone())
            {
                out.push(AccountSparkline {
                    account_id,
                    points: vec![],
                });
            }
        }
    }
    Ok(out)
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

    #[test]
    fn list_balance_history_returns_simplefin_over_manual_for_same_date() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let today = chrono::Utc::now().date_naive().to_string();
        upsert_balance_snapshot(&mut conn, &acc.id, &today, 100_000, None, Some("manual")).unwrap();
        upsert_balance_snapshot(&mut conn, &acc.id, &today, 250_000, None, Some("simplefin"))
            .unwrap();
        let hist = list_balance_history(&mut conn, &acc.id, 30).unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].balance_cents, 250_000);
        assert_eq!(hist[0].date, today);
    }

    #[test]
    fn list_balance_history_returns_opening_balance_when_only_seed_snapshot() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let hist = list_balance_history(&mut conn, &acc.id, 30).unwrap();
        // insert() seeds today's row with the opening balance, so there is
        // always at least one snapshot.
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].balance_cents, 0);
    }

    #[test]
    fn list_all_balance_sparklines_groups_by_account() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let a = sample_account(&mut conn);
        let b = insert(
            &mut conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Savings,
                name: "Savings".into(),
                last4: None,
                currency: "USD".into(),
                color: "#0f0".into(),
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
        let today = chrono::Utc::now().date_naive().to_string();
        upsert_balance_snapshot(&mut conn, &a.id, &today, 100_000, None, Some("simplefin"))
            .unwrap();
        upsert_balance_snapshot(&mut conn, &b.id, &today, 500_000, None, Some("simplefin"))
            .unwrap();
        let series = list_all_balance_sparklines(&mut conn, 30).unwrap();
        assert_eq!(series.len(), 2);
        let a_series = series.iter().find(|s| s.account_id == a.id).unwrap();
        let b_series = series.iter().find(|s| s.account_id == b.id).unwrap();
        assert_eq!(a_series.points.len(), 1);
        assert_eq!(a_series.points[0].balance_cents, 100_000);
        assert_eq!(b_series.points[0].balance_cents, 500_000);
    }

    /// Inserts a cleared transaction directly (bypassing the higher-level repo
    /// so the test controls the posted date precisely).
    fn insert_txn(conn: &Connection, account_id: &str, cents: i64, posted_date: &str) {
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, status, created_at) \
             VALUES(?1, ?2, ?3, ?4, 'X', 'cleared', ?5)",
            params![
                Uuid::new_v4().to_string(),
                account_id,
                format!("{posted_date}T12:00:00+00:00"),
                cents,
                Utc::now().to_rfc3339(),
            ],
        )
        .unwrap();
    }

    #[test]
    fn recompute_derives_balance_from_opening_plus_history_after_csv_import() {
        // The user chose the YNAB/Actual model: the entered opening balance is
        // the anchor before the imported history, so current = opening + all
        // activity. An account whose only baseline is the same-day creation seed
        // ($0), with all transactions in the past, derives a real balance.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn); // seed: $0 @ today
        insert_txn(&conn, &acc.id, -8_432, "2023-06-01");
        insert_txn(&conn, &acc.id, 500_000, "2023-06-15");

        recompute_balance_if_linked(&mut conn, &acc.id).unwrap();

        // A 'derived' snapshot is written: 0 + (-8_432 + 500_000).
        let derived: i64 = conn
            .query_row(
                "SELECT balance_cents FROM account_balances WHERE account_id = ?1 AND source = 'derived'",
                params![acc.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(derived, 491_568);

        // ...and the summary reports the derived balance as known.
        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert!(summary.balance_known, "derived balance must read as known");
        assert_eq!(summary.balance_cents, 491_568);
    }

    #[test]
    fn user_set_balance_wins_over_derived_and_survives_reimport() {
        // A balance the user set explicitly must never be clobbered by a later
        // re-derivation on import.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        insert_txn(&conn, &acc.id, -8_432, "2023-06-01");
        recompute_balance_if_linked(&mut conn, &acc.id).unwrap(); // derived

        let today = Utc::now().date_naive().to_string();
        upsert_balance_snapshot(&mut conn, &acc.id, &today, 999_99, None, Some("manual")).unwrap();

        // Simulate a later import re-running recompute — must not overwrite.
        insert_txn(&conn, &acc.id, -1_000, "2024-01-01");
        recompute_balance_if_linked(&mut conn, &acc.id).unwrap();

        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert!(summary.balance_known);
        assert_eq!(summary.balance_cents, 999_99, "user-set balance must win");
    }

    #[test]
    fn balance_known_true_once_user_sets_a_balance_after_import() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        insert_txn(&conn, &acc.id, -8_432, "2023-06-01");
        recompute_balance_if_linked(&mut conn, &acc.id).unwrap();

        // User confirms a real balance (this is what set_account_balance does).
        let today = Utc::now().date_naive().to_string();
        upsert_balance_snapshot(&mut conn, &acc.id, &today, 1_234_56, None, Some("manual")).unwrap();

        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert!(summary.balance_known);
        assert_eq!(summary.balance_cents, 1_234_56);
    }

    #[test]
    fn balance_known_true_for_account_with_no_transactions() {
        // A fresh account with a seeded opening balance and no activity: its
        // seed value IS the truth, so the balance is known.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert!(summary.balance_known);
        assert_eq!(summary.balance_cents, 0);
    }

    #[test]
    fn recompute_still_tracks_manual_account_with_future_dated_activity() {
        // Regression guard: the fix must not break the normal manual-entry path
        // where transactions are dated after the opening baseline.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn); // seed $0 @ today
        let tomorrow = (Utc::now().date_naive() + chrono::Duration::days(1)).to_string();
        insert_txn(&conn, &acc.id, 250_000, &tomorrow);

        recompute_balance_if_linked(&mut conn, &acc.id).unwrap();

        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert!(summary.balance_known);
        assert_eq!(summary.balance_cents, 250_000);
    }
}
