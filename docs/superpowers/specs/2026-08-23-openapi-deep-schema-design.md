# OpenAPI Deep Schema (Big-Bang) — Design

**Date:** 2026-08-23
**Status:** Approved design (follow-up to `2026-08-24-pure-selfhost-openapi-design.md`)
**Scope:** Big-bang, delete entirely (no shim, no `@tauri-apps/api`)

## Problem

`2026-08-24-pure-selfhost-openapi` scaffolded `finsight-openapi` with `COMMANDS` + `build_openapi() -> Value` that emits 229 `POST /api/rpc/{cmd}` paths but every `requestBody`/`response` is `schema: {type:"object"}` (shallow). `ui/src/api/openapi.ts` therefore has no real `operations` types, `openapiClient.ts` only has one ergonomic alias (`listAccounts`) + a `rpc<T>(cmd,body)` with `as never`, and `ui/src/api/hooks/*` still import `bindings.ts`/`client.ts` shim (`__TAURI_INTERNALS__` via `httpBackend.ts`). `@tauri-apps/api` remains in `ui/package.json:31` solely for the shim, `crates/finsight-bindings` (tauri-specta) is dead code, and a typo in `arg(&p,"balanceCents")` is only caught by `parity.rs`, not by TS.

Goal: make `openapi.json` the single typed contract, delete the Tauri shim entirely, and keep the pure PWA transport.

## Decisions (settled 2026-08-23)

| Question | Decision |
|---|---|
| Migration | **Big-bang** — all DTOs + handlers + hooks in one PR |
| Tauri dep | **Delete entirely** — remove `@tauri-apps/api`, `bindings.ts`, `client.ts` shim, `httpBackend.ts`, `crates/finsight-bindings` (mock harness rewritten to mock `fetch`) |
| Schema source | `finsight-core` DTOs derive `ToSchema` (utoipa), handlers carry `#[utoipa::path]` — spec is `OpenApi` typed, not `Value` |

## Architecture

```
finsight-core (models/*.rs: ToSchema) ──┐
finsight-api (commands/*: #[utoipa::path]) ─┼─► finsight-openapi (derive(OpenApi) collects paths + components) ─► openapi.json ─► openapi-typescript → ui/src/api/openapi.ts (operations with real types)
                                                                                                      │
                                                                                                      └─► export_openapi codegen → ui/src/api/openapiClient.ts `api` object (229 typed methods) ─► ui/src/api/hooks/* (migrated) ─► fetch POST /api/rpc/{cmd}
```

`GET /api/openapi.json` stays `no-cache` + compressed, `GET /api/events` stays never compressed. `finsight-server` unchanged except `build_openapi()` now returns `OpenApi` (typed) and `dispatch.rs` `SUPPORTED` stays the source for `COMMANDS` parity.

## Components

### S1 — Crate/model layer (finsight-core)

- Add `utoipa = { version="4" }` to `crates/finsight-core/Cargo.toml` (no `axum_extras`, already removed).
- Derive `#[derive(ToSchema, Serialize, Deserialize)]` on every `pub struct`/`enum` in `crates/finsight-core/src/models/*` (24 files: `account.rs`, `transaction.rs`, `category.rs`, `budget.rs`, `manual_asset.rs`, `household.rs`, `planned_transaction.rs`, `recipes.rs`, `holdings.rs`, `net_worth.rs`, `copilot.rs`, etc.). Keep `#[serde(rename_all="camelCase")]` — utoipa respects it. No DB migration.

### S2 — Handler layer (finsight-api)

- Add `utoipa` dep to `crates/finsight-api/Cargo.toml`.
- Annotate each `pub async fn` in `crates/finsight-api/src/commands/*` (~10 files, 229 handlers) with `#[utoipa::path(post, path="/api/rpc/{cmd}", request_body(content=InputDto), responses((status=200, body=OutputDto)))]` referencing the `ToSchema` types. No logic change.

### S3 — Spec generation (finsight-openapi)

- Change `crates/finsight-openapi/src/lib.rs` from `Value`-based `json!` to `#[derive(OpenApi)] #[openapi(paths(...), components(schemas(...)))]` that lists all handler paths + DTO schemas. `build_openapi() -> OpenApi` (typed), `build_openapi_value() -> Value` via `to_value` for file write. `export_openapi.rs` writes `openapi.json` + `ui/src/api/openapi.json`. `pnpm openapi:gen` (`openapi-typescript`) now generates real `operations` with `requestBody`/`responses` types. Add `cargo test` that asserts every `operationId` has a non-shallow schema (no `type: object` without `properties`).

### S4 — Frontend transport (ui)

- `ui/src/api/openapi.ts` now has real `operations` types; `ui/src/api/openapiClient.ts` codegen emits `export const api = { listAccounts(...): Promise<Result<...>>, createAccount(...), ... }` (229 methods, typed, no `as never`) using `openapi-fetch` + `Result<T,AppError>` envelope + 401 → `FINSIGHT_AUTH_REQUIRED` + `__FINSIGHT_ES__` close.
- Migrate all `ui/src/api/hooks/*` from `commands.*`/`unwrap` to `api.*` (same `Result` envelope, `AppError` unchanged).
- Rewrite `ui/src/dev/mockBackend.ts` to mock `fetch` (`POST /api/rpc/*` + `GET /api/events` SSE) instead of `__TAURI_INTERNALS__` (keeps `?mock` harness without Tauri).
- Delete `ui/src/api/bindings.ts`, `ui/src/api/client.ts`, `ui/src/api/httpBackend.ts`, `ui/src/api/openapiClient.ts` stub (replaced by generated one), and `ui/src/api/bindings.test.ts` etc.

### S5 — Deletion + deps

- Delete `crates/finsight-bindings` crate (codegen-only, replaced), remove `specta`/`tauri-specta`/`tauri` workspace deps, remove `ui/package.json` `@tauri-apps/api`, clean `Cargo.lock`/`pnpm-lock.yaml`, update `Cargo.toml` workspace members, `README`/`CLAUDE`/`AGENTS` (remove `bindings.ts` refs, document `pnpm openapi` as sole contract).

### S6 — Testing + rollout

- `cargo test --workspace` (openapi 7 tests + `parity.rs` checks `openapi.json` schemas not shallow + `COMMANDS == SUPPORTED`), `pnpm typecheck`, `pnpm test` (134/954, mock via fetch), `pnpm build` (no Tauri). Single PR, `BREAKING` (no shim) — follow-up can delete `finsight-bindings` crate entirely after this lands.

## Error handling

- `AppError` stays `Result<T,AppError>` over `POST /api/rpc/{cmd}`; `openapiClient` `wrap` keeps same 401 → `FINSIGHT_AUTH_REQUIRED` + `EventSource` close as `httpBackend` did. No new error types.

## Testing

- Rust: `cargo test -p finsight-openapi` (shallow-schema guard), `cargo test -p finsight-server --test parity` (now also checks `openapi.json` vs `SUPPORTED` + file identity), `cargo test --workspace` (existing parity + new).
- Frontend: `pnpm typecheck`, `pnpm test` (mock via fetch, `isBackendAvailable` still true in vitest), `pnpm build` (precompressed assets).
- Manual: `cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen` regenerates `openapi.ts` with real types; `curl http://localhost:8674/api/openapi.json | jq .paths | length` == 230 (229 + openapi).

## Explicitly out of scope

- No new RPC commands, no DB migration, no split deployment (already done), no offline mutation queue.

