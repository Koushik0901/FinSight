use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_agent::{
    agent::AgentJob,
    providers::{
        anthropic::AnthropicProvider, ollama::OllamaProvider, openai_compat::OpenAiCompatProvider,
    },
    reasoning::{
        engine::ReasoningEngine,
        tools::{act, read, ToolSet},
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
    OpenAiCompat {
        preset: String,
        base_url: String,
        model: String,
    },
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
    let cfg_json =
        serde_json::to_value(&config).map_err(|e| AppError::new("agent", e.to_string()))?;
    run(&db, move |conn| {
        settings::set(conn, "completion_provider", &cfg_json)
    })
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
pub async fn get_completion_provider(
    state: tauri::State<'_, AppState>,
) -> AppResult<CompletionProviderConfig> {
    let db = (*state.db).clone();
    crate::load_completion_provider_config(&db)
        .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn save_provider_api_key(
    _state: tauri::State<'_, AppState>,
    provider_id: String,
    key: String,
) -> AppResult<()> {
    finsight_core::keychain::set_key("com.finsight.llm", &provider_id, &key).map_err(AppError::from)
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
pub async fn list_rule_proposals(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<RuleProposal>> {
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
            rules::insert(
                conn,
                NewRule {
                    pattern: p.pattern,
                    category_id: p.category_id,
                    source: "agent".to_string(),
                },
            )?;
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
    run(&db, move |conn| {
        rule_proposals::set_status(conn, &id, "declined")
    })
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

// ── Agent Status ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    pub uncategorized_count: u32,
    pub anomaly_count: u32,
    pub over_budget_count: u32,
    pub upcoming_bills_count: u32,
    pub last_scan_at: Option<String>,
    pub last_scan_categorized: Option<u32>,
}

#[tauri::command]
#[specta::specta]
pub async fn get_agent_status(state: tauri::State<'_, AppState>) -> AppResult<AgentStatus> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let this_month = chrono::Utc::now().format("%Y-%m").to_string();
        let this_month_start = chrono::Utc::now().format("%Y-%m-01").to_string();

        let uncategorized_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL",
            [],
            |r| r.get(0),
        )?;

        let anomaly_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE is_anomaly = 1",
            [],
            |r| r.get(0),
        )?;

        // Count envelopes where spending exceeds budget this month
        let over_budget_count: i64 = conn.query_row(
            "WITH spending AS (
               SELECT category_id, SUM(ABS(amount_cents)) AS cents
               FROM transactions
               WHERE amount_cents < 0
                 AND category_id IS NOT NULL
                 AND posted_at >= ?1
                 AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = transactions.id)
               GROUP BY category_id
               UNION ALL
               SELECT ts.category_id, SUM(ts.amount_cents) AS cents
               FROM transaction_splits ts
               JOIN transactions t ON t.id = ts.txn_id
               WHERE t.amount_cents < 0 AND t.posted_at >= ?1 AND ts.category_id IS NOT NULL
               GROUP BY ts.category_id
             )
             SELECT COUNT(*)
             FROM budgets b
             JOIN (SELECT category_id, SUM(cents) AS total FROM spending GROUP BY category_id) s
               ON s.category_id = b.category_id
             WHERE b.month = ?2 AND b.amount_cents > 0 AND s.total > b.amount_cents",
            rusqlite::params![this_month_start, this_month],
            |r| r.get(0),
        )?;

        // Count recurring items with expected date within 7 days
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let in_7 = (chrono::Utc::now() + chrono::Duration::days(7))
            .format("%Y-%m-%d")
            .to_string();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(395))
            .format("%Y-%m-%d")
            .to_string();

        let mut stmt = conn.prepare(
            "WITH gaps AS (
               SELECT merchant_raw,
                      date(posted_at) AS d,
                      LAG(date(posted_at)) OVER (PARTITION BY merchant_raw ORDER BY posted_at) AS prev_d
               FROM transactions WHERE posted_at >= ?1
             ),
             agg AS (
               SELECT merchant_raw,
                      AVG(julianday(d) - julianday(prev_d)) AS avg_gap,
                      MAX(d) AS last_seen,
                      COUNT(*) AS occ
               FROM gaps WHERE prev_d IS NOT NULL
               GROUP BY merchant_raw
               HAVING occ >= 2 AND AVG(julianday(d)-julianday(prev_d)) BETWEEN 5 AND 400
             )
             SELECT merchant_raw, avg_gap, last_seen FROM agg",
        )?;
        let upcoming_bills_count: i64 = stmt
            .query_map(rusqlite::params![cutoff], |r| {
                Ok((r.get::<_, f64>(1)?, r.get::<_, String>(2)?))
            })?
            .filter_map(|r| r.ok())
            .filter(|(avg_gap, last_seen)| {
                use chrono::NaiveDate;
                let Ok(last) = NaiveDate::parse_from_str(last_seen, "%Y-%m-%d") else {
                    return false;
                };
                let next = last + chrono::Duration::days(avg_gap.round() as i64);
                let next_str = next.format("%Y-%m-%d").to_string();
                next_str >= today && next_str <= in_7
            })
            .count() as i64;

        let last_scan_at: Option<String> = settings::get(conn, "agent.last_scan_at")?;
        let last_scan_categorized: Option<i64> =
            settings::get(conn, "agent.last_scan_categorized")?;

        Ok(AgentStatus {
            uncategorized_count: uncategorized_count as u32,
            anomaly_count: anomaly_count as u32,
            over_budget_count: over_budget_count as u32,
            upcoming_bills_count: upcoming_bills_count as u32,
            last_scan_at,
            last_scan_categorized: last_scan_categorized.map(|n| n as u32),
        })
    })
    .await
    .map_err(AppError::from)
}

// ── Ask the agent ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentChange {
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAnswer {
    pub prose: String,
    pub reasoning: String,
    pub trace: Vec<String>,
    pub changes: Vec<AgentChange>,
    pub action_label: Option<String>,
    pub action_path: Option<String>,
}

fn build_toolset() -> ToolSet {
    let mut tools = ToolSet::new();
    tools.register(read::get_account_balances());
    tools.register(read::get_month_totals());
    tools.register(read::get_top_spending_categories());
    tools.register(read::get_budgets());
    tools.register(read::get_goals());
    tools.register(read::get_recurring_bills());
    tools.register(read::get_liabilities());
    tools.register(read::search_transactions());
    tools.register(read::run_cashflow_projection());
    tools.register(act::update_goal_monthly());
    tools.register(act::create_planned_transaction());
    tools
}

async fn router_classify(provider: &Arc<dyn CompletionProvider>, question: &str) -> String {
    let system = "Classify this question as 'simple' (greetings, general info, single-fact lookups) or 'deep' (financial planning, pay allocation, investment decisions, debt payoff, should-I questions). Respond with JSON only: {\"mode\": \"simple\" | \"deep\"}";
    match provider.complete_json(system, question).await {
        Ok(v) => {
            if let Some(mode) = v.get("mode").and_then(|m| m.as_str()) {
                if mode == "deep" {
                    return "deep".to_string();
                }
            }
            "simple".to_string()
        }
        Err(_) => "simple".to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn ask_agent(
    state: tauri::State<'_, AppState>,
    question: String,
    mode: Option<String>,
) -> AppResult<AgentAnswer> {
    let provider = state.agent_provider.read().unwrap().clone();
    let Some(provider) = provider else {
        return Err(AppError::new(
            "no_provider",
            "Configure an AI provider in Settings → Agent to use this feature.",
        ));
    };

    let effective_mode = match mode.as_deref() {
        Some("deep") => "deep".to_string(),
        Some("quick") => "simple".to_string(),
        _ => router_classify(&provider, &question).await,
    };

    let db = (*state.db).clone();

    if effective_mode == "deep" {
        // Deep reasoning path: use the reasoning engine with tools
        let tools = build_toolset();
        let provider_clone = Arc::clone(&provider);
        let question_clone = question.clone();
        let result = run(&db, move |conn| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| {
                    finsight_core::CoreError::InvalidState(format!(
                        "Failed to create runtime: {e}"
                    ))
                })?;
            rt.block_on(ReasoningEngine::run(
                conn,
                &question_clone,
                &tools,
                provider_clone,
                10,
            ))
            .map_err(|e| {
                finsight_core::CoreError::InvalidState(format!(
                    "Reasoning engine error: {e}"
                ))
            })
        })
        .await
        .map_err(AppError::from)?;

        // Persist executed bundle if there were changes
        if !result.changes.is_empty() {
            let changes_for_bundle: Vec<AgentChange> = result
                .changes
                .iter()
                .map(|c| AgentChange {
                    kind: c.kind.clone(),
                    description: c.description.clone(),
                })
                .collect();
            let question_for_db = question.clone();
            let content_for_db = result.content.clone();
            let reasoning_for_db = result.reasoning.clone();
            let provider_id = provider.provider_id().to_string();
            let model_id = provider.model_id().to_string();
            let _ = run(&db, move |conn| {
                let bundle = finsight_core::repos::copilot_actions::insert_bundle(
                    conn,
                    None,
                    &question_for_db,
                    &content_for_db,
                    &reasoning_for_db,
                    1.0,
                    Some(&provider_id),
                    Some(&model_id),
                )?;
                for (i, change) in changes_for_bundle.iter().enumerate() {
                    finsight_core::repos::copilot_actions::insert_item(
                        conn,
                        &bundle.id,
                        &change.kind,
                        "{}",
                        &change.description,
                        1.0,
                        i as i64,
                    )?;
                }
                finsight_core::repos::copilot_actions::set_bundle_status(
                    conn,
                    &bundle.id,
                    "executed",
                )?;
                Ok::<_, finsight_core::CoreError>(())
            })
            .await;
        }

        Ok(AgentAnswer {
            prose: result.content,
            reasoning: result.reasoning,
            trace: result.trace,
            changes: result
                .changes
                .into_iter()
                .map(|c| AgentChange {
                    kind: c.kind,
                    description: c.description,
                })
                .collect(),
            action_label: None,
            action_path: None,
        })
    } else {
        // Simple path: existing single-shot logic with new AgentAnswer shape
        let context = run(&db, |conn| {
            let this_month = chrono::Utc::now().format("%Y-%m").to_string();
            let this_month_start = chrono::Utc::now().format("%Y-%m-01").to_string();

            // Net worth (all accounts)
            let net_worth: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(balance_cents), 0) FROM accounts",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            // Month totals
            let (income, expenses): (i64, i64) = conn
                .query_row(
                    "SELECT COALESCE(SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),0),
                            COALESCE(SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END),0)
                     FROM transactions WHERE posted_at >= ?1",
                    rusqlite::params![this_month_start],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or((0, 0));
            let savings_rate = if income > 0 {
                ((income - expenses) * 100 / income).max(0)
            } else {
                0
            };

            // Top 5 spending categories this month
            let mut cats_stmt = conn.prepare(
                "SELECT c.label, COALESCE(SUM(ABS(t.amount_cents)),0) AS spent
                 FROM transactions t JOIN categories c ON c.id = t.category_id
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1
                 GROUP BY c.id ORDER BY spent DESC LIMIT 5",
            )?;
            let top_cats: Vec<String> = cats_stmt
                .query_map(rusqlite::params![this_month_start], |r| {
                    Ok(format!(
                        "{} ${:.0}",
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)? as f64 / 100.0
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();

            // Over-budget count
            let over_budget: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM budgets b
                     WHERE b.month = ?1 AND b.amount_cents > 0
                       AND (SELECT COALESCE(SUM(ABS(amount_cents)),0) FROM transactions
                            WHERE category_id = b.category_id AND posted_at >= ?2
                              AND amount_cents < 0) > b.amount_cents",
                    rusqlite::params![this_month, this_month_start],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            // Goals
            let mut goals_stmt = conn.prepare(
                "SELECT name, current_cents, target_cents FROM goals WHERE target_cents > 0 LIMIT 5",
            )?;
            let goals: Vec<String> = goals_stmt
                .query_map([], |r| {
                    let name: String = r.get(0)?;
                    let current: i64 = r.get(1)?;
                    let target: i64 = r.get(2)?;
                    let pct = if target > 0 {
                        current * 100 / target
                    } else {
                        0
                    };
                    Ok(format!("{name} ({pct}% funded)"))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(format!(
                "Net worth: ${:.0} across all accounts\n\
                 This month: earned ${:.0}, spent ${:.0}, savings rate {}%\n\
                 Top spending: {}\n\
                 Budget: {} {} over limit\n\
                 Goals: {}",
                net_worth as f64 / 100.0,
                income as f64 / 100.0,
                expenses as f64 / 100.0,
                savings_rate,
                if top_cats.is_empty() {
                    "none yet".to_string()
                } else {
                    top_cats.join(", ")
                },
                over_budget,
                if over_budget == 1 {
                    "category"
                } else {
                    "categories"
                },
                if goals.is_empty() {
                    "none set".to_string()
                } else {
                    goals.join(", ")
                },
            ))
        })
        .await
        .map_err(AppError::from)?;

        let system = format!(
            "You are a personal finance assistant. Answer the user's question concisely \
             based on their real financial data provided below. \
             Respond with JSON only, shape: {{\"prose\": \"...\", \"action_label\": \"...\", \"action_path\": \"...\"}}. \
             action_label and action_path are optional — include only if a specific screen is directly relevant. \
             Valid paths: /, /accounts, /transactions, /budget, /categories, /recurring, /goals, /reports, /rules, /settings.\n\n\
             Financial context:\n{context}"
        );

        let raw = provider
            .complete_json(&system, &question)
            .await
            .map_err(|e| AppError::new("ask_agent.llm", e.to_string()))?;

        let prose = raw
            .get("prose")
            .and_then(|v| v.as_str())
            .unwrap_or("I couldn't generate a response. Try rephrasing your question.")
            .to_string();
        let action_label = raw
            .get("action_label")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let action_path = raw
            .get("action_path")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        Ok(AgentAnswer {
            prose,
            reasoning: String::new(),
            trace: Vec::new(),
            changes: Vec::new(),
            action_label,
            action_path,
        })
    }
}
