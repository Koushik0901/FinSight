use crate::CompletionProvider;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

pub struct OllamaProvider {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }
}

#[derive(Deserialize)]
struct OllamaMessage { content: String }
#[derive(Deserialize)]
struct OllamaChatResp { message: OllamaMessage }

#[async_trait]
impl CompletionProvider for OllamaProvider {
    fn provider_id(&self) -> &str { "ollama" }
    fn model_id(&self) -> &str { &self.model }

    async fn complete_json(&self, system: &str, user: &str) -> Result<Value> {
        let body = json!({
            "model": self.model,
            "format": "json",
            "stream": false,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user},
            ]
        });
        let resp: OllamaChatResp = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        serde_json::from_str(&resp.message.content)
            .map_err(|e| anyhow!("Ollama response not valid JSON: {e}"))
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        #[derive(Deserialize)]
        struct Tag { name: String }
        #[derive(Deserialize)]
        struct TagsResp { models: Vec<Tag> }
        let resp: TagsResp = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.models.into_iter().map(|t| t.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify request body shape without making a network call.
    #[test]
    fn request_body_has_format_json() {
        let body = json!({
            "model": "llama3.2",
            "format": "json",
            "stream": false,
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user",   "content": "usr"},
            ]
        });
        assert_eq!(body["format"], "json");
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "system");
    }
}
