# Pure Self-Hosted + OpenAPI — Design

**Date:** 2026-08-24
**Status:** Approved design (supersedes Phase 4 thin-shell retention)
**Inspiration:** Immich (self-hosted Docker + Tailscale + PWA) + Cookfully (`deploy/compose.yaml` + `deploy/docker/nginx.conf` + FastAPI OpenAPI)
**Decisions:** `A` (shell deleted) + `B` (OpenAPI replaces `tauri-specta`) + `C` (single-container default, split-ready seam)

## Problem

FinSight completed the Immich-style server architecture (`docs/superpowers/specs/2026-07-15-server-architecture-design.md`): `finsight-server` (Axum on `:8674`) serves `ui/dist`, `POST /api/rpc/{cmd}`, `GET /api/events` SSE, per-user SQLCipher DBs on `/data`, Docker + `docs/self-hosting.md` Tailscale/Caddy/LAN. The remaining Tauri surface is a thin shell (`src-tauri/src/main.rs:1`, `config.rs:1`, 3 keychain commands) that only remembers a server URL and navigates a webview. Maintaining `desktop.yml:1` (Windows + macOS bundles on every `v*` tag), `src-tauri/tauri.conf.json` version sync, `finsight-bindings` `tauri-specta` codegen, and `ui/src/main.tsx:70` + `runtime.ts:15` + `selectBackend.ts:18` branching costs CI, toolchain and user confusion for no feature gain — the installed PWA already provides a dock icon, tray-like standalone window, and offline read cache. Cookfully ships as pure `api + web (nginx) + postgres + redis` with no native wrapper and works perfectly over Tailscale.

Goal: delete the Tauri shell entirely, replace the `tauri-specta` contract with a standard OpenAPI contract (like Immich `open-api/` / Cookfully FastAPI), keep the proven single-container deploy, but design the boundary so a future `web` split is a non-breaking addition.

## Decisions (settled during brainstorming 2026-08-23/24)

| Question | Decision |
|---|---|
| Target end-state | **Pure self-hosted, shell deleted** (`src-tauri/` gone, PWA is the desktop app). |
| API contract | **OpenAPI** (`utoipa` on `finsight-api` handlers, `openapi.json` at `/api/openapi.json`, TS client via `openapi-typescript`) — replaces `tauri-specta` + `collect_commands!`. |
| Container shape | **Single-container default, split-ready seam** — `Dockerfile:1` 3-stage stays (UI baked), `docker-compose.yml:1` one service; `deploy/compose.split.yaml.example` shows `web: nginx:1.29-alpine` proxying `/api/` to `api:8674` for later. |
| Data layer | **Keep per-user SQLCipher files** (`users.db` + `users/<uuid>/data.sqlcipher`) — Immich's Postgres not needed for a personal ledger; no `tenant_id` migration now. |
| PWA vs native mobile | **PWA only** — `vite-plugin-pwa` Workbox, 7-day IndexedDB read cache, `share_target`, iOS 7-day eviction caveat kept. Native iOS/Android out of scope. |

## Architecture

```
Browser / PWA (installed, standalone)
        │
        │  HTTPS (Tailscale Serve / Caddy / mkcert — docs/self-hosting.md:154)
        ▼
┌──────────────────────────────┐
│  finsight-server (Axum) :8674 │  single Docker image
│  /              → ui/dist     │  SPA fallback, precompressed .br/.gz, Workbox SW
│  /api/rpc/{cmd} (OpenAPI)     │  typed POST, camelCase args from generated client
│  /api/events  (SSE)           │  Copilot frames + import progress (no compression)
│  /api/auth/*  (REST)          │  login/session/admin (unchanged)
│  /api/import/csv (multipart) │  upload token → staging `imports/`
│  /api/openapi.json            │  generated spec (no-cache)
│  /api/health, /api/server/about │ version handshake + healthcheck
│  /mcp (Streamable HTTP)       │  MCP tools (unchanged, shares finsight-api)
└──────────────┬───────────────┘
               │  /data volume
               ├── users.db (plain: verifiers, wrapped keys, token hashes)
               ├── session.key (wraps persisted sessions)
               └── users/<uuid>/data.sqlcipher (+ backups/, imports/)
```

Future split (seam, not default):
```
web: nginx:1.29-alpine  →  /api/* → api:8000 (or 8674)
                         →  /      → /usr/share/nginx/html (try_files $uri /index.html)  # Cookfully deploy/docker/nginx.conf:6
api: finsight-server (same image, no ui/dist or UI not served)
```

### Crate changes

- **`finsight-core` / `finsight-agent` / `finsight-providers`:** untouched (SQL, AgentHandle, providers).
- **`finsight-api` (`crates/finsight-api/src/lib.rs:27`):** Add `utoipa` dep, `#[derive(ToSchema)]` on DTOs, `#[utoipa::path(post, path="/api/rpc/{cmd}", ...)]` on each `pub async fn` in `commands/*`. `ApiState` (`lib.rs:27`) unchanged — still `db + agent + provider + data_dir + sync_scheduler`. No `tauri` types.
- **`finsight-bindings` → `finsight-openapi` (or keep name):** Delete `build_specta_builder()` `collect_commands!` (`src/lib.rs:58`), `#[tauri::command]` wrappers. New `src/lib.rs: build_openapi() -> OpenApi` aggregating `#[utoipa::path]`; new `bin/export_openapi.rs` writes `openapi.json` (also `ui/src/api/openapi.json` for TS gen). Workspace `Cargo.toml:12` drops `finsight-tauri`, `tauri`, `tauri-specta` deps.
- **`finsight-server` (`dispatch.rs:134`, `router.rs`):** Keep `rpc_routes!` macro + `SUPPORTED`/`ARG_CHECK_EXEMPT` invariant, but derive expected set from OpenAPI snapshot; add `GET /api/openapi.json` (no-cache, compressed) — SSE `/api/events` stays uncompressed (`router.rs` per-route compression — `sse_event_stream_is_never_compressed`). Parity test becomes schema-vs-router + schema-vs-TS.
- **`src-tauri`:** deleted.
- **`finsight-eval`:** untouched (reads `finsight_api` directly).

### Frontend transport

- Delete `ui/src/api/httpBackend.ts:33` `__TAURI_INTERNALS__` shim, `selectBackend.ts:18`, `runtime.ts:15` Tauri branches, `components/DesktopConnectGate.tsx`.
- `ui/src/api/bindings.ts:1` header becomes `// AUTO-GENERATED by openapi-typescript — do not edit`; content generated from `openapi.json` via `openapi-typescript` (or `orval`). `ui/src/api/client.ts` re-exports typed `createClient<paths>({ baseUrl:"/api/rpc" })` wrappers preserving `Result<T,AppError>` + existing hook signatures (`ui/src/api/hooks/*` unchanged externally).
- `ui/src/main.tsx:70` `boot()` simplifies to always `PersistQueryClientProvider` + `AuthGate` (no `selectBackend` branch). `ui/vite.config.ts` `precompress.mjs` chain adds `openapi:gen` step.
- `isBackendAvailable()` collapses; 401 handling (clear `QueryClient` + IndexedDB, dispatch `FINSIGHT_AUTH_REQUIRED`) moves into generated client `fetch` wrapper.

### Auth & crypto, jobs, PWA

- Unchanged: `users.db`, Argon2id KEK + recovery-key wraps, `session.key`, 30-day sliding sessions surviving restarts, lazy per-user runtime eviction (30 min idle), admin `Manage users`, CSV upload staging token validation (`dispatch.rs:44` `uploaded_csv_path`), MCP PAT/OAuth (`mcp.rs`, `oauth.rs`), `sync_scheduler`, categorizer/anomalies/recipes, PWA `vite-plugin-pwa` + `pwa/persist.ts` 7-day cache + `OfflineBanner`, `shareTarget.ts`, `push.ts`, `badge.ts`.

### Packaging & deploy

- `Dockerfile:1` stays 3-stage: `node:20-bookworm-slim` (pnpm UI build) → `rust:1-bookworm` (cargo build `-p finsight-server` only, no `finsight-tauri`) → `debian:bookworm-slim` runtime (`ca-certificates`, `libdbus-1-3` only if `keyring` remains — otherwise drop). Env `FINSIGHT_DATA_DIR=/data`, `FINSIGHT_UI_DIR=/app/ui/dist`, `FINSIGHT_PORT=8674`.
- `docker-compose.yml:1` stays single service `finsight:8674` + `finsight-data:/data`; `.env` `FINSIGHT_IMAGE` pinning, `FINSIGHT_COOKIE_SECURE` handling unchanged.
- New `deploy/compose.split.yaml.example` (example only, not default) demonstrates `web: nginx:1.29-alpine` + `api: finsight` split using `Cookfully/deploy/docker/nginx.conf:11` `location /api/ { proxy_pass http://api:8674; }` + `try_files $uri /index.html;` + `map $http_x_forwarded_proto` for `X-Forwarded-Proto`.

## Error handling

- `AppError` (`crates/finsight-api/src/error.rs`) serialization unchanged; `dispatch.rs:118` maps `rpc.unknown_command`→404, `rpc.bad_arg`→400, else 500.
- `401 auth.required` → generated client dispatches `FINSIGHT_AUTH_REQUIRED`, closes `EventSource`, clears in-memory + IndexedDB cache (same as `httpBackend.ts:129`).
- SSE `GET /api/events` stays uncompressed; static `/assets/*` `immutable` 1yr, `index.html`/`sw.js`/`manifest` `no-cache` (pinned by `router.rs` tests).

## Testing

- **Rust:** `cargo test --workspace` (existing parity `dispatch::tests::*` + new `openapi_snapshot` + `openapi_json` route test; `parity.rs` asserts `SUPPORTED == openapi paths - UNSUPPORTED`; `mcp.rs` still checks `standard_toolset()`); `finsight-core` migrations/core untouched.
- **Frontend:** `pnpm --filter ui test` (`vitest run` — new `ui/src/api/openapi.client.test.ts` for typed fetch + 401 dispatch), `pnpm typecheck` (`tsc --noEmit`), `pnpm build` (precompressed assets + Workbox SW). `ui/src/dev/mockBackend.ts` typed against generated `CommandName`.
- **Docker:** `docker compose -f docker-compose.yml up --build` + optional `docker compose -f docker-compose.yml -f deploy/compose.split.yaml.example up` both serve PWA + pass healthcheck `GET /api/health`.

## Phasing (single-PR, 6 tasks — see plan)

1. **Scaffold OpenAPI crate + `/api/openapi.json`** — empty spec, route, compression/caching tests.
2. **Annotate `finsight-api` modules** — per-module `ToSchema` + `#[utoipa::path]`, snapshot tests, `openapi.json` regen.
3. **Generate TS client, replace bindings shim** — `openapi-typescript` → `bindings.ts`, `client.ts` wrappers, delete `httpBackend`/`selectBackend`/`runtime` Tauri branches.
4. **Delete thin shell + Tauri plumbing** — remove `src-tauri/`, `desktop.yml`, `tauri` deps, `DesktopConnectGate`, simplify `main.tsx`.
5. **Single-container hardening + split-ready seam** — keep single image default, add `compose.split.yaml.example`, keep SSE uncompressed.
6. **Docs + parity** — update `README.md:372`, `docs/self-hosting.md`, `CLAUDE.md:51`, `AGENTS.md`, `parity.rs`, green bar `cargo test --workspace && pnpm typecheck && pnpm --filter ui test && pnpm build`.

## Explicitly out of scope

- Postgres/data migration, multi-service default, offline mutation queueing, mobile bottom-tab redesign, native iOS/Android apps, public third-party REST API beyond `openapi.json` + existing `mcp`.

