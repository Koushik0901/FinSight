use crate::reasoning::messages::{
    parse_plan_preamble, AssistantTurn, ChatMessage, ToolCall, ToolDefinition,
};
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
struct AnthropicResp {
    content: Vec<AnthropicContentBlock>,
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
            "max_tokens": 8192,
            "system": format!("{system}\n\nReturn valid JSON only. Do not include markdown fences or explanatory text."),
            "messages": [{"role": "user", "content": user}]
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

        let text = resp
            .content
            .into_iter()
            .filter_map(|block| {
                if block.kind == "text" {
                    block.text
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        if text.trim().is_empty() {
            return Err(anyhow!("empty text content from Anthropic"));
        }
        parse_json_response(&text)
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
                ChatMessage::Assistant {
                    content,
                    tool_calls,
                } => {
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
                ChatMessage::Tool {
                    tool_call_id,
                    content,
                } => {
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
            "max_tokens": 8192,
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
            let plan = parse_plan_preamble(&text_parts.join("\n"));
            Ok(AssistantTurn::ToolCalls {
                calls: tool_calls,
                plan,
            })
        } else {
            Ok(AssistantTurn::FinalAnswer {
                content: text_parts.join("\n"),
                reasoning: String::new(),
            })
        }
    }

    // list_models returns Ok(vec![]) — UI falls back to free-text model input
}

fn parse_json_response(content: &str) -> Result<Value> {
    let trimmed = content.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }

    let Some(start) = trimmed.find(|c| c == '{' || c == '[') else {
        return Err(anyhow!("Anthropic response did not contain JSON"));
    };
    let end_obj = trimmed.rfind('}');
    let end_arr = trimmed.rfind(']');
    let end = match (end_obj, end_arr) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return Err(anyhow!("Anthropic response did not contain complete JSON")),
    };
    if end < start {
        return Err(anyhow!("Anthropic response JSON bounds were invalid"));
    }
    serde_json::from_str(&trimmed[start..=end])
        .map_err(|e| anyhow!("Anthropic response not valid JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json_array() {
        let value = parse_json_response(
            r#"[{"txn_id":"t1","category_id":"cat1","confidence":0.95,"rationale":"r"}]"#,
        )
        .unwrap();
        assert_eq!(value[0]["txn_id"], "t1");
    }

    #[test]
    fn parses_json_inside_text() {
        let value = parse_json_response("Here is the result:\n{\"mode\":\"deep\"}").unwrap();
        assert_eq!(value["mode"], "deep");
    }

    #[test]
    fn rejects_malformed_model_output() {
        let no_json = parse_json_response("I cannot produce JSON for this request").unwrap_err();
        assert!(no_json
            .to_string()
            .contains("Anthropic response did not contain JSON"));

        let incomplete =
            parse_json_response(r#"Here is partial JSON: {"mode": "deep""#).unwrap_err();
        assert!(incomplete
            .to_string()
            .contains("Anthropic response did not contain complete JSON"));

        let invalid = parse_json_response(r#"Here is malformed JSON: {"mode": }"#).unwrap_err();
        assert!(invalid
            .to_string()
            .contains("Anthropic response not valid JSON"));
    }
}
