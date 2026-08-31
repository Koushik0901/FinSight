# Rust Crates

## finsight-core

- **DB** (`db.rs`) — r2d2 pool over SQLCipher, per-user connection.
- **Migrations** (`migrations/V00N__*.sql`, Refinery) — discovered via `embed_migrations!`, currently through V016.
- **Models** (`models/*`) — `Transaction`, `Category`, `Account`, `Goal`, `BudgetEnvelope`, `ReportWidget`, etc.
- **Repos** (`repos/*`) — SQL lives here (e.g. `transactions.rs`, `budgets.rs`, `goals.rs`, `agent_memory.rs`).
- **Metrics / Forecast / Cashflow** (`metrics.rs`, `forecast.rs`, `cashflow.rs`) — deterministic finance math shared by every screen and the Copilot.
- **Categorize / Anomaly / Recurring** (`categorize.rs`, `anomaly.rs`, `recurring.rs`, `subscriptions.rs`) — deterministic classifiers and detectors.
- **Currency** (`currency.rs`) — unconverted holdings bucket; no invented rates.
- **Crypto** (`crypto.rs` in `finsight-server`, not core) — but core owns the DB file that wraps keys describe.

## finsight-providers

CSV parsers + `CompletionProvider` trait (`providers/openai_compat.rs`, `providers/ollama.rs`, `providers/anthropic.rs`). Each provider is a thin HTTP client; the trait lets `finsight-agent` not know which is configured.

## finsight-agent

- `context.rs` — builds `FinancialContext` + `WellnessContext` for every Copilot call.
- `planner.rs` — `build_system_prompt`, `plan`, `persist_plan` (`ACTION_KINDS` allow-list).
- `executor.rs` — runs the plan via `ApiState`.
- `categorizer.rs` / `anomaly.rs` / `recipe_runner.rs` / `navigation.rs` — agent subsystems.
- `providers/*` + `reasoning/*` — LLM streaming, reasoning blocks, and typed gen-UI bindings.

## finsight-api

Transport-agnostic handlers + `ApiState`. Each handler is a pure function over the database; it does not know HTTP vs tests.

## finsight-openapi

Typed OpenAPI spec via utoipa (`COMMANDS` + `build_openapi()`). Source of truth for `openapi.json` generation.

## finsight-server

Axum server: auth (cookies, rotation, throttling, sessions), routing, CSV uploads, SSE, static UI, per-user runtime eviction. Owns `users.db` and `session.key` on disk.

See also: [API & RPC](/developers/api), [OpenAPI Bindings](/developers/bindings).
