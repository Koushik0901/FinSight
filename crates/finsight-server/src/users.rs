//! Plain-SQLite user registry at `<data_dir>/users.db`.
//! Stores Argon2id PHC verifier strings and WRAPPED db keys only — never
//! plaintext keys or passwords. Uses rusqlite directly (no SQLCipher PRAGMA).

use rusqlite::{params, Connection, Row};
use std::fmt;
use std::path::Path;
use std::sync::Mutex;

#[derive(Clone)]
pub struct UserRecord {
    pub id: String,
    pub username: String,
    pub password_phc: String,
    pub kek_salt: Vec<u8>,
    pub wrapped_key_pw: Vec<u8>,
    pub wrapped_key_recovery: Vec<u8>,
    pub is_admin: bool,
    pub created_at: String,
}

// Manual Debug: makes the no-secrets-in-logs invariant structural — a stray
// `{:?}` on a UserRecord can never leak the verifier or wrapped key material.
impl fmt::Debug for UserRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserRecord")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("password_phc", &"<redacted>")
            .field("kek_salt", &format_args!("<redacted {} bytes>", self.kek_salt.len()))
            .field(
                "wrapped_key_pw",
                &format_args!("<redacted {} bytes>", self.wrapped_key_pw.len()),
            )
            .field(
                "wrapped_key_recovery",
                &format_args!("<redacted {} bytes>", self.wrapped_key_recovery.len()),
            )
            .field("is_admin", &self.is_admin)
            .field("created_at", &self.created_at)
            .finish()
    }
}

fn row_to_user(r: &Row) -> rusqlite::Result<UserRecord> {
    Ok(UserRecord {
        id: r.get("id")?,
        username: r.get("username")?,
        password_phc: r.get("password_phc")?,
        kek_salt: r.get("kek_salt")?,
        wrapped_key_pw: r.get("wrapped_key_pw")?,
        wrapped_key_recovery: r.get("wrapped_key_recovery")?,
        is_admin: r.get::<_, i64>("is_admin")? != 0,
        created_at: r.get("created_at")?,
    })
}

pub struct UsersDb(Mutex<Connection>);

impl UsersDb {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE COLLATE NOCASE,
                password_phc TEXT NOT NULL,
                kek_salt BLOB NOT NULL,
                wrapped_key_pw BLOB NOT NULL,
                wrapped_key_recovery BLOB NOT NULL,
                is_admin INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn is_empty(&self) -> rusqlite::Result<bool> {
        let conn = self.0.lock().unwrap();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        Ok(n == 0)
    }

    pub fn create_user(
        &self,
        username: &str,
        password_phc: &str,
        kek_salt: &[u8],
        wrapped_key_pw: &[u8],
        wrapped_key_recovery: &[u8],
        is_admin: bool,
    ) -> rusqlite::Result<UserRecord> {
        let rec = UserRecord {
            id: uuid::Uuid::new_v4().to_string(),
            username: username.to_string(),
            password_phc: password_phc.to_string(),
            kek_salt: kek_salt.to_vec(),
            wrapped_key_pw: wrapped_key_pw.to_vec(),
            wrapped_key_recovery: wrapped_key_recovery.to_vec(),
            is_admin,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO users (id, username, password_phc, kek_salt, wrapped_key_pw, wrapped_key_recovery, is_admin, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                rec.id,
                rec.username,
                rec.password_phc,
                rec.kek_salt,
                rec.wrapped_key_pw,
                rec.wrapped_key_recovery,
                rec.is_admin as i64,
                rec.created_at
            ],
        )?;
        Ok(rec)
    }

    pub fn get_by_username(&self, username: &str) -> rusqlite::Result<Option<UserRecord>> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT id, username, password_phc, kek_salt, wrapped_key_pw, wrapped_key_recovery, is_admin, created_at
             FROM users WHERE username = ?1",
            params![username],
            row_to_user,
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })
    }

    pub fn get_by_id(&self, id: &str) -> rusqlite::Result<Option<UserRecord>> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT id, username, password_phc, kek_salt, wrapped_key_pw, wrapped_key_recovery, is_admin, created_at
             FROM users WHERE id = ?1",
            params![id],
            row_to_user,
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })
    }

    pub fn list_users(&self) -> rusqlite::Result<Vec<UserRecord>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, username, password_phc, kek_salt, wrapped_key_pw, wrapped_key_recovery, is_admin, created_at
             FROM users ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], row_to_user)?;
        rows.collect()
    }

    pub fn delete_user(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_temp() -> (tempfile::TempDir, UsersDb) {
        let dir = tempfile::tempdir().unwrap();
        let db = UsersDb::open(&dir.path().join("users.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn create_and_fetch_user() {
        let (_d, db) = open_temp();
        assert!(db.is_empty().unwrap());
        let rec = db
            .create_user("koushik", "pw-verifier-phc", &[1; 16], &[2; 60], &[3; 60], true)
            .unwrap();
        assert!(!db.is_empty().unwrap());
        let got = db.get_by_username("koushik").unwrap().unwrap();
        assert_eq!(got.id, rec.id);
        assert!(got.is_admin);
        assert_eq!(got.kek_salt, vec![1; 16]);
        assert_eq!(got.wrapped_key_pw, vec![2; 60]);
    }

    #[test]
    fn duplicate_username_rejected() {
        let (_d, db) = open_temp();
        db.create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true).unwrap();
        assert!(db.create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], false).is_err());
    }

    #[test]
    fn list_and_delete() {
        let (_d, db) = open_temp();
        let u1 = db.create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true).unwrap();
        db.create_user("b", "v", &[0; 16], &[0; 60], &[0; 60], false).unwrap();
        assert_eq!(db.list_users().unwrap().len(), 2);
        db.delete_user(&u1.id).unwrap();
        assert_eq!(db.list_users().unwrap().len(), 1);
    }
}
