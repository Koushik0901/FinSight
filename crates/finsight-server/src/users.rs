//! Plain-SQLite user registry at `<data_dir>/users.db`.
//! Stores Argon2id PHC verifier strings and WRAPPED db keys only — never
//! plaintext keys or passwords. Uses rusqlite directly (no SQLCipher PRAGMA).

use rusqlite::{params, Connection, OptionalExtension, Row};
use std::fmt;
use std::path::Path;
use std::sync::Mutex;

use crate::state::lock_recovered;

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
            .field(
                "kek_salt",
                &format_args!("<redacted {} bytes>", self.kek_salt.len()),
            )
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

/// A long-lived API/MCP token row. `wrapped_db_key` unwraps with the token's
/// own 32 bytes — see `crypto::unwrap_key_with_token`.
#[derive(Clone)]
pub struct ApiTokenRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    /// `"read"` or `"full"` — enforced by a CHECK constraint, not just here.
    pub scope: String,
    pub wrapped_db_key: Vec<u8>,
    pub created_at: String,
    pub last_used_at: Option<String>,
    /// `None` for a Settings-created PAT, which never expires. OAuth-issued
    /// tokens carry an expiry and are renewed with a refresh token.
    pub expires_unix: Option<i64>,
}

impl ApiTokenRecord {
    pub fn is_expired(&self, now_unix: i64) -> bool {
        self.expires_unix.is_some_and(|exp| exp <= now_unix)
    }
}

impl fmt::Debug for ApiTokenRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiTokenRecord")
            .field("id", &self.id)
            .field("user_id", &self.user_id)
            .field("name", &self.name)
            .field("scope", &self.scope)
            .field(
                "wrapped_db_key",
                &format_args!("<redacted {} bytes>", self.wrapped_db_key.len()),
            )
            .field("created_at", &self.created_at)
            .field("last_used_at", &self.last_used_at)
            .finish()
    }
}

/// What the token-management UI is allowed to see: no hash, no key material.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTokenSummary {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// A dynamically-registered OAuth public client (RFC 7591).
#[derive(Clone, Debug)]
pub struct OauthClientRecord {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub created_at: String,
}

/// A consumed refresh token. Carries everything needed to mint the next access
/// token without the user being present.
#[derive(Clone)]
pub struct RefreshTokenRecord {
    pub user_id: String,
    pub client_id: String,
    pub scope: String,
    pub wrapped_db_key: Vec<u8>,
    pub access_token_id: String,
}

impl fmt::Debug for RefreshTokenRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RefreshTokenRecord")
            .field("user_id", &self.user_id)
            .field("client_id", &self.client_id)
            .field("scope", &self.scope)
            .field(
                "wrapped_db_key",
                &format_args!("<redacted {} bytes>", self.wrapped_db_key.len()),
            )
            .field("access_token_id", &self.access_token_id)
            .finish()
    }
}

/// A consumed authorization code, minus the hash the caller already holds.
#[derive(Clone)]
pub struct OauthCodeRecord {
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub user_id: String,
    pub wrapped_db_key: Vec<u8>,
    pub scope: String,
}

impl fmt::Debug for OauthCodeRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OauthCodeRecord")
            .field("client_id", &self.client_id)
            .field("redirect_uri", &self.redirect_uri)
            .field("code_challenge", &"<redacted>")
            .field("user_id", &self.user_id)
            .field(
                "wrapped_db_key",
                &format_args!("<redacted {} bytes>", self.wrapped_db_key.len()),
            )
            .field("scope", &self.scope)
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
            );
            -- Persistent sessions (survive restarts). The DB key is stored
            -- wrapped under the server master key; the token is stored ONLY as
            -- its SHA-256 hash, so a stolen users.db never yields a live cookie.
            -- See crypto::load_or_create_server_key for the security tradeoff.
            CREATE TABLE IF NOT EXISTS sessions (
                token_hash BLOB PRIMARY KEY,
                user_id TEXT NOT NULL,
                is_admin INTEGER NOT NULL,
                wrapped_db_key BLOB NOT NULL,
                expires_unix INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
            -- Long-lived API/MCP access tokens. The token is stored ONLY as its
            -- SHA-256 hash; the DB key is wrapped under the token's own 32
            -- random bytes (crypto::wrap_key_with_token), so a bearer token
            -- alone can unlock the user's SQLCipher DB but a stolen users.db
            -- cannot.
            CREATE TABLE IF NOT EXISTS api_tokens (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                token_hash BLOB NOT NULL UNIQUE,
                wrapped_db_key BLOB NOT NULL,
                scope TEXT NOT NULL DEFAULT 'full' CHECK(scope IN ('read','full')),
                created_at TEXT NOT NULL,
                last_used_at TEXT,
                -- NULL means the token never expires: a Settings-created PAT,
                -- which a human pastes into a CLI config once and expects to
                -- keep working. OAuth-issued tokens always set this, because a
                -- connector holds a refresh token and can renew silently.
                expires_unix INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_api_tokens_user ON api_tokens(user_id);
            -- Refresh tokens for OAuth connectors. Same shape as an access
            -- token — 32 random bytes that ARE the KEK wrapping the DB key,
            -- stored only as a hash — so renewing never needs the server to
            -- hold a decryptable copy of the key between calls.
            --
            -- `access_token_id` pairs the refresh token with the access token
            -- it last minted, so rotation can retire both together.
            CREATE TABLE IF NOT EXISTS oauth_refresh_tokens (
                token_hash BLOB PRIMARY KEY,
                user_id TEXT NOT NULL,
                client_id TEXT NOT NULL,
                scope TEXT NOT NULL,
                wrapped_db_key BLOB NOT NULL,
                access_token_id TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_oauth_refresh_user ON oauth_refresh_tokens(user_id);
            -- OAuth 2.1 dynamically-registered clients (Claude/ChatGPT custom
            -- connectors). Public clients only — no secret is ever stored.
            CREATE TABLE IF NOT EXISTS oauth_clients (
                client_id TEXT PRIMARY KEY,
                client_name TEXT NOT NULL,
                redirect_uris TEXT NOT NULL, -- JSON array of exact-match URIs
                created_at TEXT NOT NULL
            );
            -- In-flight OAuth authorization codes. Same wrapping trick as
            -- api_tokens: the DB key rides through the exchange wrapped under
            -- the CODE's own 32 random bytes, so the server can hand the token
            -- endpoint an unlockable key without ever storing a bare one. Rows
            -- are single-use (consume_oauth_code deletes on read) and expire.
            CREATE TABLE IF NOT EXISTS oauth_codes (
                code_hash BLOB PRIMARY KEY,
                client_id TEXT NOT NULL,
                redirect_uri TEXT NOT NULL,
                code_challenge TEXT NOT NULL, -- base64url(SHA-256(verifier)), S256 only
                user_id TEXT NOT NULL,
                wrapped_db_key BLOB NOT NULL,
                scope TEXT NOT NULL,
                expires_unix INTEGER NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;
        // `CREATE TABLE IF NOT EXISTS` leaves an already-created table alone, so
        // a users.db written by an earlier build of this branch still lacks
        // `expires_unix`. users.db has no migration framework, and this is the
        // whole upgrade: a no-op on a fresh database, and an error meaning
        // "column already present" on one that has been through here before.
        let _ = conn.execute("ALTER TABLE api_tokens ADD COLUMN expires_unix INTEGER", []);
        // Same one-line-upgrade trick. NULL means "registered, but no human has
        // ever consented to it" — the only state `prune_unused_oauth_clients`
        // is willing to delete.
        //
        // `ADD COLUMN` succeeds only when the column was genuinely missing, so
        // an Ok here means "this database predates consent tracking". In that
        // one case every existing row is grandfathered: those clients may well
        // have been consented to before we started recording it, and there is
        // no way left to tell. Backfilling only on the upgrade is what keeps
        // this from also whitewashing junk registered after it — an
        // unconditional UPDATE on every startup would re-mark yesterday's
        // squatters as authorized and quietly defeat the prune entirely.
        if conn
            .execute(
                "ALTER TABLE oauth_clients ADD COLUMN first_authorized_at TEXT",
                [],
            )
            .is_ok()
        {
            let _ = conn.execute(
                "UPDATE oauth_clients SET first_authorized_at = created_at",
                [],
            );
        }
        Ok(Self(Mutex::new(conn)))
    }

    pub fn is_empty(&self) -> rusqlite::Result<bool> {
        let conn = lock_recovered(&self.0);
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
        let conn = lock_recovered(&self.0);
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
        let conn = lock_recovered(&self.0);
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
        let conn = lock_recovered(&self.0);
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
        let conn = lock_recovered(&self.0);
        let mut stmt = conn.prepare(
            "SELECT id, username, password_phc, kek_salt, wrapped_key_pw, wrapped_key_recovery, is_admin, created_at
             FROM users ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], row_to_user)?;
        rows.collect()
    }

    /// Rotate every credential column in one statement: the password verifier,
    /// the KEK salt, the password-wrapped key, AND the recovery-wrapped key.
    ///
    /// All four move together on purpose. Recovery redemption re-wraps the SAME
    /// db key under a new password (fresh salt) and issues a NEW recovery key —
    /// leaving `wrapped_key_recovery` untouched would keep the just-used (and
    /// possibly exposed) recovery key valid forever, which is the whole reason
    /// redemption rotates it.
    pub fn update_credentials(
        &self,
        id: &str,
        password_phc: &str,
        kek_salt: &[u8],
        wrapped_key_pw: &[u8],
        wrapped_key_recovery: &[u8],
    ) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "UPDATE users
                SET password_phc = ?2, kek_salt = ?3, wrapped_key_pw = ?4, wrapped_key_recovery = ?5
              WHERE id = ?1",
            params![
                id,
                password_phc,
                kek_salt,
                wrapped_key_pw,
                wrapped_key_recovery
            ],
        )?;
        Ok(())
    }

    pub fn delete_user(&self, id: &str) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        // Drop the user's persisted sessions in the same breath — a deleted
        // account's sessions must never be recoverable on the next restart.
        // Same reasoning for API tokens and in-flight authorization codes: both
        // carry a wrapped copy of the DB key, so leaving them behind would keep
        // a deleted account's data unlockable by whoever holds the bearer.
        conn.execute("DELETE FROM sessions WHERE user_id = ?1", params![id])?;
        conn.execute("DELETE FROM api_tokens WHERE user_id = ?1", params![id])?;
        conn.execute("DELETE FROM oauth_codes WHERE user_id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM oauth_refresh_tokens WHERE user_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM users WHERE id = ?1", params![id])?;
        Ok(())
    }

    // -------------------------------------------------- persistent sessions ---

    /// Write (or overwrite) a persisted session so it survives a restart. The
    /// caller wraps `wrapped_db_key` under the server master key first.
    pub fn persist_session(
        &self,
        token_hash: &[u8],
        user_id: &str,
        is_admin: bool,
        wrapped_db_key: &[u8],
        expires_unix: i64,
    ) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "INSERT OR REPLACE INTO sessions (token_hash, user_id, is_admin, wrapped_db_key, expires_unix)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![token_hash, user_id, is_admin as i64, wrapped_db_key, expires_unix],
        )?;
        Ok(())
    }

    /// Look up a still-valid persisted session by its token hash. Returns `None`
    /// for a missing OR expired row (the caller passes the current unix time),
    /// so a resurrected-but-expired session is never handed back.
    pub fn recover_session(
        &self,
        token_hash: &[u8],
        now_unix: i64,
    ) -> rusqlite::Result<Option<PersistedSession>> {
        let conn = lock_recovered(&self.0);
        conn.query_row(
            "SELECT user_id, is_admin, wrapped_db_key FROM sessions
             WHERE token_hash = ?1 AND expires_unix > ?2",
            params![token_hash, now_unix],
            |r| {
                Ok(PersistedSession {
                    user_id: r.get(0)?,
                    is_admin: r.get::<_, i64>(1)? != 0,
                    wrapped_db_key: r.get(2)?,
                })
            },
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })
    }

    /// Extend a persisted session's expiry (called when a session is recovered
    /// after a restart, so a continuously-used session slides forward instead of
    /// hard-expiring 30 days after it was first created).
    pub fn slide_session_expiry(
        &self,
        token_hash: &[u8],
        new_expires_unix: i64,
    ) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "UPDATE sessions SET expires_unix = ?2 WHERE token_hash = ?1",
            params![token_hash, new_expires_unix],
        )?;
        Ok(())
    }

    /// Remove one persisted session (logout). MUST run on every logout, or the
    /// signed-out session resurrects on the next restart.
    pub fn delete_session(&self, token_hash: &[u8]) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM sessions WHERE token_hash = ?1",
            params![token_hash],
        )?;
        Ok(())
    }

    /// Remove every persisted session for a user (admin delete / sign-out-all).
    pub fn delete_user_sessions(&self, user_id: &str) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute("DELETE FROM sessions WHERE user_id = ?1", params![user_id])?;
        Ok(())
    }

    /// Remove every persisted session for a user EXCEPT the one whose token
    /// hash is `keep_token_hash` (sign out other devices). Returns how many
    /// rows were removed.
    pub fn delete_user_sessions_except(
        &self,
        user_id: &str,
        keep_token_hash: &[u8],
    ) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM sessions WHERE user_id = ?1 AND token_hash != ?2",
            params![user_id, keep_token_hash],
        )
    }

    /// Drop every already-expired session. Called once at startup so the table
    /// doesn't accumulate dead rows across restarts. Returns the count removed.
    pub fn purge_expired_sessions(&self, now_unix: i64) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM sessions WHERE expires_unix <= ?1",
            params![now_unix],
        )
    }

    // ------------------------------------------------------- API tokens ---

    /// Store a new API/MCP token. `token_hash` is SHA-256 of the full token
    /// string (lookup index only); `wrapped_db_key` is the user's DB key
    /// wrapped under the token's 32 raw bytes.
    pub fn insert_api_token(
        &self,
        user_id: &str,
        name: &str,
        scope: &str,
        token_hash: &[u8],
        wrapped_db_key: &[u8],
        expires_unix: Option<i64>,
    ) -> rusqlite::Result<ApiTokenRecord> {
        let rec = ApiTokenRecord {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            scope: scope.to_string(),
            wrapped_db_key: wrapped_db_key.to_vec(),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_used_at: None,
            expires_unix,
        };
        let conn = lock_recovered(&self.0);
        conn.execute(
            "INSERT INTO api_tokens (id, user_id, name, token_hash, wrapped_db_key, scope, created_at, last_used_at, expires_unix)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
            params![
                rec.id,
                rec.user_id,
                rec.name,
                token_hash,
                rec.wrapped_db_key,
                rec.scope,
                rec.created_at,
                rec.expires_unix
            ],
        )?;
        Ok(rec)
    }

    pub fn get_api_token_by_hash(
        &self,
        token_hash: &[u8],
    ) -> rusqlite::Result<Option<ApiTokenRecord>> {
        let conn = lock_recovered(&self.0);
        conn.query_row(
            "SELECT id, user_id, name, scope, wrapped_db_key, created_at, last_used_at, expires_unix
             FROM api_tokens WHERE token_hash = ?1",
            params![token_hash],
            |r| {
                Ok(ApiTokenRecord {
                    id: r.get(0)?,
                    user_id: r.get(1)?,
                    name: r.get(2)?,
                    scope: r.get(3)?,
                    wrapped_db_key: r.get(4)?,
                    created_at: r.get(5)?,
                    last_used_at: r.get(6)?,
                    expires_unix: r.get(7)?,
                })
            },
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })
    }

    /// Stamp "last used" for the token-list UI. Best-effort telemetry, not a
    /// security control — callers throttle it so a chatty MCP client doesn't
    /// write on every single tool call.
    pub fn touch_api_token(&self, id: &str) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "UPDATE api_tokens SET last_used_at = ?2 WHERE id = ?1",
            params![id, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn list_api_tokens(&self, user_id: &str) -> rusqlite::Result<Vec<ApiTokenSummary>> {
        let conn = lock_recovered(&self.0);
        let mut stmt = conn.prepare(
            "SELECT id, name, scope, created_at, last_used_at
             FROM api_tokens WHERE user_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![user_id], |r| {
            Ok(ApiTokenSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                scope: r.get(2)?,
                created_at: r.get(3)?,
                last_used_at: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Revoke one token. Ownership is enforced in the WHERE clause rather than
    /// by a read-then-check in the handler, so there is no window in which a
    /// caller can delete another user's token by id. Returns rows removed (0 =
    /// not found OR not yours — deliberately indistinguishable).
    pub fn delete_api_token(&self, user_id: &str, id: &str) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM api_tokens WHERE user_id = ?1 AND id = ?2",
            params![user_id, id],
        )
    }

    /// Revoke every API token for a user. Called on password recovery: a leaked
    /// bearer token must not survive the flow you run precisely because you
    /// think the account is compromised.
    pub fn delete_user_api_tokens(&self, user_id: &str) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        // Refresh tokens go with them: leaving one behind would let a revoked
        // connector mint itself a fresh access token, which is the opposite of
        // what every caller of this means by "revoke everything".
        conn.execute(
            "DELETE FROM oauth_refresh_tokens WHERE user_id = ?1",
            params![user_id],
        )?;
        conn.execute(
            "DELETE FROM api_tokens WHERE user_id = ?1",
            params![user_id],
        )
    }

    // ---------------------------------------------------- refresh tokens ---

    pub fn insert_refresh_token(
        &self,
        token_hash: &[u8],
        user_id: &str,
        client_id: &str,
        scope: &str,
        wrapped_db_key: &[u8],
        access_token_id: &str,
    ) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "INSERT INTO oauth_refresh_tokens
               (token_hash, user_id, client_id, scope, wrapped_db_key, access_token_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                token_hash,
                user_id,
                client_id,
                scope,
                wrapped_db_key,
                access_token_id,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Redeem a refresh token: read it, delete it, and retire the access token
    /// it minted — all in one transaction.
    ///
    /// Rotation is mandatory for public clients (OAuth 2.1 §4.3.1) and this is
    /// where it is enforced: the presented token is gone before the caller mints
    /// a replacement, so a stolen refresh token is usable at most once, and the
    /// race where two callers both redeem it cannot happen under the write lock.
    pub fn consume_refresh_token(
        &self,
        token_hash: &[u8],
    ) -> rusqlite::Result<Option<RefreshTokenRecord>> {
        let mut conn = lock_recovered(&self.0);
        let tx = conn.transaction()?;
        let rec = tx
            .query_row(
                "SELECT user_id, client_id, scope, wrapped_db_key, access_token_id
                 FROM oauth_refresh_tokens WHERE token_hash = ?1",
                params![token_hash],
                |r| {
                    Ok(RefreshTokenRecord {
                        user_id: r.get(0)?,
                        client_id: r.get(1)?,
                        scope: r.get(2)?,
                        wrapped_db_key: r.get(3)?,
                        access_token_id: r.get(4)?,
                    })
                },
            )
            .optional()?;
        if let Some(rec) = &rec {
            tx.execute(
                "DELETE FROM oauth_refresh_tokens WHERE token_hash = ?1",
                params![token_hash],
            )?;
            // The access token this refresh token last minted is superseded by
            // the one about to be issued; leaving it live would mean every
            // renewal widened the set of usable credentials.
            tx.execute(
                "DELETE FROM api_tokens WHERE id = ?1",
                params![rec.access_token_id],
            )?;
        }
        tx.commit()?;
        Ok(rec)
    }

    /// Drop the refresh tokens tied to one access token. Called when a user
    /// revokes a connector from Settings, so revoking the visible credential
    /// also kills the invisible one that could regenerate it.
    pub fn delete_refresh_tokens_for_access_token(
        &self,
        access_token_id: &str,
    ) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM oauth_refresh_tokens WHERE access_token_id = ?1",
            params![access_token_id],
        )
    }

    /// Sweep access tokens whose expiry has passed. Expired tokens are already
    /// refused at auth time; this just stops the table growing without bound.
    pub fn purge_expired_api_tokens(&self, now_unix: i64) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM api_tokens WHERE expires_unix IS NOT NULL AND expires_unix <= ?1",
            params![now_unix],
        )
    }

    // ---------------------------------------------------- OAuth clients ---

    pub fn insert_oauth_client(
        &self,
        client_id: &str,
        client_name: &str,
        redirect_uris_json: &str,
    ) -> rusqlite::Result<String> {
        let created_at = chrono::Utc::now().to_rfc3339();
        let conn = lock_recovered(&self.0);
        conn.execute(
            "INSERT INTO oauth_clients (client_id, client_name, redirect_uris, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![client_id, client_name, redirect_uris_json, created_at],
        )?;
        Ok(created_at)
    }

    pub fn get_oauth_client(&self, client_id: &str) -> rusqlite::Result<Option<OauthClientRecord>> {
        let conn = lock_recovered(&self.0);
        let row = conn
            .query_row(
                "SELECT client_id, client_name, redirect_uris, created_at
                 FROM oauth_clients WHERE client_id = ?1",
                params![client_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        Ok(row.map(
            |(client_id, client_name, uris_json, created_at)| OauthClientRecord {
                client_id,
                client_name,
                // A malformed JSON blob degrades to "no registered URIs", which
                // fails every exact-match check — the safe direction.
                redirect_uris: serde_json::from_str(&uris_json).unwrap_or_default(),
                created_at,
            },
        ))
    }

    /// Registration is unauthenticated by spec (RFC 7591 open registration), so
    /// the table needs a ceiling to keep it from being a free write primitive.
    pub fn count_oauth_clients(&self) -> rusqlite::Result<i64> {
        let conn = lock_recovered(&self.0);
        conn.query_row("SELECT COUNT(*) FROM oauth_clients", [], |r| r.get(0))
    }

    /// Stamp a client the first time a human actually consents to it. Idempotent
    /// via `IS NULL`, so the value stays the FIRST consent rather than drifting
    /// to the most recent one — this is a "has ever been used" marker, not a
    /// last-seen timestamp, and only the former is safe to base a delete on.
    pub fn mark_oauth_client_authorized(&self, client_id: &str) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "UPDATE oauth_clients SET first_authorized_at = ?2
             WHERE client_id = ?1 AND first_authorized_at IS NULL",
            params![client_id, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Delete registrations that no one ever consented to and that are older
    /// than `created_before` (an RFC 3339 timestamp).
    ///
    /// This is the recovery path for the availability hole behind
    /// `MAX_REGISTERED_CLIENTS`: registration is unauthenticated by spec, so on
    /// an internet-reachable instance anyone can fill the table with junk and
    /// block *new* connectors until an operator hand-edits users.db.
    ///
    /// The predicate is deliberately "never authorized", not "has no live
    /// token". A connector whose refresh token expired or was revoked has still
    /// proven itself, and deleting its `client_id` would silently break its
    /// stored registration and force a re-register. Only a client that never
    /// became anything is safe to drop — and the age floor means a client
    /// registered mid-flow, moments before its user finishes clicking Approve,
    /// is never caught.
    pub fn prune_unused_oauth_clients(&self, created_before: &str) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM oauth_clients
             WHERE first_authorized_at IS NULL AND created_at < ?1",
            params![created_before],
        )
    }

    // ------------------------------------------------ OAuth auth codes ---

    #[allow(clippy::too_many_arguments)]
    pub fn insert_oauth_code(
        &self,
        code_hash: &[u8],
        client_id: &str,
        redirect_uri: &str,
        code_challenge: &str,
        user_id: &str,
        wrapped_db_key: &[u8],
        scope: &str,
        expires_unix: i64,
    ) -> rusqlite::Result<()> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "INSERT INTO oauth_codes (code_hash, client_id, redirect_uri, code_challenge, user_id, wrapped_db_key, scope, expires_unix, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                code_hash,
                client_id,
                redirect_uri,
                code_challenge,
                user_id,
                wrapped_db_key,
                scope,
                expires_unix,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Single-use redemption: reads the row and deletes it in the same critical
    /// section (the struct's `Mutex<Connection>` is the serialization point), so
    /// two concurrent exchanges of one code can never both succeed. The row is
    /// burned even when expired — a replayed code gets no second chance at the
    /// clock.
    pub fn consume_oauth_code(
        &self,
        code_hash: &[u8],
        now_unix: i64,
    ) -> rusqlite::Result<Option<OauthCodeRecord>> {
        let conn = lock_recovered(&self.0);
        let found = conn
            .query_row(
                "SELECT client_id, redirect_uri, code_challenge, user_id, wrapped_db_key, scope, expires_unix
                 FROM oauth_codes WHERE code_hash = ?1",
                params![code_hash],
                |r| {
                    Ok((
                        OauthCodeRecord {
                            client_id: r.get(0)?,
                            redirect_uri: r.get(1)?,
                            code_challenge: r.get(2)?,
                            user_id: r.get(3)?,
                            wrapped_db_key: r.get(4)?,
                            scope: r.get(5)?,
                        },
                        r.get::<_, i64>(6)?,
                    ))
                },
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                e => Err(e),
            })?;
        let Some((rec, expires_unix)) = found else {
            return Ok(None);
        };
        conn.execute(
            "DELETE FROM oauth_codes WHERE code_hash = ?1",
            params![code_hash],
        )?;
        if expires_unix <= now_unix {
            return Ok(None);
        }
        Ok(Some(rec))
    }

    pub fn purge_expired_oauth_codes(&self, now_unix: i64) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM oauth_codes WHERE expires_unix <= ?1",
            params![now_unix],
        )
    }

    pub fn delete_user_oauth_codes(&self, user_id: &str) -> rusqlite::Result<usize> {
        let conn = lock_recovered(&self.0);
        conn.execute(
            "DELETE FROM oauth_codes WHERE user_id = ?1",
            params![user_id],
        )
    }
}

/// A persisted session row, minus the token hash the caller already holds.
pub struct PersistedSession {
    pub user_id: String,
    pub is_admin: bool,
    /// DB key wrapped under the server master key — unwrap with
    /// `crypto::unwrap_key_with_server_key` before use.
    pub wrapped_db_key: Vec<u8>,
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
            .create_user(
                "koushik",
                "pw-verifier-phc",
                &[1; 16],
                &[2; 60],
                &[3; 60],
                true,
            )
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
        db.create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        assert!(db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], false)
            .is_err());
    }

    #[test]
    fn update_credentials_rotates_all_four_columns() {
        let (_d, db) = open_temp();
        let rec = db
            .create_user("a", "old-phc", &[1; 16], &[2; 60], &[3; 60], false)
            .unwrap();

        db.update_credentials(&rec.id, "new-phc", &[9; 16], &[8; 60], &[7; 60])
            .unwrap();

        let got = db.get_by_id(&rec.id).unwrap().unwrap();
        assert_eq!(got.password_phc, "new-phc");
        assert_eq!(got.kek_salt, vec![9; 16]);
        assert_eq!(got.wrapped_key_pw, vec![8; 60]);
        // The recovery wrapper MUST move too — otherwise a redeemed recovery
        // key would stay valid after rotation.
        assert_eq!(got.wrapped_key_recovery, vec![7; 60]);
        // Untouched identity columns survive.
        assert_eq!(got.username, "a");
        assert!(!got.is_admin);
    }

    #[test]
    fn list_and_delete() {
        let (_d, db) = open_temp();
        let u1 = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        db.create_user("b", "v", &[0; 16], &[0; 60], &[0; 60], false)
            .unwrap();
        assert_eq!(db.list_users().unwrap().len(), 2);
        db.delete_user(&u1.id).unwrap();
        assert_eq!(db.list_users().unwrap().len(), 1);
    }

    // ------------------------------------------------------- API tokens ---

    #[test]
    fn api_token_round_trip() {
        let (_d, db) = open_temp();
        let u = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();

        let rec = db
            .insert_api_token(&u.id, "Claude Desktop", "full", &[7; 32], &[8; 60], None)
            .unwrap();
        assert!(rec.last_used_at.is_none());

        let got = db.get_api_token_by_hash(&[7; 32]).unwrap().unwrap();
        assert_eq!(got.id, rec.id);
        assert_eq!(got.user_id, u.id);
        assert_eq!(got.scope, "full");
        assert_eq!(got.wrapped_db_key, vec![8; 60]);

        assert!(db.get_api_token_by_hash(&[9; 32]).unwrap().is_none());

        db.touch_api_token(&rec.id).unwrap();
        let summaries = db.list_api_tokens(&u.id).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "Claude Desktop");
        assert!(summaries[0].last_used_at.is_some());
    }

    #[test]
    fn api_token_delete_is_scoped_to_owner() {
        let (_d, db) = open_temp();
        let owner = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        let other = db
            .create_user("b", "v", &[0; 16], &[0; 60], &[0; 60], false)
            .unwrap();
        let tok = db
            .insert_api_token(&owner.id, "t", "read", &[1; 32], &[2; 60], None)
            .unwrap();

        // Another user naming the right id must not be able to revoke it.
        assert_eq!(db.delete_api_token(&other.id, &tok.id).unwrap(), 0);
        assert!(db.get_api_token_by_hash(&[1; 32]).unwrap().is_some());

        assert_eq!(db.delete_api_token(&owner.id, &tok.id).unwrap(), 1);
        assert!(db.get_api_token_by_hash(&[1; 32]).unwrap().is_none());
    }

    #[test]
    fn delete_user_purges_tokens_and_codes() {
        let (_d, db) = open_temp();
        let u = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        db.insert_api_token(&u.id, "t", "full", &[1; 32], &[2; 60], None)
            .unwrap();
        db.insert_oauth_code(
            &[3; 32],
            "cid",
            "https://x/cb",
            "chal",
            &u.id,
            &[4; 60],
            "full",
            9_999_999_999,
        )
        .unwrap();

        db.delete_user(&u.id).unwrap();

        assert!(db.get_api_token_by_hash(&[1; 32]).unwrap().is_none());
        assert!(db.consume_oauth_code(&[3; 32], 0).unwrap().is_none());
    }

    /// Issue #109. Registration is unauthenticated by spec, so `oauth_clients`
    /// can be filled to its cap by anyone who can reach the instance — and
    /// until now the only way back was hand-editing users.db.
    ///
    /// The prune has to cut exactly one way: junk that never became anything.
    #[test]
    fn prune_drops_only_never_consented_clients_past_the_age_floor() {
        let (_d, db) = open_temp();
        let old = (chrono::Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();

        for id in ["junk_old", "consented_old", "junk_recent"] {
            db.insert_oauth_client(id, "C", r#"["https://x/cb"]"#)
                .unwrap();
        }
        // Backdate two of them past the age floor.
        {
            let conn = db.0.lock().unwrap();
            for id in ["junk_old", "consented_old"] {
                conn.execute(
                    "UPDATE oauth_clients SET created_at = ?2 WHERE client_id = ?1",
                    params![id, old],
                )
                .unwrap();
            }
        }
        // One of the old ones was actually approved by a human.
        db.mark_oauth_client_authorized("consented_old").unwrap();

        assert_eq!(db.prune_unused_oauth_clients(&cutoff).unwrap(), 1);

        assert!(
            db.get_oauth_client("junk_old").unwrap().is_none(),
            "an old registration nobody ever consented to is the whole target"
        );
        assert!(
            db.get_oauth_client("consented_old").unwrap().is_some(),
            "a consented client must survive forever — deleting it would break a \
             working connector's stored registration and force a re-register, \
             which is why the predicate is 'never authorized' and not 'has no live token'"
        );
        assert!(
            db.get_oauth_client("junk_recent").unwrap().is_some(),
            "the age floor protects a client registered mid-flow, moments before \
             its user finishes clicking Approve"
        );
    }

    /// `mark_oauth_client_authorized` records the FIRST consent, not the most
    /// recent one. A last-seen timestamp that kept moving would be a much more
    /// tempting thing to base a delete on, and a much worse one.
    #[test]
    fn marking_authorized_is_idempotent_and_keeps_the_first_stamp() {
        let (_d, db) = open_temp();
        db.insert_oauth_client("cid", "C", r#"["https://x/cb"]"#)
            .unwrap();

        db.mark_oauth_client_authorized("cid").unwrap();
        let first: String = {
            let conn = db.0.lock().unwrap();
            conn.query_row(
                "SELECT first_authorized_at FROM oauth_clients WHERE client_id='cid'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };

        std::thread::sleep(std::time::Duration::from_millis(1100));
        db.mark_oauth_client_authorized("cid").unwrap();

        let second: String = {
            let conn = db.0.lock().unwrap();
            conn.query_row(
                "SELECT first_authorized_at FROM oauth_clients WHERE client_id='cid'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(
            first, second,
            "the stamp must not drift to the latest consent"
        );
    }

    #[test]
    fn delete_user_api_tokens_revokes_all() {
        let (_d, db) = open_temp();
        let u = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        db.insert_api_token(&u.id, "t1", "full", &[1; 32], &[0; 60], None)
            .unwrap();
        db.insert_api_token(&u.id, "t2", "read", &[2; 32], &[0; 60], None)
            .unwrap();

        assert_eq!(db.delete_user_api_tokens(&u.id).unwrap(), 2);
        assert!(db.list_api_tokens(&u.id).unwrap().is_empty());
    }

    #[test]
    fn api_token_scope_check_constraint_rejects_garbage() {
        let (_d, db) = open_temp();
        let u = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        assert!(db
            .insert_api_token(&u.id, "t", "admin", &[1; 32], &[0; 60], None)
            .is_err());
    }

    // ------------------------------------------- OAuth clients + codes ---

    #[test]
    fn oauth_client_round_trip() {
        let (_d, db) = open_temp();
        assert_eq!(db.count_oauth_clients().unwrap(), 0);
        db.insert_oauth_client("cid", "Claude", r#"["https://claude.ai/cb"]"#)
            .unwrap();

        let got = db.get_oauth_client("cid").unwrap().unwrap();
        assert_eq!(got.client_name, "Claude");
        assert_eq!(got.redirect_uris, vec!["https://claude.ai/cb".to_string()]);
        assert_eq!(db.count_oauth_clients().unwrap(), 1);
        assert!(db.get_oauth_client("nope").unwrap().is_none());
    }

    #[test]
    fn oauth_code_is_single_use() {
        let (_d, db) = open_temp();
        let u = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        db.insert_oauth_code(
            &[5; 32],
            "cid",
            "https://x/cb",
            "chal",
            &u.id,
            &[6; 60],
            "full",
            9_999_999_999,
        )
        .unwrap();

        let first = db.consume_oauth_code(&[5; 32], 0).unwrap().unwrap();
        assert_eq!(first.user_id, u.id);
        assert_eq!(first.wrapped_db_key, vec![6; 60]);
        // Replay finds nothing.
        assert!(db.consume_oauth_code(&[5; 32], 0).unwrap().is_none());
    }

    #[test]
    fn expired_oauth_code_is_rejected_and_burned() {
        let (_d, db) = open_temp();
        let u = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        db.insert_oauth_code(
            &[7; 32],
            "cid",
            "https://x/cb",
            "chal",
            &u.id,
            &[0; 60],
            "read",
            1_000,
        )
        .unwrap();

        // Expired: rejected...
        assert!(db.consume_oauth_code(&[7; 32], 2_000).unwrap().is_none());
        // ...and gone, so a later clock can't resurrect it.
        assert_eq!(db.purge_expired_oauth_codes(9_999_999_999).unwrap(), 0);
    }

    #[test]
    fn purge_expired_oauth_codes_leaves_live_rows() {
        let (_d, db) = open_temp();
        let u = db
            .create_user("a", "v", &[0; 16], &[0; 60], &[0; 60], true)
            .unwrap();
        db.insert_oauth_code(
            &[1; 32],
            "c",
            "https://x/cb",
            "ch",
            &u.id,
            &[0; 60],
            "full",
            1_000,
        )
        .unwrap();
        db.insert_oauth_code(
            &[2; 32],
            "c",
            "https://x/cb",
            "ch",
            &u.id,
            &[0; 60],
            "full",
            9_999_999_999,
        )
        .unwrap();

        assert_eq!(db.purge_expired_oauth_codes(5_000).unwrap(), 1);
        assert!(db.consume_oauth_code(&[2; 32], 5_000).unwrap().is_some());
    }
}
