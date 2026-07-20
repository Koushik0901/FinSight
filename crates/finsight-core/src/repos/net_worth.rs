use crate::error::CoreResult;
use crate::models::{AccountType, NetWorthPoint};
use crate::repos::{accounts, manual_assets};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Deterministic net-worth breakdown: assets (known-balance accounts, which
/// already include debt as negative balances on Credit/Loan accounts) plus
/// manual assets. Accounts whose balance is not confirmed (`balance_known ==
/// false`, e.g. CSV history with no balance field) are EXCLUDED from the
/// totals and surfaced separately so the Copilot can mark them clearly rather
/// than counting a phantom $0. Mirrors `record_today` and the frontend
/// `useNetWorth()`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetWorthBreakdown {
    pub net_worth_cents: i64,
    pub total_assets_cents: i64,
    pub known_account_balance_cents: i64,
    pub manual_asset_cents: i64,
    /// Informational total debt (sum of the amount owed on known-balance
    /// Credit/Loan accounts, always >= 0). This is NOT subtracted again —
    /// it's already reflected as negative numbers inside
    /// `known_account_balance_cents`. Debt used to live in a separate
    /// `liabilities` table that was summed independently, which let the same
    /// card be counted twice if also tracked as an Account; folding debt into
    /// Account removed the second ledger entirely.
    pub liability_cents: i64,
    pub accounts_with_known_balance: i64,
    pub accounts_with_unknown_balance: i64,
    /// Names of accounts excluded from the total because their balance is not
    /// confirmed. The Copilot should mention these as unknown, not as $0.
    pub unknown_balance_accounts: Vec<String>,
    /// True when there is at least one account or manual asset to compute
    /// from. When false, net worth is not meaningful (no data).
    pub has_data: bool,
}

/// Compute the current net-worth breakdown from live account and manual-asset
/// data. Uses the exact same inclusion rules as [`record_today`], and is scoped
/// to the primary currency so the total is never a cross-currency sum.
pub fn breakdown(conn: &mut Connection) -> CoreResult<NetWorthBreakdown> {
    let profile = crate::currency::currency_profile(conn)?;
    let scope = if profile.is_mixed() {
        profile.primary()
    } else {
        None
    };
    breakdown_in_currency(conn, scope)
}

/// [`breakdown`] with the currency scope supplied by the caller, so a caller
/// that already resolved the profile does not resolve it twice and — more
/// importantly — cannot resolve it *differently*. `None` includes every
/// currency, which is correct only when there is at most one.
///
/// Manual assets are scoped alongside accounts: an asset recorded in another
/// currency is real, but adding it to a total denominated in this one produces
/// the same meaningless number as mixing the accounts would.
pub fn breakdown_in_currency(
    conn: &mut Connection,
    scope: Option<&str>,
) -> CoreResult<NetWorthBreakdown> {
    let in_scope = |raw: &str| match scope {
        None => true,
        Some(code) => crate::currency::normalize_code(raw) == code,
    };
    let accounts: Vec<_> = accounts::list_summaries(conn)?
        .into_iter()
        .filter(|a| in_scope(&a.currency))
        .collect();
    let assets: Vec<_> = manual_assets::list(conn)?
        .into_iter()
        .filter(|a| in_scope(&a.currency))
        .collect();

    let has_data = !(accounts.is_empty() && assets.is_empty());

    let known_account_balance_cents: i64 = accounts
        .iter()
        .filter(|a| a.balance_known)
        .map(|a| a.balance_cents)
        .sum();
    let accounts_with_known_balance = accounts.iter().filter(|a| a.balance_known).count() as i64;
    let unknown_balance_accounts: Vec<String> = accounts
        .iter()
        .filter(|a| !a.balance_known)
        .map(|a| a.name.clone())
        .collect();
    let accounts_with_unknown_balance = unknown_balance_accounts.len() as i64;
    let manual_asset_cents: i64 = assets.iter().map(|a| a.value_cents).sum();
    let liability_cents: i64 = accounts
        .iter()
        .filter(|a| {
            a.balance_known
                && matches!(a.r#type, AccountType::Credit | AccountType::Loan)
                && a.balance_cents < 0
        })
        .map(|a| -a.balance_cents)
        .sum();
    let total_assets_cents = known_account_balance_cents + manual_asset_cents;

    Ok(NetWorthBreakdown {
        net_worth_cents: total_assets_cents,
        total_assets_cents,
        known_account_balance_cents,
        manual_asset_cents,
        liability_cents,
        accounts_with_known_balance,
        accounts_with_unknown_balance,
        unknown_balance_accounts,
        has_data,
    })
}

pub fn record_snapshot(conn: &mut Connection, total_cents: i64) -> CoreResult<()> {
    let id = Uuid::new_v4().to_string();
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO net_worth_snapshots(id, date, total_cents, created_at) \
         VALUES(?1, ?2, ?3, ?4) \
         ON CONFLICT(date) DO UPDATE SET total_cents = excluded.total_cents",
        params![id, today, total_cents, now],
    )?;
    Ok(())
}

fn record_snapshot_dated(conn: &mut Connection, date: &str, total_cents: i64) -> CoreResult<()> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO net_worth_snapshots(id, date, total_cents, created_at) \
         VALUES(?1, ?2, ?3, ?4) \
         ON CONFLICT(date) DO UPDATE SET total_cents = excluded.total_cents",
        params![id, date, total_cents, now],
    )?;
    Ok(())
}

/// Reconstruct monthly net-worth snapshots from transaction history so the trend
/// chart is populated right after an import instead of only building forward from
/// today. For each month-end from the earliest activity to today, net worth =
/// current net worth − activity dated after that month-end (each known-balance
/// account's derived balance already folds in all of its activity). Manual assets
/// and liabilities are held at today's value (their history isn't tracked), so
/// the reconstructed trend reflects account-balance movement.
pub fn backfill_history_from_transactions(conn: &mut Connection) -> CoreResult<()> {
    use chrono::{Datelike, NaiveDate};

    let known_ids: Vec<String> = accounts::list_summaries(conn)?
        .into_iter()
        .filter(|a| a.balance_known)
        .map(|a| a.id)
        .collect();
    if known_ids.is_empty() {
        return Ok(());
    }
    let current_nw = breakdown(conn)?.net_worth_cents;
    let placeholders = std::iter::repeat("?")
        .take(known_ids.len())
        .collect::<Vec<_>>()
        .join(",");

    let earliest: Option<NaiveDate> = conn
        .query_row(
            &format!(
                "SELECT MIN(date(posted_at)) FROM transactions \
                 WHERE pending = 0 AND account_id IN ({placeholders})"
            ),
            rusqlite::params_from_iter(known_ids.iter()),
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let Some(earliest) = earliest else {
        return Ok(());
    };

    let today = Utc::now().date_naive();
    let (mut year, mut month) = (earliest.year(), earliest.month());
    while let Some(first_of_month) = NaiveDate::from_ymd_opt(year, month, 1) {
        if first_of_month > today {
            break;
        }
        let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
        let month_end = NaiveDate::from_ymd_opt(ny, nm, 1)
            .and_then(|d| d.pred_opt())
            .unwrap_or(first_of_month)
            .min(today);
        let snap = month_end.format("%Y-%m-%d").to_string();

        let activity_after: i64 = conn.query_row(
            &format!(
                "SELECT COALESCE(SUM(amount_cents), 0) FROM transactions \
                 WHERE pending = 0 AND date(posted_at) > ?1 AND account_id IN ({placeholders})"
            ),
            rusqlite::params_from_iter(
                std::iter::once(snap.clone()).chain(known_ids.iter().cloned()),
            ),
            |r| r.get(0),
        )?;
        record_snapshot_dated(conn, &snap, current_nw - activity_after)?;

        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }
    Ok(())
}

/// Sum account balances (debt already included as negative Credit/Loan
/// balances) + manual assets, then upsert today's snapshot. Keeps the
/// recorded net worth consistent with the headline shown on the
/// Today/Accounts screens.
pub fn record_today(conn: &mut Connection) -> CoreResult<()> {
    let accounts = accounts::list_summaries(conn)?;
    let assets = manual_assets::list(conn)?;

    // If the user has removed every account and asset, there is nothing
    // meaningful to trend. Wipe stale snapshots so the homepage chart does
    // not keep showing a phantom net-worth history.
    if accounts.is_empty() && assets.is_empty() {
        conn.execute("DELETE FROM net_worth_snapshots", [])?;
        return Ok(());
    }

    // Delegate the arithmetic to `breakdown` rather than re-summing here. The
    // two used to be parallel implementations of the same rule, which is how
    // they came to disagree the moment currency scoping was added to one of
    // them: the trend chart's newest point would have been a cross-currency
    // sum while the headline beside it was not — visible to the user as a
    // data-integrity bug. One definition, one place.
    //
    // (Snapshots written before scoping existed remain cross-currency sums for
    // mixed-currency users; they are historical observations, not derived
    // values, so they are left alone rather than silently rewritten.)
    let total_cents = breakdown(conn)?.net_worth_cents;
    record_snapshot(conn, total_cents)
}

pub fn list_history(conn: &mut Connection, days: u32) -> CoreResult<Vec<NetWorthPoint>> {
    let cutoff = format!("-{} days", days);
    let mut stmt = conn.prepare(
        "SELECT date, total_cents FROM net_worth_snapshots \
         WHERE date >= date('now', ?1) ORDER BY date ASC",
    )?;
    let rows = stmt.query_map(params![cutoff], |r| {
        Ok(NetWorthPoint {
            date: r.get(0)?,
            total_cents: r.get(1)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("nw.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn record_snapshot_upserts_one_row_per_day() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        record_snapshot(&mut conn, 100_000).unwrap();
        record_snapshot(&mut conn, 250_000).unwrap();
        let hist = list_history(&mut conn, 30).unwrap();
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].total_cents, 250_000);
    }

    #[test]
    fn record_today_folds_assets_and_debt_accounts() {
        use crate::models::{AccountType, NewAccount, NewManualAsset};
        use crate::repos::{accounts, manual_assets};

        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        accounts::insert(
            &mut conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                owner: "me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#3B82F6".into(),
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                opening_balance_cents: 10_000_000,
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
        manual_assets::create(
            &mut conn,
            NewManualAsset {
                name: "House".into(),
                asset_type: "property".into(),
                value_cents: 50_000_000,
                currency: "USD".into(),
                notes: None,
            },
        )
        .unwrap();
        // Debt is now just an Account with a negative balance, not a
        // separate liabilities-table row.
        accounts::insert(
            &mut conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                owner: "me".into(),
                bank: "Manual".into(),
                r#type: AccountType::Loan,
                name: "Mortgage".into(),
                last4: None,
                currency: "USD".into(),
                color: "#F87171".into(),
                source: "manual".into(),
                liquidity_type: "restricted".into(),
                emergency_fund_eligible: false,
                goal_earmark: None,
                apy_pct: None,
                opening_balance_cents: -30_000_000,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "debt".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
                apr_pct: Some(5.5),
                min_payment_cents: Some(180_000),
                payoff_date: None,
                limit_cents: Some(35_000_000),
                original_balance_cents: None,
                started_at: None,
            },
        )
        .unwrap();

        record_today(&mut conn).unwrap();

        let hist = list_history(&mut conn, 30).unwrap();
        assert_eq!(hist.len(), 1);
        // 10,000,000 checking + 50,000,000 house + (−30,000,000) mortgage
        assert_eq!(hist[0].total_cents, 30_000_000);
    }

    /// The trend chart's newest point and the headline net worth beside it are
    /// computed by different code paths. If only one of them narrowed by
    /// currency, a mixed-currency user would see the chart disagree with the
    /// number above it — which reads as corrupted data, not as a caveat.
    #[test]
    fn recorded_snapshot_matches_the_scoped_breakdown_for_a_mixed_currency_user() {
        use crate::models::{NewAccount, NewManualAsset};
        use crate::repos::{accounts, manual_assets};

        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let acct = |name: &str, currency: &str, cents: i64| NewAccount {
            name: name.into(),
            currency: currency.into(),
            opening_balance_cents: cents,
            ..base_account(name, cents, "manual")
        };
        // CAD is primary on account count; the USD account is deliberately the
        // larger balance so a cross-currency sum would be obvious.
        accounts::insert(&mut conn, acct("CAD Chq", "CAD", 1_000_000)).unwrap();
        accounts::insert(&mut conn, acct("CAD Sav", "CAD", 500_000)).unwrap();
        accounts::insert(&mut conn, acct("USD Sav", "USD", 9_000_000)).unwrap();
        manual_assets::create(
            &mut conn,
            NewManualAsset {
                name: "US property".into(),
                asset_type: "property".into(),
                value_cents: 20_000_000,
                currency: "USD".into(),
                notes: None,
            },
        )
        .unwrap();

        record_today(&mut conn).unwrap();
        let hist = list_history(&mut conn, 30).unwrap();
        let live = breakdown(&mut conn).unwrap().net_worth_cents;

        assert_eq!(
            hist.last().unwrap().total_cents,
            live,
            "snapshot and live headline must agree"
        );
        assert_eq!(live, 1_500_000, "CAD only — no USD account or asset folded in");
    }

    #[test]
    fn backfill_reconstructs_monthly_history_ending_at_current() {
        use crate::models::{NewTransaction, TransactionStatus};
        use crate::repos::{accounts, transactions};

        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = accounts::insert(&mut conn, base_account("Card", 0, "seed")).unwrap();

        let mk = |amt: i64, date: &str| NewTransaction {
            account_id: acc.id.clone(),
            posted_at: chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap()
                .and_utc(),
            amount_cents: amt,
            merchant_raw: "M".into(),
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
        };
        // Two months of activity in the past; current derived balance = −150.
        transactions::insert(&mut conn, mk(-100_00, "2024-01-10")).unwrap();
        transactions::insert(&mut conn, mk(-50_00, "2024-02-10")).unwrap();

        backfill_history_from_transactions(&mut conn).unwrap();

        let hist = list_history(&mut conn, 36500).unwrap();
        assert!(hist.len() >= 2, "expected a monthly trend, got {}", hist.len());
        // Latest snapshot equals the current net worth (all activity folded in).
        assert_eq!(hist.last().unwrap().total_cents, -150_00);
        // The Jan-end snapshot only reflects the −100 charge (the −50 is later).
        let jan = hist.iter().find(|p| p.date.starts_with("2024-01")).unwrap();
        assert_eq!(jan.total_cents, -100_00);
    }

    #[test]
    fn breakdown_includes_derived_balances_after_import() {
        use crate::models::{AccountType, NewAccount, NewTransaction, TransactionStatus};
        use crate::repos::{accounts, transactions};
        use chrono::Duration;

        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        // Known-balance account (manual, no unaccounted history).
        let known = accounts::insert(&mut conn, base_account("Checking", 500_000, "manual")).unwrap();
        let _ = known;

        // Imported account: seed opening ($0) + activity now DERIVES a balance
        // (YNAB/Actual model), so it counts toward net worth (0 + −4,200).
        let imported = accounts::insert(&mut conn, base_account("Imported Card", 0, "seed")).unwrap();
        transactions::insert(
            &mut conn,
            NewTransaction {
                account_id: imported.id.clone(),
                posted_at: Utc::now() - Duration::days(5),
                amount_cents: -4_200,
                merchant_raw: "Store".into(),
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

        // Debt is now a Credit-type Account with a negative manual balance,
        // not a separate liabilities-table row.
        accounts::insert(
            &mut conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                r#type: AccountType::Credit,
                opening_balance_cents: -120_000,
                account_group: "debt".into(),
                liquidity_type: "restricted".into(),
                emergency_fund_eligible: false,
                apr_pct: Some(19.9),
                min_payment_cents: Some(3_000),
                limit_cents: Some(500_000),
                ..base_account("Card", 0, "manual")
            },
        )
        .unwrap();

        let b = breakdown(&mut conn).unwrap();
        assert!(b.has_data);
        // Checking 500,000 + imported card (0 − 4,200) + credit card (−120,000)
        assert_eq!(b.known_account_balance_cents, 375_800);
        assert_eq!(b.accounts_with_known_balance, 3);
        assert_eq!(b.accounts_with_unknown_balance, 0);
        assert!(b.unknown_balance_accounts.is_empty());
        assert_eq!(b.liability_cents, 120_000, "informational debt total, not subtracted again");
        // 375,800 — the debt is already folded into known_account_balance_cents.
        assert_eq!(b.net_worth_cents, 375_800);
    }

    #[test]
    fn breakdown_has_no_data_on_empty_db() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let b = breakdown(&mut conn).unwrap();
        assert!(!b.has_data);
        assert_eq!(b.net_worth_cents, 0);
    }

    fn base_account(
        name: &str,
        opening_balance_cents: i64,
        source: &str,
    ) -> crate::models::NewAccount {
        use crate::models::{AccountType, NewAccount};
        NewAccount {
            promo_apr_expires_on: None,
            post_promo_apr_pct: None,
            owner: "me".into(),
            bank: "Bank".into(),
            r#type: AccountType::Checking,
            name: name.into(),
            last4: None,
            currency: "USD".into(),
            color: "#3B82F6".into(),
            source: source.into(),
            liquidity_type: "liquid".into(),
            emergency_fund_eligible: true,
            goal_earmark: None,
            apy_pct: None,
            opening_balance_cents,
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
        }
    }

    #[test]
    fn record_today_clears_stale_snapshots_when_nothing_to_track() {
        use crate::models::{AccountType, NewAccount};
        use crate::repos::accounts;

        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let acct = accounts::insert(
            &mut conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                owner: "me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#3B82F6".into(),
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                opening_balance_cents: 100_000,
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

        record_today(&mut conn).unwrap();
        assert_eq!(list_history(&mut conn, 30).unwrap().len(), 1);

        // Remove the only source of net-worth data.
        accounts::archive(&mut conn, &acct.id).unwrap();

        // Recording today with nothing tracked should clean up stale snapshots
        // instead of leaving a phantom trendline on the homepage.
        record_today(&mut conn).unwrap();
        assert!(list_history(&mut conn, 30).unwrap().is_empty());
    }
}
