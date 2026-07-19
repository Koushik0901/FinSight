use crate::error::AppResult;
use crate::AppState;
use finsight_core::models::RuleProposal;

// Types live in finsight-api now; re-exported so existing imports of
// `finsight_app::commands::agent::*` (lib.rs, tests) keep resolving. The
// shared reasoning-pipeline helpers (build_toolset, enrich_agent_answer, etc.)
// are pub(crate) in finsight-api since Task 6 moved their only cross-crate
// consumer (copilot_chat) into the same crate — no longer re-exported here.
pub use finsight_api::commands::agent::{
    AgentAccountRow, AgentAccountsOverviewBlock, AgentActionPlanBlock, AgentActivity,
    AgentAffordabilityVerdictBlock, AgentAllocationSegment, AgentAllocationSplitBlock,
    AgentAnswer, AgentCategoryBreakdownBlock, AgentCategoryRow, AgentChange, AgentChartBlock,
    AgentChartPoint, AgentComparisonBarsBlock, AgentDriver, AgentFundingSource,
    AgentMetricBlock, AgentMoneyPoint, AgentRankedOption, AgentRankedOptionsBlock,
    AgentRecatRow, AgentRecategorizationPreviewBlock, AgentResponseBlock, AgentReviewCategory,
    AgentReviewMonth, AgentScenarioAlternative, AgentSpendTimelineBlock, AgentSpendingDriversBlock,
    AgentSpendingReviewBlock, AgentStatus, AgentTableBlock, AgentTimelinePoint,
    AgentTransactionTableBlock, AgentTxRow, AgentTxnSearchQuery, AgentWatchItem,
    AgentWatchListBlock, CompletionProviderConfig, ProviderTestResult,
};

#[tauri::command]
#[specta::specta]
pub async fn set_completion_provider(
    state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
) -> AppResult<()> {
    finsight_api::commands::agent::set_completion_provider(&state.api, config).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_completion_provider(
    state: tauri::State<'_, AppState>,
) -> AppResult<CompletionProviderConfig> {
    finsight_api::commands::agent::get_completion_provider(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn save_provider_api_key(
    state: tauri::State<'_, AppState>,
    provider_id: String,
    key: String,
) -> AppResult<()> {
    finsight_api::commands::agent::save_provider_api_key(&state.api, provider_id, key).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_provider_models(
    _state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
) -> AppResult<Vec<String>> {
    finsight_api::commands::agent::list_provider_models(&_state.api, config).await
}

#[tauri::command]
#[specta::specta]
pub async fn test_completion_provider(
    _state: tauri::State<'_, AppState>,
    config: CompletionProviderConfig,
    api_key: Option<String>,
) -> AppResult<ProviderTestResult> {
    finsight_api::commands::agent::test_completion_provider(&_state.api, config, api_key).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_needs_review_count(state: tauri::State<'_, AppState>) -> AppResult<u32> {
    finsight_api::commands::agent::get_needs_review_count(&state.api).await
}

/// Recompute statistical anomaly flags deterministically from transaction
/// patterns. Returns the number of transactions now flagged.
#[tauri::command]
#[specta::specta]
pub async fn recompute_anomalies(state: tauri::State<'_, AppState>) -> AppResult<u32> {
    finsight_api::commands::agent::recompute_anomalies(&state.api).await
}

/// Mark a flagged anomaly as reviewed-and-fine (dismiss) or restore it. A
/// dismissed charge is cleared and the detector will not re-flag it on the next
/// recompute; un-dismissing makes it flaggable again. Keeps the Insights anomaly
/// feed trustworthy without per-transaction drawer edits.
#[tauri::command]
#[specta::specta]
pub async fn set_anomaly_dismissed(
    state: tauri::State<'_, AppState>,
    txn_id: String,
    dismissed: bool,
) -> AppResult<()> {
    finsight_api::commands::agent::set_anomaly_dismissed(&state.api, txn_id, dismissed).await
}

#[tauri::command]
#[specta::specta]
pub async fn trigger_categorize(state: tauri::State<'_, AppState>) -> AppResult<()> {
    finsight_api::commands::agent::trigger_categorize(&state.api).await
}

#[tauri::command]
#[specta::specta]
/// Queue a re-categorization pass for all low-confidence LLM assignments.
/// Runs the rule engine first (picks up any new rules the user created), then
/// the LLM for whatever remains uncertain.
pub async fn trigger_recategorize_low_confidence(
    state: tauri::State<'_, AppState>,
) -> AppResult<()> {
    finsight_api::commands::agent::trigger_recategorize_low_confidence(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_rule_proposals(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<RuleProposal>> {
    finsight_api::commands::agent::list_rule_proposals(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn accept_rule_proposal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::agent::accept_rule_proposal(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn decline_rule_proposal(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    finsight_api::commands::agent::decline_rule_proposal(&state.api, id).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_recent_agent_activity(
    state: tauri::State<'_, AppState>,
    limit: u32,
) -> AppResult<Vec<AgentActivity>> {
    finsight_api::commands::agent::list_recent_agent_activity(&state.api, limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_agent_status(state: tauri::State<'_, AppState>) -> AppResult<AgentStatus> {
    finsight_api::commands::agent::get_agent_status(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn ask_agent(
    state: tauri::State<'_, AppState>,
    question: String,
    mode: Option<String>,
) -> AppResult<AgentAnswer> {
    finsight_api::commands::agent::ask_agent(&state.api, question, mode).await
}
