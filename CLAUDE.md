# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Full dev environment (Tauri backend + Vite frontend with hot reload)
# NOTE: debug builds use an ISOLATED `<identifier>.dev` app-data dir and start
# EMPTY — they never touch the real production DB (see resolve_app_data_dir in
# crates/finsight-app/src/lib.rs). To test against real data, copy the prod DB
# into the `.dev` dir, or build --release.
pnpm tauri:dev

# Frontend only (no Tauri, faster for UI-only work)
cd ui && npm run dev

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

# Regenerate TypeScript bindings after ANY Rust command change (run from repo root)
cargo run -p finsight-tauri --bin export_bindings

# Build for production
cd ui && npm run build
```

## Architecture

### Rust workspace (5 crates)

**`crates/finsight-core`** — domain layer: models, SQLCipher DB pool, migrations, repository functions, settings KV store. All SQL lives here. No Tauri dependency.

**`crates/finsight-providers`** — CSV import parsers, LLM provider HTTP clients (`CompletionProvider` trait with Ollama / OpenAI-compat / Anthropic impls).

**`crates/finsight-agent`** — AI layer: Copilot context engine, planner, executor, recipe runner, categorizer pipeline, anomaly detection. Runs on a background Tokio task via `AgentHandle`.

**`crates/finsight-app`** — Tauri command surface. Commands in `src/commands/` call into `finsight-core` repos via the `run()` helper. `AppState` holds a `Db` clone and an `AgentHandle`. All commands registered in `build_specta_builder()` in `src/lib.rs`.

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

### Adding a Tauri command

1. Write `pub async fn my_cmd(...) -> AppResult<T>` in `crates/finsight-app/src/commands/`
2. Add `#[tauri::command]` and `#[specta::specta]` attributes
3. Register in `build_specta_builder()` → `collect_commands![..., commands::mymod::my_cmd]` in `crates/finsight-app/src/lib.rs`
4. `cargo run -p finsight-tauri --bin export_bindings` — regenerates `ui/src/api/bindings.ts`

### Database migrations

SQL files in `crates/finsight-core/migrations/` named `V00N__description.sql`. Refinery (`embed_migrations!`) discovers them by filename prefix. Check the directory for the current highest version and name the next migration one higher.

### Frontend data flow

```
ui/src/api/bindings.ts   ← generated, never edit
ui/src/api/client.ts     ← re-exports bindings (import from here, not bindings directly)
ui/src/api/hooks/        ← tanstack-query wrappers (useTransactions, useBudgetEnvelopes, etc.)
ui/src/screens/          ← one file per screen, consumes hooks
ui/src/components/       ← shared: Sidebar, CommandPalette, TransactionDrawer, Drawer, Icons
ui/src/components/copilot ← Copilot generative-UI: cards/ (one per block kind) + agUi/artifacts.ts (Zod validation) + renderers
ui/src/state/tweaks.ts   ← zustand store for theme/density/accent/privacy (persisted to localStorage)
```

### Copilot generative-UI blocks

The Copilot renders **typed, validated finance blocks** natively (not just markdown). The block union is the Rust `AgentResponseBlock` enum (`#[serde(tag="kind")]`) in `crates/finsight-app/src/commands/agent.rs`; the mirror is the Zod `CopilotResponseBlockSchema` in `ui/src/components/copilot/agUi/artifacts.ts`, rendered by one card per kind in `ui/src/components/copilot/cards/`. **When you add or change a block, keep Rust bounds, the Zod schema, and the card in lockstep** (there's a Rust↔Zod parity corpus test). Numbers for grounded blocks (e.g. accountsOverview, spendingReview) are server-synthesized from `finsight-core`, not trusted from the model; the model may also be pushed to structured JSON output on final-answer turns when the provider supports it (probe-gated, with a heal/fallback net).

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
- **Tauri commands must be async** even if the underlying work is synchronous; specta requires `pub async fn`

## Testing

Frontend tests use vitest + jsdom + `@testing-library/react`. Setup file: `ui/src/test/setup.ts`. The axe a11y tests produce jsdom canvas warnings in stderr — these are expected and non-fatal.

The two `keychain::tests::*` tests are marked `#[cfg_attr(target_os = "linux", ignore)]` — gnome-keyring 46 in headless CI never initialises its default Secret Service collection. They run normally on macOS and Windows. The `set_key_round_trip` test is additionally intermittently flaky under parallel execution on Windows (pre-existing, not caused by code changes).

A fresh git worktree is missing the gitignored `samples/` directory (CSV fixtures), so `prepare_csv_cmd`, `prepare_edge`, and `prepare_parity` (6 tests total) fail with a "path not found" error there — copy `samples/` in from the primary checkout to run them; this is an environment gap, not a code regression.

**Green bar:** 546 Rust tests (+12 ignored live-DB/keychain), 430 frontend tests, 0 TypeScript errors.

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
