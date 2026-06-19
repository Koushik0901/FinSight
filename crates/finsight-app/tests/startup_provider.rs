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
        CompletionProviderConfig::OpenAiCompat { preset, base_url, model } => {
            assert_eq!(preset, "openrouter");
            assert_eq!(base_url, "https://openrouter.ai/api/v1");
            assert_eq!(model, "gpt-4o-mini");
        }
        other => panic!("expected OpenAiCompat, got {:?}", other),
    }
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
