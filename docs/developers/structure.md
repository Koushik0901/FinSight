# Repository Structure

```text
FinSight/
├── crates/
│   ├── finsight-core/       # SQLCipher DB, migrations, models, repos
│   ├── finsight-providers/  # CSV parsers and LLM HTTP providers
│   ├── finsight-agent/      # Copilot, finance tools, categorizer, anomalies, recipes
│   ├── finsight-api/        # Transport-agnostic command bodies and ApiState
│   ├── finsight-openapi/    # Typed OpenAPI spec (utoipa) — COMMANDS + build_openapi()
│   ├── finsight-server/     # Axum auth, RPC, uploads, SSE, static UI, user runtimes
│   └── finsight-eval/       # Evaluation fixtures and runners
├── deploy/
│   ├── compose.split.yaml.example  # optional web (nginx) + api split
│   └── docker/nginx.conf
├── docs/                    # VitePress site (this site) + self-hosting guide
│   ├── .vitepress/
│   └── public/
└── ui/
    └── src/
        ├── api/             # Generated openapi.ts + openapiClient, auth, query hooks
        ├── components/      # Shared UI, auth/offline gates, Copilot renderers
        ├── pwa/             # IndexedDB persistence and online state
        ├── screens/         # Product, server-auth, admin screens
        └── styles/          # Design tokens and component styles
```

## ui/src layout

| Path | Notes |
|---|---|
| `api/openapi.ts` + `api/openapi.json` | Generated; never edit manually |
| `api/openapiClient.ts` | `openapi-fetch` transport over `POST /api/rpc/{cmd}` + `GET /api/events` |
| `api/hooks/` | tanstack-query wrappers (e.g. `useTransactions`, `useBudget`) |
| `api/prefetch.ts` + `api/invalidation.ts` | Anticipatory prefetch on hover; invalidation map |
| `components/` | Reusable UI: Drawer, CommandPalette, copilot renderers, etc. |
| `screens/` | One file per screen, consumes hooks |
| `styles/tokens.css` | Design tokens; $& — never hardcode colors outside |
| `styles/app.css` | Utility classes (`.card`, `.chip`, `.btn`, `.tbl`, `.stat`, `.eyebrow`) |

## Key invariants

- Frontend imports the generated client from `ui/src/api/openapiClient.ts`, never raw `fetch`.
- Migrations live in `crates/finsight-core/migrations/` as `V00N__description.sql` (Refinery, auto-discovered).
- Amounts that respect privacy mode use class `"money"`.

Next: [API & RPC](/developers/api).
