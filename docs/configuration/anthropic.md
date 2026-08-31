# Anthropic

Claude models via the Anthropic API, native—not via a compat shim.

## Setup

1. In **Settings → Agent**, choose **Anthropic**.
2. Fill model (e.g. `claude-3-5-sonnet`) and API key.
3. Save. The key lives per-user in `data.sqlcipher`.

## What is sent

Same as other remote providers: auto-categorization → merchant + amount of uncategorized rows; Copilot → question-scoped `FinancialContext` from `build_context`. Change or remove the key at any time; local data is untouched.

## When to choose Anthropic

When you already have an Anthropic key and prefer Claude for reasoning-heavy planning and scenario narratives. Provider choice does not affect deterministic finance math (`finsight-core::forecast`) — only the narration around it.

See also: [Ollama](/configuration/ollama), [OpenAI-Compatible](/configuration/openai-compatible).
