use finsight_app::commands::agent::CompletionProviderConfig;
use finsight_core::{db::run_migrations, keychain, settings, Db};
use tempfile::TempDir;

/// Verifies that the llm_provider → completion_provider migration runs
/// when completion_provider is absent but llm_provider is present.
#[test]
fn migrate_llm_provider_to_completion_provider() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("mp.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    // Simulate a Phase 2 DB state: llm_provider set, completion_provider absent.
    {
        let conn = db.get().unwrap();
        settings::set(
            &conn,
            "llm_provider",
            &serde_json::json!({
                "kind": "ollama",
                "base_url": "http://localhost:11434",
                "completion_model": "llama3.2",
                "embedding_model": "nomic-embed-text"
            }),
        )
        .unwrap();
    }

    finsight_app::migrate_provider_settings(&db).unwrap();

    let conn = db.get().unwrap();
    let new_cfg: Option<serde_json::Value> = settings::get(&conn, "completion_provider").unwrap();
    assert!(new_cfg.is_some(), "completion_provider should be written");
    let cfg = new_cfg.unwrap();
    assert_eq!(cfg["kind"], "ollama");
    assert_eq!(cfg["base_url"], "http://localhost:11434");
    assert_eq!(cfg["model"], "llama3.2");
}

/// Verifies that load_completion_provider_config returns the saved config.
#[test]
fn load_completion_provider_config_round_trip() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("rt.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    let saved = CompletionProviderConfig::OpenAiCompat {
        preset: "openrouter".to_string(),
        base_url: "https://openrouter.ai/api/v1".to_string(),
        model: "gpt-4o-mini".to_string(),
    };
    {
        let conn = db.get().unwrap();
        settings::set(&conn, "completion_provider", &saved).unwrap();
    }

    let loaded = finsight_app::load_completion_provider_config(&db).unwrap();
    match loaded {
        CompletionProviderConfig::OpenAiCompat {
            preset,
            base_url,
            model,
        } => {
            assert_eq!(preset, "openrouter");
            assert_eq!(base_url, "https://openrouter.ai/api/v1");
            assert_eq!(model, "gpt-4o-mini");
        }
        other => panic!("expected OpenAiCompat, got {:?}", other),
    }
}

/// Seeding an OpenRouter provider from a key writes the expected config,
/// stores the key ONLY in the keychain (never in the settings row), and
/// returns true.
#[test]
fn seed_openrouter_provider_writes_config_without_leaking_key() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("seed.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    let secret = "sk-or-test-DEADBEEF-should-not-appear-in-settings";
    let seeded = finsight_app::seed_openrouter_provider_if_unconfigured(&db, secret).unwrap();
    assert!(seeded, "should seed when unconfigured");

    let conn = db.get().unwrap();
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").unwrap().unwrap();
    assert_eq!(cfg["kind"], "openai_compat");
    assert_eq!(cfg["preset"], "openrouter");
    assert_eq!(cfg["base_url"], "https://openrouter.ai/api/v1");
    assert_eq!(cfg["model"], "google/gemma-4-31b-it");

    // Secret hygiene: the raw key must never be serialized into the settings row.
    let settings_blob = serde_json::to_string(&cfg).unwrap();
    assert!(
        !settings_blob.contains(secret),
        "API key must not appear in the completion_provider settings row"
    );
}

/// Seeding must NOT overwrite a user-configured provider (override contract).
#[test]
fn seed_openrouter_provider_preserves_user_override() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("override.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    {
        let conn = db.get().unwrap();
        settings::set(
            &conn,
            "completion_provider",
            &serde_json::json!({
                "kind": "ollama",
                "base_url": "http://localhost:11434",
                "model": "llama3.2"
            }),
        )
        .unwrap();
    }

    let seeded = finsight_app::seed_openrouter_provider_if_unconfigured(&db, "sk-or-ignored").unwrap();
    assert!(!seeded, "must not seed over an existing provider");

    let conn = db.get().unwrap();
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").unwrap().unwrap();
    assert_eq!(cfg["kind"], "ollama", "user override must be preserved");
}

/// An explicit `unconfigured` provider row is treated as seedable.
#[test]
fn seed_openrouter_provider_seeds_over_unconfigured() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("unconf.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    {
        let conn = db.get().unwrap();
        settings::set(
            &conn,
            "completion_provider",
            &serde_json::json!({ "kind": "unconfigured" }),
        )
        .unwrap();
    }

    let seeded = finsight_app::seed_openrouter_provider_if_unconfigured(&db, "sk-or-test").unwrap();
    assert!(seeded, "unconfigured row should be replaced");
    let conn = db.get().unwrap();
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").unwrap().unwrap();
    assert_eq!(cfg["kind"], "openai_compat");
}

/// An empty key is a no-op (missing/invalid key handling).
#[test]
fn seed_openrouter_provider_ignores_empty_key() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("empty.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    let seeded = finsight_app::seed_openrouter_provider_if_unconfigured(&db, "   ").unwrap();
    assert!(!seeded, "empty key must not seed a provider");
    let loaded = finsight_app::load_completion_provider_config(&db).unwrap();
    assert!(matches!(loaded, CompletionProviderConfig::Unconfigured));
}

/// Verifies that load_completion_provider_config returns Unconfigured when absent.
#[test]
fn load_completion_provider_config_unconfigured_when_missing() {
    let dir = TempDir::new().unwrap();
    let key = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("missing.sqlcipher"), &key).unwrap();
    run_migrations(&db).unwrap();

    let loaded = finsight_app::load_completion_provider_config(&db).unwrap();
    assert!(
        matches!(loaded, CompletionProviderConfig::Unconfigured),
        "expected Unconfigured when setting is absent"
    );
}
