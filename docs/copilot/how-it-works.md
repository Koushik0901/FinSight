# How the Copilot Works

A request traverses four stages, each observable in code.

## 1. Build context

`finsight-agent::context::build_context` queries your encrypted database and assembles:

- `CashflowContext` — balances, recent income/expenses, run rate
- `BudgetContext` — this month’s envelopes, To Budget, carryover
- `GoalContextItem[]` — targets, progress, quick-fill eligibility
- `TransactionContext` — windowed actuals the question touches
- `MemoryItem[]` — stored, reviewable memories
- `WellnessContext` — emergency-fund months (+ reliability), snowball order, allocation, 10/20/30-year projection
- `CurrencyContext` — primary currency and explicitly unconverted holdings
- `InvestmentAccountContext[]` — positions only as imported; no price discovery

The wellness block is the only place the six financial principles become numbers. Everything downstream reads that block.

## 2. Build the system prompt

`planner.rs::build_system_prompt` embeds:

- The **Financial Freedom Framework** verbatim (Pay Yourself First, Conscious Spending, Emergency Fund, Debt Snowball, Compound Growth, Behaviour over math, Journey).
- Tool allow-list (`ACTION_KINDS`), navigation guardrails, and the “do not invent totals when unconverted holdings exist” rule.
- Provider and model identity for disclosure.

## 3. Call the provider

`CompletionProvider` trait (`crates/finsight-agent/src/providers`) abstracts Ollama, OpenAI-compatible, and Anthropic over HTTP. The request sends the system prompt + context + user question. Streaming responses flow back as reasoning deltas, tool-call parts, and text.

Provider keys live per-user in `data.sqlcipher`; the server never uses a global key. See [Configuration](/configuration/settings).

## 4. Plan, execute, summarize

1. **Plan** — `planner::plan` parses the LLM JSON into a `PlanResult` (title, steps, tool calls) and persists it.
2. **Execute** — `executor` runs each step via `ApiState` handlers; finance tools run against `finsight-core` and return typed results.
3. **Summarize** — the agent re-calls the provider with tool results to narrate the outcome, citing figures and caveats.

Post-execution the Copilot shows an action bundle with per-step status and undo guidance where applicable. Raw reasoning blocks are collapsible; everything streams and can be cancelled.

## Deterministic finance

Tools like `forecast`, `cashflow`, and month-close are **not** LLM math — they are Rust functions in `finsight-core`. The model writes the story; the ledger writes the numbers.

Next: [Plans & Actions](/copilot/plans).
