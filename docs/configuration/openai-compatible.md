# OpenAI-Compatible Providers

Use OpenAI or any gateway that speaks the OpenAI API.

## Supported

- OpenAI (`api.openai.com`)
- Azure OpenAI (deployment URL)
- Self-hosted compatibles: vLLM, LocalAI, Together, custom proxies that expose `/v1/chat/completions`

FinSight’s `CompletionProvider` trait (`crates/finsight-providers`) abstracts the HTTP layer so the planner does not know which is behind it.

## Setup

1. In **Settings → Agent**, choose **OpenAI-Compatible**.
2. Fill base URL (e.g. `https://api.openai.com/v1`), model (e.g. `gpt-4o-mini`), and API key.
3. Save. The key is stored per-user inside your encrypted `data.sqlcipher` — not a global server slot.

Auto-categorization uses the same provider and sends only the merchant + amount of uncategorized rows; the Copilot sends the question-scoped `FinancialContext`. See [Privacy](/copilot/privacy).

## Verifying

Same as Ollama — ask the Copilot a grounded question and look for a cited total. On failure, the server logs show the HTTP status and the Copilot surface renders the provider error, not a silent miss.

See also: [Anthropic](/configuration/anthropic), [Settings](/configuration/settings).
