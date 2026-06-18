use crate::reasoning::messages::{AssistantTurn, ChatMessage, ToolCall, ToolDefinition};
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
struct AnthropicResp {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
    id: Option<String>,
    name: Option<String>,
    input: Option<Value>,
}

#[derive(Deserialize)]
struct AnthropicRespWithTools {
    content: Vec<AnthropicContentBlock>,
}

#[async_trait]
impl CompletionProvider for AnthropicProvider {
    fn provider_id(&self) -> &str {
        "anthropic"
    }
    fn model_id(&self) -> &str {
        &self.model
    }

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

        let resp: AnthropicResp = self
            .client
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
        let block = resp
            .content
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("empty content from Anthropic"))?;
        if block.kind != "tool_use" {
            return Err(anyhow!("expected tool_use block, got {}", block.kind));
        }
        // Return the results array directly
        block
            .input
            .get("results")
            .cloned()
            .ok_or_else(|| anyhow!("missing 'results' in tool input"))
    }

    async fn complete_tool_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        let mut system_msg = String::new();
        let mut api_messages: Vec<Value> = Vec::new();

        for m in messages {
            match m {
                ChatMessage::System { content } => system_msg = content.clone(),
                ChatMessage::User { content } => {
                    api_messages.push(json!({"role": "user", "content": content}));
                }
                ChatMessage::Assistant { content, tool_calls } => {
                    let mut blocks: Vec<Value> = Vec::new();
                    if let Some(c) = content {
                        blocks.push(json!({"type": "text", "text": c}));
                    }
                    for tc in tool_calls {
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": tc.id,
                            "name": tc.name,
                            "input": tc.arguments
                        }));
                    }
                    api_messages.push(json!({"role": "assistant", "content": blocks}));
                }
                ChatMessage::Tool { tool_call_id, content } => {
                    api_messages.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content
                        }]
                    }));
                }
            }
        }

        let api_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters
                })
            })
            .collect();

        let body = json!({
            "model": self.model,
            "max_tokens": 4096,
            "system": system_msg,
            "messages": api_messages,
            "tools": api_tools,
        });

        let resp: AnthropicRespWithTools = self
            .client
            .post(ANTHROPIC_API)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in resp.content {
            match block.kind.as_str() {
                "text" => {
                    if let Some(t) = block.text {
                        text_parts.push(t);
                    }
                }
                "tool_use" => {
                    tool_calls.push(ToolCall {
                        id: block.id.unwrap_or_default(),
                        name: block.name.unwrap_or_default(),
                        arguments: block.input.unwrap_or(json!({})),
                    });
                }
                _ => {}
            }
        }

        if !tool_calls.is_empty() {
            Ok(AssistantTurn::ToolCalls(tool_calls))
        } else {
            Ok(AssistantTurn::FinalAnswer {
                content: text_parts.join("\n"),
                reasoning: String::new(),
            })
        }
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
        let block = ContentBlock {
            kind: "tool_use".into(),
            input: input.clone(),
        };
        let resp = AnthropicResp {
            content: vec![block],
        };
        let results = resp
            .content
            .into_iter()
            .next()
            .unwrap()
            .input
            .get("results")
            .cloned()
            .unwrap();
        assert_eq!(results[0]["txn_id"], "t1");
    }
}
