use crate::CompletionProvider;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

const ANTHROPIC_API: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    input: Value,
}
#[derive(Deserialize)]
struct AnthropicResp { content: Vec<ContentBlock> }

#[async_trait]
impl CompletionProvider for AnthropicProvider {
    fn provider_id(&self) -> &str { "anthropic" }
    fn model_id(&self) -> &str { &self.model }

    async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system,
            "messages": [{"role": "user", "content": user}],
            "tools": [{
                "name": "classify",
                "description": "Return the classification results",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "results": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "txn_id":      {"type": "string"},
                                    "category_id": {"type": "string"},
                                    "confidence":  {"type": "number"},
                                    "rationale":   {"type": "string"}
                                },
                                "required": ["txn_id", "category_id", "confidence", "rationale"]
                            }
                        }
                    },
                    "required": ["results"]
                }
            }],
            "tool_choice": {"type": "tool", "name": "classify"}
        });

        let resp: AnthropicResp = self.client
            .post(ANTHROPIC_API)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        // Response is content[0].input.results
        let block = resp.content.into_iter().next()
            .ok_or_else(|| anyhow!("empty content from Anthropic"))?;
        if block.kind != "tool_use" {
            return Err(anyhow!("expected tool_use block, got {}", block.kind));
        }
        // Return the results array directly
        block.input.get("results")
            .cloned()
            .ok_or_else(|| anyhow!("missing 'results' in tool input"))
    }

    // list_models returns Ok(vec![]) — UI falls back to free-text model input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_uses_tool_use() {
        let body = json!({
            "tools": [{"name": "classify"}],
            "tool_choice": {"type": "tool", "name": "classify"}
        });
        assert_eq!(body["tool_choice"]["type"], "tool");
        assert_eq!(body["tools"][0]["name"], "classify");
    }

    #[test]
    fn extracts_results_from_tool_input() {
        let input = json!({"results": [{"txn_id": "t1", "category_id": "cat1", "confidence": 0.95, "rationale": "r"}]});
        let block = ContentBlock { kind: "tool_use".into(), input: input.clone() };
        let resp = AnthropicResp { content: vec![block] };
        let results = resp.content.into_iter().next().unwrap().input
            .get("results").cloned().unwrap();
        assert_eq!(results[0]["txn_id"], "t1");
    }
}
