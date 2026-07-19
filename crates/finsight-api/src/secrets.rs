//! Server-safe secret storage. Secrets live in the USER'S OWN encrypted
//! SQLCipher DB (settings KV), not the OS keychain: the keychain is
//! process-global (so every user on a server shared one slot) and is absent
//! entirely in the Docker image (no Secret Service), which made Copilot and
//! SimpleFIN unconfigurable there.
//!
//! Keys are namespaced under a `secret.` prefix inside the existing `settings`
//! table, e.g. `secret.llm.openrouter`, `secret.simplefin.<bridge_id>`. Because
//! the whole DB file is SQLCipher-encrypted and one DB exists per user, this is
//! both per-tenant and encrypted at rest.
//!
//! A one-time read-through migration (`get_secret_migrating`) moves values that
//! still live at the legacy OS-keychain address into the DB, so existing
//! desktop installs keep working with no user action.
//!
//! NOTE: this module deliberately does NOT cover the SQLCipher database key
//! itself — that still comes from `finsight_core::keychain` (`com.finsight.app`)
//! on desktop, since it is the key that would decrypt this very store.

use finsight_core::{settings, CoreResult};
use rusqlite::Connection;

/// Legacy OS-keychain service that held LLM provider API keys.
pub const LEGACY_LLM_SERVICE: &str = "com.finsight.llm";
/// Legacy OS-keychain service that held SimpleFIN access URLs.
pub const LEGACY_SIMPLEFIN_ACCESS_SERVICE: &str = "com.finsight.simplefin.access";

/// Settings key for an LLM provider API key. `provider_id` is the same value the
/// UI sends to `save_provider_api_key` (an OpenAI-compat preset name, or the
/// literal `"anthropic"`), which is also what the provider builder looks up.
pub fn llm_key(provider_id: &str) -> String {
    format!("secret.llm.{provider_id}")
}

/// Settings key for a SimpleFIN access URL, keyed by bridge/access-url ref id.
pub fn simplefin_key(id: &str) -> String {
    format!("secret.simplefin.{id}")
}

/// Read a secret from the user's DB. Returns `None` when unset.
pub fn get_secret(conn: &Connection, key: &str) -> CoreResult<Option<String>> {
    settings::get::<String>(conn, key)
}

/// Store a secret in the user's DB, replacing any existing value.
pub fn set_secret(conn: &Connection, key: &str, value: &str) -> CoreResult<()> {
    settings::set(conn, key, &value.to_string())
}

/// Remove a secret. No-op when absent.
pub fn delete_secret(conn: &Connection, key: &str) -> CoreResult<()> {
    settings::delete(conn, key)
}

/// Read a secret, transparently migrating it out of the legacy OS keychain.
///
/// Lookup order:
/// 1. the user's DB (`db_key`) — the steady state;
/// 2. the legacy keychain address `(legacy_service, legacy_user)`. When found
///    there, the value is written into the DB and the keychain entry deleted,
///    so the fallback runs at most once per secret.
///
/// Every keychain interaction is best-effort: on a machine with no Secret
/// Service (the Docker image) `keyring` returns `Err`, which is treated as
/// "absent" rather than propagated. A failed delete is logged and ignored — the
/// value is already safely in the DB, so the read still succeeds.
pub fn get_secret_migrating(
    conn: &Connection,
    db_key: &str,
    legacy_service: &str,
    legacy_user: &str,
) -> CoreResult<Option<String>> {
    if let Some(v) = get_secret(conn, db_key)? {
        return Ok(Some(v));
    }

    // `Err` here means "no keychain on this platform" far more often than it
    // means a real failure, so it collapses to None.
    let legacy = finsight_core::keychain::get_key(legacy_service, legacy_user)
        .ok()
        .flatten();
    let Some(value) = legacy else {
        return Ok(None);
    };

    set_secret(conn, db_key, &value)?;
    if let Err(e) = finsight_core::keychain::delete_key(legacy_service, legacy_user) {
        tracing::warn!(
            "migrated secret {db_key} into the database but could not remove the \
             legacy keychain entry ({legacy_service}/{legacy_user}): {e}"
        );
    } else {
        tracing::info!("migrated secret {db_key} from the OS keychain into the database");
    }
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("secrets.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn key_builders_are_namespaced() {
        assert_eq!(llm_key("openrouter"), "secret.llm.openrouter");
        assert_eq!(llm_key("anthropic"), "secret.llm.anthropic");
        assert_eq!(simplefin_key("bridge-1"), "secret.simplefin.bridge-1");
    }

    #[test]
    fn get_returns_none_when_absent() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        assert_eq!(get_secret(&conn, &llm_key("openrouter")).unwrap(), None);
    }

    #[test]
    fn set_get_round_trip() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        let k = llm_key("openrouter");
        set_secret(&conn, &k, "sk-or-abc123").unwrap();
        assert_eq!(get_secret(&conn, &k).unwrap().as_deref(), Some("sk-or-abc123"));
    }

    #[test]
    fn set_overwrites_existing() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        let k = llm_key("anthropic");
        set_secret(&conn, &k, "old").unwrap();
        set_secret(&conn, &k, "new").unwrap();
        assert_eq!(get_secret(&conn, &k).unwrap().as_deref(), Some("new"));
    }

    #[test]
    fn delete_removes_secret() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        let k = simplefin_key("bridge-1");
        set_secret(&conn, &k, "https://user:pass@bridge.example/simplefin").unwrap();
        delete_secret(&conn, &k).unwrap();
        assert_eq!(get_secret(&conn, &k).unwrap(), None);
    }

    #[test]
    fn secrets_are_isolated_per_database() {
        // The cross-tenant leak this module exists to fix: two users' stores
        // must not see each other's key even under the same provider id.
        let (_d1, alice) = fresh_db();
        let (_d2, bob) = fresh_db();
        let k = llm_key("openrouter");
        set_secret(&alice.get().unwrap(), &k, "alice-key").unwrap();
        set_secret(&bob.get().unwrap(), &k, "bob-key").unwrap();
        assert_eq!(
            get_secret(&alice.get().unwrap(), &k).unwrap().as_deref(),
            Some("alice-key")
        );
        assert_eq!(
            get_secret(&bob.get().unwrap(), &k).unwrap().as_deref(),
            Some("bob-key")
        );
    }

    #[test]
    fn migrating_read_prefers_the_database_and_never_touches_the_keychain() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        let k = llm_key("openrouter");
        set_secret(&conn, &k, "db-value").unwrap();
        // A bogus legacy address: if the DB hit short-circuits (as it must),
        // the keychain is never consulted and this still returns the DB value.
        let got = get_secret_migrating(&conn, &k, "com.finsight.test.nonexistent", "nobody").unwrap();
        assert_eq!(got.as_deref(), Some("db-value"));
    }

    #[test]
    fn migrating_read_returns_none_when_neither_store_has_it() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        let got = get_secret_migrating(
            &conn,
            &llm_key("openrouter"),
            "com.finsight.test.nonexistent",
            &format!("absent-{}", uuid::Uuid::new_v4()),
        )
        .unwrap();
        assert_eq!(got, None);
    }

    // Exercises the real OS keychain, so it shares the platform caveat of
    // `finsight_core::keychain`'s own tests: headless Linux CI has no
    // initialised Secret Service collection.
    #[test]
    #[cfg_attr(target_os = "linux", ignore)]
    fn migrating_read_moves_a_legacy_keychain_value_into_the_database() {
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        let svc = "com.finsight.test.secrets";
        let usr = format!("legacy-{}", uuid::Uuid::new_v4());
        let k = llm_key("openrouter");
        keychain::set_key(svc, &usr, "legacy-value").unwrap();

        let got = get_secret_migrating(&conn, &k, svc, &usr).unwrap();
        assert_eq!(got.as_deref(), Some("legacy-value"));
        // Now in the DB...
        assert_eq!(get_secret(&conn, &k).unwrap().as_deref(), Some("legacy-value"));
        // ...and gone from the keychain, so the fallback runs only once.
        assert_eq!(keychain::get_key(svc, &usr).unwrap(), None);

        // A second read is served purely from the DB.
        assert_eq!(
            get_secret_migrating(&conn, &k, svc, &usr).unwrap().as_deref(),
            Some("legacy-value")
        );
        let _ = keychain::delete_key(svc, &usr);
    }
}
