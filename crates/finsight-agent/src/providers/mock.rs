use crate::CompletionProvider;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Test double that returns a canned JSON value for any prompt.
pub struct MockCompletionProvider {
    pub provider_id: String,
    pub model_id: String,
    pub response: Value,
}

#[async_trait]
impl CompletionProvider for MockCompletionProvider {
    fn provider_id(&self) -> &str { &self.provider_id }
    fn model_id(&self) -> &str { &self.model_id }
    async fn complete_json(&self, _system: &str, _user: &str) -> Result<Value> {
        Ok(self.response.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn mock_returns_canned_value() {
        let p = MockCompletionProvider {
            provider_id: "mock".into(),
            model_id: "test".into(),
            response: json!([{"txn_id": "t1", "category_id": "cat1", "confidence": 0.9, "rationale": "test"}]),
        };
        let got = p.complete_json("sys", "user").await.unwrap();
        assert_eq!(got[0]["txn_id"], "t1");
    }
}
