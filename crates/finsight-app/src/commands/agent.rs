use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_agent::{
    agent::AgentJob,
    providers::{
        anthropic::AnthropicProvider, ollama::OllamaProvider,
        openai_compat::OpenAiCompatProvider,
    },
    CompletionProvider,
};
use finsight_core::repos::run;
use finsight_core::settings;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind")]
pub enum CompletionProviderConfig {
    #[serde(rename = "unconfigured")]
    Unconfigured,
    #[serde(rename = "ollama")]
    Ollama { base_url: String, model: String },
    #[serde(rename = "openai_compat")]
    OpenAiCompat { preset: String, base_url: String, model: String },
    #[serde(rename = "anthropic")]
    Anthropic { model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderTestResult {
    pub ok: bool,
    pub error: Option<String>,
    pub latency_ms: u64,
}

#[tauri::command]
#[specta::specta]
pub async fn set_completion_provider(
    state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
) -> AppResult<()> {
    let db = (*state.db).clone();
    let cfg_json = serde_json::to_value(&config)
        .map_err(|e| AppError::new("agent", e.to_string()))?;
    run(&db, move |conn| settings::set(conn, "completion_provider", &cfg_json))
        .await
        .map_err(AppError::from)?;

    // Also update the live provider in AppState
    let provider = crate::build_provider_from_config(&serde_json::to_value(&config).unwrap());
    if let Some(p) = provider {
        state.agent.set_provider(p);
    } else {
        *state.agent_provider.write().unwrap() = None;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn save_provider_api_key(
    _state: tauri::State<'_, AppState>,
    provider_id: String,
    key: String,
) -> AppResult<()> {
    finsight_core::keychain::set_key("com.finsight.llm", &provider_id, &key)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_provider_models(
    _state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
) -> AppResult<Vec<String>> {
    match &config {
        CompletionProviderConfig::Ollama { base_url, model } => {
            let provider = OllamaProvider::new(base_url.clone(), model.clone());
            provider
                .list_models()
                .await
                .map_err(|e| AppError::new("agent", e.to_string()))
        }
        _ => Ok(vec![]),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn test_completion_provider(
    _state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
    api_key: Option<String>,
) -> AppResult<ProviderTestResult> {
    let provider: Arc<dyn CompletionProvider> = match &config {
        CompletionProviderConfig::Ollama { base_url, model } => {
            Arc::new(OllamaProvider::new(base_url.clone(), model.clone()))
        }
        CompletionProviderConfig::OpenAiCompat {
            preset,
            base_url,
            model,
        } => {
            let key = api_key
                .or_else(|| {
                    finsight_core::keychain::get_key("com.finsight.llm", preset)
                        .ok()
                        .flatten()
                })
                .unwrap_or_default();
            Arc::new(OpenAiCompatProvider::new(
                base_url.clone(),
                key,
                model.clone(),
                preset.clone(),
            ))
        }
        CompletionProviderConfig::Anthropic { model } => {
            let key = api_key
                .or_else(|| {
                    finsight_core::keychain::get_key("com.finsight.llm", "anthropic")
                        .ok()
                        .flatten()
                })
                .unwrap_or_default();
            Arc::new(AnthropicProvider::new(key, model.clone()))
        }
        CompletionProviderConfig::Unconfigured => {
            return Ok(ProviderTestResult {
                ok: false,
                error: Some("Not configured".into()),
                latency_ms: 0,
            })
        }
    };
    let start = std::time::Instant::now();
    let result = provider
        .complete_json(
            "You are a test assistant. Respond with valid JSON only.",
            r#"Reply with exactly: {"ok": true}"#,
        )
        .await;
    let latency_ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(_) => Ok(ProviderTestResult {
            ok: true,
            error: None,
            latency_ms,
        }),
        Err(e) => Ok(ProviderTestResult {
            ok: false,
            error: Some(e.to_string()),
            latency_ms,
        }),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_needs_review_count(state: tauri::State<'_, AppState>) -> AppResult<u32> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions \
             WHERE ai_confidence < 0.6 \
               AND (SELECT source FROM categorizations c \
                    WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm'",
            [],
            |r| r.get(0),
        )?;
        Ok(count as u32)
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn trigger_categorize(state: tauri::State<'_, AppState>) -> AppResult<()> {
    state
        .agent
        .tx
        .try_send(AgentJob::CategorizeAll)
        .map_err(|e| AppError::new("agent", format!("queue full: {e}")))?;
    Ok(())
}
