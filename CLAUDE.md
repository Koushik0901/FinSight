# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Thin desktop shell (Phase 4) + Vite frontend with hot reload.
# NOTE: as of Phase 4 the desktop app is a THIN WEBVIEW SHELL with no local
# database and no local command surface — it shows a ConnectScreen asking for a
# self-hosted server URL, then navigates the window there and behaves exactly
# like the browser/PWA. So `tauri:dev` launches that shell; point it at a
# running `cargo run -p finsight-server` instance to see real data. (The old
# `.dev`-isolated local-DB behavior is gone — the shell owns no DB to isolate.)
pnpm tauri:dev

# Frontend only (no Tauri, faster for UI-only work)
cd ui && npm run dev

# Server mode (Immich-style self-hosted): API + SSE + serves ui/dist on :8674.
# Data dir defaults to ./data (gitignored; FINSIGHT_DATA_DIR to override):
# users.db (account registry + wrapped keys) + one SQLCipher DB per user.
# Sessions are in memory; each DB key is wrapped by Argon2id(password) and by
# a printable recovery key.
# A legacy Phase-1 plaintext `db.key` is migrated and deleted on first setup.
cargo run -p finsight-server

# Browser dev against the server: start the server, then `cd ui && npm run dev`
# (vite proxies /api → :8674; the HTTP/SSE shim auto-installs when no Tauri and
# no ?mock). For the served-from-server experience: `cd ui && npm run build`
# then open http://localhost:8674 directly.

# Self-hosting (Docker + Tailscale/Caddy/LAN): see docs/self-hosting.md

# All Rust tests
cargo test --workspace

# Single Rust test
cargo test -p finsight-core --lib repos::transactions::tests::update_transaction_notes

# All frontend tests
cd ui && npx vitest run

# Single frontend test file
cd ui && npx vitest run src/screens/Settings.test.tsx

# TypeScript type-check (no emit)
cd ui && npx tsc --noEmit

# Regenerate TypeScript bindings after changing the shared command contract
cargo run -p finsight-tauri --bin export_bindings

# Build for production
cd ui && npm run build
```

## Architecture

FinSight has completed the self-hosted client/server architecture described in
`docs/superpowers/specs/2026-07-15-server-architecture-design.md`. Every shared
command body lives in the Tauri-free **`finsight-api`** crate. The browser, PWA,
and navigated desktop shell use **`finsight-server`** over HTTP/SSE; the shipped
Tauri binary is only a server-URL webview shell. Server events (Copilot
streaming, import progress) flow through `FrameSink` to SSE `/api/events`, and
`ui/src/api/httpBackend.ts` preserves the generated bindings' invoke/event
contract. The parity tests in `crates/finsight-server/tests/parity.rs` enforce
that every generated command is routed with exactly the camelCase argument keys
sent by `bindings.ts`.

### Rust workspace (8 crates)

**`crates/finsight-core`** — domain layer: models, SQLCipher DB pool, migrations, repository functions, settings KV store. All SQL lives here. No Tauri dependency.

**`crates/finsight-providers`** — CSV import parsers, LLM provider HTTP clients (`CompletionProvider` trait with Ollama / OpenAI-compat / Anthropic impls).

**`crates/finsight-agent`** — AI layer: Copilot context engine, planner, executor, recipe runner, categorizer pipeline, anomaly detection. Runs on a background Tokio task via `AgentHandle`.

**`crates/finsight-api`** — transport-agnostic application layer (NO Tauri dependency — guarded by `cargo tree -p finsight-api -i tauri`). `ApiState` (db/agent/provider/sync scheduler/data_dir), `AppError`, the `FrameSink` event-emission trait, provider construction helpers, and EVERY command body as `pub async fn name(state: &ApiState, …)`. **Command logic changes happen here**, not in the wrappers.

**`crates/finsight-app`** — codegen-only Tauri wrapper layer. Each `#[tauri::command]` delegates to the same-named `finsight_api::commands::*` function through `&state.api`; `build_specta_builder()` supplies the contract used by `export_bindings`. This crate is not linked into the shipped desktop binary. The real desktop entry point is `src-tauri/src/main.rs`, which exposes only the three local server-URL commands.

**`crates/finsight-server`** — Axum self-host server: first-run setup, multi-user authentication and recovery, lazy per-user SQLCipher runtimes, admin user management, CSV upload staging, `POST /api/rpc/{cmd}`, `GET /api/events`, public health/about routes, and static PWA serving with SPA fallback. `tests/parity.rs` machine-checks the dispatcher against `bindings.ts`.

**`crates/finsight-eval`** — evaluation fixtures and runners for Copilot/provider quality checks. Live-provider tests remain opt-in.

**`src-tauri`** (crate alias `finsight-tauri`) — binary entry point + `export_bindings` binary that writes `ui/src/api/bindings.ts`.

### The `run()` pattern

All DB access in commands uses:
```rust
run(&db, move |conn| {
    // conn: &mut rusqlite::Connection
    // return CoreResult<T>
})
.await
.map_err(AppError::from)
```
This offloads blocking I/O to a Tokio blocking thread from the r2d2 pool.

### Adding or changing a shared command

1. Write the BODY as `pub async fn my_cmd(state: &ApiState, ...) -> AppResult<T>` in
   `crates/finsight-api/src/commands/` — command logic lives here, never in the wrapper.
2. Add a thin wrapper in `crates/finsight-app/src/commands/` that delegates to it via
   `&state.api`, with `#[tauri::command]` + `#[specta::specta]` (must be `pub async fn`).
3. Register in `build_specta_builder()` → `collect_commands![..., commands::mymod::my_cmd]` in `crates/finsight-app/src/lib.rs`
4. **Add a `finsight-server` route**: one match arm in `crates/finsight-server/src/dispatch.rs`
   using the strict `arg(&p, "camelCaseKey")` convention, plus the command name in
   `SUPPORTED` (or `UNSUPPORTED` if it genuinely can't work over HTTP — e.g. it takes a
   client-supplied filesystem path). Skipping this fails `tests/parity.rs`.
5. `cargo run -p finsight-tauri --bin export_bindings` — regenerates `ui/src/api/bindings.ts`

### Database migrations

SQL files in `crates/finsight-core/migrations/` named `V00N__description.sql`. Refinery (`embed_migrations!`) discovers them by filename prefix. Check the directory for the current highest version and name the next migration one higher.

### Frontend data flow

```
ui/src/api/bindings.ts   ← generated, never edit
ui/src/api/client.ts     ← re-exports bindings (import from here, not bindings directly)
ui/src/api/httpBackend.ts← server-mode invoke/event shim over HTTP + SSE
ui/src/api/auth.ts       ← plain REST client for `/api/auth/*`
ui/src/api/hooks/        ← tanstack-query wrappers (useTransactions, useBudgetEnvelopes, etc.)
ui/src/pwa/              ← seven-day IndexedDB query persistence (AES-GCM encrypted
                            at rest via cacheCrypto.ts) + online state + installed-PWA
                            surfaces: badge.ts/useAppBadge.ts (icon badge),
                            shareTarget.ts (OS share sheet), push.ts (Web Push)
ui/public/*-sw.js        ← plain-JS service worker handlers pulled into the generated
                            Workbox SW via `workbox.importScripts` (vite.config.ts).
                            NOT bundled — they cannot import from src/, so their
                            contracts are pinned by tests in ui/src/pwa/
ui/src/screens/          ← one file per screen, consumes hooks
ui/src/components/       ← shared: Sidebar, CommandPalette, TransactionDrawer, Drawer, Icons
ui/src/components/copilot ← Copilot generative-UI: cards/ (one per block kind) + agUi/artifacts.ts (Zod validation) + renderers
ui/src/state/tweaks.ts   ← zustand store for theme/density/accent/privacy (persisted to localStorage)
```

### Copilot generative-UI blocks

The Copilot renders **typed, validated finance blocks** natively (not just markdown). The block union is the Rust `AgentResponseBlock` enum (`#[serde(tag="kind")]`) in `crates/finsight-api/src/commands/agent.rs` (the `finsight-app` module of the same name only re-exports it); the mirror is the Zod `CopilotResponseBlockSchema` in `ui/src/components/copilot/agUi/artifacts.ts`, rendered by one card per kind in `ui/src/components/copilot/cards/`. **When you add or change a block, keep Rust bounds, the Zod schema, and the card in lockstep** (there's a Rust↔Zod parity corpus test). Numbers for grounded blocks (e.g. accountsOverview, spendingReview) are server-synthesized from `finsight-core`, not trusted from the model; the model may also be pushed to structured JSON output on final-answer turns when the provider supports it (probe-gated, with a heal/fallback net).

### TypeScript type field naming

**Inconsistency to know about:** The `Transaction` type in bindings uses **snake_case** (`t.merchant_raw`, `t.posted_at`, `t.amount_cents`) because its Rust struct lacks `rename_all`. Most other types (e.g. `BudgetEnvelope`, `CategoryWithSpending`, `TxnFilterInput`) use **camelCase** via `#[serde(rename_all = "camelCase")]`. Always check `bindings.ts` when accessing fields on a newly encountered type.

### CSS conventions

- Design tokens: `var(--ink)`, `var(--ink-mute)`, `var(--ink-faint)`, `var(--line)`, `var(--elevated)`, `var(--accent)`, `var(--negative)`, `var(--surface-2)` — defined in `ui/src/styles/tokens.css`. Never use hardcoded colors.
- Component utility classes: `.card`, `.chip`, `.btn`, `.tbl`, `.stat`, `.eyebrow`, `.toolbar`, `.stream`, `.goal-bar`, `.stub`, `.muted`, `.num`, `.money` — defined in `ui/src/styles/app.css`.
- Icons: import from `ui/src/components/Icons.tsx` using the `icon()` factory pattern.

### Key app-level patterns

- **Toasts:** `import { toast } from "sonner"` → `toast.success()`, `toast.error()`, `toast("text", { description, action })`
- **Slide-in panels:** reuse `ui/src/components/Drawer.tsx`
- **Privacy mode:** `useTweaks().privacy` — screens must blur amounts with `className="money"` (CSS handles blurring)
- **Tauri codegen wrappers must be async** even if the underlying work is synchronous; specta requires `pub async fn`
- **Server auth state:** the prior-session marker contains no credential; logout/401 must clear it and purge both the in-memory QueryClient and IndexedDB cache
- **Server secrets:** LLM keys and SimpleFIN access URLs belong in the authenticated user's SQLCipher settings through `finsight-api::secrets`, never in a process-global keychain slot

## Testing

Frontend tests use vitest + jsdom + `@testing-library/react`. Setup file: `ui/src/test/setup.ts`. The axe a11y tests produce jsdom canvas warnings in stderr — these are expected and non-fatal.

The two `keychain::tests::*` tests are marked `#[cfg_attr(target_os = "linux", ignore)]` — gnome-keyring 46 in headless CI never initialises its default Secret Service collection. They run normally on macOS and Windows. The `set_key_round_trip` test is additionally intermittently flaky under parallel execution on Windows (pre-existing, not caused by code changes).

A fresh git worktree is missing the gitignored `samples/` directory (CSV fixtures), so `prepare_csv_cmd`, `prepare_edge`, and `prepare_parity` (6 tests total) fail with a "path not found" error there — copy `samples/` in from the primary checkout to run them; this is an environment gap, not a code regression.

**Green bar:** run `cargo test --workspace`, `pnpm --filter ui test`, `pnpm typecheck`, and `pnpm build`. Test counts change as coverage grows; ignored tests must remain limited to the explicitly marked live-provider/live-DB/keychain cases.

## Financial Freedom Framework

The Copilot AI is guided by principles from six personal finance books. When writing prompts, building features, or extending the Copilot, align with these:

| Principle | Source | Implementation |
|---|---|---|
| Pay Yourself First (≥10%) | *Babylon* / *Ramsey* | Savings rate card on Today; Babylon nudge when <10%; Copilot priority #1 |
| Emergency Fund (3–6 months) | *Ramsey* / *Sethi* | Goals quick-fill button; `WellnessContext.emergency_fund_months` |
| Debt Snowball (smallest first) | *Ramsey* | `WellnessContext.debt_snowball` ordered by remaining balance ASC |
| Conscious Spending | *Sethi* | `spending_type` on categories (Need/Want/Saving/Investment); allocation donut on Budget |
| Compound Growth | *Hill* / *Kiyosaki* | Compound Growth Projector on Goals (10/20/30-year at 7% annual) |
| Behaviour > math | *Housel* | Copilot surfaces patterns and nudges, not just raw numbers |
| Financial Journey | *All* | `/journey` screen: 7 milestones from stability → freedom |

The Copilot system prompt (`crates/finsight-agent/src/planner.rs → build_system_prompt()`) embeds this framework. Financial context is populated by `crates/finsight-agent/src/context.rs → wellness_context()`.
