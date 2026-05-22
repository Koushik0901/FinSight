//! FinSight agent — LLM provider traits + categorization/anomaly/palette (Phase 3+).

use async_trait::async_trait;

#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn model_id(&self) -> &str;
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
}
