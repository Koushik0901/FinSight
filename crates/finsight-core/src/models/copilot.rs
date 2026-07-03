use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    pub status: String,
    pub task_type: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActionBundle {
    pub id: String,
    pub session_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub rationale: String,
    pub confidence: f64,
    pub status: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<AgentActionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActionItem {
    pub id: String,
    pub bundle_id: String,
    pub action_kind: String,
    pub payload_json: String,
    pub preview_json: Option<String>,
    pub rationale: String,
    pub confidence: f64,
    pub status: String,
    pub validation_errors: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentExecutionEntry {
    pub id: String,
    pub item_id: String,
    pub bundle_id: String,
    pub action_kind: String,
    pub status: String,
    pub result_json: Option<String>,
    pub error: Option<String>,
    pub executed_at: String,
}

/// Summary of a conversation thread shown in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// A single message within a conversation thread.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    /// JSON-encoded array of tool names used, e.g. `["spending_by_category"]`
    pub tool_trace: Option<String>,
    pub action_bundle_id: Option<String>,
    pub branch_parent_id: Option<String>,
    /// JSON-encoded assistant-ui message parts. `content` remains the text fallback.
    pub parts_json: Option<String>,
    /// Run lifecycle state for AG-UI/assistant reload semantics.
    pub run_status: String,
    /// JSON-encoded AG-UI metadata for tool calls, artifacts, approvals, and usage.
    pub ag_ui_metadata_json: Option<String>,
    pub created_at: String,
}
