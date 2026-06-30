//! CRUD for securities referenced by investment holdings.

use crate::error::CoreResult;
use crate::models::Security;
use rusqlite::{params, Connection};

pub fn upsert(conn: &mut Connection, input: Security) -> CoreResult<Security> {
    conn.execute(
        "INSERT INTO securities \
         (id, connection_id, external_security_id, ticker_symbol, name, currency) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(connection_id, external_security_id) \
         DO UPDATE SET ticker_symbol = excluded.ticker_symbol, name = excluded.name, currency = excluded.currency",
        params![
            &input.id,
            &input.connection_id,
            &input.external_security_id,
            &input.ticker_symbol,
            &input.name,
            &input.currency,
        ],
    )?;
    Ok(input)
}

pub fn get_by_external_id(
    conn: &mut Connection,
    connection_id: &str,
    external_id: &str,
) -> CoreResult<Option<Security>> {
    let mut stmt = conn.prepare(
        "SELECT id, connection_id, external_security_id, ticker_symbol, name, currency \
         FROM securities \
         WHERE connection_id = ?1 AND external_security_id = ?2",
    )?;
    let mut rows = stmt.query_map(params![connection_id, external_id], |r| {
        Ok(Security {
            id: r.get(0)?,
            connection_id: r.get(1)?,
            external_security_id: r.get(2)?,
            ticker_symbol: r.get(3)?,
            name: r.get(4)?,
            currency: r.get(5)?,
        })
    })?;
    Ok(rows.next().transpose()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::run_migrations, keychain, models::NewSimpleFinConnection, repos::connections, Db,
    };
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("t.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_connection(conn: &mut rusqlite::Connection, conn_id: &str) -> String {
        connections::insert(
            conn,
            NewSimpleFinConnection {
                access_url_ref: format!("ref-{}", conn_id),
                conn_id: Some(conn_id.into()),
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

    fn security(conn_id: &str, ext_id: &str, ticker: &str) -> Security {
        Security {
            id: format!("{}-{}", conn_id, ext_id),
            connection_id: conn_id.into(),
            external_security_id: ext_id.into(),
            ticker_symbol: Some(ticker.into()),
            name: Some("Acme Corp".into()),
            currency: Some("USD".into()),
        }
    }

    #[test]
    fn upsert_inserts_and_updates() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let conn_id = seed_connection(&mut conn, "c1");
        let s = security(&conn_id, "ACME", "ACME");
        upsert(&mut conn, s.clone()).unwrap();

        let fetched = get_by_external_id(&mut conn, &conn_id, "ACME")
            .unwrap()
            .unwrap();
        assert_eq!(fetched.ticker_symbol.as_deref(), Some("ACME"));
        assert_eq!(fetched.name.as_deref(), Some("Acme Corp"));

        let updated = Security {
            id: s.id.clone(),
            connection_id: s.connection_id.clone(),
            external_security_id: s.external_security_id.clone(),
            ticker_symbol: Some("ACME2".into()),
            name: Some("Acme2".into()),
            currency: Some("CAD".into()),
        };
        upsert(&mut conn, updated).unwrap();

        let fetched = get_by_external_id(&mut conn, &conn_id, "ACME")
            .unwrap()
            .unwrap();
        assert_eq!(fetched.ticker_symbol.as_deref(), Some("ACME2"));
        assert_eq!(fetched.currency.as_deref(), Some("CAD"));
    }

    #[test]
    fn get_by_external_id_missing_returns_none() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let conn_id = seed_connection(&mut conn, "c1");
        let result = get_by_external_id(&mut conn, &conn_id, "MISSING").unwrap();
        assert!(result.is_none());
    }
}
