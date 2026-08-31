# OpenAPI Bindings

The TypeScript contract is generated — never hand-written.

## Chain

```text
Rust  →  COMMANDS (utoipa)  →  build_openapi()  →  openapi.json  →  openapi.ts (openapi-typescript)  →  openapiClient.ts (openapi-fetch)
```

- `crates/finsight-openapi/src/lib.rs` — `COMMANDS` (sorted) + `build_openapi()`.
- `crates/finsight-openapi/bin/export_openapi.rs` — writes `openapi.json` at repo root and in `ui/src/api/`.
- `ui/package.json` — `"openapi:gen": "openapi-typescript ../openapi.json -o src/api/openapi.ts"`.
- `ui/src/api/openapiClient.ts` — `createClient` over `POST /api/rpc/{cmd}` + SSE, unwraps `Result<T,AppError>`, maps 401 → `FINSIGHT_AUTH_REQUIRED`.

Every hook imports `api` from `openapiClient.ts`. Raw `fetch` is not used for RPC.

## After changing the API

```bash
pnpm openapi
cargo test -p finsight-server --test parity
cargo test -p finsight-openapi
pnpm --filter ui typecheck
```

If `COMMANDS` and `dispatch.rs::SUPPORTED` diverge, the parity test fails. If a DTO shape changed, typecheck fails until hooks are updated.

## Field naming

`Transaction` uses **snake_case** on the wire (its Rust struct lacks `rename_all`): `t.merchant_raw`, `t.posted_at`, `t.amount_cents`. Most other types (e.g. `BudgetEnvelope`, `CategoryWithSpending`, `TxnFilterInput`) use **camelCase** via `#[serde(rename_all = "camelCase")]`. Check `bindings.ts` (historical) or the generated `openapi.ts` when touching a new type.

See also: [API & RPC](/developers/api), [Frontend](/developers/frontend).
