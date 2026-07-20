//! CRUD for SimpleFin sync alerts.

use crate::error::CoreResult;
use crate::models::SimpleFinAlert;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

pub fn create(conn: &mut Connection, input: SimpleFinAlert) -> CoreResult<SimpleFinAlert> {
    conn.execute(
        "INSERT INTO simplefin_alerts \
         (id, account_id, alert_type, severity, message, details_json, acknowledged_at, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            &input.id,
            &input.account_id,
            &input.alert_type,
            &input.severity,
            &input.message,
            &input.details_json,
            input.acknowledged_at.map(|d| d.to_rfc3339()),
            input.created_at.to_rfc3339(),
        ],
    )?;
    Ok(input)
}

pub fn list_unacknowledged(conn: &mut Connection) -> CoreResult<Vec<SimpleFinAlert>> {
    let mut stmt = conn.prepare(
        "SELECT id, account_id, alert_type, severity, message, details_json, acknowledged_at, created_at \
         FROM simplefin_alerts \
         WHERE acknowledged_at IS NULL \
         ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let acknowledged_s: Option<String> = r.get(6)?;
        let created_s: String = r.get(7)?;
        Ok(SimpleFinAlert {
            id: r.get(0)?,
            account_id: r.get(1)?,
            alert_type: r.get(2)?,
            severity: r.get(3)?,
            message: r.get(4)?,
            details_json: r.get(5)?,
            acknowledged_at: acknowledged_s.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }),
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .unwrap()
                .with_timezone(&Utc),
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn acknowledge(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE simplefin_alerts SET acknowledged_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

pub fn has_recent_unacknowledged(
    conn: &mut Connection,
    account_id: &str,
    alert_type: &str,
) -> CoreResult<bool> {
    let since = (Utc::now() - chrono::Duration::hours(24)).to_rfc3339();
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM simplefin_alerts \
         WHERE account_id = ?1 AND alert_type = ?2 AND acknowledged_at IS NULL AND created_at >= ?3)",
        params![account_id, alert_type, since],
        |r| r.get(0),
    )?;
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
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

    fn seed_account(conn: &mut rusqlite::Connection) -> String {
        accounts::insert(
            conn,
            NewAccount {
                promo_apr_expires_on: None,
                post_promo_apr_pct: None,
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Ch".into(),
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
        .id
    }

    fn alert(account_id: &str, alert_type: &str, severity: &str, message: &str) -> SimpleFinAlert {
        SimpleFinAlert {
            id: format!("{}-{}", account_id, alert_type),
            account_id: account_id.into(),
            alert_type: alert_type.into(),
            severity: severity.into(),
            message: message.into(),
            details_json: None,
            acknowledged_at: None,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn create_and_list_unacknowledged() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        let a = alert(&acc, "drift", "warning", "Drift detected");
        create(&mut conn, a.clone()).unwrap();

        let list = list_unacknowledged(&mut conn).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, a.id);
    }

    #[test]
    fn acknowledge_hides_from_list() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        let a = alert(&acc, "drift", "warning", "Drift detected");
        create(&mut conn, a.clone()).unwrap();
        acknowledge(&mut conn, &a.id).unwrap();

        let list = list_unacknowledged(&mut conn).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn has_recent_unacknowledged_within_24h() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        let a = alert(&acc, "drift", "warning", "Drift detected");
        create(&mut conn, a.clone()).unwrap();

        assert!(has_recent_unacknowledged(&mut conn, &acc, "drift").unwrap());
        assert!(!has_recent_unacknowledged(&mut conn, &acc, "sync_error").unwrap());
        assert!(!has_recent_unacknowledged(&mut conn, "other-acc", "drift").unwrap());
    }

    #[test]
    fn old_acknowledged_alert_not_recent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let acc = seed_account(&mut conn);
        let a = SimpleFinAlert {
            id: "old".into(),
            account_id: acc.clone(),
            alert_type: "drift".into(),
            severity: "warning".into(),
            message: "old".into(),
            details_json: None,
            acknowledged_at: None,
            created_at: Utc::now() - chrono::Duration::hours(25),
        };
        create(&mut conn, a).unwrap();
        assert!(!has_recent_unacknowledged(&mut conn, &acc, "drift").unwrap());
    }
}
