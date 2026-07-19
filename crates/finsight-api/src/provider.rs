//! Provider-construction helpers: turn saved settings into a live
//! `CompletionProvider`. Tauri-free — used by both the Tauri app's setup and
//! `finsight-server`'s bootstrap.

use crate::commands::agent::CompletionProviderConfig;
use finsight_agent::{
    providers::{
        anthropic::AnthropicProvider, ollama::OllamaProvider, openai_compat::OpenAiCompatProvider,
    },
    CompletionProvider,
};
use finsight_core::{settings, Db};
use std::sync::Arc;

/// Migrate legacy `llm_provider` key → `completion_provider`.
/// Called from `configure_app` setup before managing AppState.
/// Exported for integration tests.
pub fn migrate_provider_settings(db: &Db) -> Result<(), finsight_core::CoreError> {
    let conn = db.get()?;
    // Only migrate if completion_provider is absent
    let new_cfg: Option<serde_json::Value> = settings::get(&conn, "completion_provider")?;
    if new_cfg.is_some() {
        return Ok(());
    }
    let old_cfg: Option<serde_json::Value> = settings::get(&conn, "llm_provider")?;
    let Some(old) = old_cfg else { return Ok(()) };
    let migrated = match old.get("kind").and_then(|k| k.as_str()) {
        Some("ollama") => serde_json::json!({
            "kind": "ollama",
            "base_url": old["base_url"],
            "model": old["completion_model"]
        }),
        _ => serde_json::json!({ "kind": "unconfigured" }),
    };
    settings::set(&conn, "completion_provider", &migrated)?;
    Ok(())
}

/// Load the raw CompletionProviderConfig from settings.
/// Returns Unconfigured when the setting is missing.
pub fn load_completion_provider_config(
    db: &Db,
) -> Result<CompletionProviderConfig, finsight_core::CoreError> {
    let conn = db.get()?;
    let cfg: Option<serde_json::Value> = settings::get(&conn, "completion_provider")?;
    match cfg {
        Some(v) => serde_json::from_value(v).map_err(|e| {
            finsight_core::CoreError::InvalidState(format!("completion_provider parse: {e}"))
        }),
        None => Ok(CompletionProviderConfig::Unconfigured),
    }
}

/// Load the saved CompletionProviderConfig from settings and instantiate the provider.
/// Returns None if unconfigured or key absent.
pub fn load_provider_from_settings(db: &Db) -> Option<Arc<dyn CompletionProvider>> {
    // Scoped so the pooled connection is released before `build_provider_from_config`
    // takes its own to read the API key — no need to hold two at once.
    let cfg: serde_json::Value = {
        let conn = db.get().ok()?;
        settings::get(&conn, "completion_provider").ok()??
    };
    build_provider_from_config(db, &cfg)
}

/// Look up a provider API key for `provider_id` in the user's own encrypted DB,
/// falling back once to the legacy OS-keychain address (see `crate::secrets`).
/// Trims defensively: a key stored with stray whitespace (e.g. a paste with a
/// trailing newline) must not corrupt the auth header.
fn provider_api_key(db: &Db, provider_id: &str) -> Option<String> {
    let conn = db.get().ok()?;
    let key = crate::secrets::get_secret_migrating(
        &conn,
        &crate::secrets::llm_key(provider_id),
        crate::secrets::LEGACY_LLM_SERVICE,
        provider_id,
    )
    .ok()??;
    let key = key.trim().to_string();
    if key.is_empty() {
        return None;
    }
    Some(key)
}

/// Takes `db` because API keys now live in the user's own encrypted database
/// (per-tenant, and available inside Docker) rather than the OS keychain.
pub fn build_provider_from_config(
    db: &Db,
    cfg: &serde_json::Value,
) -> Option<Arc<dyn CompletionProvider>> {
    match cfg.get("kind")?.as_str()? {
        "ollama" => {
            let base_url = cfg["base_url"].as_str()?.to_string();
            let model = cfg["model"].as_str()?.to_string();
            Some(Arc::new(OllamaProvider::new(base_url, model)))
        }
        "openai_compat" => {
            let base_url = cfg["base_url"].as_str()?.to_string();
            let model = cfg["model"].as_str()?.to_string();
            let preset = cfg["preset"].as_str().unwrap_or("custom").to_string();
            let api_key = provider_api_key(db, &preset)?;
            // Structured output on final-answer turns: probe-validated on
            // OpenRouter (tools + json_schema coexist there). Only the main
            // synthesizer gets it, not the fast router (tool-selection only).
            // The provider falls back to unconstrained on empty/error, so a
            // non-supporting endpoint is safe.
            let structured = base_url.contains("openrouter");
            Some(Arc::new(
                OpenAiCompatProvider::new(base_url, api_key, model, preset)
                    .with_structured_final_answer(structured),
            ))
        }
        "anthropic" => {
            let model = cfg["model"].as_str()?.to_string();
            let api_key = provider_api_key(db, "anthropic")?;
            Some(Arc::new(AnthropicProvider::new(api_key, model)))
        }
        _ => None,
    }
}

/// Optional fast "router" model for the Copilot tool loop. When the user sets a
/// `copilot.router_model` in settings AND the main provider is OpenAI-compatible
/// (OpenRouter etc.), build a cheap, small-budget router that drives the many
/// tool-selection turns while the configured (strong) model writes the final
/// answer. Returns None when unset or not applicable → single-model loop.
pub fn build_copilot_router_from_settings(db: &Db) -> Option<Arc<dyn CompletionProvider>> {
    let conn = db.get().ok()?;
    let router_model: String = settings::get(&conn, "copilot.router_model").ok()??;
    let router_model = router_model.trim().to_string();
    if router_model.is_empty() {
        return None;
    }
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").ok()??;
    // Only OpenAI-compatible endpoints can serve a cheap sibling model on the
    // same base URL + key.
    if cfg.get("kind")?.as_str()? != "openai_compat" {
        return None;
    }
    let base_url = cfg["base_url"].as_str()?.to_string();
    let preset = cfg["preset"].as_str().unwrap_or("custom").to_string();
    // Release the pooled connection before the key lookup takes its own.
    drop(conn);
    let api_key = provider_api_key(db, &preset)?;
    // Small completion budget: a routing turn only emits a short tool call.
    Some(Arc::new(
        OpenAiCompatProvider::new(base_url, api_key, router_model, preset).with_max_tokens(1024),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets;
    use finsight_core::{db::run_migrations, keychain};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("provider.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    /// A preset name that exists nowhere in the real OS keychain.
    ///
    /// This matters: on a DB miss the builder falls back to the legacy address
    /// `("com.finsight.llm", preset)` and, on a hit, MOVES the value out of the
    /// keychain. A test using a real preset name ("openrouter", "anthropic")
    /// would therefore delete the developer's own working key. Tests must never
    /// name a real provider id on a path that can miss in the DB.
    fn scratch_preset() -> String {
        format!("finsight-test-{}", uuid::Uuid::new_v4())
    }

    fn compat_cfg(preset: &str) -> serde_json::Value {
        serde_json::json!({
            "kind": "openai_compat",
            "preset": preset,
            "base_url": "https://openrouter.ai/api/v1",
            "model": "google/gemma-3-27b-it",
        })
    }

    fn save_key(db: &Db, provider_id: &str, key: &str) {
        let conn = db.get().unwrap();
        secrets::set_secret(&conn, &secrets::llm_key(provider_id), key).unwrap();
    }

    #[test]
    fn openai_compat_provider_builds_from_a_key_in_the_database() {
        let (_d, db) = fresh_db();
        let preset = scratch_preset();
        save_key(&db, &preset, "sk-or-abc123");
        assert!(build_provider_from_config(&db, &compat_cfg(&preset)).is_some());
    }

    #[test]
    fn provider_is_none_when_this_users_database_has_no_key() {
        // The multi-user guarantee: an unconfigured tenant gets no provider,
        // rather than silently inheriting whichever key another tenant saved
        // into the single process-global keychain slot.
        let (_d, db) = fresh_db();
        assert!(build_provider_from_config(&db, &compat_cfg(&scratch_preset())).is_none());
    }

    #[test]
    fn a_key_saved_in_one_database_is_invisible_to_another() {
        let (_d1, alice) = fresh_db();
        let (_d2, bob) = fresh_db();
        let preset = scratch_preset();
        save_key(&alice, &preset, "alice-key");
        assert!(build_provider_from_config(&alice, &compat_cfg(&preset)).is_some());
        assert!(build_provider_from_config(&bob, &compat_cfg(&preset)).is_none());
    }

    #[test]
    fn a_whitespace_only_key_does_not_produce_a_provider() {
        let (_d, db) = fresh_db();
        // Safe to name the real "anthropic" id here: the DB hit short-circuits
        // the lookup, so the OS keychain is never consulted or mutated.
        save_key(&db, "anthropic", "   \n");
        let cfg = serde_json::json!({ "kind": "anthropic", "model": "claude-sonnet-4-5" });
        assert!(build_provider_from_config(&db, &cfg).is_none());
    }

    #[test]
    fn load_provider_from_settings_reads_config_and_key_from_the_same_database() {
        let (_d, db) = fresh_db();
        let preset = scratch_preset();
        {
            let conn = db.get().unwrap();
            settings::set(&conn, "completion_provider", &compat_cfg(&preset)).unwrap();
        }
        assert!(
            load_provider_from_settings(&db).is_none(),
            "config alone is not enough — the key must be present too"
        );
        save_key(&db, &preset, "sk-or-abc123");
        assert!(load_provider_from_settings(&db).is_some());
    }

    #[test]
    fn ollama_needs_no_key_at_all() {
        let (_d, db) = fresh_db();
        let cfg = serde_json::json!({
            "kind": "ollama",
            "base_url": "http://localhost:11434",
            "model": "llama3",
        });
        assert!(build_provider_from_config(&db, &cfg).is_some());
    }
}
