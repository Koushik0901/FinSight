# FinSight Server Architecture — Design

**Date:** 2026-07-15
**Status:** Approved design, pre-implementation
**Inspiration:** Immich's self-hosted client/server model

## Problem

FinSight is a Tauri desktop app with a local SQLCipher DB. The user wants the
same data on their phone and every computer, always in sync — without paying
Apple developer fees for a native iOS app. Immich solves the same shape of
problem with a self-hosted Docker server plus thin clients.

## Decisions (settled during brainstorming)

| Question | Decision |
|---|---|
| Standalone (local-only) mode | **Dropped.** Server-only, Immich model. All clients talk to one self-hosted server. |
| User model | **Multi-user, isolated data** — one SQLCipher DB *file* per user (no `tenant_id` columns; `finsight-core` queries untouched). |
| PWA offline scope | **Read-only cache.** View last-synced data offline; mutations require a connection. |
| Desktop client | **Thin Tauri shell**: webview → configured server URL, token in OS keychain. Phase 4; installed PWA covers desktop until then. |
| TLS / network path | Server speaks plain HTTP on a port; **TLS is the reverse proxy's job**. Docs cover Tailscale Serve, Caddy + domain, LAN + mkcert. (PWA service workers require HTTPS.) |
| Secrets | **Per-user random DB key, wrapped by an Argon2id password-derived KEK** plus a printable recovery-key wrap (Bitwarden pattern). Password change = re-wrap, not re-encrypt. LLM API keys move from OS keychain into each user's encrypted DB settings KV. |
| API style | **Invoke shim over HTTP.** `bindings.ts` and all screens/hooks untouched; web builds install an `invoke(cmd, args) → POST /api/rpc/{cmd}` shim and `listen → SSE` shim. RPC-shaped, first-party-client API. |

## Architecture

```
Phone (PWA)      Browser (web UI)      Desktop (thin Tauri shell)
      \                 |                  /
       HTTPS (reverse proxy / Tailscale)
                        |
              ┌─────────▼──────────┐
              │  finsight-server    │  axum, one Docker container
              │  /            → UI  │  serves built React app
              │  /api/rpc/{cmd}     │  same command surface as today
              │  /api/events        │  SSE (Copilot frames + agent events)
              │  /api/auth/*        │  login / session / admin
              │  /api/health        │  healthcheck
              │  /api/server/about  │  version handshake
              └─────────┬──────────┘
              /data volume:
                users.db                     (plain SQLite: users, verifiers, wrapped keys)
                users/<user_id>/finsight.db  (per-user SQLCipher DB)
```

### Crate changes

- **`finsight-core` / `finsight-agent` / `finsight-providers`:** untouched.
- **New `finsight-server`:** axum router, auth, sessions, per-user pool +
  `AgentHandle` registry, SSE fan-out, static UI serving.
- **`finsight-app`:** command bodies become transport-agnostic functions
  taking `(&Db, &AgentHandle, args)`. One command registry drives BOTH the
  axum RPC router and the TypeScript bindings generation — no drift by
  construction. `tauri::AppHandle` in the Copilot stream path is replaced by
  a `FrameSink` trait (below). The Tauri command surface is deleted once the
  server is proven.
- **`src-tauri`:** shrinks to the thin desktop shell (Phase 4).

### Frontend transport (the shim)

- `bindings.ts` stays generated from the same command list.
- A web-only module (same slot as `ui/src/dev/mockBackend.ts` today) installs
  `window.__TAURI_INTERNALS__.invoke = (cmd, args) => fetch POST /api/rpc/{cmd}`
  and `listen(event, cb)` backed by one shared SSE connection to `/api/events`,
  demuxed by event name.
- Responses keep the `Result<T, AppError>` shape. Any `401` clears the session
  and routes to the login screen.

### Auth & crypto

- **users.db** (plain SQLite): id, username, Argon2id password verifier, salt,
  wrapped DB key (password KEK), wrapped DB key (recovery KEK), is_admin,
  created_at.
- **Key lifecycle:** random 32-byte SQLCipher key per user at creation.
  Login = verify password → derive KEK → unwrap DB key → open pool + spawn
  `AgentHandle`. Recovery key shown exactly once at account creation.
  Both password and recovery key lost ⇒ data unrecoverable, by design.
- **Sessions:** opaque token in an HttpOnly, Secure, SameSite cookie.
  Server-side session map: token → { user_id, unwrapped DB key }. Pools and
  agent handles are lazily created and evicted after an idle timeout
  (key dropped from memory on eviction).
- First run (empty users.db) serves a **setup wizard**: create admin →
  configure LLM provider → optionally import an existing desktop DB.

### Copilot & agent, server-side

- **`FrameSink` trait** (one method, `send(CopilotStreamFrame)`) replaces
  `tauri::AppHandle` in `stream_copilot_message` / `emit_copilot_frame`.
  Server impl pushes into a per-user `tokio::broadcast` channel feeding SSE.
  `AgentHandle`'s existing `EventCallback` plugs into the same channel.
- **Runs survive disconnects:** a Copilot run never dies with its SSE
  connection (phones drop connections constantly). The assistant message is
  persisted on completion, so on reconnect the client refetches the
  conversation; if the run is still live it re-attaches to new frames (missed
  middle tokens are not replayed in v1 — no frame ring buffer).
- Explicit `cancel_copilot_run` RPC for deliberate stops.
- Per-user cost: one SQLCipher pool + one agent thread per *active* user;
  fine at self-host scale, bounded by idle eviction.
- Ollama endpoint becomes a server-reachable URL (docker-compose service);
  per-user provider config already lives in settings.

### Background jobs

- **v1: session-scoped + catch-up.** Jobs (categorizer, recipes, due checks)
  run only while the user's key is in memory. On login, a catch-up pass runs
  whatever came due while logged out.
- **Later (explicitly out of v1 scope):** per-user opt-in "keep unlocked on
  this server" — unwrapped key cached in memory until server restart,
  enabling true background sync at a slightly weaker at-rest posture.

### PWA

- `vite-plugin-pwa` (Workbox): manifest, icons, app-shell precache.
- Offline reads via tanstack-query `persistQueryClient` → IndexedDB, with an
  "offline — showing last synced" banner; mutations disabled offline.
- **`share_target`** in the manifest: share a CSV from a bank app straight
  into the FinSight import flow.
- Documented iOS caveat: Safari can evict PWA storage after ~7 days of
  disuse; the cache is a convenience, never a source of truth.
- Mobile-first navigation (bottom tab bar) is a **separate follow-up
  project**, not in this scope.

### Immich-inspired product touches

- `/api/server/about`: server version + minimum client version; clients show
  a mismatch banner.
- Admin **jobs page**: queue visibility (categorizer, recipe runs) with
  manual triggers, built on existing `AgentEvent`s.
- `/api/health` for Docker healthchecks and uptime monitors.
- Versioned Docker image tags.

### Packaging & migration

- Multi-stage Dockerfile (ui build → cargo build → slim runtime), single
  container, `/data` volume. `docker-compose.yml` example with optional
  Ollama service.
- Deployment docs: Tailscale Serve, Caddy + Let's Encrypt, LAN + mkcert.
- **Desktop-DB migration:** wizard/admin flow to upload the current
  `finsight.db`; server rekeys it to the user's new wrapped key.

## Error handling

- `AppError` serialization unchanged end-to-end (shim preserves the
  `Result<T, AppError>` contract).
- 401 → session cleared, login screen. SSE auto-reconnects with backoff;
  reconnect triggers conversation refetch (see run-survival rule).
- Wrong-password vs locked-DB errors are distinguished server-side; clients
  never see SQLCipher internals.

## Testing

- Server: axum `tower::ServiceExt` integration tests — auth flow, key
  wrap/unwrap round-trip, RPC routing, session eviction drops keys,
  per-user isolation (user A's token cannot touch user B's data).
- Parity test: every command in the registry is routable over HTTP (same
  spirit as the existing Rust↔Zod corpus test).
- Frontend: existing vitest suite runs unchanged against the mock backend;
  new unit tests for the HTTP/SSE shim.
- Existing 509 Rust + 424 frontend tests stay green throughout; core crates
  are untouched.

## Phasing

1. **Phase 1 — end-to-end skeleton:** `finsight-server` crate, command
   registry extraction, invoke shim + SSE, single hardcoded user. Exit
   criterion: full app works in a plain browser against `localhost`.
2. **Phase 2 — auth & multi-user:** users.db, Argon2id + key wrapping,
   sessions, login screen, admin user management, per-user pools/agents,
   session-scoped jobs + catch-up.
3. **Phase 3 — PWA & packaging:** manifest/service worker, offline read
   cache, share-target, Docker image, compose file, deployment docs,
   healthcheck, version handshake, setup wizard, desktop-DB migration flow.
4. **Phase 4 — thin desktop shell:** Tauri webview + keychain token +
   server URL config; delete the old Tauri command surface.

## Explicitly out of scope (v1)

- Offline mutation queueing / conflict resolution.
- "Keep unlocked" background-sync toggle.
- Mobile bottom-tab navigation redesign.
- Public/third-party REST API (RPC shim is the API; revisit if an external
  consumer appears).
- Native iOS/Android apps.
