# Copilot — Privacy

The Copilot is useful only because it can see your money. Here is exactly what it sees and when.

## What the Copilot receives

At each Copilot question, `finsight-agent::context::build_context` assembles a **FinancialContext** scoped to the question:

- Cashflow (balances, run rate), Budget (envelopes, To Budget), Goals (targets, progress), windowed transactions, wellness metrics, currency and unconverted holdings, and investment positions as imported.

No more is sent than the task needs. Auto-categorization sends even less — only the merchant description and amount of *uncategorized* transactions.

## Provider choice matters

| Provider | Where inference runs | Data leaves your server? |
|---|---|---|
| **Ollama** (local) | Your machine / LAN | No — same context over your local network |
| **OpenAI-compatible** | Remote API you configured | Yes — the context for that question |
| **Anthropic** | Remote API you configured | Yes — the context for that question |

- Provider API keys and SimpleFIN URLs are stored **per-user inside your encrypted `data.sqlcipher`**, not in a global server slot.
- No provider is configured by default. Settings → Agent shows the data-flow explanation before you save.
- Change or remove the provider at any time; local ledger data is untouched.

## What the Copilot never does

- Store your ledger on a vendor’s servers.
- Invent exchange rates for unconverted currencies — the `CurrencyContext` block forces disclosure.
- Remember across users. Households share a server image but not a database — each user’s context is isolated.

## Offline cache

The browser’s seven-day IndexedDB cache is **read-only, encrypted when the PWA has a secure context, and purged on logout**. It is not the vault — `data.sqlcipher` is.

Questions about the overall encryption design belong on [Privacy & Local Data](/getting-started/privacy) and [Security & Privacy](/help/security).
