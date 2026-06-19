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
