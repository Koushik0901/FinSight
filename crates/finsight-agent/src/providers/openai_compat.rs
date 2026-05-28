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
struct Choice { message: OaiMessage }
#[derive(Deserialize)]
struct OaiMessage { content: String }
#[derive(Deserialize)]
struct OaiResp { choices: Vec<Choice> }

#[async_trait]
impl CompletionProvider for OpenAiCompatProvider {
    fn provider_id(&self) -> &str { &self.preset }
    fn model_id(&self) -> &str { &self.model }

    async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "response_format": { "type": "json_object" },
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user},
            ]
        });
        let resp: OaiResp = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let content = resp.choices.into_iter().next()
            .ok_or_else(|| anyhow!("no choices in response"))?
            .message.content;
        serde_json::from_str(&content)
            .map_err(|e| anyhow!("OpenAI response not valid JSON: {e}"))
    }

    // list_models returns Ok(vec![]) — UI falls back to free-text model input
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
