//! Parse and import SimpleFin investment holdings.
//!
//! Investment accounts may carry an `extra` field with a `holdings` array.
//! Each entry describes a security (ticker, name), quantity, unit price,
//! and currency. We upsert securities and daily holdings snapshots.

use chrono::Utc;
use finsight_core::models::{Holding, Security};
use finsight_core::repos::{holdings, securities};
use rusqlite::Connection;
use serde_json::Value;
use uuid::Uuid;

pub fn import_holdings(
    conn: &mut Connection,
    connection_id: &str,
    account_id: &str,
    extra: Option<&Value>,
) -> Result<Vec<Holding>, finsight_core::CoreError> {
    let today = Utc::now().date_naive().to_string();
    let mut out = Vec::new();

    let holdings_arr = extra
        .and_then(|v| v.get("holdings"))
        .and_then(|v| v.as_array());

    let Some(arr) = holdings_arr else {
        return Ok(out);
    };

    for item in arr {
        let ticker = item
            .get("ticker")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let quantity = item.get("quantity").and_then(|v| v.as_f64());
        let unit_price_str = item
            .get("unit_price")
            .and_then(|v| v.as_str())
            .unwrap_or("0");
        let currency = item
            .get("currency")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let unit_cents = parse_amount_cents(unit_price_str).unwrap_or(0);
        let market_value_cents = quantity.map(|q| (q * unit_cents as f64).round() as i64);

        let sec = securities::upsert(
            conn,
            Security {
                id: Uuid::new_v4().to_string(),
                connection_id: connection_id.to_string(),
                external_security_id: ticker.clone(),
                ticker_symbol: if ticker.is_empty() {
                    None
                } else {
                    Some(ticker)
                },
                name,
                currency: currency.clone(),
            },
        )?;

        let holding = holdings::upsert(
            conn,
            Holding {
                id: Uuid::new_v4().to_string(),
                account_id: account_id.to_string(),
                security_id: sec.id,
                quantity,
                cost_basis_cents: None,
                market_value_cents,
                currency,
                as_of_date: today.clone(),
            },
        )?;
        out.push(holding);
    }

    Ok(out)
}

fn parse_amount_cents(amount: &str) -> Option<i64> {
    use rust_decimal::prelude::*;
    use rust_decimal::Decimal;
    let decimal = amount.trim().parse::<Decimal>().ok()?;
    let rounded = decimal.round_dp(2);
    let cents = (rounded * Decimal::from(100)).round_dp(0).to_i64()?;
    Some(cents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{
        db::run_migrations,
        keychain,
        models::{AccountType, NewAccount},
        repos::accounts,
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

    fn seed_connection(conn: &mut Connection) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO simplefin_connections (id, access_url_ref, status, created_at) VALUES (?1, ?2, 'active', ?3)",
            rusqlite::params![&id, "bridge-1", Utc::now().to_rfc3339()],
        ).unwrap();
        id
    }

    fn seed_account(conn: &mut Connection, connection_id: &str) -> String {
        accounts::insert(
            conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                owner: "Me".into(),
                bank: "Schwab".into(),
                r#type: AccountType::Investment,
                name: "Brokerage".into(),
                last4: None,
                currency: "USD".into(),
                color: "#fff".into(),
                opening_balance_cents: 0,
                source: "simplefin".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: false,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: Some("sf-1".into()),
                nickname: None,
                connection_id: Some(connection_id.to_string()),
                institution_id: None,
                external_account_id: Some("ext-1".into()),
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "investment".into(),
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

    #[test]
    fn imports_holdings_from_extra() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let cid = seed_connection(&mut conn);
        let acc = seed_account(&mut conn, &cid);

        let extra: Value = serde_json::from_str(
            r#"{"holdings":[{"ticker":"VTI","name":"Vanguard Total Stock","quantity":10.5,"unit_price":"250.00","currency":"USD"}]}"#
        ).unwrap();

        let results = import_holdings(&mut conn, &cid, &acc, Some(&extra)).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].quantity, Some(10.5));
        assert_eq!(results[0].market_value_cents, Some(262500));
    }

    #[test]
    fn no_holdings_when_extra_is_none() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let cid = seed_connection(&mut conn);
        let acc = seed_account(&mut conn, &cid);
        let results = import_holdings(&mut conn, &cid, &acc, None).unwrap();
        assert!(results.is_empty());
    }
}
