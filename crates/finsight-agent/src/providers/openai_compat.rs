use crate::reasoning::messages::{
    parse_plan_preamble, AssistantTurn, ChatMessage, ToolCall, ToolDefinition,
};
use crate::{CompletionProvider, TurnUsage};
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
    /// Completion budget per request. 8192 suits the strong "synthesizer" model
    /// (thinking tokens + a large final JSON answer); a fast "router" model used
    /// only to pick the next tool can run far smaller (see [`with_max_tokens`]).
    max_tokens: u32,
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
            max_tokens: 8192,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }

    /// Cap the per-request completion budget — used to build a cheap, fast
    /// tool-selection "router" whose turns only emit a short tool call.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
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

#[derive(Deserialize, Default)]
struct PromptTokensDetails {
    #[serde(default)]
    cached_tokens: u32,
}

#[derive(Deserialize, Default)]
struct OaiUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Deserialize)]
struct OaiRespWithTools {
    choices: Vec<OaiChoiceWithTools>,
    #[serde(default)]
    usage: Option<OaiUsage>,
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
            // Give the model room to finish; without this some OpenRouter routes
            // default to a small completion budget and truncate the response.
            // Reasoning models spend part of the budget on thinking tokens and
            // the final structured-JSON answer is large, so a small cap truncates
            // the JSON mid-object and the parse fails downstream.
            "max_tokens": self.max_tokens,
            "messages": [
                {"role": "system", "content": format!("{system}\n\nReturn valid JSON only. Do not include markdown fences or explanatory text.")},
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
        parse_json_response(&content)
    }

    // list_models returns Ok(vec![]) — UI falls back to free-text model input

    async fn complete_tool_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        Ok(self
            .complete_tool_turn_with_choice(messages, tools, None)
            .await?
            .0)
    }

    async fn complete_tool_turn_forced(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        Ok(self
            .complete_tool_turn_with_choice(messages, tools, Some("required"))
            .await?
            .0)
    }

    async fn complete_final_answer_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<AssistantTurn> {
        // tool_choice: "none" — the model may not call tools this turn, so it
        // must return its final text answer from what it already gathered.
        Ok(self
            .complete_tool_turn_with_choice(messages, tools, Some("none"))
            .await?
            .0)
    }

    // Usage-reporting variants the reasoning loop calls: same requests as above,
    // but they surface the token usage `complete_tool_turn_with_choice` parses.

    async fn complete_tool_turn_with_usage(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<(AssistantTurn, TurnUsage)> {
        self.complete_tool_turn_with_choice(messages, tools, None)
            .await
    }

    async fn complete_tool_turn_forced_with_usage(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<(AssistantTurn, TurnUsage)> {
        self.complete_tool_turn_with_choice(messages, tools, Some("required"))
            .await
    }

    async fn complete_final_answer_turn_with_usage(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<(AssistantTurn, TurnUsage)> {
        self.complete_tool_turn_with_choice(messages, tools, Some("none"))
            .await
    }
}

impl OpenAiCompatProvider {
    async fn complete_tool_turn_with_choice(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<(AssistantTurn, TurnUsage)> {
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

        let mut body = json!({
            "model": self.model,
            // See complete_json: reasoning models need headroom for thinking
            // tokens plus the large final structured-JSON answer. A fast router
            // instance overrides this down (with_max_tokens) since its turns only
            // emit a short tool call.
            "max_tokens": self.max_tokens,
            // Prompt caching: the large system prefix + tool schemas are re-sent
            // every turn, so caching the stable prefix cuts per-turn latency and
            // cost dramatically. Automatic on OpenAI/Gemini 2.5/DeepSeek/Grok
            // (this flag is a harmless no-op there); explicit for Anthropic/Qwen,
            // which honour the ephemeral breakpoint. GLM does not cache — pick a
            // caching-capable model (e.g. google/gemini-2.5-flash, deepseek) to
            // realise the win. See openrouter.ai/docs/.../prompt-caching.
            "cache_control": {"type": "ephemeral"},
            "messages": oai_messages,
            "tools": oai_tools,
        });
        if let Some(choice) = tool_choice {
            body["tool_choice"] = json!(choice);
        }

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

        // Surface cache hits (visible in the eval harness + debug builds). Full
        // UI telemetry (a cached-tokens usage chip) is a bounded follow-up.
        #[cfg(debug_assertions)]
        if let Some(cached) = resp
            .usage
            .as_ref()
            .and_then(|u| u.prompt_tokens_details.as_ref())
            .map(|d| d.cached_tokens)
            .filter(|c| *c > 0)
        {
            let prompt = resp.usage.as_ref().map(|u| u.prompt_tokens).unwrap_or(0);
            eprintln!("copilot cache: {cached}/{prompt} prompt tokens cached ({})", self.model);
        }

        // Token usage for this turn (cache hits included). Threaded back to the
        // reasoning loop, which sums it across the run for the UI's cache chip.
        let usage = resp
            .usage
            .as_ref()
            .map(|u| TurnUsage {
                prompt_tokens: u.prompt_tokens,
                cached_tokens: u
                    .prompt_tokens_details
                    .as_ref()
                    .map(|d| d.cached_tokens)
                    .unwrap_or(0),
            })
            .unwrap_or_default();

        let choice = resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no choices"))?;
        let msg = choice.message;

        if let Some(tool_calls) = msg.tool_calls {
            if !tool_calls.is_empty() {
                let plan = msg.content.as_deref().and_then(parse_plan_preamble);
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
                return Ok((AssistantTurn::ToolCalls { calls, plan }, usage));
            }
        }

        let content = msg.content.unwrap_or_default();
        Ok((
            AssistantTurn::FinalAnswer {
                content,
                reasoning: "".to_string(),
            },
            usage,
        ))
    }
}

fn parse_json_response(content: &str) -> Result<Value> {
    let trimmed = content.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }
    let Some(start) = trimmed.find(|c| c == '{' || c == '[') else {
        return Err(anyhow!("OpenAI response did not contain JSON"));
    };
    let end_obj = trimmed.rfind('}');
    let end_arr = trimmed.rfind(']');
    let end = match (end_obj, end_arr) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return Err(anyhow!("OpenAI response did not contain complete JSON")),
    };
    if end < start {
        return Err(anyhow!("OpenAI response JSON bounds were invalid"));
    }
    serde_json::from_str(&trimmed[start..=end])
        .map_err(|e| anyhow!("OpenAI response not valid JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cached_tokens_from_openrouter_usage() {
        // The prompt-caching telemetry: cache hits arrive under
        // usage.prompt_tokens_details.cached_tokens.
        let resp: OaiRespWithTools = serde_json::from_str(
            r#"{"choices":[{"message":{"content":"hi"}}],
                "usage":{"prompt_tokens":5000,"prompt_tokens_details":{"cached_tokens":4800}}}"#,
        )
        .unwrap();
        let cached = resp
            .usage
            .and_then(|u| u.prompt_tokens_details)
            .map(|d| d.cached_tokens)
            .unwrap_or(0);
        assert_eq!(cached, 4800);
    }

    #[test]
    fn tolerates_a_response_with_no_usage() {
        let resp: OaiRespWithTools =
            serde_json::from_str(r#"{"choices":[{"message":{"content":"hi"}}]}"#).unwrap();
        assert!(resp.usage.is_none());
    }

    #[test]
    fn parses_json_array_response() {
        let value = parse_json_response(r#"[{"txn_id":"t1"}]"#).unwrap();
        assert_eq!(value[0]["txn_id"], "t1");
    }

    #[test]
    fn parses_json_object_inside_text() {
        let value = parse_json_response("Result:\n{\"mode\":\"deep\"}").unwrap();
        assert_eq!(value["mode"], "deep");
    }

    #[test]
    fn rejects_malformed_model_output() {
        let no_json = parse_json_response("I cannot produce JSON for this request").unwrap_err();
        assert!(no_json
            .to_string()
            .contains("OpenAI response did not contain JSON"));

        let incomplete =
            parse_json_response(r#"Here is partial JSON: {"mode": "deep""#).unwrap_err();
        assert!(incomplete
            .to_string()
            .contains("OpenAI response did not contain complete JSON"));

        let invalid = parse_json_response(r#"Here is malformed JSON: {"mode": }"#).unwrap_err();
        assert!(invalid
            .to_string()
            .contains("OpenAI response not valid JSON"));
    }
}
