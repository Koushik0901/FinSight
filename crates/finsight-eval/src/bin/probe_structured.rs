//! Phase-B go/kill probe for structured outputs (see
//! docs/superpowers/plans/2026-07-15-robust-copilot-blocks.md, Task B1).
//!
//! Issues ONE real chat/completions per model with a strict `json_schema`
//! `response_format` AND a non-empty `tools` array present (mirroring the
//! Copilot final-answer turn), and reports whether the request was accepted and
//! whether the returned content conforms. The result decides whether Phase B2
//! (schemars-generated schema + response_format) is worth building.
//!
//! Run: `cargo run -p finsight-eval --bin probe_structured`

use anyhow::{anyhow, Result};
use finsight_core::keychain;
use serde_json::{json, Value};

fn resolve_key() -> Result<String> {
    if let Ok(k) = std::env::var("OPENROUTER_API_KEY") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Ok(k);
        }
    }
    if let Ok(Some(k)) = keychain::get_key("com.finsight.llm", "openrouter") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Ok(k);
        }
    }
    Err(anyhow!("No OpenRouter key (env OPENROUTER_API_KEY or keychain)."))
}

fn schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["answer", "response_blocks"],
        "properties": {
            "answer": { "type": "string" },
            "response_blocks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["kind"],
                    "properties": { "kind": { "type": "string" } }
                }
            }
        }
    })
}

async fn probe(client: &reqwest::Client, key: &str, model: &str) -> (bool, String) {
    let body = json!({
        "model": model,
        "max_tokens": 512,
        "messages": [
            {"role": "system", "content": "You are a finance assistant. Reply with the answer object."},
            {"role": "user", "content": "List my accounts. Emit response_blocks with one {kind:'accountsOverview'} entry."}
        ],
        // tools PRESENT with tool_choice:none — exactly the final-answer turn shape.
        "tools": [{
            "type": "function",
            "function": {"name": "noop", "description": "unused", "parameters": {"type":"object","properties":{}}}
        }],
        "tool_choice": "none",
        "response_format": {
            "type": "json_schema",
            "json_schema": { "name": "answer", "strict": true, "schema": schema() }
        }
    });
    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .bearer_auth(key)
        .json(&body)
        .send()
        .await;
    let resp = match resp {
        Ok(r) => r,
        Err(e) => return (false, format!("request error: {e}")),
    };
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return (false, format!("HTTP {status}: {}", text.chars().take(300).collect::<String>()));
    }
    // Accepted. Did the content conform to the schema shape?
    let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let content = parsed["choices"][0]["message"]["content"].as_str().unwrap_or("");
    let conforms = serde_json::from_str::<Value>(content)
        .ok()
        .map(|v| v.get("answer").is_some() && v.get("response_blocks").is_some())
        .unwrap_or(false);
    (
        true,
        format!("HTTP {status} · content conforms to schema: {conforms} · content head: {}", content.chars().take(160).collect::<String>()),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let key = resolve_key()?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    for model in ["z-ai/glm-5.2:exacto", "google/gemma-4-31b-it"] {
        let (accepted, detail) = probe(&client, &key, model).await;
        println!("[{}] accepted={accepted} · {detail}", model);
    }
    Ok(())
}
