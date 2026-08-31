# Architecture

FinSight is a single server binary serving an SPA over same-origin HTTP/SSE. No Tauri binary remains — `finsight-server` is the product.

## High-level

```text
Browser / PWA
      │
 HTTP RPC + CSV upload + SSE (+ GET /api/openapi.json)
      │
 crates/finsight-server  (Axum, serves ui/dist)
      │
 crates/finsight-api  (transport-agnostic handlers + ApiState)
      │
 ┌────────────┼──────────────┐
 finsight-core  finsight-agent  finsight-providers
```

## Crates

| Crate | Role | Depends on Tauri? |
|---|---|---|
| `finsight-core` | Domain: models, SQLCipher pool (r2d2), migrations (Refinery), repos, metrics/forecast/cashflow, settings KV | No |
| `finsight-providers` | CSV parsers, LLM providers (`CompletionProvider` with Ollama / OpenAI-compat / Anthropic) | No |
| `finsight-agent` | Copilot context (`build_context`), planner, executor, recipe runner, categorizer, anomalies | No |
| `finsight-api` | Transport-agnostic command bodies + `ApiState` | No |
| `finsight-openapi` | Typed OpenAPI spec (utoipa) — `COMMANDS` + `build_openapi()` | No |
| `finsight-server` | Axum auth/RPC/uploads/SSE/static UI/user runtimes | No |
| `finsight-eval` | Evaluation fixtures & runners | — |

## Data layout

```text
/data                  # Docker volume; dev ./data
├── users.db            # registry: usernames, Argon2id verifiers, wrapped keys, hashed sessions
├── session.key         # wraps persisted sessions
└── users/<uuid>/
    ├── data.sqlcipher   # per-user encrypted ledger
    ├── backups/
    └── imports/
```

Per-user runtimes are lazy, single-flighted, and evicted after 30 minutes idle (unless SSE attached). Sessions have a sliding 30-day lifetime and survive restarts.

## Transport

- `POST /api/rpc/{cmd}` — typed RPC, envelope `Result<T,AppError>`, camelCase args via `arg(&p, "camelCase")`
- `GET /api/events` — SSE for Copilot streaming and invalidations
- `GET /api/openapi.json` — generated spec at runtime
- `ui/dist` — served by `finsight-server` at `/` (SPA fallback)

All DB access is through repos in `finsight-core`, not inline SQL in handlers.

Next: [Development Setup](/developers/setup).
