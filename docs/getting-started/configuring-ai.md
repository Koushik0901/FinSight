# Configuring AI

AI in FinSight is **opt-in and per-user**. Two features use it: auto-categorization and the Copilot. Everything else — ledger, budgets, goals, reports — is deterministic finance math in `finsight-core`.

Without a provider, FinSight still works: categorization falls back to rules + deterministic heuristics, and the Copilot simply does not run.

## Providers

Configure a provider at **Settings → Agent**. The key is stored inside your encrypted per-user database, not a process-global keychain.

<ProviderCard title="Ollama" subtitle="Local inference — stays on your hardware" icon="cpu" badge="Private" description="Run a local model (e.g. Qwen, Llama) via the Ollama HTTP API. FinSight sends the same context it would to a cloud provider, but the bytes never leave your network." :bullets="['No data leaves your machine', 'Good for categorization and Copilot Q&A', 'Configure base URL (default http://ollama:11434)']" />

<ProviderCard title="OpenAI-Compatible" subtitle="Any OpenAI API + compatible gateways" icon="box" description="Works with OpenAI, Azure OpenAI, and self-hosted OpenAI-compatible endpoints (e.g. vLLM, LocalAI). FinSight’s CompletionProvider trait abstracts the HTTP layer." :bullets="['Set base URL, model, and API key', 'Use any compatible gateway you operate']" />

<ProviderCard title="Anthropic" subtitle="Claude models via the Anthropic API" icon="brain" description="Native Anthropic provider for Claude models. Useful when you already have an Anthropic key and want their models for Copilot reasoning." :bullets="['Set model and API key in Settings → Agent', 'Same context controls as other providers']" />

## What is sent

- **Auto-categorization (batch):** the redacted merchant description and amount of *uncategorized* transactions only — enough to classify, not your whole ledger.
- **Copilot:** the relevant financial context for the question — cashflow, budget, goals, transactions in scope, plus stored memories — assembled in `finsight-agent::context::build_context`. See [Copilot → Privacy](/copilot/privacy).

Settings → Agent explains the flow before you save. You can change or remove the provider at any time; existing ledger data is untouched.

## Can I use FinSight without cloud AI?

Yes. Categories can be set manually, rules handle repetition, and Reports/Scenarios are deterministic. Add a provider later and both categorization and Copilot light up with no migration.

## Verifying your setup

1. Save the provider in Settings → Agent.
2. Look for the agent status indicator in the sidebar (last scan time).
3. Ask the Copilot a simple question (“How much did I spend on groceries last month?”) — the Composer streams the response via SSE.

If streaming fails, check the server logs (`docker compose logs -f finsight`) and that `FINSIGHT_PUBLIC_ORIGIN` matches your external HTTPS origin when behind a proxy.

Next: [Privacy & Local Data](/getting-started/privacy).
