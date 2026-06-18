use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatMessage {
    System { content: String },
    User { content: String },
    Assistant { content: Option<String>, tool_calls: Vec<ToolCall> },
    Tool { tool_call_id: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone)]
pub enum AssistantTurn {
    ToolCalls(Vec<ToolCall>),
    FinalAnswer { content: String, reasoning: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChange {
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ReasoningResult {
    pub content: String,
    pub reasoning: String,
    pub trace: Vec<String>,
    pub changes: Vec<AgentChange>,
}
