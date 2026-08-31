# Copilot — Overview

The Copilot is a conversational, goal-aware planner that reads your actual ledger and turns it into next steps — with figures you can verify and actions you approve.

It is **not** a generic chatbot. Every answer is grounded in `finsight-agent::context::FinancialContext` built from your encrypted database at request time: cashflow, budget, goals, transactions in scope, memories, and wellness metrics.

## What it does

- **Q&A** — “How much did I spend on dining last month?” — answers with cited totals and the query that produced them.
- **Planning** — “Help me save $10k by December.” — drafts a plan whose tool calls you review before they run.
- **Nudges** — the Today next-action and anomaly callouts point you to the Copilot with a prefilled, groundsable prompt.
- **Scenarios & recipes** — natural language maps to deterministic finance tools; recipes propose multi-step automations you confirm.

## What it does not do

- Move money, call banks, or execute silently. Every actionable step is shown as a plan and waits for you.
- Invent totals. The system prompt forbids presenting a single-currency subtotal as your whole position when unconverted holdings exist — and the prompt’s currency block will warn the model not to.
- Remember across users. Memories and provider keys are per-user; households share a server but not a database.

## Surface

The Copilot lives at **/copilot**. The composer streams over SSE (`GET /api/events`), renders streamed reasoning blocks as a collapsible `<details>`, and surfaces typed generative-UI cards (metrics, charts, tables) via `Streamdown`.

Next: [How the Copilot Works](/copilot/how-it-works).
