//! FinSight agent — LLM provider traits, agent task, categorizer pipeline.

pub mod agent;
pub mod anomaly;
pub mod categorizer;
pub mod context;
pub mod executor;
pub mod finance;
pub mod planner;
pub mod planning;
pub mod providers;
pub mod reasoning;
pub mod recipe_runner;

pub use categorizer::LOW_CONFIDENCE_THRESHOLD;
pub use reasoning::engine::ReasoningEngine;
pub use reasoning::messages::{
    AgentChange, AgentDraftAction, AssistantTurn, ChatMessage, ReasoningResult, ToolCall,
    ToolDefinition,
};
pub use reasoning::tools::{Tool, ToolContext, ToolSet};

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
        Err(anyhow::anyhow!(
            "Tool calling not implemented for this provider"
        ))
    }

    /// Like `complete_tool_turn`, but asks the provider to FORCE a tool call
    /// on this turn (`tool_choice: "required"`) rather than leaving it to the
    /// model's discretion. Used for a single retry after the model stalls on
    /// a text-only turn (a bare plan, or "let me pull that data now" with no
    /// tool calls) — a deterministic nudge is far more reliable than asking
    /// nicely in prose, which the model can (and does) ignore.
    /// Default: providers that don't support forcing just behave normally: no
    /// call errors, no different outcome, since the run loop's own
    /// nudge-then-best-effort-fallback logic already tolerates a turn that
    /// stalls again.
    async fn complete_tool_turn_forced(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        self.complete_tool_turn(messages, tools).await
    }

    /// Ask the provider for a FINAL text answer with NO further tool calls
    /// (`tool_choice: "none"`). Used by the reasoning loop when it hits its
    /// wall-clock budget: rather than time out with nothing, it forces the model
    /// to synthesize a best-effort answer from the tool results already gathered.
    /// Default: providers that can't set `tool_choice` fall back to a normal
    /// turn — the loop still appends an explicit "answer now, no tools" message,
    /// so the model is strongly steered even without hard enforcement.
    async fn complete_final_answer_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        self.complete_tool_turn(messages, tools).await
    }
}

/// Stub retained for Phase 5 (embedding-based nearest-neighbor search).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
}
