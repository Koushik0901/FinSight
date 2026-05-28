//! FinSight agent — LLM provider traits, agent task, categorizer pipeline.

pub mod providers;

use async_trait::async_trait;
use anyhow::Result;
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
}

/// Stub retained for Phase 5 (embedding-based nearest-neighbor search).
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
}
