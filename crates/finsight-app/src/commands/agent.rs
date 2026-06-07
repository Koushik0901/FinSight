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
use finsight_core::models::{NewRule, RuleProposal};
use finsight_core::repos::{rule_proposals, rules, run};
use finsight_core::settings;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivity {
    pub text: String,
    pub sub: String,
    pub minutes_ago: i64,
}

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

#[tauri::command]
#[specta::specta]
pub async fn list_rule_proposals(state: tauri::State<'_, AppState>) -> AppResult<Vec<RuleProposal>> {
    let db = (*state.db).clone();
    run(&db, |conn| rule_proposals::list(conn, Some("pending")))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn accept_rule_proposal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        if let Some(p) = rule_proposals::get(conn, &id)? {
            rules::insert(conn, NewRule {
                pattern: p.pattern,
                category_id: p.category_id,
                source: "agent".to_string(),
            })?;
            rule_proposals::set_status(conn, &id, "accepted")?;
        }
        Ok(())
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn decline_rule_proposal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| rule_proposals::set_status(conn, &id, "declined"))
        .await
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_recent_agent_activity(
    state: tauri::State<'_, AppState>,
    limit: u32,
) -> AppResult<Vec<AgentActivity>> {
    let db = (*state.db).clone();
    let limit = limit as i64;
    run(&db, move |conn| {
        let mut stmt = conn.prepare(
            "SELECT t.merchant_raw,
                    COALESCE(c.label, 'Uncategorized'),
                    cat.source,
                    CAST(ROUND(cat.confidence * 100) AS INTEGER),
                    CAST((julianday('now') - julianday(cat.at)) * 1440 AS INTEGER)
             FROM categorizations cat
             JOIN transactions t ON t.id = cat.txn_id
             LEFT JOIN categories c ON c.id = cat.category_id
             WHERE cat.at >= datetime('now', '-24 hours')
             ORDER BY cat.at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (merchant, category, source, pct, mins) = row?;
            out.push(AgentActivity {
                text: format!("'{}' → {}", merchant, category),
                sub: format!("{} · {}% conf", source, pct),
                minutes_ago: mins,
            });
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
