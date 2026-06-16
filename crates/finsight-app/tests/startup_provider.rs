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
