# Introduction

FinSight is a private, self-hosted financial copilot — a quiet way to understand, plan, and master your money.

It combines encrypted personal-finance storage, account and transaction management, budgeting, goals, reporting, and an AI-assisted Copilot. The primary deployment is a server you operate yourself, accessed from a browser or an installable PWA.

## Why FinSight

Most finance apps ask you to hand your ledger to a vendor. FinSight inverts that: **your server is the source of truth**. A single Docker image, a single `/data` volume, and your finances stay on hardware you control.

- **Local-first, encrypted.** One SQLCipher database per user. The `users.db` registry holds only usernames, Argon2id verifiers, and wrapped keys — no financial records.
- **You choose the AI.** Use Ollama to keep inference on your infrastructure, or opt into a remote provider (OpenAI-compatible, Anthropic). Data leaves your server only when you configure and trigger it.
- **Behaviour over charts.** The Copilot applies proven principles — Pay Yourself First, Conscious Spending, Emergency Fund, Debt Snowball, Compound Growth — to your actual numbers and turns them into next steps.
- **Self-hosting without ceremony.** `docker compose up -d` is the whole install. Tailscale is the recommended path to HTTPS and PWA.

## At a glance

| Layer | What lives there |
|---|---|
| `finsight-server` | Serves UI, authenticated RPC API, CSV uploads, SSE events, `GET /api/openapi.json` |
| Browser / PWA | Same-origin HTTP/SSE client; seven-day, read-only IndexedDB cache for offline viewing (purged on logout) |
| `/data` on your server | `users.db`, `session.key`, `users/<uuid>/data.sqlcipher` per user |

## What you can do

- Track net worth, income, and expenses from daily transactions
- Plan with envelope budgets, goals, and cash-flow forecasting
- Run what-if scenarios in natural language
- Let rules and recipes handle repetition — pattern rules you can read
- Ask the Copilot “what should I fix this month?” and get grounded answers with tool-cited figures

Next: [What is FinSight?](/getting-started/what-is-finsight) — the product in one page.
