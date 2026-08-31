# Ollama — Local Inference

Keep inference on infrastructure you control.

## Setup

1. Run Ollama ([ollama.com](https://ollama.com)) on your LAN or on the FinSight host itself (uncomment the `ollama` service in `docker-compose.yml`).
2. Pull a model the Copilot supports (e.g. `qwen2.5`, `llama3`) — the model list shown in Settings → Agent comes from Ollama’s `/api/tags`.
3. In **Settings → Agent**, choose **Ollama**, set the base URL (default `http://ollama:11434` when co-located, or `http://<lan-ip>:11434` otherwise), pick the model, and save.

No API key is needed.

## What stays local

The same `FinancialContext` FinSight would send to a cloud provider is sent to your Ollama instance over your local network. No data leaves your LAN or machine. Auto-categorization sends only the merchant + amount of uncategorized rows, over the same local HTTP.

## Verifying

Ask the Copilot a simple grounded question (“How much did I spend on groceries last month?”). If streaming shows reasoning and a cited total, Ollama is wired. Check `docker compose logs -f finsight` and the Ollama logs for errors if not.

Tip: for the Copilot to be fast enough to be pleasant, prefer a quantized 7–8B model on a machine with enough RAM.

See also: [Configuring AI](/getting-started/configuring-ai), [Privacy](/copilot/privacy).
