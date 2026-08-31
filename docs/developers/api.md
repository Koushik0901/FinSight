# API & RPC

FinSight exposes a typed RPC surface described by an OpenAPI spec that is generated at build time, not hand-written.

## Routes

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/api/rpc/{cmd}` | Authenticated RPC; body is camelCase DTO |
| `GET` | `/api/events` | SSE stream for Copilot + query invalidations |
| `GET` | `/api/openapi.json` | Generated spec |
| `POST` | `/api/uploads/*` | Authenticated CSV staging |
| `*` | `/` | Serves `ui/dist` (SPA fallback) |

Every RPC response is enveloped as `Result<T, AppError>`; `401` surfaces as `FINSIGHT_AUTH_REQUIRED` on the client.

## Adding or changing a command

1. Implement `pub async fn my_cmd(state: &ApiState, ...)` in `crates/finsight-api/src/commands/`.
2. Add `#[utoipa::path]` + `ToSchema` on handler/DTOs; keep `crates/finsight-openapi/src/lib.rs::COMMANDS` sorted and identical to `crates/finsight-server/src/dispatch.rs::SUPPORTED`.
3. Add the dispatcher arm in `dispatch.rs` via `rpc_routes!(api, events, cmd, p, c: …)` — use `arg(&p, "camelCase")`.
4. Run `pnpm openapi` and verify:

```bash
cargo test -p finsight-server --test parity
cargo test -p finsight-openapi
```

5. If the DTO shape changed, `pnpm typecheck` fails until hooks in `ui/src/api/hooks` are updated.

## Client

Generated contract: `ui/src/api/openapi.ts` (from `openapi.json` via `openapi-typescript`).

Typed transport: `ui/src/api/openapiClient.ts` (`openapi-fetch`) — every hook imports `api` from there. No second transport exists. See [OpenAPI Bindings](/developers/bindings).

Next: [Frontend](/developers/frontend).
