use crate::error::{AppError, AppResult};
use crate::AppState;
#[cfg(test)]
use finsight_agent::finance::{self, FinanceQuestionKind};
use finsight_agent::{
    agent::AgentJob,
    planning,
    providers::{
        anthropic::AnthropicProvider, ollama::OllamaProvider, openai_compat::OpenAiCompatProvider,
    },
    reasoning::{
        engine::ReasoningEngine,
        tools::{act, read, ToolSet},
    },
    CompletionProvider, ReasoningResult, LOW_CONFIDENCE_THRESHOLD,
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
    crate::load_completion_provider_config(&db).map_err(AppError::from)
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
             WHERE ai_confidence < ?1 \
               AND (SELECT source FROM categorizations c \
                    WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm'",
            rusqlite::params![LOW_CONFIDENCE_THRESHOLD],
            |r| r.get(0),
        )?;
        Ok(count as u32)
    })
    .await
    .map_err(AppError::from)
}

/// Recompute statistical anomaly flags deterministically from transaction
/// patterns. Returns the number of transactions now flagged.
#[tauri::command]
#[specta::specta]
pub async fn recompute_anomalies(state: tauri::State<'_, AppState>) -> AppResult<u32> {
    let db = (*state.db).clone();
    run(&db, |conn| finsight_core::anomaly::recompute_anomalies(conn))
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
/// Queue a re-categorization pass for all low-confidence LLM assignments.
/// Runs the rule engine first (picks up any new rules the user created), then
/// the LLM for whatever remains uncertain.
pub async fn trigger_recategorize_low_confidence(
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    state
        .agent
        .tx
        .try_send(AgentJob::RecategorizeLowConfidence)
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

        // Exclude transfers: the categorizer never assigns them a category, so
        // counting them here would leave the status perpetually "N uncategorized"
        // that the user can never clear (they are already identified as transfers).
        let uncategorized_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL AND is_transfer = 0",
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
pub struct AgentScenarioAlternative {
    pub name: String,
    pub summary: String,
    pub tradeoff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentTableBlock {
    pub title: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentChartPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentChartBlock {
    pub title: Option<String>,
    pub series_label: Option<String>,
    pub data: Vec<AgentChartPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentMetricBlock {
    pub label: String,
    pub value: String,
    pub detail: Option<String>,
    pub tone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentResponseBlock {
    Markdown {
        markdown: String,
    },
    Table(AgentTableBlock),
    BarChart(AgentChartBlock),
    LineChart(AgentChartBlock),
    MetricGrid {
        metrics: Vec<AgentMetricBlock>,
    },
    Callout {
        tone: String,
        title: Option<String>,
        body: String,
    },
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
    pub bundle_id: Option<String>,
    pub assumptions: Vec<String>,
    pub data_sources: Vec<String>,
    pub missing_data: Vec<String>,
    pub alternatives: Vec<AgentScenarioAlternative>,
    pub follow_up_questions: Vec<String>,
    pub response_blocks: Vec<AgentResponseBlock>,
}

pub(crate) fn enrich_agent_answer(answer: &mut AgentAnswer) {
    if answer.response_blocks.is_empty() {
        if !answer.prose.trim().is_empty() {
            answer.response_blocks.push(AgentResponseBlock::Markdown {
                markdown: answer.prose.clone(),
            });
        }
        if !answer.reasoning.trim().is_empty() {
            answer.response_blocks.push(AgentResponseBlock::Callout {
                tone: "info".to_string(),
                title: Some("Reasoning".to_string()),
                body: answer.reasoning.clone(),
            });
        }
        if !answer.alternatives.is_empty() {
            answer
                .response_blocks
                .push(AgentResponseBlock::Table(AgentTableBlock {
                    title: Some("Alternatives compared".to_string()),
                    columns: vec![
                        "Scenario".to_string(),
                        "Numbers used".to_string(),
                        "Tradeoff".to_string(),
                    ],
                    rows: answer
                        .alternatives
                        .iter()
                        .map(|alt| {
                            vec![alt.name.clone(), alt.summary.clone(), alt.tradeoff.clone()]
                        })
                        .collect(),
                }));
        }
    }
}

pub(crate) fn parse_response_blocks(raw: &serde_json::Value) -> Vec<AgentResponseBlock> {
    raw.get("response_blocks")
        .or_else(|| raw.get("responseBlocks"))
        .and_then(|v| serde_json::from_value::<Vec<AgentResponseBlock>>(v.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(valid_response_block)
        .take(8)
        .collect()
}

fn valid_response_block(block: &AgentResponseBlock) -> bool {
    match block {
        AgentResponseBlock::Markdown { markdown } => !markdown.trim().is_empty(),
        AgentResponseBlock::Table(table) => {
            !table.columns.is_empty()
                && table.columns.len() <= 8
                && table.rows.len() <= 50
                && table
                    .rows
                    .iter()
                    .all(|row| row.len() == table.columns.len())
        }
        AgentResponseBlock::BarChart(chart) | AgentResponseBlock::LineChart(chart) => {
            !chart.data.is_empty() && chart.data.len() <= 30
        }
        AgentResponseBlock::MetricGrid { metrics } => !metrics.is_empty() && metrics.len() <= 12,
        AgentResponseBlock::Callout { body, .. } => !body.trim().is_empty(),
    }
}

pub(crate) fn build_toolset() -> ToolSet {
    let mut tools = ToolSet::new();
    tools.register(read::get_financial_snapshot());
    tools.register(read::analyze_cash_inflow());
    tools.register(read::calculate_goal_eta());
    tools.register(read::rank_debt_payoff());
    tools.register(read::compare_debt_vs_goal());
    tools.register(read::get_account_balances());
    tools.register(read::get_net_worth());
    tools.register(read::get_month_totals());
    tools.register(read::get_top_spending_categories());
    tools.register(read::get_spending_breakdown());
    tools.register(read::get_budgets());
    tools.register(read::get_goals());
    tools.register(read::get_recurring_bills());
    tools.register(read::get_liabilities());
    tools.register(read::search_transactions());
    tools.register(read::find_anomalies());
    tools.register(read::list_uncategorized_transactions());
    tools.register(read::run_cashflow_projection());
    tools.register(read::run_debt_payoff_scenarios());
    tools.register(read::run_goal_allocation_scenarios());
    tools.register(read::run_goal_conflict_scenario());
    tools.register(read::run_emergency_fund_scenarios());
    tools.register(read::run_cashflow_timeline());
    tools.register(read::run_purchase_affordability());
    tools.register(read::get_data_quality_report());
    tools.register(act::set_budget());
    tools.register(act::update_goal_monthly());
    tools.register(act::create_planned_transaction());
    tools.register(act::save_scenario());
    tools.register(act::create_debt_payoff_plan());
    tools.register(act::draft_recategorization());
    tools
}

#[cfg(test)]
fn normalize_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
fn find_best_goal_match<'a>(
    question: &str,
    goals: &'a [finance::SnapshotGoal],
) -> Option<&'a finance::SnapshotGoal> {
    let q = normalize_name(question);
    goals
        .iter()
        .find(|goal| {
            let name = normalize_name(&goal.name);
            !name.is_empty() && q.contains(&name)
        })
        .or_else(|| {
            goals.iter().find(|goal| {
                let name = normalize_name(&goal.name);
                if name.is_empty() {
                    return false;
                }
                name.split_whitespace().any(|token| q.contains(token))
            })
        })
}

#[cfg(test)]
fn find_best_liability_match<'a>(
    question: &str,
    liabilities: &'a [finance::SnapshotLiability],
) -> Option<&'a finance::SnapshotLiability> {
    let q = normalize_name(question);
    liabilities
        .iter()
        .find(|liability| {
            let name = normalize_name(&liability.name);
            !name.is_empty() && q.contains(&name)
        })
        .or_else(|| {
            liabilities.iter().find(|liability| {
                let name = normalize_name(&liability.name);
                if name.is_empty() {
                    return false;
                }
                name.split_whitespace().any(|token| q.contains(token))
            })
        })
}

#[cfg(test)]
fn format_cents(cents: i64) -> String {
    let value = cents as f64 / 100.0;
    if value.fract().abs() < 0.005 {
        format!("${:.0}", value)
    } else {
        format!("${:.2}", value)
    }
}

#[cfg(test)]
fn default_finance_data_sources() -> Vec<String> {
    vec![
        "Accounts and latest account balances".to_string(),
        "Transactions over the last 90 and 365 days".to_string(),
        "Active goals".to_string(),
        "Tracked liabilities, APRs, and minimum payments".to_string(),
        "Detected recurring bills and planned transactions".to_string(),
    ]
}

fn mentions_investing(question: &str) -> bool {
    let q = question.to_lowercase();
    ["invest", "stocks", "stock", "etf", "ticker", "portfolio"]
        .iter()
        .any(|term| q.contains(term))
}

pub(crate) fn validate_finance_answer(question: &str, answer: &mut AgentAnswer) {
    answer.missing_data.sort();
    answer.missing_data.dedup();
    answer.assumptions.sort();
    answer.assumptions.dedup();
    answer.data_sources.sort();
    answer.data_sources.dedup();

    if mentions_investing(question) {
        let guardrail = "I can discuss investing readiness and principles from your local cashflow/debt data, but this app does not use external market data and should not recommend specific tickers, ETFs, or market timing.";
        if !answer.assumptions.iter().any(|item| item == guardrail) {
            answer.assumptions.push(guardrail.to_string());
        }
        if !answer.prose.to_lowercase().contains("specific tickers") {
            answer.prose.push(' ');
            answer.prose.push_str(
                "I would keep investing advice principles-only here rather than naming specific tickers or ETFs.",
            );
        }
    }
}

fn planner_alternatives_to_agent(
    alternatives: &[planning::FinanceAlternative],
) -> Vec<AgentScenarioAlternative> {
    alternatives
        .iter()
        .map(|alt| AgentScenarioAlternative {
            name: alt.name.clone(),
            summary: alt.summary.clone(),
            tradeoff: alt.tradeoff.clone(),
        })
        .collect()
}

#[cfg(test)]
fn debt_goal_alternatives_to_agent(
    alternatives: &[finance::ScenarioAlternative],
) -> Vec<AgentScenarioAlternative> {
    alternatives
        .iter()
        .map(|alt| {
            let payoff = alt
                .payoff_months
                .map(|m| format!("{m} mo payoff"))
                .unwrap_or_else(|| "payoff unknown".to_string());
            let interest = alt
                .interest_cents
                .map(format_cents)
                .unwrap_or_else(|| "interest unknown".to_string());
            AgentScenarioAlternative {
                name: alt.name.clone(),
                summary: format!(
                    "Use {}; debt payment {}; {}; estimated interest {}.",
                    format_cents(alt.cash_used_cents),
                    alt.monthly_debt_payment_cents
                        .map(format_cents)
                        .unwrap_or_else(|| "unknown".to_string()),
                    payoff,
                    interest
                ),
                tradeoff: alt.tradeoff.clone(),
            }
        })
        .collect()
}
pub(crate) fn planner_answer_to_agent_answer(
    answer: planning::StructuredFinanceAnswer,
) -> AgentAnswer {
    let mut prose_parts = Vec::new();
    if !answer.recommendation.trim().is_empty() {
        prose_parts.push(answer.recommendation.clone());
    }
    if !answer.summary.trim().is_empty() {
        prose_parts.push(answer.summary.clone());
    }
    if !answer.alternatives.is_empty() {
        let alternatives = answer
            .alternatives
            .iter()
            .map(|alt| format!("{}: {} {}", alt.name, alt.summary, alt.tradeoff))
            .collect::<Vec<_>>()
            .join(" ");
        prose_parts.push(format!("Alternatives compared: {alternatives}"));
    }
    if !answer.what_would_change_recommendation.is_empty() {
        prose_parts.push(format!(
            "What would change this recommendation: {}",
            answer.what_would_change_recommendation.join(" ")
        ));
    }

    let mut missing_data = answer.missing_data.clone();
    if answer.verification.severity != planning::VerificationSeverity::Ok {
        missing_data.extend(answer.verification.findings.clone());
    }
    missing_data.sort();
    missing_data.dedup();

    let mut assumptions = answer.assumptions.clone();
    assumptions.extend(answer.risks.iter().map(|risk| format!("Risk flag: {risk}")));
    assumptions.extend(
        answer
            .what_would_change_recommendation
            .iter()
            .map(|item| format!("What would change this: {item}")),
    );
    assumptions.sort();
    assumptions.dedup();

    let verifier_note = if answer.verification.findings.is_empty() {
        format!(
            "Verifier: {:?}; confidence {:.0}%.",
            answer.verification.severity,
            answer.confidence * 100.0
        )
    } else {
        format!(
            "Verifier: {:?}; confidence {:.0}%. Findings: {}",
            answer.verification.severity,
            answer.confidence * 100.0,
            answer.verification.findings.join("; ")
        )
    };

    AgentAnswer {
        prose: prose_parts.join(" "),
        reasoning: [answer.reasoning, verifier_note]
            .into_iter()
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        trace: answer.trace,
        changes: Vec::new(),
        action_label: None,
        action_path: None,
        bundle_id: None,
        assumptions,
        data_sources: answer.data_sources,
        missing_data,
        alternatives: planner_alternatives_to_agent(&answer.alternatives),
        follow_up_questions: answer.follow_up_questions,
        response_blocks: Vec::new(),
    }
}
pub(crate) fn is_usable_tool_answer(result: &ReasoningResult) -> bool {
    let used_tool = result
        .trace
        .iter()
        .any(|entry| entry.starts_with("Called tool:"));
    used_tool && !result.content.trim().is_empty() && !result.data_sources.is_empty()
}

pub(crate) fn reasoning_result_to_agent_answer(
    result: ReasoningResult,
    bundle_id: Option<String>,
) -> AgentAnswer {
    let mut data_sources = result.data_sources;
    if data_sources.is_empty() {
        data_sources.extend([
            "Agent tool calls shown in the trace".to_string(),
            "Local FinSight database snapshots returned by tools".to_string(),
        ]);
    }

    AgentAnswer {
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
        bundle_id,
        assumptions: result.assumptions,
        data_sources,
        missing_data: result.missing_data,
        alternatives: Vec::new(),
        follow_up_questions: result.follow_up_questions,
        response_blocks: Vec::new(),
    }
}
#[cfg(test)]
fn direct_finance_answer(
    conn: &mut rusqlite::Connection,
    question: &str,
) -> AppResult<Option<AgentAnswer>> {
    let profile = finance::infer_question_profile(question);
    let snapshot =
        finance::build_snapshot(conn).map_err(|e| AppError::new("agent.finance", e.to_string()))?;

    let mut assumptions = Vec::new();
    let mut missing_data = snapshot.data_warnings.clone();
    let mut follow_up_questions = Vec::new();
    let mut trace = Vec::new();

    let answer = match profile.kind {
        FinanceQuestionKind::CashInflow => {
            let amount_cents = match profile.amount_cents {
                Some(amount) if amount > 0 => amount,
                _ => {
                    follow_up_questions
                        .push("How much is the paycheck or windfall, in dollars?".to_string());
                    return Ok(Some(AgentAnswer {
                        prose: "I need the amount before I can split it across debt, savings, and goals.".to_string(),
                        reasoning: "The question is missing the cash inflow amount.".to_string(),
                        trace,
                        changes: Vec::new(),
                        action_label: None,
                        action_path: None,
                        bundle_id: None,
                        assumptions,
                        data_sources: default_finance_data_sources(),
                        missing_data,
                        alternatives: Vec::new(),
                        follow_up_questions,
                        response_blocks: Vec::new(),
                    }));
                }
            };
            let advice = finance::analyze_cash_inflow(conn, amount_cents)
                .map_err(|e| AppError::new("agent.finance", e.to_string()))?;
            trace.push("Called tool: analyze_cash_inflow".to_string());
            if !advice.missing_data.is_empty() {
                missing_data.extend(advice.missing_data.clone());
            }
            if advice.investing_allowed {
                assumptions.push("Investing is allowed only after the emergency fund and high-interest debt checks pass.".to_string());
            } else {
                assumptions.push("Investing is deferred until emergency coverage and debt priorities are addressed.".to_string());
            }
            let reasoning = advice.rationale.join(" ");
            let mut prose_lines = vec![format!(
                "For ${:.2}, I would prioritize liquidity first, then high-interest debt, then goals.",
                amount_cents as f64 / 100.0
            )];
            for allocation in advice.allocations {
                prose_lines.push(format!(
                    "{}: {} ({})",
                    allocation.bucket.replace('_', " "),
                    format_cents(allocation.amount_cents),
                    allocation.reason
                ));
            }
            if !advice.investing_allowed {
                prose_lines.push(
                    "I would not direct this into stocks or ETFs yet; keep the answer principles-only and focus on debt and cash reserves.".to_string(),
                );
            }
            Some(AgentAnswer {
                prose: prose_lines.join(" "),
                reasoning,
                trace,
                changes: Vec::new(),
                action_label: None,
                action_path: None,
                bundle_id: None,
                assumptions,
                data_sources: default_finance_data_sources(),
                missing_data,
                alternatives: Vec::new(),
                follow_up_questions,
                response_blocks: Vec::new(),
            })
        }
        FinanceQuestionKind::GoalEta => {
            let amount_cents = match profile.amount_cents {
                Some(amount) if amount > 0 => amount,
                _ => {
                    follow_up_questions
                        .push("How much do you want to save each pay period?".to_string());
                    return Ok(Some(AgentAnswer {
                        prose: "I need your contribution amount to estimate the goal timeline."
                            .to_string(),
                        reasoning: "The question is missing the contribution amount.".to_string(),
                        trace,
                        changes: Vec::new(),
                        action_label: None,
                        action_path: None,
                        bundle_id: None,
                        assumptions,
                        data_sources: default_finance_data_sources(),
                        missing_data,
                        alternatives: Vec::new(),
                        follow_up_questions,
                        response_blocks: Vec::new(),
                    }));
                }
            };
            let cadence = profile
                .cadence
                .clone()
                .unwrap_or_else(|| "monthly".to_string());
            let goal = find_best_goal_match(question, &snapshot.goals);
            let Some(goal) = goal else {
                follow_up_questions.push("Which goal should I use for the ETA?".to_string());
                let goal_names = snapshot
                    .goals
                    .iter()
                    .map(|g| g.name.clone())
                    .collect::<Vec<_>>();
                if !goal_names.is_empty() {
                    assumptions.push(format!("Available goals: {}.", goal_names.join(", ")));
                }
                return Ok(Some(AgentAnswer {
                    prose: "I need the specific goal before I can estimate when you will reach it."
                        .to_string(),
                    reasoning: "No goal match was confident enough to calculate ETA.".to_string(),
                    trace,
                    changes: Vec::new(),
                    action_label: None,
                    action_path: None,
                    bundle_id: None,
                    assumptions,
                    data_sources: default_finance_data_sources(),
                    missing_data,
                    alternatives: Vec::new(),
                    follow_up_questions,
                    response_blocks: Vec::new(),
                }));
            };
            let eta = finance::calculate_goal_eta(conn, &goal.id, amount_cents, &cadence)
                .map_err(|e| AppError::new("agent.finance", e.to_string()))?;
            trace.push("Called tool: calculate_goal_eta".to_string());
            if eta.eta_months.is_none() {
                missing_data.push("Goal ETA is provisional because the contribution is zero or the goal is fully funded.".to_string());
            }
            let reasoning = format!(
                "{} needs {} remaining. At {} per {}, that is about {} month(s).",
                eta.goal_name,
                format_cents(eta.remaining_cents),
                format_cents(amount_cents),
                cadence,
                eta.eta_months
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            );
            let eta_text = eta
                .eta_months
                .map(|m| format!("{m} month(s)"))
                .unwrap_or_else(|| "an unknown timeline".to_string());
            Some(AgentAnswer {
                prose: format!(
                    "If you save {} {}, you should reach {} in about {}. That is {} per month equivalent.",
                    format_cents(amount_cents),
                    cadence,
                    eta.goal_name,
                    eta_text,
                    format_cents(eta.monthly_equivalent_cents)
                ),
                reasoning,
                trace,
                changes: Vec::new(),
                action_label: None,
                action_path: None,
                bundle_id: None,
                assumptions,
                data_sources: default_finance_data_sources(),
                missing_data,
                alternatives: Vec::new(),
                follow_up_questions,
                response_blocks: Vec::new(),
            })
        }
        FinanceQuestionKind::DebtVsGoal => {
            let Some(goal) = find_best_goal_match(question, &snapshot.goals) else {
                follow_up_questions
                    .push("Which savings goal should I compare against the loan?".to_string());
                return Ok(Some(AgentAnswer {
                    prose: "I need the goal name before I can compare it against your debt."
                        .to_string(),
                    reasoning: "The goal could not be identified confidently.".to_string(),
                    trace,
                    changes: Vec::new(),
                    action_label: None,
                    action_path: None,
                    bundle_id: None,
                    assumptions,
                    data_sources: default_finance_data_sources(),
                    missing_data,
                    alternatives: Vec::new(),
                    follow_up_questions,
                    response_blocks: Vec::new(),
                }));
            };
            let liability = find_best_liability_match(question, &snapshot.liabilities);
            let comparison =
                finance::compare_debt_vs_goal(conn, &goal.id, liability.map(|d| d.id.as_str()))
                    .map_err(|e| AppError::new("agent.finance", e.to_string()))?;
            trace.push("Called tool: compare_debt_vs_goal".to_string());
            if !comparison.missing_data.is_empty() {
                missing_data.extend(comparison.missing_data.clone());
            }
            assumptions.push(format!(
                "{} current savings is {}.",
                comparison.goal_name,
                format_cents(comparison.goal_current_cents)
            ));
            if let Some(apr) = comparison.highest_apr_pct {
                assumptions.push(format!("Highest relevant debt APR is {apr:.1}%."));
            }
            if let Some(months) = comparison.payoff_months_with_redirect {
                assumptions.push(format!(
                    "Fastest modeled payoff scenario clears the compared debt in about {months} month(s)."
                ));
            }
            let mut prose = vec![format!("Short answer: {}", comparison.recommendation)];
            if comparison.suggested_goal_drawdown_cents > 0 {
                prose.push(format!(
                    "The safe amount to move from {} is {}, which leaves about {:.1} month(s) of emergency coverage.",
                    comparison.goal_name,
                    format_cents(comparison.suggested_goal_drawdown_cents),
                    comparison.emergency_fund_months_after_drawdown
                ));
            }
            if let Some(saved) = comparison.estimated_interest_saved_cents {
                prose.push(format!(
                    "Compared with keeping the debt on its current minimum-payment track, the modeled safe-drawdown-plus-redirect plan avoids about {} of interest.",
                    format_cents(saved)
                ));
            }
            if !comparison.alternatives.is_empty() {
                let alternatives = comparison
                    .alternatives
                    .iter()
                    .map(|alt| {
                        let payoff = alt
                            .payoff_months
                            .map(|m| format!("{m} mo payoff"))
                            .unwrap_or_else(|| "payoff unknown".to_string());
                        let interest = alt
                            .interest_cents
                            .map(format_cents)
                            .unwrap_or_else(|| "interest unknown".to_string());
                        format!(
                            "{}: use {}, debt payment {}, {}, estimated interest {}. {}",
                            alt.name,
                            format_cents(alt.cash_used_cents),
                            alt.monthly_debt_payment_cents
                                .map(format_cents)
                                .unwrap_or_else(|| "unknown".to_string()),
                            payoff,
                            interest,
                            alt.tradeoff
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                prose.push(format!("Alternatives compared: {alternatives}"));
            }
            Some(AgentAnswer {
                prose: prose.join(" "),
                reasoning: comparison.rationale.join(" "),
                trace,
                changes: Vec::new(),
                action_label: None,
                action_path: None,
                bundle_id: None,
                assumptions,
                data_sources: default_finance_data_sources(),
                missing_data,
                alternatives: debt_goal_alternatives_to_agent(&comparison.alternatives),
                follow_up_questions,
                response_blocks: Vec::new(),
            })
        }
        FinanceQuestionKind::DebtRanking => {
            let method = profile.method.as_deref().unwrap_or_else(|| {
                if question.to_lowercase().contains("snowball") {
                    "snowball"
                } else {
                    "avalanche"
                }
            });
            let ranking = finance::rank_debt_payoff(conn, method)
                .map_err(|e| AppError::new("agent.finance", e.to_string()))?;
            trace.push("Called tool: rank_debt_payoff".to_string());
            if !ranking.missing_data.is_empty() {
                missing_data.extend(ranking.missing_data.clone());
            }
            let ordered = ranking
                .items
                .iter()
                .map(|item| {
                    format!(
                        "{}. {} ({}, {})",
                        item.rank,
                        item.name,
                        format_cents(item.balance_cents),
                        item.reason
                    )
                })
                .collect::<Vec<_>>();
            Some(AgentAnswer {
                prose: if ordered.is_empty() {
                    "I do not see any active debts to rank.".to_string()
                } else {
                    format!("Use {} ordering. {}", ranking.method, ordered.join(" "))
                },
                reasoning: if ordered.is_empty() {
                    "No positive-balance liabilities were found.".to_string()
                } else {
                    format!("{} debts ranked with {}.", ordered.len(), ranking.method)
                },
                trace,
                changes: Vec::new(),
                action_label: None,
                action_path: None,
                bundle_id: None,
                assumptions,
                data_sources: default_finance_data_sources(),
                missing_data,
                alternatives: Vec::new(),
                follow_up_questions,
                response_blocks: Vec::new(),
            })
        }
        FinanceQuestionKind::Snapshot => {
            trace.push("Called tool: get_financial_snapshot".to_string());
            let mut prose = vec![format!(
                "You have {} in liquid accounts and {} total across all accounts.",
                format_cents(snapshot.liquid_balance_cents),
                format_cents(snapshot.total_account_balance_cents)
            )];
            prose.push(format!(
                "Your emergency fund covers about {:.1} month(s) of expenses.",
                snapshot.emergency_fund_months
            ));
            if !snapshot.data_warnings.is_empty() {
                missing_data.extend(snapshot.data_warnings.clone());
            }
            Some(AgentAnswer {
                prose: prose.join(" "),
                reasoning: "Snapshot built from local accounts, goals, debts, recurring bills, and planned transactions.".to_string(),
                trace,
                changes: Vec::new(),
                action_label: None,
                action_path: None,
                bundle_id: None,
                assumptions,
                data_sources: default_finance_data_sources(),
                missing_data,
                alternatives: Vec::new(),
                follow_up_questions,
                response_blocks: Vec::new(),
            })
        }
        FinanceQuestionKind::GeneralPlanning | FinanceQuestionKind::Unknown => None,
    };

    Ok(answer)
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
        let tools = build_toolset();
        let provider_clone = Arc::clone(&provider);
        let question_clone = question.clone();
        let tool_result = run(&db, move |conn| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| {
                    finsight_core::CoreError::InvalidState(format!("Failed to create runtime: {e}"))
                })?;
            rt.block_on(ReasoningEngine::run(
                conn,
                &question_clone,
                &tools,
                provider_clone,
                10,
            ))
            .map_err(|e| {
                finsight_core::CoreError::InvalidState(format!("Reasoning engine error: {e}"))
            })
        })
        .await;

        match tool_result {
            Ok(result) if is_usable_tool_answer(&result) => {
                let draft_actions = result.draft_actions.clone();
                let question_for_db = question.clone();
                let content_for_db = result.content.clone();
                let reasoning_for_db = if result.reasoning.is_empty() {
                    "Tool-driven financial analysis".to_string()
                } else {
                    result.reasoning.clone()
                };
                let provider_id = provider.provider_id().to_string();
                let model_id = provider.model_id().to_string();
                let bundle_id = run(&db, move |conn| {
                    let mut bundle = finsight_core::repos::copilot_actions::insert_bundle(
                        conn,
                        None,
                        &question_for_db,
                        &content_for_db,
                        &reasoning_for_db,
                        0.9,
                        Some(&provider_id),
                        Some(&model_id),
                    )?;
                    for (i, draft) in draft_actions.iter().enumerate() {
                        let item = finsight_core::repos::copilot_actions::insert_item(
                            conn,
                            &bundle.id,
                            &draft.action_kind,
                            &draft.payload_json,
                            &draft.rationale,
                            draft.confidence,
                            i as i64,
                        )?;
                        bundle.items.push(item);
                    }
                    Ok::<_, finsight_core::CoreError>(bundle.id)
                })
                .await
                .map_err(AppError::from)?;

                let mut answer = reasoning_result_to_agent_answer(result, Some(bundle_id));
                validate_finance_answer(&question, &mut answer);
                enrich_agent_answer(&mut answer);
                return Ok(answer);
            }
            Ok(result) => {
                let planned = run(&db, {
                    let question = question.clone();
                    move |conn| {
                        planning::answer_finance_question(conn, &question)
                            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
                    }
                })
                .await
                .map_err(AppError::from)?;
                if let Some(answer) = planned {
                    let mut mapped = planner_answer_to_agent_answer(answer);
                    mapped
                        .trace
                        .insert(0, "Tool loop produced an incomplete structured answer; used verified deterministic planner fallback.".to_string());
                    validate_finance_answer(&question, &mut mapped);
                    enrich_agent_answer(&mut mapped);
                    return Ok(mapped);
                }

                let mut answer = reasoning_result_to_agent_answer(result, None);
                answer.missing_data.push(
                    "The tool loop answered without the full structured finance schema; treat this broad answer as provisional.".to_string(),
                );
                validate_finance_answer(&question, &mut answer);
                enrich_agent_answer(&mut answer);
                return Ok(answer);
            }
            Err(tool_err) => {
                let planned = run(&db, {
                    let question = question.clone();
                    move |conn| {
                        planning::answer_finance_question(conn, &question)
                            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
                    }
                })
                .await
                .map_err(AppError::from)?;
                if let Some(answer) = planned {
                    let mut mapped = planner_answer_to_agent_answer(answer);
                    mapped.trace.insert(
                        0,
                        format!("Tool loop failed; used verified deterministic planner fallback: {tool_err}"),
                    );
                    validate_finance_answer(&question, &mut mapped);
                    enrich_agent_answer(&mut mapped);
                    return Ok(mapped);
                }
                return Err(AppError::new("agent.reasoning", tool_err.to_string()));
            }
        }
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
             Respond with JSON only. Shape: {{\"prose\": \"...\", \"action_label\": \"...\", \"action_path\": \"...\", \"response_blocks\": [...]}}. \
             response_blocks is optional. Use it only when it improves clarity. Supported blocks: \
             {{\"kind\":\"markdown\",\"markdown\":\"...\"}}, \
             {{\"kind\":\"table\",\"title\":\"...\",\"columns\":[\"...\"],\"rows\":[[\"...\"]]}}, \
             {{\"kind\":\"barChart\",\"title\":\"...\",\"seriesLabel\":\"...\",\"data\":[{{\"label\":\"...\",\"value\":123}}]}}, \
             {{\"kind\":\"lineChart\",\"title\":\"...\",\"seriesLabel\":\"...\",\"data\":[{{\"label\":\"...\",\"value\":123}}]}}, \
             {{\"kind\":\"metricGrid\",\"metrics\":[{{\"label\":\"...\",\"value\":\"...\",\"detail\":\"...\",\"tone\":\"neutral\"}}]}}, \
             {{\"kind\":\"callout\",\"tone\":\"info\",\"title\":\"...\",\"body\":\"...\"}}. \
             Do not include HTML. \
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

        let mut answer = AgentAnswer {
            prose,
            reasoning: String::new(),
            trace: Vec::new(),
            changes: Vec::new(),
            action_label,
            action_path,
            bundle_id: None,
            assumptions: Vec::new(),
            data_sources: vec!["Monthly account, transaction, budget, and goal summary".to_string()],
            missing_data: Vec::new(),
            alternatives: Vec::new(),
            follow_up_questions: Vec::new(),
            response_blocks: parse_response_blocks(&raw),
        };
        validate_finance_answer(&question, &mut answer);
        enrich_agent_answer(&mut answer);
        Ok(answer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("agent.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &mut rusqlite::Connection) {
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('a1','Me','Bank','Checking','Checking','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id, as_of_date, balance_cents) VALUES('a1','2026-06-01',500000)", []).unwrap();
        conn.execute("INSERT INTO goals(id,name,type,target_cents,current_cents,monthly_cents,color,sort_order,created_at) VALUES('car','Car','save-by-date',2000000,500000,50000,'#fff',0,datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO liabilities(id,name,liability_type,balance_cents,limit_cents,apr_pct,min_payment_cents,currency,created_at,updated_at) VALUES('cc','Credit Card','credit-card',250000,500000,24.9,5000,'USD',datetime('now'),datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO liabilities(id,name,liability_type,balance_cents,limit_cents,apr_pct,min_payment_cents,currency,created_at,updated_at) VALUES('loan','Loan','loan',1800000,NULL,5.0,30000,'USD',datetime('now'),datetime('now'))", []).unwrap();
    }

    #[test]
    fn direct_cash_inflow_answer_uses_deterministic_allocation() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer =
            direct_finance_answer(&mut conn, "I got a pay of around $3,000. What should I do?")
                .unwrap()
                .expect("direct answer");
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("analyze_cash_inflow")));
        assert!(answer.prose.contains("high-interest debt"));
        assert!(
            answer.missing_data.is_empty() || answer.missing_data.iter().any(|m| m.contains("APR"))
        );
    }

    #[test]
    fn direct_goal_eta_answer_uses_goal_calculator() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = direct_finance_answer(
            &mut conn,
            "If I save up $500 bi-weekly, how soon will I reach my car goal?",
        )
        .unwrap()
        .expect("direct answer");
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("calculate_goal_eta")));
        assert!(answer.prose.contains("Car"));
    }

    #[test]
    fn direct_debt_vs_goal_answer_compares_scenarios() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = direct_finance_answer(
            &mut conn,
            "Should I use my car savings to pay off a similar-sized loan?",
        )
        .unwrap()
        .expect("direct answer");

        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("compare_debt_vs_goal")));
        assert!(answer.prose.contains("Alternatives compared"));
        assert!(answer.prose.contains("estimated interest"));
        assert!(answer.alternatives.len() >= 2);
        assert!(answer
            .alternatives
            .iter()
            .any(|alt| alt.summary.contains("estimated interest")));
        assert!(answer.reasoning.contains("Highest compared APR"));
        assert!(!answer.data_sources.is_empty());
    }
}
