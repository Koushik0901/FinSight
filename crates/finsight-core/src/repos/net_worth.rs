use crate::error::CoreResult;
use crate::models::NetWorthPoint;
use crate::repos::{accounts, liabilities, manual_assets};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

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

/// Sum account balances + manual assets − liabilities, then upsert today's
/// snapshot. Keeps the recorded net worth consistent with the headline shown
/// on the Today/Accounts screens.
pub fn record_today(conn: &mut Connection) -> CoreResult<()> {
    let accounts_sum: i64 = accounts::list_summaries(conn)?
        .iter()
        .map(|a| a.balance_cents)
        .sum();
    let assets: i64 = manual_assets::list(conn)?
        .iter()
        .map(|a| a.value_cents)
        .sum();
    let liabilities: i64 = liabilities::list(conn)?
        .iter()
        .map(|l| l.balance_cents)
        .sum();
    record_snapshot(conn, accounts_sum + assets - liabilities)
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
    fn record_today_folds_assets_and_liabilities() {
        use crate::models::{AccountType, NewAccount, NewLiability, NewManualAsset};
        use crate::repos::{accounts, liabilities, manual_assets};

        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        accounts::insert(
            &mut conn,
            NewAccount {
                owner: "me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#3B82F6".into(),
                source: "manual".into(),
                opening_balance_cents: 10_000_000,
                simplefin_account_id: None,
                nickname: None,
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
        liabilities::create(
            &mut conn,
            NewLiability {
                name: "Mortgage".into(),
                liability_type: "mortgage".into(),
                balance_cents: 30_000_000,
                limit_cents: Some(35_000_000),
                apr_pct: Some(5.5),
                payoff_date: None,
                currency: "USD".into(),
            },
        )
        .unwrap();

        record_today(&mut conn).unwrap();

        let hist = list_history(&mut conn, 30).unwrap();
        assert_eq!(hist.len(), 1);
        // 10,000,000 accounts + 50,000,000 assets − 30,000,000 liabilities
        assert_eq!(hist[0].total_cents, 30_000_000);
    }
}
