# FinSight Server — Phase 3: PWA, Packaging & Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make FinSight installable as a PWA with an offline read-only cache, add a server↔client version handshake, and ship a Docker image + docker-compose + self-hosting docs so a non-developer can run the server behind Tailscale/Caddy/LAN.

**Architecture:** `vite-plugin-pwa` (Workbox `generateSW`, `autoUpdate`) adds a web-app manifest + service worker that precaches the built app shell — the app installs on Android/iOS/desktop from the browser. `@tanstack/react-query-persist-client` persists the query cache to IndexedDB so last-synced balances/budgets/transactions render offline (mutations pause; a banner shows the offline state); the persisted cache is **purged on logout/401** so a shared device never leaks a prior user's financials. A new `GET /api/server/about` returns a protocol version; the client (which may be a stale cached PWA after a server upgrade) shows a "refresh to update" banner on mismatch. A multi-stage `Dockerfile` builds `ui/dist` then the tauri-free `finsight-server` release binary into a slim runtime image; `docker-compose.yml` + `docs/self-hosting.md` cover the deployment recipes.

**Tech Stack:** `vite-plugin-pwa` + `@vite-pwa/assets-generator` (icons), `@tanstack/react-query-persist-client` + `@tanstack/query-async-storage-persister` + `idb-keyval`, axum (about endpoint), multi-stage Docker (`rust:bookworm` builder → `debian:bookworm-slim` runtime), Markdown docs.

**Spec:** `docs/superpowers/specs/2026-07-15-server-architecture-design.md` — Phase 3 scope ("PWA & packaging"). Note: healthcheck (Phase 1), setup wizard + desktop-DB migration (Phase 2) are already done and are NOT in this plan.

---

## Ground rules (read first)

- **Baseline (verified green at Phase 2 close):** Rust workspace **590 passed / 0 failed**, frontend **499 tests / 91 files** + `tsc --noEmit` clean, `bindings.ts` byte-identical to `origin/main`, `crates/finsight-server/tests/parity.rs` untouched. Branch: `pwa-desktop-architecture-72a060`.
- **Cargo:** run via **PowerShell**, not Git Bash (Strawberry Perl vs MSYS perl). Cargo tests are **single foreground blocking calls** (`timeout: 600000`); one cargo at a time; `LNK1102` → retry `CARGO_BUILD_JOBS=2`; `LNK1318`/`os error 112` → disk full, BLOCKED.
- **No new Tauri commands** in this phase. `/api/server/about` is server-only REST (like `/api/auth/*`) — it must NOT appear in `bindings.ts`, and `parity.rs` must stay green untouched. After any Rust change: `cargo run -p finsight-tauri --bin export_bindings && git diff --exit-code ui/src/api/bindings.ts` → 0.
- **finsight-server stays tauri-free:** `cargo tree -p finsight-server -i tauri` must remain empty.
- **Desktop unaffected:** every PWA/offline/version behavior is gated so the Tauri desktop build is byte-for-byte behaviorally unchanged. PWA registration and version/offline logic run only in the browser (`window.__FINSIGHT_HTTP__` truthy — the flag `installHttpBackend()` sets; reuse `isServerMode()` from `ui/src/api/auth.ts`). Under Tauri, none of it activates.
- Commit per task, normal commits on top of HEAD.

## File structure (what changes)

```
crates/finsight-server/src/
  server_info.rs          NEW  — PROTOCOL_VERSION const + `about` handler ({version, protocol, minClientProtocol})
  router.rs               MOD  — route GET /api/server/about
ui/
  vite.config.ts          MOD  — VitePWA plugin (manifest + generateSW + pwaAssets)
  public/logo.svg         NEW  — brandmark source for icon generation
  index.html              MOD  — theme-color meta (manifest link is injected by the plugin)
  src/
    api/serverInfo.ts     NEW  — CLIENT_PROTOCOL const + fetchServerAbout()
    pwa/persist.ts        NEW  — IndexedDB query-cache persister + purge()
    pwa/useOnline.ts      NEW  — navigator.onLine hook (online/offline events)
    components/
      OfflineBanner.tsx   NEW  — "Offline — showing last synced" bar
      VersionBanner.tsx   NEW  — "A new version is available — refresh" bar
      AuthGate.tsx        MOD  — purge the persisted cache on logout/401
    main.tsx              MOD  — persistQueryClient wiring + banners (browser/server-mode only)
Dockerfile                NEW  — multi-stage ui build → server release → slim runtime
.dockerignore             NEW
docker-compose.yml        NEW  — finsight-server service + volume + optional ollama profile
docs/self-hosting.md      NEW  — Tailscale / Caddy / LAN(mkcert) recipes + cookie/TLS/streaming caveats
```

---

### Task 1: Version handshake — `GET /api/server/about` (server)

**Files:** Create `crates/finsight-server/src/server_info.rs`; Modify `crates/finsight-server/src/lib.rs` (`pub mod server_info;`), `crates/finsight-server/src/router.rs`.

- [ ] **Step 1: Failing test** (`server_info.rs` `#[cfg(test)]`):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn about_returns_version_and_protocol_without_auth() {
        let state = crate::router::tests::test_state().await;
        let app = crate::router::build_router(state, &crate::router::tests::test_ui_dir());
        let res = app
            .oneshot(Request::get("/api/server/about").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK); // NO auth cookie — must be open
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["protocol"], PROTOCOL_VERSION);
        assert_eq!(v["minClientProtocol"], MIN_CLIENT_PROTOCOL);
        assert!(v["version"].as_str().is_some());
    }
}
```
- [ ] **Step 2: Run** — PowerShell `cargo test -p finsight-server about_returns` → FAIL (module missing).
- [ ] **Step 3: Implement** `server_info.rs`:
```rust
//! Server↔client version handshake. `protocol` is an integer bumped whenever a
//! breaking wire change ships (RPC arg shapes, event frames, auth flow). A PWA
//! cached offline may be older than the server after an upgrade — the client
//! compares its own CLIENT_PROTOCOL (ui/src/api/serverInfo.ts) against
//! `minClientProtocol` and shows a "refresh to update" banner on mismatch.
use axum::Json;

/// Wire-protocol version. Bump on any breaking RPC/event/auth change.
pub const PROTOCOL_VERSION: u32 = 1;
/// Oldest client protocol this server still serves. Raise it only when a change
/// genuinely breaks older cached clients (forces them to refresh).
pub const MIN_CLIENT_PROTOCOL: u32 = 1;

pub async fn about() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": PROTOCOL_VERSION,
        "minClientProtocol": MIN_CLIENT_PROTOCOL,
    }))
}
```
Add `pub mod server_info;` to `lib.rs`. Route in `router.rs` (with the other `/api/*` routes, BEFORE the fallback): `.route("/api/server/about", axum::routing::get(crate::server_info::about))`.
- [ ] **Step 4: Run** — `cargo test -p finsight-server about_returns` → PASS. Then `cargo test -p finsight-server` (all incl. parity) green.
- [ ] **Step 5: Gate** — `cargo run -p finsight-tauri --bin export_bindings; git diff --exit-code ui/src/api/bindings.ts` → 0 (server-only route, no binding). `git diff --exit-code crates/finsight-server/tests/parity.rs` → 0.
- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat(server): GET /api/server/about version handshake endpoint"`

---

### Task 2: Version handshake — client fetch + `VersionBanner` (UI)

**Files:** Create `ui/src/api/serverInfo.ts`, `ui/src/components/VersionBanner.tsx` (+ tests); Modify `ui/src/main.tsx`.

- [ ] **Step 1: Failing tests** (`ui/src/api/serverInfo.test.ts`):
```typescript
import { describe, it, expect, vi, afterEach } from "vitest";
import { fetchServerAbout, isClientOutdated, CLIENT_PROTOCOL } from "./serverInfo";

afterEach(() => vi.unstubAllGlobals());

describe("serverInfo", () => {
  it("fetchServerAbout parses /api/server/about", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify({ version: "0.0.0", protocol: 1, minClientProtocol: 1 }), { status: 200 })));
    const about = await fetchServerAbout();
    expect(fetch).toHaveBeenCalledWith("/api/server/about", expect.anything());
    expect(about.protocol).toBe(1);
  });

  it("isClientOutdated is true only when the client is below the server's minimum", () => {
    expect(isClientOutdated({ version: "x", protocol: 2, minClientProtocol: CLIENT_PROTOCOL + 1 })).toBe(true);
    expect(isClientOutdated({ version: "x", protocol: 1, minClientProtocol: CLIENT_PROTOCOL })).toBe(false);
  });
});
```
- [ ] **Step 2: Run** — `cd ui && npx vitest run src/api/serverInfo.test.ts` → FAIL.
- [ ] **Step 3: Implement** `ui/src/api/serverInfo.ts`:
```typescript
/** Bumped in lockstep with the server's PROTOCOL_VERSION when the client ships a
 *  breaking change. The server's minClientProtocol vs this decides the banner. */
export const CLIENT_PROTOCOL = 1;

export type ServerAbout = { version: string; protocol: number; minClientProtocol: number };

export async function fetchServerAbout(): Promise<ServerAbout> {
  const res = await fetch("/api/server/about", { credentials: "same-origin" });
  if (!res.ok) throw new Error(`about ${res.status}`);
  return res.json();
}

export function isClientOutdated(about: ServerAbout): boolean {
  return CLIENT_PROTOCOL < about.minClientProtocol;
}
```
- [ ] **Step 4: Implement `VersionBanner.tsx`** — a `.card`-styled bar (no hardcoded colors; use tokens) that, in server mode only, fetches about on mount and renders a dismissible "A new version of FinSight is available — refresh to update." with a Reload button (`window.location.reload()`) when `isClientOutdated`. Renders `null` otherwise. Component test (`VersionBanner.test.tsx`): outdated → banner + reload button calls reload; up-to-date → nothing; desktop mode (`isServerMode()` false) → nothing, no fetch.
```tsx
import { useEffect, useState } from "react";
import { fetchServerAbout, isClientOutdated } from "../api/serverInfo";
import { isServerMode } from "../api/auth";

export default function VersionBanner() {
  const [outdated, setOutdated] = useState(false);
  useEffect(() => {
    if (!isServerMode()) return;
    let alive = true;
    fetchServerAbout().then((a) => { if (alive) setOutdated(isClientOutdated(a)); }).catch(() => {});
    return () => { alive = false; };
  }, []);
  if (!outdated) return null;
  return (
    <div className="card" role="status" style={{ /* full-width top bar via tokens */ }}>
      A new version of FinSight is available.
      <button className="btn" onClick={() => window.location.reload()}>Reload</button>
    </div>
  );
}
```
- [ ] **Step 5: Wire into `main.tsx`** — render `<VersionBanner />` above the router inside the providers (it self-gates to server mode, so it's inert under Tauri). Do not block first paint on the fetch.
- [ ] **Step 6: Gates** — `cd ui && npx vitest run` (499 + new, 0 failures) and `npx tsc --noEmit` clean.
- [ ] **Step 7: Commit** — `git add ui/ && git commit -m "feat(ui): server version handshake + refresh-to-update banner"`

---

### Task 3: Installable PWA — manifest + icons + service worker

**Files:** Modify `ui/package.json` (deps), `ui/vite.config.ts`; Create `ui/public/logo.svg`.

- [ ] **Step 1: Install deps** — `cd ui && npm i -D vite-plugin-pwa@^0.20 @vite-pwa/assets-generator@^0.2`. (vite-plugin-pwa ^0.20 supports vite 5.4.)
- [ ] **Step 2: Create the brandmark** `ui/public/logo.svg` — a simple, self-contained square mark on the accent color (a rounded square filled `#C9F950` with an ink `#0A0F02` "F" / upward tick). Full SVG, 512×512, no external refs:
```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">
  <rect width="512" height="512" rx="112" fill="#C9F950"/>
  <path d="M170 150 h180 v54 h-118 v52 h104 v54 h-104 v98 h-62 z" fill="#0A0F02"/>
  <path d="M300 300 l60 -60 40 40 -60 60 z" fill="#0A0F02"/>
</svg>
```
(The mark is an "F" plus a rising tick — refine visually later; this is a valid placeholder that generates clean icons.)
- [ ] **Step 3: Configure VitePWA** in `ui/vite.config.ts` — add the import and plugin to the `plugins` array (keep the existing `react()` and `server.proxy`):
```typescript
import { VitePWA } from "vite-plugin-pwa";
// …
plugins: [
  react(),
  VitePWA({
    registerType: "autoUpdate",
    includeAssets: ["logo.svg"],
    pwaAssets: { image: "public/logo.svg" }, // generates 192/512/maskable/apple-touch icons
    manifest: {
      name: "FinSight",
      short_name: "FinSight",
      description: "Private, self-hosted personal finance.",
      theme_color: "#0A0F02",
      background_color: "#0A0F02",
      display: "standalone",
      start_url: "/",
      scope: "/",
    },
    workbox: {
      // Precache the built app shell. Navigation falls back to index.html
      // (SPA). Do NOT cache /api/* — those are live, auth'd, and event streams.
      globPatterns: ["**/*.{js,css,html,svg,woff2,png,ico}"],
      navigateFallback: "/index.html",
      navigateFallbackDenylist: [/^\/api\//],
      runtimeCaching: [],
    },
    devOptions: { enabled: false }, // never register the SW in `npm run dev`
  }),
],
```
- [ ] **Step 4: theme-color meta** — add to `ui/index.html` `<head>`: `<meta name="theme-color" content="#0A0F02" />`. (vite-plugin-pwa injects the `<link rel="manifest">` and apple-touch-icon links automatically.)
- [ ] **Step 5: Build + verify artifacts** — `cd ui && npm run build`. Assert the build produced the PWA outputs:
  - `ui/dist/manifest.webmanifest` exists and contains `"name":"FinSight"` and an `icons` array with 192 + 512 + a `"purpose":"maskable"` entry.
  - `ui/dist/sw.js` (or `sw.js` + `workbox-*.js`) exists.
  - `ui/dist/index.html` contains `rel="manifest"`.
  Do this with a small check (bash): `grep -q '"name": *"FinSight"' ui/dist/manifest.webmanifest && ls ui/dist/sw.js && grep -q 'rel="manifest"' ui/dist/index.html && echo PWA_OK`.
- [ ] **Step 6: Guard the test suite** — vitest must not try to register the SW. Confirm `cd ui && npx vitest run` is still green (the plugin only affects `build`, and `devOptions.enabled:false` keeps dev/test clear) and `npx tsc --noEmit` clean. If any test imports `virtual:pwa-register`, mock it in `ui/src/test/setup.ts`.
- [ ] **Step 7: Commit** — `git add ui/ && git commit -m "feat(ui): installable PWA (manifest, icons, precache service worker)"`

---

### Task 4: Offline read-cache — persist query cache to IndexedDB + offline banner

**Files:** Modify `ui/package.json`; Create `ui/src/pwa/persist.ts`, `ui/src/pwa/useOnline.ts`, `ui/src/components/OfflineBanner.tsx` (+ tests); Modify `ui/src/main.tsx`, `ui/src/components/AuthGate.tsx`.

- [ ] **Step 1: Install deps** — `cd ui && npm i @tanstack/react-query-persist-client @tanstack/query-async-storage-persister idb-keyval`.
- [ ] **Step 2: Failing test** (`ui/src/pwa/persist.test.ts`) — the persister round-trips through a mocked idb-keyval and `purgePersistedCache()` clears it:
```typescript
import { describe, it, expect, vi, beforeEach } from "vitest";

const store: Record<string, unknown> = {};
vi.mock("idb-keyval", () => ({
  get: vi.fn(async (k: string) => store[k]),
  set: vi.fn(async (k: string, v: unknown) => { store[k] = v; }),
  del: vi.fn(async (k: string) => { delete store[k]; }),
}));

import { createIdbPersister, purgePersistedCache, PERSIST_KEY } from "./persist";

beforeEach(() => { for (const k of Object.keys(store)) delete store[k]; });

describe("idb persister", () => {
  it("persists and restores a client value", async () => {
    const p = createIdbPersister();
    await p.persistClient({ buster: "", timestamp: 1, clientState: { mutations: [], queries: [] } });
    const back = await p.restoreClient();
    expect(back?.timestamp).toBe(1);
  });
  it("purgePersistedCache removes the stored key", async () => {
    const p = createIdbPersister();
    await p.persistClient({ buster: "", timestamp: 2, clientState: { mutations: [], queries: [] } });
    await purgePersistedCache();
    expect(store[PERSIST_KEY]).toBeUndefined();
  });
});
```
- [ ] **Step 3: Implement `ui/src/pwa/persist.ts`:**
```typescript
import { createAsyncStoragePersister } from "@tanstack/query-async-storage-persister";
import { get, set, del } from "idb-keyval";

export const PERSIST_KEY = "finsight-rq-cache";

/** IndexedDB-backed persister for the tanstack-query cache. Financial data is
 *  device-local here — it is purged on logout/401 (purgePersistedCache) so a
 *  shared browser can't leak a prior user's cached balances. */
export function createIdbPersister() {
  return createAsyncStoragePersister({
    key: PERSIST_KEY,
    storage: {
      getItem: (k) => get(k),
      setItem: (k, v) => set(k, v),
      removeItem: (k) => del(k),
    },
    throttleTime: 1000,
  });
}

export async function purgePersistedCache(): Promise<void> {
  await del(PERSIST_KEY);
}
```
- [ ] **Step 4: Implement `ui/src/pwa/useOnline.ts`** — `useOnline(): boolean` from `navigator.onLine` + `online`/`offline` events (SSR-safe: default true). Trivial test optional.
- [ ] **Step 5: Implement `OfflineBanner.tsx`** — server-mode only; when `!useOnline()` render a token-styled bar "Offline — showing your last synced data. Changes are paused until you reconnect." Test: offline → banner; online → null; desktop → null.
- [ ] **Step 6: Wire persistence in `main.tsx`** — replace the plain `<QueryClientProvider client={queryClient}>` with `PersistQueryClientProvider` ONLY in server mode (keep the plain provider for Tauri so desktop behavior is unchanged):
```tsx
import { PersistQueryClientProvider } from "@tanstack/react-query-persist-client";
import { createIdbPersister } from "./pwa/persist";
import { isServerMode } from "./api/auth";
// …
const persister = createIdbPersister();
// in renderApp():
const tree = (
  <AuthGate>
    <VersionBanner />
    <OfflineBanner />
    <BrowserRouter><App /></BrowserRouter>
  </AuthGate>
);
root.render(
  <React.StrictMode>
    {isServerMode()
      ? <PersistQueryClientProvider client={queryClient} persistOptions={{ persister, maxAge: 1000*60*60*24*7 }}>{tree}</PersistQueryClientProvider>
      : <QueryClientProvider client={queryClient}>{tree}</QueryClientProvider>}
  </React.StrictMode>
);
```
(Confirm the exact current `main.tsx` render shape and preserve it — AuthGate already wraps the app from Phase 2. Keep the ordering: providers → AuthGate → banners → router.)
- [ ] **Step 7: Purge on logout/401 in `AuthGate.tsx`** — wherever AuthGate handles logout and the `finsight:auth-required` event (Phase 2), call `queryClient.clear()` AND `await purgePersistedCache()` so the on-disk cache doesn't outlive the session. Add/extend an AuthGate test asserting `purgePersistedCache` is called on the auth-required path (mock the module).
- [ ] **Step 8: Gates** — `cd ui && npx vitest run` (green) + `npx tsc --noEmit` clean. `git diff --exit-code ui/src/api/bindings.ts` → 0.
- [ ] **Step 9: Commit** — `git add ui/ && git commit -m "feat(ui): offline read-cache (IndexedDB-persisted query cache, purged on logout) + offline banner"`

---

### Task 5: Docker image — multi-stage Dockerfile + .dockerignore + compose

**Files:** Create `Dockerfile`, `.dockerignore`, `docker-compose.yml` (repo root).

- [ ] **Step 1: `.dockerignore`** (keep the build context small + avoid leaking local data):
```
target/
**/node_modules/
.git/
data/
.claude/
*.log
ui/dist/
```
- [ ] **Step 2: `Dockerfile`** (multi-stage; the Linux release build of the tauri-free server needs perl/make/gcc for vendored OpenSSL/SQLCipher — `rust:bookworm` has them, `webkit2gtk` is NOT needed because finsight-server never pulls tauri):
```dockerfile
# ---- 1. Build the UI ----
FROM node:20-bookworm-slim AS ui
WORKDIR /ui
COPY ui/package.json ui/package-lock.json* ./
RUN npm ci
COPY ui/ ./
RUN npm run build            # → /ui/dist (PWA manifest + sw + assets)

# ---- 2. Build the server (release) ----
FROM rust:1-bookworm AS server
RUN apt-get update && apt-get install -y --no-install-recommends perl make && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src-tauri/ src-tauri/
# Build ONLY the server bin — never the Tauri app (no webkit deps in this image).
RUN cargo build --release -p finsight-server

# ---- 3. Slim runtime ----
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /src/target/release/finsight-server /usr/local/bin/finsight-server
COPY --from=ui /ui/dist /app/ui/dist
ENV FINSIGHT_DATA_DIR=/data \
    FINSIGHT_UI_DIR=/app/ui/dist \
    FINSIGHT_PORT=8674 \
    FINSIGHT_COOKIE_SECURE=1 \
    RUST_LOG=info
VOLUME /data
EXPOSE 8674
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD ["/bin/sh","-c","exec 3<>/dev/tcp/127.0.0.1/8674 && printf 'GET /api/health HTTP/1.0\\r\\n\\r\\n' >&3 && grep -q '\"status\":\"ok\"' <&3"]
ENTRYPOINT ["/usr/local/bin/finsight-server"]
```
NOTE on `FINSIGHT_COOKIE_SECURE=1`: the container assumes a TLS-terminating reverse proxy in front (the documented deployment). If a user runs it bare over http they must override it to `0`, documented in Task 6.
NOTE on `src-tauri/`: it's copied because it's a workspace member Cargo needs to resolve the workspace, but `-p finsight-server` never compiles it beyond metadata; if the workspace resolves without it, omit the COPY. Verify during the build.
- [ ] **Step 3: `docker-compose.yml`:**
```yaml
services:
  finsight:
    build: .
    # image: finsight-server:latest   # or pull a published tag
    restart: unless-stopped
    ports:
      - "8674:8674"
    volumes:
      - finsight-data:/data
    environment:
      # Set to 0 ONLY if you are NOT behind an HTTPS reverse proxy (bare http LAN).
      FINSIGHT_COOKIE_SECURE: "1"
    # Optional local LLM for the Copilot — uncomment + point the app's provider at http://ollama:11434
    # depends_on: [ollama]

  # ollama:
  #   image: ollama/ollama:latest
  #   restart: unless-stopped
  #   volumes: [ ollama:/root/.ollama ]

volumes:
  finsight-data:
  # ollama:
```
- [ ] **Step 4: Verify the build IF Docker is available.** Check first: `docker version`. 
  - If Docker is present: `docker build -t finsight-server:e2e .` (long — vendored OpenSSL + release build). Then `docker run -d --rm -p 8674:8674 -e FINSIGHT_COOKIE_SECURE=0 -v finsight-e2e:/data --name finsight-e2e finsight-server:e2e`, wait ~8s, `curl -fsS http://127.0.0.1:8674/api/health` → `{"status":"ok"}` and `curl -fsS http://127.0.0.1:8674/api/server/about` → has `protocol`. Then `curl -fsS http://127.0.0.1:8674/ | grep -q 'rel="manifest"'` (PWA served). Stop + remove the container + the `finsight-e2e` volume.
  - If Docker is NOT available in this environment: report DONE_WITH_CONCERNS — the Dockerfile is written and reviewed but the image build was not executed here; note it for the Task 8 verification / the user's own machine. Do a static sanity check instead: `hadolint Dockerfile` if available, else confirm every `COPY --from` source path matches a prior stage's output path by reading the file.
- [ ] **Step 5: Commit** — `git add Dockerfile .dockerignore docker-compose.yml && git commit -m "feat(deploy): multi-stage Dockerfile + docker-compose (tauri-free server image)"`

---

### Task 6: Self-hosting documentation

**Files:** Create `docs/self-hosting.md`; Modify `CLAUDE.md` (one pointer line under server-mode docs).

- [ ] **Step 1: Write `docs/self-hosting.md`** — a real, followable guide (not a stub). Sections, each with copy-pasteable commands:
  1. **What you get / prerequisites** — Docker + docker-compose; the server binds `:8674` plain HTTP and expects TLS from a reverse proxy; `/data` volume holds `users.db` + per-user encrypted DBs (back this up).
  2. **Quick start (docker-compose)** — `docker compose up -d`; first visit → setup wizard → **save the recovery key**; note `FINSIGHT_COOKIE_SECURE` (leave `1` behind HTTPS; set `0` for bare http LAN).
  3. **Recipe A — Tailscale (recommended)** — install Tailscale on the host + devices; `tailscale serve https / http://localhost:8674` (MagicDNS gives a valid HTTPS cert automatically); reach it at `https://<host>.<tailnet>.ts.net`. Cookie secure stays `1`. No domain/port-forward needed; server never faces the internet.
  4. **Recipe B — Public domain + Caddy** — a `Caddyfile` reverse-proxying `finsight.example.com` → `finsight:8674` with automatic Let's Encrypt; sample compose snippet adding the caddy service. Warn: this exposes a finance app to the internet — strong admin password, keep the host patched.
  5. **Recipe C — LAN only + mkcert** — `mkcert` a cert for the host's LAN IP/hostname, install the mkcert root CA on each device (required so the PWA can install — service workers need a trusted HTTPS origin), front the server with Caddy/nginx using that cert. No away-from-home access.
  6. **Installing the app** — Android/desktop Chrome "Install app"; iOS Safari "Add to Home Screen" (note the ~7-day Safari PWA storage-eviction caveat — the offline cache is a convenience, never the source of truth).
  7. **Backups & upgrades** — snapshot the `/data` volume; upgrade by pulling a new image + `docker compose up -d` (the version banner tells connected clients to refresh); the server auto-migrates its DB schema on start.
  8. **Known limits (Phase 3)** — Copilot streaming holds a long HTTP request; some proxies cut idle requests at 30–60s (Caddy default is generous; if you see truncated Copilot answers, raise the proxy's read timeout). CSV share-target and offline *editing* are not yet supported.
- [ ] **Step 2: CLAUDE.md** — add one line under the server-mode Commands block: `# Self-hosting (Docker + Tailscale/Caddy/LAN): see docs/self-hosting.md`.
- [ ] **Step 3: Commit** — `git add docs/self-hosting.md CLAUDE.md && git commit -m "docs(deploy): self-hosting guide (Tailscale/Caddy/LAN, install, backups, limits)"`

---

### Task 7: End-to-end verification (Phase 3 exit criterion)

- [ ] **Step 1: Full green bar** — `cargo test --workspace` (PowerShell; jobs=2 if OOM) 0 failures; `cd ui && npx vitest run && npx tsc --noEmit` green; `cargo run -p finsight-tauri --bin export_bindings; git diff --exit-code ui/src/api/bindings.ts` → 0; `git diff --exit-code crates/finsight-server/tests/parity.rs` → 0; `cargo tree -p finsight-server -i tauri` empty.
- [ ] **Step 2: Build + serve** — `cd ui && npm run build`; `cargo build -p finsight-server`; launch `finsight-server` against a fresh scratch `FINSIGHT_DATA_DIR` with `FINSIGHT_UI_DIR=ui/dist`, `FINSIGHT_COOKIE_SECURE=0` (localhost http).
- [ ] **Step 3: Browser checklist** (drive via the browser tools, like Phases 1/2):
  1. `GET /api/server/about` → 200 with `{version, protocol, minClientProtocol}`.
  2. Load `http://localhost:8674` → the response HTML has `rel="manifest"`; `GET /manifest.webmanifest` → 200 with name "FinSight" + 192/512/maskable icons; `GET /sw.js` → 200 (`content-type` JS). (Installability: manifest + SW + start_url + icons all present. Note: Chrome's install prompt also needs HTTPS or localhost — localhost qualifies.)
  3. Complete setup (fresh DB), create an account. Then simulate offline: with the query cache warm, set the browser offline (or stub `navigator.onLine=false` + dispatch `offline`) and reload — the **OfflineBanner shows** and last-synced balances still render from the IndexedDB cache (no white screen). Back online → banner clears.
  4. Sign out → confirm the persisted cache is purged (IndexedDB key `finsight-rq-cache` gone; the next user/login starts clean).
  5. Version banner: temporarily stub `/api/server/about` to return `minClientProtocol: 999` → the **VersionBanner shows** with a working Reload button. (Or bump the const in a scratch run.)
- [ ] **Step 4: Docker** — if Docker is available, `docker build` + run + `/api/health` + `/` manifest check (Task 5 Step 4); else record that the image was not built in this environment and must be validated on a Docker host.
- [ ] **Step 5: Record results** in Linear + update the plan checkboxes; capture screenshots of the installed-manifest + offline states. Then `superpowers:finishing-a-development-branch`.

---

## Explicitly out of scope (Phase 3)

- **CSV share-target** (`share_target` manifest + custom injectManifest SW) — Android/Chrome only (iOS Safari, the primary target, doesn't support it) and needs a custom service worker; deferred to a focused follow-up. When done: switch VitePWA to `strategies: "injectManifest"` with a hand-written `sw.ts` that intercepts the share POST, stashes the file in the Cache API, and redirects to an import route.
- **Offline mutation queueing / editing** — Phase 3 is read-only offline (spec).
- **Mobile bottom-tab navigation redesign** — separate follow-up project (spec).
- **Publishing a versioned Docker image to a registry / CI** — the repo's CI is disabled; publishing tags is a manual/ops task outside this plan.
- **Thin Tauri desktop shell** — Phase 4.
