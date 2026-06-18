//! FinSight agent — LLM provider traits, agent task, categorizer pipeline.

pub mod agent;
pub mod anomaly;
pub mod categorizer;
pub mod context;
pub mod executor;
pub mod planner;
pub mod providers;
pub mod reasoning;
pub mod recipe_runner;

pub use reasoning::messages::{AgentChange, AssistantTurn, ChatMessage, ReasoningResult, ToolCall, ToolDefinition};
pub use reasoning::tools::{Tool, ToolContext, ToolSet};
pub use reasoning::engine::ReasoningEngine;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Core provider abstraction. All impls must be Send + Sync so they can be
/// shared across tokio tasks behind Arc<RwLock<...>>.
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;
    /// Send a system + user prompt; expect a JSON-parseable response.
    async fn complete_json(&self, system: &str, user: &str) -> Result<Value>;
    /// Return available model names. Returns Ok(vec![]) for providers
    /// that don't expose a model listing API (OpenAiCompat, Anthropic).
    async fn list_models(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
    /// Multi-turn completion with tool calling support.
    /// Returns either a set of tool calls or a final answer.
    async fn complete_tool_turn(
        &self,
        _messages: &[ChatMessage],
        _tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        Err(anyhow::anyhow!("Tool calling not implemented for this provider"))
    }
}

/// Stub retained for Phase 5 (embedding-based nearest-neighbor search).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
}
