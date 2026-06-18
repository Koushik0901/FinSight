use crate::reasoning::messages::{AssistantTurn, ChatMessage, ToolCall, ToolDefinition};
use crate::CompletionProvider;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

/// Covers OpenAI, OpenRouter, Google (v1beta/openai), Mistral, Groq,
/// and any other OpenAI-compatible chat completions endpoint.
pub struct OpenAiCompatProvider {
    base_url: String,
    api_key: String,
    model: String,
    preset: String,
    client: reqwest::Client,
}

impl OpenAiCompatProvider {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        preset: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            preset: preset.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }
}

#[derive(Deserialize)]
struct Choice {
    message: OaiMessage,
}
#[derive(Deserialize)]
struct OaiMessage {
    content: String,
}
#[derive(Deserialize)]
struct OaiResp {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct OaiToolCall {
    id: String,
    function: OaiFunction,
}

#[derive(Deserialize)]
struct OaiFunction {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OaiMessageWithTools {
    content: Option<String>,
    tool_calls: Option<Vec<OaiToolCall>>,
}

#[derive(Deserialize)]
struct OaiChoiceWithTools {
    message: OaiMessageWithTools,
}

#[derive(Deserialize)]
struct OaiRespWithTools {
    choices: Vec<OaiChoiceWithTools>,
}

#[async_trait]
impl CompletionProvider for OpenAiCompatProvider {
    fn provider_id(&self) -> &str {
        &self.preset
    }
    fn model_id(&self) -> &str {
        &self.model
    }

    async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "response_format": { "type": "json_object" },
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user},
            ]
        });
        let resp: OaiResp = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let content = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no choices in response"))?
            .message
            .content;
        serde_json::from_str(&content).map_err(|e| anyhow!("OpenAI response not valid JSON: {e}"))
    }

    // list_models returns Ok(vec![]) — UI falls back to free-text model input

    async fn complete_tool_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        let oai_messages: Vec<Value> = messages
            .iter()
            .map(|m| match m {
                ChatMessage::System { content } => {
                    json!({"role": "system", "content": content})
                }
                ChatMessage::User { content } => {
                    json!({"role": "user", "content": content})
                }
                ChatMessage::Assistant { content, tool_calls } => {
                    let mut msg = json!({"role": "assistant"});
                    if let Some(c) = content {
                        msg["content"] = json!(c);
                    }
                    if !tool_calls.is_empty() {
                        msg["tool_calls"] = json!(tool_calls.iter().map(|tc| {
                            json!({"id": tc.id, "type": "function", "function": {"name": tc.name, "arguments": tc.arguments.to_string()}})
                        }).collect::<Vec<_>>());
                    }
                    msg
                }
                ChatMessage::Tool { tool_call_id, content } => {
                    json!({"role": "tool", "tool_call_id": tool_call_id, "content": content})
                }
            })
            .collect();

        let oai_tools: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({"type": "function", "function": {"name": t.name, "description": t.description, "parameters": t.parameters}})
            })
            .collect();

        let body = json!({
            "model": self.model,
            "messages": oai_messages,
            "tools": oai_tools,
        });

        let resp: OaiRespWithTools = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let choice = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no choices"))?;
        let msg = choice.message;

        if let Some(tool_calls) = msg.tool_calls {
            if !tool_calls.is_empty() {
                let calls: Vec<ToolCall> = tool_calls
                    .into_iter()
                    .map(|tc| {
                        let args: Value =
                            serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));
                        ToolCall {
                            id: tc.id,
                            name: tc.function.name,
                            arguments: args,
                        }
                    })
                    .collect();
                return Ok(AssistantTurn::ToolCalls(calls));
            }
        }

        let content = msg.content.unwrap_or_default();
        Ok(AssistantTurn::FinalAnswer {
            content,
            reasoning: "".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_body_has_json_response_format() {
        let body = json!({
            "model": "gpt-4o-mini",
            "response_format": { "type": "json_object" },
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user",   "content": "usr"},
            ]
        });
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["messages"][1]["role"], "user");
    }
}
