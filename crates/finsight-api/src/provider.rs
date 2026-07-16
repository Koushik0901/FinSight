//! Provider-construction helpers: turn saved settings into a live
//! `CompletionProvider`. Tauri-free — used by both the Tauri app's setup and
//! `finsight-server`'s bootstrap.
//!
//! NOTE: `load_completion_provider_config` deliberately stays in
//! `finsight-app` (not moved here) because its return type
//! `commands::agent::CompletionProviderConfig` hasn't moved out of
//! finsight-app yet (that's Task 5 of the server-phase1-skeleton plan).
//! finsight-api must stay tauri-free and cannot depend on finsight-app, so
//! moving this one helper now would be circular. It moves alongside the
//! `agent` command module in Task 5.

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

/// Load the saved CompletionProviderConfig from settings and instantiate the provider.
/// Returns None if unconfigured or key absent.
pub fn load_provider_from_settings(db: &Db) -> Option<Arc<dyn CompletionProvider>> {
    let conn = db.get().ok()?;
    let cfg: serde_json::Value = settings::get(&conn, "completion_provider").ok()??;
    build_provider_from_config(&cfg)
}

pub fn build_provider_from_config(cfg: &serde_json::Value) -> Option<Arc<dyn CompletionProvider>> {
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
            // Trim defensively: a key stored with stray whitespace (e.g. a
            // paste with a trailing newline) must not corrupt the auth header.
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", &preset)
                .ok()??
                .trim()
                .to_string();
            if api_key.is_empty() {
                return None;
            }
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
            let api_key = finsight_core::keychain::get_key("com.finsight.llm", "anthropic")
                .ok()??
                .trim()
                .to_string();
            if api_key.is_empty() {
                return None;
            }
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
    let api_key = finsight_core::keychain::get_key("com.finsight.llm", &preset)
        .ok()??
        .trim()
        .to_string();
    if api_key.is_empty() {
        return None;
    }
    // Small completion budget: a routing turn only emits a short tool call.
    Some(Arc::new(
        OpenAiCompatProvider::new(base_url, api_key, router_model, preset).with_max_tokens(1024),
    ))
}
