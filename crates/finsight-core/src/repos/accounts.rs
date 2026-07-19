use crate::error::CoreResult;
use crate::models::{
    Account, AccountBalancePoint, AccountBalanceTimeline, AccountPatch, AccountSparkline,
    AccountSummary, AccountType, BalanceAnchorQuality, NewAccount,
};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Insert a new account and seed today's `account_balances` row with the
/// opening balance. Must be called at most once per account-id; the seed
/// row's PK is (account_id, as_of_date), so a same-day repeat would fail.
pub fn insert(conn: &mut Connection, input: NewAccount) -> CoreResult<Account> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO accounts (id, owner, bank, type, name, last4, currency, color, source, liquidity_type, emergency_fund_eligible, goal_earmark, apy_pct, created_at, simplefin_account_id, nickname, apr_pct, min_payment_cents, payoff_date, limit_cents, original_balance_cents, started_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
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
            &input.apr_pct,
            &input.min_payment_cents,
            &input.payoff_date,
            &input.limit_cents,
            &input.original_balance_cents,
            &input.started_at,
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
        apr_pct: input.apr_pct,
        min_payment_cents: input.min_payment_cents,
        payoff_date: input.payoff_date,
        limit_cents: input.limit_cents,
        original_balance_cents: input.original_balance_cents,
        started_at: input.started_at,
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
                END AS balance_known, \
                a.apr_pct, a.min_payment_cents, a.payoff_date, a.limit_cents, \
                a.original_balance_cents, a.started_at, \
                (SELECT b.source FROM account_balances b \
                   WHERE b.account_id = a.id \
                   ORDER BY b.as_of_date DESC, \
                     CASE b.source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END \
                   LIMIT 1) AS balance_source \
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
            balance_source: r.get(35)?,
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
            apr_pct: r.get(29)?,
            min_payment_cents: r.get(30)?,
            payoff_date: r.get(31)?,
            limit_cents: r.get(32)?,
            original_balance_cents: r.get(33)?,
            started_at: r.get(34)?,
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
    if let Some(apr_pct) = &patch.apr_pct {
        conn.execute(
            "UPDATE accounts SET apr_pct = ?1 WHERE id = ?2",
            params![apr_pct, id],
        )?;
    }
    if let Some(min_payment_cents) = &patch.min_payment_cents {
        conn.execute(
            "UPDATE accounts SET min_payment_cents = ?1 WHERE id = ?2",
            params![min_payment_cents, id],
        )?;
    }
    if let Some(payoff_date) = &patch.payoff_date {
        conn.execute(
            "UPDATE accounts SET payoff_date = ?1 WHERE id = ?2",
            params![payoff_date, id],
        )?;
    }
    if let Some(limit_cents) = &patch.limit_cents {
        conn.execute(
            "UPDATE accounts SET limit_cents = ?1 WHERE id = ?2",
            params![limit_cents, id],
        )?;
    }
    if let Some(original_balance_cents) = &patch.original_balance_cents {
        conn.execute(
            "UPDATE accounts SET original_balance_cents = ?1 WHERE id = ?2",
            params![original_balance_cents, id],
        )?;
    }
    if let Some(started_at) = &patch.started_at {
        conn.execute(
            "UPDATE accounts SET started_at = ?1 WHERE id = ?2",
            params![started_at, id],
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
                available_balance_cents, balance_date, extra_json, raw_json, import_pending, \
                apr_pct, min_payment_cents, payoff_date, limit_cents, original_balance_cents, started_at \
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
                apr_pct: r.get(29)?,
                min_payment_cents: r.get(30)?,
                payoff_date: r.get(31)?,
                limit_cents: r.get(32)?,
                original_balance_cents: r.get(33)?,
                started_at: r.get(34)?,
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

    // An investment/brokerage account's value is its MARKET value — what the
    // brokerage says the holdings are worth — not the sum of its cash flows. A
    // fully-invested account nets ~$0 cash (money in as contributions, straight
    // out into securities), and folding transaction activity on top of a
    // user-entered market value would double-count it. So never derive an
    // investment account's balance; it comes from a market-value snapshot the
    // user (or a sync) sets. Importing the brokerage's activity CSV still gives a
    // useful contribution/trade history without corrupting the account value.
    let is_investment: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND type = 'Investment')",
        params![account_id],
        |r| r.get(0),
    )?;
    if is_investment {
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
    let anchor = opening_anchor(conn, account_id);

    // With no activity, the seed value is itself the balance — leave it alone.
    let txn_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE account_id = ?1 AND pending = 0",
        params![account_id],
        |r| r.get(0),
    )?;
    if txn_count == 0 {
        return Ok(());
    }

    // Derive the current balance (YNAB/Actual model) by folding activity onto the
    // opening anchor per `OpeningAnchor::fold_all_activity`. Written as a distinct
    // 'derived' snapshot so it reads as known but a later user-set balance still wins.
    let txn_sum: i64 = if anchor.fold_all_activity {
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
            params![account_id, anchor.date],
            |r| r.get(0),
        )?
    };

    let today = Utc::now().date_naive().to_string();
    upsert_balance_snapshot(
        conn,
        account_id,
        &today,
        anchor.balance_cents + txn_sum,
        None,
        Some("derived"),
    )?;
    Ok(())
}

/// The opening anchor a derived balance is built on: the earliest recorded
/// balance snapshot for an account.
///
/// The anchor's SOURCE decides how activity folds onto it. When the only
/// baseline is the account-creation seed (dated at creation, typically *after*
/// an imported back-history), the opening is treated as pre-history and ALL
/// activity is folded in. When a real prior baseline exists, only activity dated
/// after it counts, so nothing is double-counted.
///
/// Shared by [`recompute_balance_if_linked`] and [`balance_timeline`] so the
/// derived current balance and the reconstructed curve can never disagree about
/// where the account started.
struct OpeningAnchor {
    balance_cents: i64,
    date: String,
    fold_all_activity: bool,
}

/// The point a reconstructed curve is pinned to.
///
/// Prefers the LATEST user-confirmed or bank-reported balance, because that is
/// the most trustworthy thing known about the account and pinning to it keeps
/// the curve consistent with the balance every other screen displays. Only when
/// no confirmed balance exists does it fall back to the opening anchor.
///
/// Anchoring on the *earliest* row instead would ignore a later confirmation
/// entirely while still reporting the curve as calibrated against it.
struct CurveAnchor {
    balance_cents: i64,
    date: String,
    /// The anchor is the creation seed, which sits conceptually BEFORE all
    /// history — so no activity precedes it.
    fold_all_activity: bool,
    /// The anchor is a real confirmed balance, not an assumed opening.
    confirmed: bool,
}

fn curve_anchor(conn: &Connection, account_id: &str) -> CurveAnchor {
    let confirmed: Option<(i64, String)> = conn
        .query_row(
            "SELECT COALESCE(balance_cents, 0), as_of_date FROM account_balances \
             WHERE account_id = ?1 AND source NOT IN ('seed', 'derived') \
             ORDER BY as_of_date DESC, rowid DESC LIMIT 1",
            params![account_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    if let Some((balance_cents, date)) = confirmed {
        return CurveAnchor {
            balance_cents,
            date,
            fold_all_activity: false,
            confirmed: true,
        };
    }
    let opening = opening_anchor(conn, account_id);
    CurveAnchor {
        balance_cents: opening.balance_cents,
        date: opening.date,
        fold_all_activity: opening.fold_all_activity,
        confirmed: false,
    }
}

fn opening_anchor(conn: &Connection, account_id: &str) -> OpeningAnchor {
    let (balance_cents, date, source): (i64, String, Option<String>) = conn
        .query_row(
            "SELECT COALESCE(balance_cents, 0), as_of_date, source FROM account_balances \
             WHERE account_id = ?1 ORDER BY as_of_date ASC, rowid ASC LIMIT 1",
            params![account_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, "1970-01-01".to_string(), None));
    OpeningAnchor {
        fold_all_activity: source.as_deref() == Some("seed"),
        balance_cents,
        date,
    }
}

/// Set an account's CURRENT balance by BACK-SOLVING its opening anchor.
///
/// The balance model is YNAB/Actual-style: `current = opening + Σ(all cleared
/// activity)`. So to make the current balance equal a user-known value (e.g.
/// entered after a CSV import that carried no balance field), we solve for the
/// opening: `opening = current − Σ(all cleared txns)`, write it to the seed
/// snapshot, and let [`recompute_balance_if_linked`] derive today's balance.
///
/// Unlike stamping a fixed "today = X" snapshot, this keeps the balance LIVE:
/// every later transaction (edit, add, forward import) re-derives from the fixed
/// opening, so the number tracks reality instead of freezing at X. Any prior
/// user-stamped `manual` snapshot is cleared, otherwise `recompute` would treat
/// the account as pinned and refuse to re-derive.
///
/// Limitation: the opening is solved against the activity present *now*. If you
/// later import *older* history (dated before everything currently loaded), that
/// activity folds onto the same fixed opening and shifts today's balance — just
/// re-run "set current balance" after backfilling older statements.
pub fn set_current_balance(
    conn: &mut Connection,
    account_id: &str,
    current_cents: i64,
) -> CoreResult<()> {
    // An investment account's entered value IS the market value — stamp it as
    // today's snapshot verbatim. Back-solving an opening from cash flows (the
    // cash-account path below) would display `market value − net contributions`
    // instead of what the user just typed: the seed row would hold the solved
    // opening, and `recompute_balance_if_linked` deliberately never re-derives
    // investment balances, so the skewed seed would BE the shown balance —
    // and since nothing ever re-derives a "today" row for this type, the
    // account would stay "Balance not set" forever, not just show a wrong
    // number.
    let is_investment: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND type = 'Investment')",
        params![account_id],
        |r| r.get(0),
    )?;
    if is_investment {
        let today = Utc::now().date_naive().to_string();
        upsert_balance_snapshot(conn, account_id, &today, current_cents, None, Some("manual"))?;
        return Ok(());
    }

    let sum_all: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions \
         WHERE account_id = ?1 AND pending = 0",
        params![account_id],
        |r| r.get(0),
    )?;
    let opening = current_cents - sum_all;

    // Reset the opening (seed) anchor to the back-solved value.
    let updated = conn.execute(
        "UPDATE account_balances SET balance_cents = ?1 \
         WHERE account_id = ?2 AND source = 'seed'",
        params![opening, account_id],
    )?;
    if updated == 0 {
        // No seed row (unusual) — anchor one at the account's creation date.
        let created_date: String = conn.query_row(
            "SELECT substr(created_at, 1, 10) FROM accounts WHERE id = ?1",
            params![account_id],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO account_balances (account_id, as_of_date, balance_cents, source) \
             VALUES (?1, ?2, ?3, 'seed')",
            params![account_id, created_date, opening],
        )?;
    }

    // Clear any stale user-stamped 'manual' snapshot from the old freeze path;
    // otherwise `recompute_balance_if_linked` sees a user-set balance and bails,
    // leaving the account frozen at the previous number.
    conn.execute(
        "DELETE FROM account_balances WHERE account_id = ?1 AND source = 'manual'",
        params![account_id],
    )?;

    // Derive today's balance from the fresh opening. No-op for linked accounts,
    // whose balances are bank-reported during sync.
    recompute_balance_if_linked(conn, account_id)?;
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
                extra_json, raw_json, import_pending, \
                apr_pct, min_payment_cents, payoff_date, limit_cents, original_balance_cents, started_at \
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
            apr_pct: r.get(29)?,
            min_payment_cents: r.get(30)?,
            payoff_date: r.get(31)?,
            limit_cents: r.get(32)?,
            original_balance_cents: r.get(33)?,
            started_at: r.get(34)?,
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
            // Debt fields (apr_pct, min_payment_cents, ...) are user-managed,
            // never synced from SimpleFin — leaving them None here means
            // "don't touch," preserving whatever the user already entered.
            ..Default::default()
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
            apr_pct: r.get(29)?,
            min_payment_cents: r.get(30)?,
            payoff_date: r.get(31)?,
            limit_cents: r.get(32)?,
            original_balance_cents: r.get(33)?,
            started_at: r.get(34)?,
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

/// Reconstruct an account's balance over time from its opening anchor plus
/// cleared transaction activity: `balance(t) = opening + Σ(cleared ≤ t)`.
///
/// This is the per-account, per-activity-day counterpart to
/// `net_worth::backfill_history_from_transactions`, and it exists because stored
/// `account_balances` rows are written opportunistically rather than on a
/// schedule — they are a sparse scatter, so `MAX(balance_cents)` over them would
/// confidently report a peak that is merely the highest day someone happened to
/// record, not the highest the account actually reached.
///
/// Points are END-OF-DAY: intra-day ordering isn't recoverable from `posted_at`
/// alone, so a day with several transactions yields one point carrying the net.
///
/// `since` (ISO `YYYY-MM-DD`) trims the RETURNED points only — the running
/// balance is always accumulated from the account's true beginning, and the
/// window's first point carries the balance as of that date so the series never
/// starts mid-air. `peak`/`trough` describe the returned window.
pub fn balance_timeline(
    conn: &mut Connection,
    account_id: &str,
    since: Option<&str>,
) -> CoreResult<AccountBalanceTimeline> {
    let (account_name, account_type, created_date): (String, String, String) = conn.query_row(
        "SELECT name, type, substr(created_at, 1, 10) FROM accounts WHERE id = ?1",
        params![account_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    // Both refusals below mirror `recompute_balance_if_linked`, which declines to
    // derive a balance for exactly these two cases. Reconstructing anyway would
    // not merely be imprecise — it would contradict the balance the rest of the
    // app shows for the same account, while carrying a confident label.
    let skip_reason = if account_type == "Investment" {
        // Market value, not summed cash flow. A fully-invested account nets ~$0
        // cash, so a ledger reconstruction is meaningless rather than merely off.
        Some(
            "an investment account's value is its market value, not the sum of its cash flows, \
             so it cannot be reconstructed from transactions"
                .to_string(),
        )
    } else {
        let is_linked: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1 AND simplefin_account_id IS NOT NULL)",
            params![account_id],
            |r| r.get(0),
        )?;
        // Every sync writes a fresh bank-dated snapshot, so a linked account
        // accumulates real balance readings over time. Those ARE the history —
        // and folding transactions onto the OLDEST of them would drift away from
        // the newest, which is what the rest of the app displays.
        is_linked.then(|| {
            "this account is linked to a bank feed, so its balances are bank-reported rather than \
             derived — its recorded balance history is the source of truth"
                .to_string()
        })
    };
    if let Some(reason) = skip_reason {
        return Ok(AccountBalanceTimeline {
            account_id: account_id.to_string(),
            account_name,
            points: Vec::new(),
            peak: None,
            trough: None,
            current_cents: 0,
            anchor: BalanceAnchorQuality::AssumedZero,
            earliest_txn_date: None,
            reconstructable: false,
            skip_reason: Some(reason),
        });
    }

    let anchor = curve_anchor(conn, account_id);
    let earliest_txn_date: Option<String> = conn.query_row(
        "SELECT MIN(date(posted_at)) FROM transactions WHERE account_id = ?1 AND pending = 0",
        params![account_id],
        |r| r.get::<_, Option<String>>(0),
    )?;

    // Every cleared day's net movement, oldest first. ALL of it — activity
    // before the anchor is walked backward rather than dropped.
    let mut stmt = conn.prepare(
        "SELECT date(posted_at) AS d, COALESCE(SUM(amount_cents), 0) FROM transactions \
         WHERE account_id = ?1 AND pending = 0 GROUP BY d ORDER BY d ASC",
    )?;
    let deltas: Vec<(String, i64)> = stmt
        .query_map(params![account_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    // Solve for the balance before ANY activity, such that the curve passes
    // through the anchor: `balance(t) = base + Σ(activity ≤ t)`, so
    // `base = anchor − Σ(activity ≤ anchor_date)`. A creation seed is
    // conceptually pre-history, so nothing precedes it and the offset is zero.
    let offset: i64 = if anchor.fold_all_activity {
        0
    } else {
        deltas
            .iter()
            .filter(|(d, _)| d.as_str() <= anchor.date.as_str())
            .map(|(_, delta)| delta)
            .sum()
    };
    let base = anchor.balance_cents - offset;

    let curve_start = earliest_txn_date
        .as_deref()
        .and_then(day_before)
        .unwrap_or_else(|| anchor.date.clone());

    let mut running = base;
    let mut all_points = vec![AccountBalancePoint {
        date: curve_start,
        balance_cents: running,
    }];
    for (date, delta) in deltas {
        running += delta;
        // Activity landing on the curve's own start date updates that point
        // rather than appending a duplicate date.
        match all_points.last_mut() {
            Some(last) if last.date == date => last.balance_cents = running,
            _ => all_points.push(AccountBalancePoint {
                date,
                balance_cents: running,
            }),
        }
    }

    let points = match since {
        None => all_points,
        Some(since) => {
            let mut carried: Option<i64> = None;
            let mut kept: Vec<AccountBalancePoint> = Vec::new();
            for p in all_points {
                if p.date.as_str() < since {
                    carried = Some(p.balance_cents);
                } else {
                    kept.push(p);
                }
            }
            // Only synthesise the carry-in point when the window doesn't already
            // open on a real one. Activity landing exactly ON `since` would
            // otherwise emit that date twice — once pre-activity, once post —
            // and the stale first copy could win peak/trough.
            if let Some(balance_cents) = carried {
                if kept.first().map_or(true, |p| p.date != since) {
                    kept.insert(
                        0,
                        AccountBalancePoint {
                            date: since.to_string(),
                            balance_cents,
                        },
                    );
                }
            }
            kept
        }
    };

    // Strict comparisons keep the EARLIEST date on ties, since points ascend.
    let mut peak: Option<&AccountBalancePoint> = None;
    let mut trough: Option<&AccountBalancePoint> = None;
    for p in &points {
        if peak.map_or(true, |c| p.balance_cents > c.balance_cents) {
            peak = Some(p);
        }
        if trough.map_or(true, |c| p.balance_cents < c.balance_cents) {
            trough = Some(p);
        }
    }

    // Calibrated only when the curve is actually PINNED to a confirmed balance —
    // not merely when one exists somewhere on the account.
    let anchor_quality = if anchor.confirmed {
        BalanceAnchorQuality::Calibrated
    } else if anchor.balance_cents != 0 {
        // Either an opening the user entered at creation, or one back-solved by
        // `set_current_balance` — which rewrites the seed row in place and leaves
        // no separate marker behind. Both are real anchors.
        BalanceAnchorQuality::AnchoredOpening
    } else if earliest_txn_date
        .as_deref()
        .is_some_and(|d| d < created_date.as_str())
    {
        // Zero opening with history imported behind it: the anchor never
        // accounted for that history, so every absolute figure is off by an
        // unknown constant. Movement and timing are still correct.
        BalanceAnchorQuality::AssumedZero
    } else {
        BalanceAnchorQuality::AnchoredOpening
    };

    Ok(AccountBalanceTimeline {
        account_id: account_id.to_string(),
        account_name,
        current_cents: points
            .last()
            .map_or(anchor.balance_cents, |p| p.balance_cents),
        peak: peak.cloned(),
        trough: trough.cloned(),
        points,
        anchor: anchor_quality,
        earliest_txn_date,
        reconstructable: true,
        skip_reason: None,
    })
}

fn day_before(date: &str) -> Option<String> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.pred_opt())
        .map(|d| d.format("%Y-%m-%d").to_string())
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
        sample_account_typed(conn, AccountType::Checking, "Checking")
    }

    fn sample_account_typed(
        conn: &mut rusqlite::Connection,
        r#type: AccountType,
        name: &str,
    ) -> Account {
        insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type,
                name: name.into(),
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
    fn update_account_debt_fields_round_trip() {
        // Mirrors update_account_apy_round_trip: the debt fields that used to
        // live only on the retired `liabilities` table are now optional,
        // patchable fields on any Credit/Loan account.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        let updated = update(
            &mut conn,
            &acc.id,
            AccountPatch {
                apr_pct: Some(Some(24.9)),
                min_payment_cents: Some(Some(5_000)),
                payoff_date: Some(Some("2027-01-01".into())),
                limit_cents: Some(Some(500_000)),
                original_balance_cents: Some(Some(200_000)),
                started_at: Some(Some("2023-05-01".into())),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(updated.apr_pct, Some(24.9));
        assert_eq!(updated.min_payment_cents, Some(5_000));
        assert_eq!(updated.payoff_date.as_deref(), Some("2027-01-01"));
        assert_eq!(updated.limit_cents, Some(500_000));
        assert_eq!(updated.original_balance_cents, Some(200_000));
        assert_eq!(updated.started_at.as_deref(), Some("2023-05-01"));

        let summaries = list_summaries(&mut conn).unwrap();
        let summary = summaries.iter().find(|a| a.id == acc.id).unwrap();
        assert_eq!(summary.apr_pct, Some(24.9));
        assert_eq!(summary.min_payment_cents, Some(5_000));
        assert_eq!(summary.payoff_date.as_deref(), Some("2027-01-01"));
        assert_eq!(summary.limit_cents, Some(500_000));
        assert_eq!(summary.original_balance_cents, Some(200_000));
        assert_eq!(summary.started_at.as_deref(), Some("2023-05-01"));

        // Clearing a field back to None must work too (Some(None), not the
        // outer None that means "leave untouched").
        let cleared = update(
            &mut conn,
            &acc.id,
            AccountPatch {
                payoff_date: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(cleared.payoff_date, None);
        assert_eq!(cleared.apr_pct, Some(24.9), "unrelated debt fields untouched");
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
                apr_pct: None,
                min_payment_cents: None,
                payoff_date: None,
                limit_cents: None,
                original_balance_cents: None,
                started_at: None,
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

    /// The premise of the feature: stored `account_balances` rows are written
    /// opportunistically, so the true peak lands on a day nobody recorded. The
    /// reconstruction has to find it where `MAX(balance_cents)` over stored rows
    /// cannot.
    #[test]
    fn balance_timeline_finds_a_peak_no_stored_snapshot_holds() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn); // seed: $0 @ today
        insert_txn(&conn, &acc.id, 500_000, "2024-01-10");
        insert_txn(&conn, &acc.id, 300_000, "2024-02-10"); // peak: $8,000
        insert_txn(&conn, &acc.id, -600_000, "2024-03-10");

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        let peak = tl.peak.expect("a peak");
        assert_eq!(peak.balance_cents, 800_000);
        assert_eq!(peak.date, "2024-02-10");
        assert_eq!(tl.current_cents, 200_000);

        // The peak exists nowhere in the stored snapshots — querying those would
        // have missed it entirely, which is the whole reason this fn exists.
        let stored_max: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(balance_cents), 0) FROM account_balances WHERE account_id = ?1",
                params![acc.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(stored_max, 800_000);
    }

    /// The date is trustworthy even when the amount isn't, so the caveat has to
    /// ride along with the answer rather than being inferred by callers.
    #[test]
    fn balance_timeline_flags_a_zero_anchor_with_history_behind_it() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn); // seed: $0 @ today
        insert_txn(&conn, &acc.id, 100_000, "2020-01-01"); // predates creation

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        assert_eq!(tl.anchor, BalanceAnchorQuality::AssumedZero);
        // Shape is still right: the rise is real, only the level is unanchored.
        assert_eq!(tl.peak.unwrap().date, "2020-01-01");
    }

    #[test]
    fn balance_timeline_reports_calibrated_once_a_balance_is_confirmed() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account_typed(&mut conn, AccountType::Savings, "Savings");
        insert_txn(&conn, &acc.id, 100_000, "2024-01-01");
        // A bank-reported balance is a confirmed anchor.
        upsert_balance_snapshot(&mut conn, &acc.id, "2024-06-01", 250_000, None, Some("simplefin"))
            .unwrap();

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        assert_eq!(tl.anchor, BalanceAnchorQuality::Calibrated);
    }

    /// An account whose opening was entered is anchored even though the app
    /// cannot tell an entered opening from one back-solved by
    /// `set_current_balance` — both rewrite the same seed row.
    #[test]
    fn balance_timeline_treats_a_nonzero_opening_as_anchored() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        conn.execute(
            "UPDATE account_balances SET balance_cents = ?1 \
             WHERE account_id = ?2 AND source = 'seed'",
            params![400_000, acc.id],
        )
        .unwrap();
        insert_txn(&conn, &acc.id, -50_000, "2024-05-01");

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        assert_eq!(tl.anchor, BalanceAnchorQuality::AnchoredOpening);
        assert_eq!(tl.current_cents, 350_000);
    }

    /// An investment account holds market value, not the sum of its cash flows —
    /// the same reason `recompute_balance_if_linked` refuses to derive one. A
    /// reconstruction here would be confidently meaningless, so it must refuse
    /// rather than return a number.
    #[test]
    fn balance_timeline_refuses_investment_accounts() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account_typed(&mut conn, AccountType::Investment, "Brokerage");
        insert_txn(&conn, &acc.id, 500_000, "2024-01-01");

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        assert!(!tl.reconstructable);
        assert!(tl.points.is_empty());
        assert!(tl.peak.is_none());
    }

    /// In production the creation seed is dated in the PAST, so a later
    /// confirmed balance is newer than it — and picking the earliest row as the
    /// anchor would ignore the confirmed one entirely while still labelling the
    /// result `Calibrated`. The curve has to be pinned to the confirmed balance
    /// it claims to be calibrated against.
    #[test]
    fn balance_timeline_anchors_on_the_confirmed_balance_not_the_older_seed() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        // Backdate creation + seed so the seed is genuinely older than the pin.
        conn.execute(
            "UPDATE accounts SET created_at = '2023-12-01T00:00:00+00:00' WHERE id = ?1",
            params![acc.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE account_balances SET as_of_date = '2023-12-01' \
             WHERE account_id = ?1 AND source = 'seed'",
            params![acc.id],
        )
        .unwrap();
        insert_txn(&conn, &acc.id, 900_000, "2024-01-01");
        // A confirmed balance the user reconciled to, AFTER the seed.
        upsert_balance_snapshot(&mut conn, &acc.id, "2024-06-01", 250_000, None, Some("manual"))
            .unwrap();

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        assert_eq!(tl.anchor, BalanceAnchorQuality::Calibrated);
        // Nothing happened after the pin, so today's balance IS the pin.
        assert_eq!(
            tl.current_cents, 250_000,
            "curve must be pinned to the confirmed balance, not the stale seed"
        );
    }

    /// A linked account accumulates a real bank-reported balance row per sync.
    /// Anchoring on the OLDEST of those and folding later transactions onto it
    /// drifts away from the NEWEST — which is the balance the rest of the app
    /// displays — so the reconstruction would contradict the account's own
    /// screen while claiming a calibrated anchor. It has to refuse instead.
    #[test]
    fn balance_timeline_refuses_bank_linked_accounts() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account_typed(&mut conn, AccountType::Savings, "Linked Savings");
        conn.execute(
            "UPDATE accounts SET simplefin_account_id = 'sf-123' WHERE id = ?1",
            params![acc.id],
        )
        .unwrap();
        // Two bank readings that don't reconcile with local activity alone.
        upsert_balance_snapshot(&mut conn, &acc.id, "2024-01-01", 100_000, None, Some("simplefin"))
            .unwrap();
        upsert_balance_snapshot(&mut conn, &acc.id, "2024-02-01", 150_000, None, Some("simplefin"))
            .unwrap();
        insert_txn(&conn, &acc.id, -20_000, "2024-01-15");

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        assert!(!tl.reconstructable);
        assert!(tl.skip_reason.unwrap().contains("bank-reported"));
        assert!(tl.peak.is_none(), "must not report a peak it cannot stand behind");
    }

    /// Activity BEFORE the confirmed anchor is walked backward rather than
    /// dropped, so the curve covers the account's whole history and still passes
    /// exactly through the confirmed balance.
    #[test]
    fn balance_timeline_walks_backward_through_pre_anchor_activity() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        conn.execute(
            "UPDATE account_balances SET as_of_date = '2022-12-01' \
             WHERE account_id = ?1 AND source = 'seed'",
            params![acc.id],
        )
        .unwrap();
        insert_txn(&conn, &acc.id, 100_000, "2023-01-01"); // BEFORE the anchor
        upsert_balance_snapshot(&mut conn, &acc.id, "2024-03-01", 500_000, None, Some("manual"))
            .unwrap();
        insert_txn(&conn, &acc.id, 50_000, "2024-06-01"); // after the anchor

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        assert_eq!(tl.anchor, BalanceAnchorQuality::Calibrated);
        assert_eq!(tl.earliest_txn_date.as_deref(), Some("2023-01-01"));
        // Back-solved: $5,000 at the pin minus the $1,000 that preceded it.
        assert_eq!(tl.points[0].balance_cents, 400_000);
        // The curve passes through the pin, then tracks activity after it.
        assert_eq!(
            tl.points.iter().find(|p| p.date == "2023-01-01").unwrap().balance_cents,
            500_000
        );
        assert_eq!(tl.current_cents, 550_000);
    }

    /// Windowing trims the returned points but must not restart the arithmetic,
    /// or the curve would begin at the opening balance instead of wherever the
    /// account actually stood on that date.
    #[test]
    fn balance_timeline_window_carries_the_running_balance_in() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        insert_txn(&conn, &acc.id, 900_000, "2024-01-01");
        insert_txn(&conn, &acc.id, -100_000, "2024-06-01");

        let tl = balance_timeline(&mut conn, &acc.id, Some("2024-05-01")).unwrap();

        // First point is the window edge carrying January's balance forward.
        assert_eq!(tl.points[0].date, "2024-05-01");
        assert_eq!(tl.points[0].balance_cents, 900_000);
        assert_eq!(tl.current_cents, 800_000);
        // The pre-window peak is excluded; peak/trough describe the window.
        assert_eq!(tl.peak.unwrap().date, "2024-05-01");
    }

    /// A window opening exactly on an active day must not emit that date twice —
    /// once carried in pre-activity, once real post-activity — or the stale copy
    /// can win peak/trough.
    #[test]
    fn balance_timeline_window_opening_on_an_active_day_emits_it_once() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        insert_txn(&conn, &acc.id, 900_000, "2024-01-01");
        insert_txn(&conn, &acc.id, -200_000, "2024-05-01"); // ON the window edge

        let tl = balance_timeline(&mut conn, &acc.id, Some("2024-05-01")).unwrap();

        let on_edge = tl.points.iter().filter(|p| p.date == "2024-05-01").count();
        assert_eq!(on_edge, 1, "window edge duplicated: {:?}", tl.points);
        // The surviving point is post-activity, not the carried-in balance.
        assert_eq!(tl.points[0].balance_cents, 700_000);
        assert_eq!(tl.peak.unwrap().balance_cents, 700_000);
    }

    /// Intra-day ordering isn't recoverable from `posted_at`, so a day collapses
    /// to one end-of-day point carrying the net.
    #[test]
    fn balance_timeline_nets_same_day_activity_into_one_point() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        insert_txn(&conn, &acc.id, 100_000, "2024-04-01");
        insert_txn(&conn, &acc.id, -30_000, "2024-04-01");

        let tl = balance_timeline(&mut conn, &acc.id, None).unwrap();

        let on_day = tl.points.iter().filter(|p| p.date == "2024-04-01").count();
        assert_eq!(on_day, 1);
        assert_eq!(tl.current_cents, 70_000);
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
    fn investment_account_balance_is_not_derived_from_cash_flows() {
        // A brokerage account's value is its MARKET value, not the sum of its
        // cash activity. Importing the brokerage's contribution/trade history must
        // NOT change the user-entered market value — otherwise a fully-invested
        // account would read ~$0, or a market value with activity on top would
        // double-count. (F1: local CSV import of investment activity.)
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('tfsa','Me','Wealthsimple','Investment','TFSA','CAD','#22C55E','manual',?1)",
            params![Utc::now().to_rfc3339()],
        )
        .unwrap();
        // The market value the user entered (seeded at account creation).
        conn.execute(
            "INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) \
             VALUES('tfsa',date('now'),1000000,'seed')",
            [],
        )
        .unwrap();
        // A $500 contribution and a $300 securities buy — net +$200 cash, which
        // is NOT the account's value change (prices move independently).
        insert_txn(&conn, "tfsa", 50_000, "2024-12-18");
        insert_txn(&conn, "tfsa", -30_000, "2024-12-22");

        recompute_balance_if_linked(&mut conn, "tfsa").unwrap();

        let derived_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM account_balances WHERE account_id='tfsa' AND source='derived'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(derived_count, 0, "an investment balance is never derived from transactions");
        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == "tfsa")
            .unwrap();
        assert_eq!(summary.balance_cents, 1_000_000, "market value stands; cash flows ignored");
    }

    #[test]
    fn set_current_balance_on_investment_account_stamps_market_value_verbatim() {
        // Regression: the back-solve path computed `opening = entered − Σflows`
        // and wrote it to the seed; since investment balances are never derived,
        // the skewed seed became the displayed balance (and, worse, never even
        // became "known" — see `feat(investments)` PR notes). Entering a
        // $10,000 market value on a TFSA with +$200 net cash activity showed
        // $9,800 (or nothing at all).
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('tfsa','Me','Wealthsimple','Investment','TFSA','CAD','#22C55E','manual',?1)",
            params![Utc::now().to_rfc3339()],
        )
        .unwrap();
        insert_txn(&conn, "tfsa", 50_000, "2024-12-18"); // contribution in
        insert_txn(&conn, "tfsa", -30_000, "2024-12-22"); // securities buy

        set_current_balance(&mut conn, "tfsa", 1_000_000).unwrap();

        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == "tfsa")
            .unwrap();
        assert!(
            summary.balance_known,
            "an explicitly set investment balance must read as known"
        );
        assert_eq!(
            summary.balance_cents, 1_000_000,
            "the entered market value is shown verbatim, not skewed by cash flows"
        );
        let manual_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM account_balances WHERE account_id='tfsa' AND source='manual'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(manual_count, 1);

        // Setting it again later replaces the market value (no accumulation).
        set_current_balance(&mut conn, "tfsa", 1_100_000).unwrap();
        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == "tfsa")
            .unwrap();
        assert_eq!(summary.balance_cents, 1_100_000);
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
    fn set_current_balance_backsolves_opening_and_keeps_tracking() {
        // P1-5: after a PARTIAL import (history that doesn't reach account
        // opening), the naive opening-0 derivation is wrong. Setting the real
        // current balance back-solves the opening so (a) the number is exactly
        // right AND (b) it keeps tracking forward activity — the whole reason to
        // back-solve instead of freezing a fixed "today = X" snapshot.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn); // seed $0 @ today
        insert_txn(&conn, &acc.id, -50_000, "2026-04-10");
        insert_txn(&conn, &acc.id, 38_000, "2026-05-01");
        recompute_balance_if_linked(&mut conn, &acc.id).unwrap();
        let naive = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert_eq!(
            naive.balance_cents, -12_000,
            "opening-0 derivation is the wrong number after a partial import"
        );

        // (a) User sets their real current balance → shown balance equals it.
        set_current_balance(&mut conn, &acc.id, 4_800_00).unwrap();
        let after = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert_eq!(after.balance_cents, 4_800_00, "current balance equals the value the user set");
        assert!(after.balance_known, "a set balance reads as known");

        // (b) A later forward transaction MOVES the balance (a frozen snapshot
        // would not) — this is the property that distinguishes back-solve.
        insert_txn(&conn, &acc.id, -20_000, "2026-07-01");
        recompute_balance_if_linked(&mut conn, &acc.id).unwrap();
        let tracked = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert_eq!(
            tracked.balance_cents,
            4_800_00 - 20_000,
            "balance tracks forward activity after back-solve"
        );

        // (c) Re-confirming the balance still lands exactly (idempotent-ish:
        // clears the prior anchor and re-solves against current activity).
        set_current_balance(&mut conn, &acc.id, 5_000_00).unwrap();
        let reconfirmed = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert_eq!(reconfirmed.balance_cents, 5_000_00, "re-setting lands exactly");
    }

    #[test]
    fn set_current_balance_overrides_stale_manual_freeze() {
        // Migration guard: an account that was pinned under the OLD freeze path
        // (a 'manual' snapshot) must re-derive after a back-solve, not stay
        // frozen — the manual snapshot has to be cleared or recompute bails.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = sample_account(&mut conn);
        insert_txn(&conn, &acc.id, -30_000, "2026-04-10");
        let today = Utc::now().date_naive().to_string();
        // Simulate the old freeze: a user-stamped manual balance.
        upsert_balance_snapshot(&mut conn, &acc.id, &today, 1_000_00, None, Some("manual")).unwrap();

        set_current_balance(&mut conn, &acc.id, 2_500_00).unwrap();

        let no_manual: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM account_balances WHERE account_id = ?1 AND source = 'manual'",
                params![acc.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(no_manual, 0, "stale manual freeze snapshot cleared");
        let summary = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert_eq!(summary.balance_cents, 2_500_00, "re-derives to the new value, not the old freeze");

        // And it now tracks forward (the freeze would have stayed at 2_500_00).
        insert_txn(&conn, &acc.id, -5_000, "2026-07-02");
        recompute_balance_if_linked(&mut conn, &acc.id).unwrap();
        let tracked = list_summaries(&mut conn)
            .unwrap()
            .into_iter()
            .find(|a| a.id == acc.id)
            .unwrap();
        assert_eq!(tracked.balance_cents, 2_500_00 - 5_000, "tracks forward after clearing the freeze");
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
