# FinSight Server — Phase 2: Auth & Multi-User Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Phase 1's single hardcoded user + plaintext keyfile with real accounts: Argon2id-verified logins, per-user SQLCipher DBs under password-wrapped random keys (plus a printable recovery key), cookie sessions, per-user runtimes with idle eviction, an admin user-management surface, and session-scoped background jobs with a login-time catch-up cascade.

**Architecture:** `finsight-server` gains four modules — `crypto` (Argon2id KDF + XChaCha20-Poly1305 key wrapping), `users` (plain-SQLite registry at `<data>/users.db`), `sessions` (opaque-token cookie store holding each logged-in user's unwrapped DB key in memory), and `registry` (lazy per-user `UserRuntime` = ApiState + its own event broadcast, idle-evicted). `ServerState` becomes `{users, sessions, registry, data_dir}`; the dispatcher and SSE endpoint resolve the runtime from the session cookie. The desktop startup cascade is extracted into `finsight_api::startup` and reused as the per-user login catch-up (closing Phase 1's deferred cascade-parity item). The UI gets server-mode-only Setup/Login screens that call `/api/auth/*` with plain `fetch` (these are NOT Tauri commands; bindings stay byte-identical).

**Tech Stack:** axum 0.8 + `axum-extra` (cookie), `argon2` 0.5, `chacha20poly1305` 0.10, rusqlite (plain mode for users.db), existing finsight-api/core; React screens + vitest.

**Spec:** `docs/superpowers/specs/2026-07-15-server-architecture-design.md` (Phase 2 scope: "auth, multi-user, key wrapping, sessions, login screen, admin user management, per-user pools/agents, session-scoped jobs + catch-up").

---

## Ground rules (read first — same as Phase 1, plus Phase 2 specifics)

- **Run every `cargo` command via PowerShell, NOT Git Bash** (vendored-OpenSSL needs Strawberry Perl). Run cargo tests as **single foreground blocking calls** (`timeout: 600000`); never background cargo, never Monitor-wait on cargo. One cargo invocation at a time (Windows link 1104). If you hit `LNK1102` (linker OOM) retry with `CARGO_BUILD_JOBS=2`; `LNK1318`/`os error 112` = disk full → report BLOCKED.
- **Baseline (verified green at Phase 1 close):** Rust workspace **523 passed/0 failed**, frontend **428 tests/83 files** + `tsc --noEmit` clean, bindings byte-identical.
- **Bindings invariant continues:** Phase 2 adds NO Tauri commands and touches no wrappers except where a task explicitly says so. After any task touching `crates/finsight-app` or `finsight-api` command signatures: `cargo run -p finsight-tauri --bin export_bindings && git diff --exit-code ui/src/api/bindings.ts` → exit 0. Auth endpoints are server-only REST — they must NOT appear in bindings.
- **Standalone-build invariant:** any change to `finsight-api` must pass an ISOLATED `cargo test -p finsight-api`.
- **Parity tests keep passing untouched:** `/api/auth/*` routes are not RPC commands; `crates/finsight-server/tests/parity.rs` must stay green with zero edits. If a task makes it red, the task is wrong.
- **Security invariants:** unwrapped DB keys live ONLY in server memory (SessionStore + UserRuntime), never on disk, never in logs (`tracing` lines must not include key/password material). `users.db` stores only Argon2id PHC verifier strings and WRAPPED keys. The plaintext-recovery-key is returned exactly once (setup/create response) and never stored.
- Commit per task, normal commits on top of HEAD (no amends/rebases). Current branch: `pwa-desktop-architecture-72a060`.

## Data layout after Phase 2

```
<FINSIGHT_DATA_DIR>/
  users.db                      # plain SQLite: user registry (verifiers + wrapped keys)
  users/<user_id>/
    data.sqlcipher              # per-user SQLCipher DB (random key, wrapped in users.db)
    data.sqlcipher-wal/-shm
    backups/                    # per-user backup dir (used by data_health + cascade)
```
Phase 1's `<data>/data.sqlcipher` + `<data>/db.key` are migrated into the first admin's dir at setup (Task 6) and `db.key` is deleted after its key is wrapped.

---

### Task 1: `crypto` module — Argon2id verify/KDF + XChaCha20-Poly1305 key wrapping

**Files:**
- Create: `crates/finsight-server/src/crypto.rs`
- Modify: `crates/finsight-server/src/lib.rs` (add `pub mod crypto;`), `crates/finsight-server/Cargo.toml`

- [ ] **Step 1: Add deps** to `crates/finsight-server/Cargo.toml` `[dependencies]`:
```toml
argon2 = "0.5"
chacha20poly1305 = "0.10"
hex = "0.4"
```
(`rand` is already a dep. `argon2` pulls `password-hash` for PHC-string verify.)

- [ ] **Step 2: Write the failing tests** (in `crypto.rs` `#[cfg(test)]`):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_verify_round_trip() {
        let phc = hash_password("hunter2").unwrap();
        assert!(verify_password("hunter2", &phc));
        assert!(!verify_password("wrong", &phc));
    }

    #[test]
    fn wrap_unwrap_round_trip_with_password_kek() {
        let dbkey = generate_db_key(); // 32 bytes
        let salt = generate_salt();    // 16 bytes
        let wrapped = wrap_key_with_password("hunter2", &salt, &dbkey).unwrap();
        let back = unwrap_key_with_password("hunter2", &salt, &wrapped).unwrap();
        assert_eq!(back, dbkey);
    }

    #[test]
    fn wrong_password_fails_to_unwrap() {
        let dbkey = generate_db_key();
        let salt = generate_salt();
        let wrapped = wrap_key_with_password("hunter2", &salt, &dbkey).unwrap();
        assert!(unwrap_key_with_password("wrong", &salt, &wrapped).is_err());
    }

    #[test]
    fn recovery_key_wraps_and_unwraps() {
        let dbkey = generate_db_key();
        let recovery = generate_recovery_key(); // RecoveryKey { bytes, display }
        let wrapped = wrap_key_with_recovery(&recovery.bytes, &dbkey).unwrap();
        let back = unwrap_key_with_recovery_display(&recovery.display, &wrapped).unwrap();
        assert_eq!(back, dbkey);
        // display form is 8 groups of 8 hex chars, dash separated
        assert_eq!(recovery.display.split('-').count(), 8);
        assert!(unwrap_key_with_recovery_display("bad-key", &wrapped).is_err());
    }

    #[test]
    fn db_key_is_64_hex_for_sqlcipher() {
        let k = generate_db_key();
        assert_eq!(db_key_to_hex(&k).len(), 64); // Db::open requires 64 hex chars
    }
}
```

- [ ] **Step 3: Run** — PowerShell `cargo test -p finsight-server crypto` → FAIL (module empty).

- [ ] **Step 4: Implement** (complete module):
```rust
//! Password verification and DB-key wrapping.
//!
//! Design (Bitwarden pattern, per the spec): each user's SQLCipher key is a
//! RANDOM 32-byte key. It is stored only in WRAPPED form, twice:
//!   - under KEK1 = Argon2id(password, kek_salt)   → password changes re-wrap, not re-encrypt
//!   - under KEK2 = the recovery key bytes directly → recovery key IS high-entropy, no KDF needed
//! Password *verification* uses a separate Argon2id PHC string (its own salt) so
//! the verifier can't be used to derive the KEK.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng as AeadOsRng};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;

pub const DB_KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24; // XChaCha20

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("password hashing failed: {0}")]
    Hash(String),
    #[error("wrong password or corrupted wrapped key")]
    Unwrap,
    #[error("malformed recovery key")]
    BadRecoveryKey,
}

pub fn hash_password(password: &str) -> Result<String, CryptoError> {
    let salt = SaltString::generate(&mut AeadOsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| CryptoError::Hash(e.to_string()))
}

pub fn verify_password(password: &str, phc: &str) -> bool {
    PasswordHash::new(phc)
        .map(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

pub fn generate_db_key() -> [u8; DB_KEY_LEN] {
    let mut k = [0u8; DB_KEY_LEN];
    rand::thread_rng().fill_bytes(&mut k);
    k
}

pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut s = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut s);
    s
}

pub fn db_key_to_hex(key: &[u8; DB_KEY_LEN]) -> String {
    hex::encode(key)
}

fn derive_kek(password: &str, salt: &[u8]) -> Result<[u8; 32], CryptoError> {
    let mut kek = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut kek)
        .map_err(|e| CryptoError::Hash(e.to_string()))?;
    Ok(kek)
}

fn wrap_with_kek(kek: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = XChaCha20Poly1305::new(kek.into());
    let mut nonce = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| CryptoError::Unwrap)?;
    let mut out = nonce.to_vec();
    out.extend(ct);
    Ok(out)
}

fn unwrap_with_kek(kek: &[u8; 32], wrapped: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if wrapped.len() < NONCE_LEN + 16 {
        return Err(CryptoError::Unwrap);
    }
    let (nonce, ct) = wrapped.split_at(NONCE_LEN);
    XChaCha20Poly1305::new(kek.into())
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| CryptoError::Unwrap)
}

pub fn wrap_key_with_password(
    password: &str, kek_salt: &[u8], dbkey: &[u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    wrap_with_kek(&derive_kek(password, kek_salt)?, dbkey)
}

pub fn unwrap_key_with_password(
    password: &str, kek_salt: &[u8], wrapped: &[u8],
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    let v = unwrap_with_kek(&derive_kek(password, kek_salt)?, wrapped)?;
    v.try_into().map_err(|_| CryptoError::Unwrap)
}

/// Recovery key: 32 random bytes, shown once as 8 dash-separated hex groups.
pub struct RecoveryKey {
    pub bytes: [u8; 32],
    pub display: String,
}

pub fn generate_recovery_key() -> RecoveryKey {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    let h = hex::encode(b);
    let display = h.as_bytes().chunks(8)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>().join("-");
    RecoveryKey { bytes: b, display }
}

pub fn recovery_display_to_bytes(display: &str) -> Result<[u8; 32], CryptoError> {
    let h: String = display.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    let v = hex::decode(&h).map_err(|_| CryptoError::BadRecoveryKey)?;
    v.try_into().map_err(|_| CryptoError::BadRecoveryKey)
}

pub fn wrap_key_with_recovery(
    recovery_bytes: &[u8; 32], dbkey: &[u8; DB_KEY_LEN],
) -> Result<Vec<u8>, CryptoError> {
    wrap_with_kek(recovery_bytes, dbkey)
}

pub fn unwrap_key_with_recovery_display(
    display: &str, wrapped: &[u8],
) -> Result<[u8; DB_KEY_LEN], CryptoError> {
    let bytes = recovery_display_to_bytes(display)?;
    let v = unwrap_with_kek(&bytes, wrapped)?;
    v.try_into().map_err(|_| CryptoError::Unwrap)
}
```
Add `thiserror.workspace = true` to Cargo.toml if not present.

- [ ] **Step 5: Run** — `cargo test -p finsight-server crypto` → all 5 PASS. (Argon2 default params make each test ~100ms — acceptable.)
- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat(server): crypto module — Argon2id verify/KDF + XChaCha20 key wrapping"`

---

### Task 2: `users` module — registry DB + CRUD

**Files:**
- Create: `crates/finsight-server/src/users.rs`
- Modify: `crates/finsight-server/src/lib.rs` (`pub mod users;`), Cargo.toml (`rusqlite.workspace = true`, `uuid.workspace = true`)

users.db is PLAIN SQLite (the workspace rusqlite builds SQLCipher, which acts as plain SQLite when no `PRAGMA key` is issued — that's what we want; do NOT set a key).

- [ ] **Step 1: Failing tests** (`#[cfg(test)]` in users.rs; use `tempfile::tempdir`):
```rust
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
        let rec = db.create_user("koushik", "pw-verifier-phc", &[1; 16], &[2; 60], &[3; 60], true).unwrap();
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
```

- [ ] **Step 2: Run** — FAIL. **Step 3: Implement:**
```rust
//! Plain-SQLite user registry at `<data_dir>/users.db`.
//! Stores Argon2id PHC verifier strings and WRAPPED db keys only — never
//! plaintext keys or passwords. Uses rusqlite directly (no SQLCipher PRAGMA).

use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone)]
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
        &self, username: &str, password_phc: &str, kek_salt: &[u8],
        wrapped_key_pw: &[u8], wrapped_key_recovery: &[u8], is_admin: bool,
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
            params![rec.id, rec.username, rec.password_phc, rec.kek_salt, rec.wrapped_key_pw, rec.wrapped_key_recovery, rec.is_admin as i64, rec.created_at],
        )?;
        Ok(rec)
    }

    pub fn get_by_username(&self, username: &str) -> rusqlite::Result<Option<UserRecord>> { /* SELECT … WHERE username = ?1, map row */ }
    pub fn get_by_id(&self, id: &str) -> rusqlite::Result<Option<UserRecord>> { /* SELECT … WHERE id = ?1 */ }
    pub fn list_users(&self) -> rusqlite::Result<Vec<UserRecord>> { /* SELECT … ORDER BY created_at */ }
    pub fn delete_user(&self, id: &str) -> rusqlite::Result<()> { /* DELETE WHERE id */ }
}
```
(The three query fns are 8-line row-mappers — write them out fully; one shared `fn row_to_user(r: &rusqlite::Row) -> rusqlite::Result<UserRecord>`.)

- [ ] **Step 4: Run** — 3 tests PASS. **Step 5: Commit** — `feat(server): users.db registry (verifiers + wrapped keys)`

---

### Task 3: `sessions` + `registry` — cookie sessions and per-user runtimes

**Files:**
- Create: `crates/finsight-server/src/sessions.rs`, `crates/finsight-server/src/registry.rs`
- Modify: `lib.rs` (add mods), Cargo.toml (`axum-extra = { version = "0.10", features = ["cookie"] }`)

- [ ] **Step 1: `sessions.rs`** — failing tests then implement:
```rust
//! Opaque-token session store. The UNWRAPPED per-user DB key lives here, in
//! memory only, for the life of the session (spec: background work possible
//! only while a session holds the key). Sessions do not survive restarts.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const SESSION_COOKIE: &str = "finsight_session";
const SESSION_TTL: Duration = Duration::from_secs(30 * 24 * 3600); // 30d sliding

pub struct SessionEntry {
    pub user_id: String,
    pub db_key_hex: String, // 64-hex SQLCipher key
    pub is_admin: bool,
    pub expires: Instant,
}

#[derive(Default)]
pub struct SessionStore(Mutex<HashMap<String, SessionEntry>>);

impl SessionStore {
    pub fn create(&self, user_id: &str, db_key_hex: String, is_admin: bool) -> String {
        let mut tok = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut tok);
        let token = hex::encode(tok);
        self.0.lock().unwrap().insert(token.clone(), SessionEntry {
            user_id: user_id.to_string(), db_key_hex, is_admin,
            expires: Instant::now() + SESSION_TTL,
        });
        token
    }
    /// Sliding expiry: touch on every successful lookup.
    pub fn get(&self, token: &str) -> Option<(String, String, bool)> { /* expire check, touch, clone out (user_id, key, is_admin) */ }
    pub fn remove(&self, token: &str) { /* … */ }
    /// Drop every session for a user (admin deletion).
    pub fn remove_user(&self, user_id: &str) { /* retain != user_id */ }
}
```
Tests: create→get round-trip; get after `remove` is None; `remove_user` clears; expired entry (construct with a past `expires` via a test-only helper or make TTL injectable) is None.

- [ ] **Step 2: `registry.rs`** — the per-user runtime:
```rust
//! Lazy per-user runtimes. Each logged-in user gets: their own SQLCipher pool
//! (ApiState), their own event broadcast (SSE), their own agent thread — built
//! on first authenticated request, evicted after idle timeout (pools dropped;
//! the session still holds the unwrapped key, so the next request rebuilds).
use crate::state::OutboundEvent;
use finsight_api::ApiState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

pub struct UserRuntime {
    pub api: Arc<ApiState>,
    pub events: broadcast::Sender<OutboundEvent>,
    pub last_active: Mutex<Instant>,
    /// Handle for this runtime's background sync loop; aborted on eviction.
    pub sync_task: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
pub struct Registry(Mutex<HashMap<String, Arc<UserRuntime>>>);

pub fn user_data_dir(data_dir: &Path, user_id: &str) -> PathBuf {
    data_dir.join("users").join(user_id)
}

impl Registry {
    /// Get or lazily bootstrap the runtime for `user_id`. Mirrors Phase 1's
    /// ServerState::bootstrap but per-user: open Db with `db_key_hex`, run
    /// migrations + provider migration, wire AgentEvent→broadcast (the same
    /// names configure_app uses), ApiState::new, load+set provider, run the
    /// login catch-up cascade (finsight_api::startup) + send CheckDueRecipes,
    /// then start the sync scheduler on the current runtime handle.
    pub async fn get_or_bootstrap(
        &self, data_dir: &Path, user_id: &str, db_key_hex: &str,
    ) -> anyhow::Result<Arc<UserRuntime>> { /* full body in Task 6 (it reuses startup extraction from Task 4) */ }

    pub fn touch(&self, user_id: &str) { /* update last_active */ }
    pub fn evict(&self, user_id: &str) { /* remove + abort sync_task */ }
    /// Called by a background interval task: evict runtimes idle > 30 min.
    pub fn evict_idle(&self, max_idle: Duration) -> Vec<String> { /* collect + evict, return ids for logging */ }
}
```
Test now (registry mechanics only, no DB): a fake-runtime constructor isn't feasible without a DB — so Task 3's registry tests cover `user_data_dir` and `evict_idle`'s bookkeeping with a runtime built against a tempdir DB (cheap: `ApiState::new` needs a real Db — use `Db::open(tmp, <64-hex>)` + `run_migrations`, same as Phase 1's `test_state`). Assert: second `get_or_bootstrap` returns the same Arc (no double bootstrap); after `evict`, a new Arc is built.
NOTE: `get_or_bootstrap`'s full body lands in Task 6; for THIS task implement it complete enough for the tests (bootstrap without cascade/scheduler — leave two `// wired in Task 6` no-op sites for cascade + sync start, but the Db/agent/broadcast wiring must be real).

- [ ] **Step 3: Run tests** (`cargo test -p finsight-server sessions registry`) → PASS. **Step 4: Commit** — `feat(server): session store + per-user runtime registry with idle eviction`

---

### Task 4: Extract the startup cascade into `finsight_api::startup` (shared desktop/server)

This closes Phase 1's deferred "desktop startup cascade parity" item and gives Task 6 its login catch-up.

**Files:**
- Create: `crates/finsight-api/src/startup.rs`
- Modify: `crates/finsight-api/src/lib.rs` (`pub mod startup;`), `crates/finsight-app/src/lib.rs` (setup calls the shared fn)

- [ ] **Step 1:** Move the cascade body from `crates/finsight-app/src/lib.rs` (~lines 275–410: integrity check + integrity settings writes, pre-migration backup when `pending_migration_count() > 0`, `run_migrations`, `migrate_provider_settings`, builtin categorization, `pair_transfers`, per-account `recompute_balance_if_linked`, `net_worth::record_today` + `backfill_history_from_transactions`, `recompute_anomalies`, `data.startup_summary`/`data.startup_warnings` settings writes, final `db.checkpoint()`) into:
```rust
/// Everything FinSight refreshes when a database "wakes up": desktop app
/// startup, and server-side user login (catch-up for jobs missed while the
/// user's DB key was not in memory). Extracted verbatim from the desktop
/// setup; behavior-preserving.
pub struct StartupReport {
    pub summary: String,
    pub warnings: Vec<String>,
}

pub fn run_startup_cascade(db: &finsight_core::Db, backups_dir: &std::path::Path) -> StartupReport
```
Keep the staged-restore file swap and keychain OUT (desktop-only, stays in finsight-app before `Db::open`). The desktop setup becomes: resolve dir → staged-restore swap → keychain key → `Db::open` → `finsight_api::startup::run_startup_cascade(&db, &backups_dir)` → window/event wiring. The moved code is behavior-preserving — same order, same error recording.
- [ ] **Step 2:** Add one unit test in `startup.rs`: run against a fresh migrated tempdir DB → returns without error, `data.startup_summary` setting exists (may be empty string), integrity status setting written.
- [ ] **Step 3: Gates** — `cargo test -p finsight-api` (isolated) PASS; `cargo test -p finsight-app` PASS; **bindings zero-diff** (`cargo run -p finsight-tauri --bin export_bindings && git diff --exit-code ui/src/api/bindings.ts`).
- [ ] **Step 4: Commit** — `refactor(server): extract startup cascade to finsight-api (shared desktop/login catch-up)`

---

### Task 5: Auth endpoints + `AuthedUser` extractor

**Files:**
- Create: `crates/finsight-server/src/auth.rs`
- Modify: `router.rs` (routes + state), `lib.rs` (`pub mod auth;`)

Endpoints (all JSON; NOT in bindings; errors use the AppError shape `{code, message}`):
- `GET  /api/auth/status` → `{ "needsSetup": bool, "authenticated": bool, "username": string|null, "isAdmin": bool|null }` (no auth required)
- `POST /api/auth/setup` `{username, password}` → 200 `{ "recoveryKey": "xxxxxxxx-…" }` + session cookie. Only when `users.is_empty()`; else 409 `auth.already_setup`. Runs the Task 6 legacy migration if `<data>/data.sqlcipher` exists.
- `POST /api/auth/login` `{username, password}` → 200 `{}` + cookie, or 401 `auth.bad_credentials` (same code for unknown user vs wrong password — no username oracle).
- `POST /api/auth/logout` → clears session + cookie.
- Admin only: `GET /api/auth/users` → `[{id, username, isAdmin, createdAt}]`; `POST /api/auth/users` `{username, password}` → `{recoveryKey}`; `DELETE /api/auth/users/{id}` → 200 (refuses deleting yourself: 400 `auth.cannot_delete_self`; kills the target's sessions + runtime; deletes their user dir — destructive and admin-gated).

The extractor:
```rust
/// Axum extractor: resolves the session cookie to (user_id, db_key_hex, is_admin).
/// 401 AppError {code:"auth.required"} when missing/expired — the UI shim keys
/// its login redirect off this code.
pub struct AuthedUser {
    pub user_id: String,
    pub db_key_hex: String,
    pub is_admin: bool,
}
impl<S> axum::extract::FromRequestParts<S> for AuthedUser where … // read CookieJar (axum-extra), SessionStore lookup
```
`login`/`setup` flow: verify password (crypto) → unwrap `wrapped_key_pw` with password → `sessions.create(user_id, db_key_hex, is_admin)` → set cookie `finsight_session=<token>; HttpOnly; SameSite=Lax; Path=/` (+ `Secure` only when env `FINSIGHT_COOKIE_SECURE=1` — localhost dev is http; document that reverse-proxy deployments should set it).

- [ ] **Step 1: Failing integration tests** (`crates/finsight-server/tests/auth.rs`, tower::ServiceExt against `build_router` with a tempdir ServerState — Task 6 reshapes ServerState first? NO: see ordering note below):

**ORDERING NOTE:** Tasks 5 and 6 are interlocked — the extractor needs ServerState to hold `users`/`sessions`, and dispatch needs the extractor. Implement them as ONE unit in this order: (a) Task 6 Step 1 (ServerState reshape, compiles with dispatch temporarily broken), (b) Task 5 (auth module + routes), (c) Task 6 remainder (dispatch/events rewiring). The PLAN keeps them as separate tasks for review granularity, but the implementer of Task 5 should read Task 6 FIRST and coordinate; if executing with separate subagents, give both tasks to the same agent run. Tests land with Task 6's completion.

Test list (write them now, they compile after (c)):
```text
setup_when_empty_creates_admin_and_logs_in     → POST setup → 200 with recoveryKey; GET status → authenticated:true
setup_twice_is_409                              → second POST setup → 409 auth.already_setup
login_logout_lifecycle                          → logout → rpc 401; login → rpc 200
bad_password_is_401_bad_credentials             → and unknown username → same code
rpc_without_cookie_is_401_auth_required        → POST /api/rpc/list_accounts no cookie → 401 {code:"auth.required"}
admin_create_user_and_user_isolation           → admin creates "bob"; bob logs in; bob's list_accounts is empty after admin created an account in their OWN db
non_admin_cannot_manage_users                  → bob POST /api/auth/users → 403 auth.admin_required
delete_user_removes_dir_and_sessions           → admin deletes bob → bob's next rpc 401; users/<bob-id> dir gone
```
- [ ] **Step 2: Commit** (with Task 6) — see Task 6.

---

### Task 6: Rewire the server to per-user runtimes (ServerState reshape + legacy migration)

**Files:**
- Modify: `crates/finsight-server/src/state.rs` (ServerState reshape; remove single-user bootstrap + keyfile), `dispatch.rs` (rpc handler auth), `events.rs` (per-user subscribe), `router.rs`, `main.rs`, `registry.rs` (complete `get_or_bootstrap`)

- [ ] **Step 1: ServerState reshape** (`state.rs`):
```rust
pub struct ServerState {
    pub users: crate::users::UsersDb,
    pub sessions: crate::sessions::SessionStore,
    pub registry: crate::registry::Registry,
    pub data_dir: PathBuf,
}
impl ServerState {
    /// Phase 2 bootstrap: open users.db only. Per-user DBs open lazily at login.
    pub fn bootstrap(data_dir: &Path) -> anyhow::Result<Arc<Self>> { /* create_dir_all, UsersDb::open, defaults */ }
}
```
`OutboundEvent` stays in state.rs (registry + events use it). DELETE `load_or_create_keyfile` and the old async bootstrap (the keyfile is only read once more, by the legacy migration below). `main.rs`: bootstrap is now sync; spawn the idle-eviction interval task here (`tokio::time::interval(300s)` → `registry.evict_idle(Duration::from_secs(1800))`, log evicted ids).
- [ ] **Step 2: Complete `Registry::get_or_bootstrap`** (from Task 3): per-user dir create → `Db::open(user_dir.join("data.sqlcipher"), db_key_hex)` → broadcast channel(256) → EventCallback (same 3 names) → `ApiState::new(db.clone(), user_dir, on_event)` → provider load/set → `finsight_api::startup::run_startup_cascade(&db, &user_dir.join("backups"))` (spawn_blocking — it does heavy DB work) → `agent.tx.send(CheckDueRecipes)` → `sync_scheduler.start(&tokio::runtime::Handle::current())` storing the JoinHandle in `UserRuntime.sync_task`.
- [ ] **Step 3: dispatch.rs** — `rpc` handler gains the extractor:
```rust
pub async fn rpc(
    State(st): State<Arc<ServerState>>,
    user: crate::auth::AuthedUser,
    Path(cmd): Path<String>,
    Json(p): Json<serde_json::Value>,
) -> Response {
    let rt = match st.registry.get_or_bootstrap(&st.data_dir, &user.user_id, &user.db_key_hex).await {
        Ok(rt) => rt, Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(AppError::new("auth.runtime", e.to_string()))).into_response(),
    };
    st.registry.touch(&user.user_id);
    // …existing UNSUPPORTED/dispatch flow, with `api = &rt.api` and BroadcastSink(rt.events.clone())
}
```
The `dispatch()` fn signature changes from `&Arc<ServerState>` to `(api: &Arc<ApiState>, events: &broadcast::Sender<OutboundEvent>)` — mechanical; the 173 arms don't change (parity tests must stay green untouched).
- [ ] **Step 4: events.rs** — `events` handler takes `AuthedUser`, resolves the runtime the same way, subscribes to `rt.events`. Keep `sse_data` unchanged.
- [ ] **Step 5: Legacy migration** (in auth.rs setup handler): if `<data>/data.sqlcipher` exists AND `<data>/db.key` exists → read keyfile hex, move `data.sqlcipher{,-wal,-shm}` into `users/<admin_id>/`, wrap THAT key (instead of `generate_db_key()`) as the admin's dbkey, delete `db.key`. Log one info line. (Phase 1 dev data survives into the admin account.)
- [ ] **Step 6: Fix the existing test helpers** — `router::tests::test_state()` and the 4 dispatch tests + SSE/static tests now need an authenticated flow: add a test helper in `tests/` common or `router::tests`: `async fn setup_and_login(app) -> CookieJar` doing POST setup + extracting the cookie; dispatch tests attach the cookie header. The parity tests need NO changes (they don't hit HTTP).
- [ ] **Step 7: Run the full Task 5 test list** (`cargo test -p finsight-server --test auth`) → all 8 PASS; `cargo test -p finsight-server` all green (incl. reworked dispatch/router/SSE tests + untouched parity).
- [ ] **Step 8: Gates** — `cargo tree -p finsight-server -i tauri` absent; `cargo test --workspace` (foreground, jobs=2 if OOM) 0 failures; bindings zero-diff.
- [ ] **Step 9: Commit** — `feat(server): auth endpoints + per-user runtimes (sessions, isolation, legacy migration)` (Tasks 5+6 land together; the plan keeps them separate for reading, the commit is one because they're not independently green).

---

### Task 7: UI — auth API client, Setup/Login screens, 401 handling

**Files:**
- Create: `ui/src/api/auth.ts`, `ui/src/screens/server/SetupScreen.tsx`, `ui/src/screens/server/LoginScreen.tsx`, tests for each
- Modify: `ui/src/api/httpBackend.ts` (401 → event + EventSource lifecycle), `ui/src/main.tsx` or the router root (server-mode route gating)

- [ ] **Step 1: `auth.ts`** — plain fetch client (NOT bindings):
```typescript
export type AuthStatus = { needsSetup: boolean; authenticated: boolean; username: string | null; isAdmin: boolean | null };
export async function fetchAuthStatus(): Promise<AuthStatus> { … }        // GET /api/auth/status
export async function setup(username: string, password: string): Promise<{ recoveryKey: string }> { … } // POST, throws AppError-shaped object on !ok
export async function login(username: string, password: string): Promise<void> { … }
export async function logout(): Promise<void> { … }
export function isServerMode(): boolean { return Boolean((window as AnyRec).__FINSIGHT_HTTP__); }
```
- [ ] **Step 2: shim changes** (`httpBackend.ts`): set `w.__FINSIGHT_HTTP__ = true` on install; on any RPC 401 with `code === "auth.required"` → `window.dispatchEvent(new CustomEvent("finsight:auth-required"))` (still throw the plain object). Close + null the shared EventSource on that event; `ensureEventSource` re-opens on next listen after re-login. Add/extend shim unit tests: 401 fires the CustomEvent; ES closed.
- [ ] **Step 3: screens** — minimal, using existing form patterns/`.card`/`.btn` classes: Setup (username, password ×2, then a "save your recovery key" screen showing the returned key with a Copy button + explicit "I saved it" confirm before continuing — the key is shown exactly once); Login (username, password, error line for `auth.bad_credentials`). Both navigate to `/` on success and invalidate the tanstack-query cache (`queryClient.clear()`).
- [ ] **Step 4: gating** — in the app root (server mode only): before rendering routes, fetch `fetchAuthStatus()`; `needsSetup` → SetupScreen; `!authenticated` → LoginScreen; listen for `finsight:auth-required` → route to login. Desktop/Tauri path completely unaffected (gate on `isServerMode()`).
- [ ] **Step 5: Settings** — add a "Sign out" button in a server-mode-only block (calls `logout()`, routes to login).
- [ ] **Step 6: Gates** — `cd ui && npx vitest run` (428 baseline + new tests, 0 failures) and `npx tsc --noEmit` clean.
- [ ] **Step 7: Commit** — `feat(ui): server-mode auth (setup wizard, login, 401 handling, sign out)`

---

### Task 8: Admin user management UI

**Files:**
- Create: `ui/src/screens/server/UsersAdmin.tsx` (+ test)
- Modify: `ui/src/api/auth.ts` (list/create/delete users calls), Settings (server-mode + admin-only section link)

- [ ] Minimal table: username, admin badge, created date, delete button (confirm dialog; self-delete disabled). "Add user" form (username + password) → shows the returned recovery key once (same reveal component as Setup — extract it to `ui/src/components/RecoveryKeyReveal.tsx`). Gate visibility on `isAdmin` from auth status. vitest: renders list, create flow shows recovery key, non-admin sees nothing.
- [ ] Gates: vitest + tsc clean. Commit — `feat(ui): admin user management (server mode)`

---

### Task 9: End-to-end verification (Phase 2 exit criterion)

- [ ] **Step 1:** Full green bar: workspace (foreground), `cargo test -p finsight-api` isolated, frontend vitest + tsc, bindings zero-diff, parity tests green UNTOUCHED (`git diff --exit-code crates/finsight-server/tests/parity.rs` vs the Phase 1 commit `6b0452f` — proves no weakening).
- [ ] **Step 2:** `cd ui && npm run build`; `cargo build -p finsight-server`; launch against a FRESH scratch dir.
- [ ] **Step 3:** Browser checklist (drive it like Phase 1's):
  1. First load → Setup wizard renders (`needsSetup:true`). Create admin → recovery key shown once → confirm → lands in onboarding/app.
  2. Create an account + transaction (data flows under the admin's DB).
  3. Sign out (Settings) → login screen; refresh → still login (cookie cleared). Log back in → data still there (unwrap + rekey round-trip through a real restart of the session).
  4. **Restart the server process** → reload → login again (sessions are memory-only) → data intact (wrapped-key persistence proven).
  5. Admin → add user "bob" (capture recovery key) → sign out → login as bob → EMPTY app (isolation); bob's Copilot/events work (per-user SSE — verify a live frame like Phase 1 item 8).
  6. Wrong password → readable error; `/api/rpc/list_accounts` via fetch without cookie → 401 `auth.required`.
  7. Legacy migration: place a Phase 1-style `data.sqlcipher` + `db.key` in a fresh data dir, run setup → admin sees the old data.
- [ ] **Step 4:** Record results in Linear; update CLAUDE.md server-mode section (setup flow, multi-user, cookie note); update the plan's checkboxes; commit.

---

## Explicitly out of scope (Phase 2)

- Password change / recovery-key login flows (recovery key WRAPS exist; the login-with-recovery + rewrap UI is Phase 2.5 — the data is recoverable via a small CLI if ever needed before then).
- "Keep unlocked on this server" background-sync toggle (spec: later phase).
- Rate limiting/2FA (spec assigns hardening to the deployment recipes; revisit in Phase 3 docs).
- PWA/Docker/deployment docs (Phase 3); thin shell (Phase 4).
- Session persistence across server restarts (memory-only is deliberate — the unwrapped key must not touch disk).
