//! CRUD for investment account holdings.

use crate::error::CoreResult;
use crate::models::Holding;
use rusqlite::{params, Connection};

pub fn upsert(conn: &mut Connection, input: Holding) -> CoreResult<Holding> {
    conn.execute(
        "INSERT INTO holdings \
         (id, account_id, security_id, quantity, cost_basis_cents, market_value_cents, currency, as_of_date) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
         ON CONFLICT(account_id, security_id, as_of_date) \
         DO UPDATE SET quantity = excluded.quantity, cost_basis_cents = excluded.cost_basis_cents, \
                       market_value_cents = excluded.market_value_cents, currency = excluded.currency",
        params![
            &input.id,
            &input.account_id,
            &input.security_id,
            &input.quantity,
            &input.cost_basis_cents,
            &input.market_value_cents,
            &input.currency,
            &input.as_of_date,
        ],
    )?;
    Ok(input)
}

pub fn list_by_account(conn: &mut Connection, account_id: &str) -> CoreResult<Vec<Holding>> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, security_id, quantity, cost_basis_cents, market_value_cents, currency, as_of_date \
         FROM holdings \
         WHERE account_id = ?1 \
         ORDER BY as_of_date DESC, security_id",
    )?;
    let rows = stmt.query_map(params![account_id], |r| {
        Ok(Holding {
            id: r.get(0)?,
            account_id: r.get(1)?,
            security_id: r.get(2)?,
            quantity: r.get(3)?,
            cost_basis_cents: r.get(4)?,
            market_value_cents: r.get(5)?,
            currency: r.get(6)?,
            as_of_date: r.get(7)?,
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
    use crate::{
        db::run_migrations,
        keychain,
        models::{AccountType, NewAccount, NewSimpleFinConnection, Security},
        repos::{accounts, connections, securities},
        Db,
    };
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_connection(conn: &mut rusqlite::Connection) -> String {
        connections::insert(
            conn,
            NewSimpleFinConnection {
                access_url_ref: "ref-c1".into(),
                conn_id: Some("c1".into()),
                org_id: None,
                org_name: None,
                org_url: None,
                sfin_url: None,
                label: None,
            },
        )
        .unwrap()
        .id
    }

    fn seed_account(conn: &mut rusqlite::Connection, connection_id: Option<String>) -> String {
        accounts::insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Investment,
                name: "Brokerage".into(),
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
                connection_id,
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

    fn seed_security(conn: &mut rusqlite::Connection, connection_id: &str) -> String {
        securities::upsert(
            conn,
            Security {
                id: "sec1".into(),
                connection_id: connection_id.into(),
                external_security_id: "ACME".into(),
                ticker_symbol: Some("ACME".into()),
                name: Some("Acme Corp".into()),
                currency: Some("USD".into()),
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn upsert_and_list_by_account() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let conn_id = seed_connection(&mut conn);
        let acc = seed_account(&mut conn, Some(conn_id.clone()));
        let sec = seed_security(&mut conn, &conn_id);

        let holding = Holding {
            id: "h1".into(),
            account_id: acc.clone(),
            security_id: sec.clone(),
            quantity: Some(10.5),
            cost_basis_cents: Some(10000),
            market_value_cents: Some(15000),
            currency: Some("USD".into()),
            as_of_date: "2024-06-28".into(),
        };
        upsert(&mut conn, holding.clone()).unwrap();

        let list = list_by_account(&mut conn, &acc).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, holding.id);
        assert_eq!(list[0].quantity, Some(10.5));

        let updated = Holding {
            id: holding.id.clone(),
            account_id: acc.clone(),
            security_id: sec.clone(),
            quantity: Some(20.0),
            cost_basis_cents: Some(20000),
            market_value_cents: Some(30000),
            currency: Some("USD".into()),
            as_of_date: "2024-06-28".into(),
        };
        upsert(&mut conn, updated).unwrap();
        let list = list_by_account(&mut conn, &acc).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].quantity, Some(20.0));
    }

    #[test]
    fn list_by_account_filters_other_accounts() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let conn_id = seed_connection(&mut conn);
        let acc1 = seed_account(&mut conn, Some(conn_id.clone()));
        let sec = seed_security(&mut conn, &conn_id);

        let acc2 = accounts::insert(
            &mut conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Investment,
                name: "Other".into(),
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
                connection_id: Some(conn_id.clone()),
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
        .id;

        upsert(
            &mut conn,
            Holding {
                id: "h1".into(),
                account_id: acc1.clone(),
                security_id: sec.clone(),
                quantity: Some(1.0),
                cost_basis_cents: None,
                market_value_cents: Some(1000),
                currency: Some("USD".into()),
                as_of_date: "2024-06-28".into(),
            },
        )
        .unwrap();
        upsert(
            &mut conn,
            Holding {
                id: "h2".into(),
                account_id: acc2.clone(),
                security_id: sec.clone(),
                quantity: Some(2.0),
                cost_basis_cents: None,
                market_value_cents: Some(2000),
                currency: Some("USD".into()),
                as_of_date: "2024-06-28".into(),
            },
        )
        .unwrap();

        let list = list_by_account(&mut conn, &acc1).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "h1");
    }
}
