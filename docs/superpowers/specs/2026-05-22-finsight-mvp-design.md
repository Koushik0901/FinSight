# FinSight MVP — Design Document

**Date:** 2026-05-22
**Status:** Approved (brainstorming complete; ready for implementation plan)

## 1. Summary

FinSight is a local-first personal finance application — cross-platform desktop now, mobile later. It's a "quiet, private instrument" for understanding your money: show the signal, not the verdict; earn trust through restraint. Agentic AI is the substrate (categorization, anomaly detection, ⌘K Ask), never the centerpiece.

This document specifies the **MVP slice**. The full design covers 13 screens and many agentic features; the MVP delivers a working vertical slice with four primary screens plus onboarding and settings, end-to-end real data flow, and three agent surfaces. Everything else is a follow-on spec.

### What's in the MVP
- **Screens:** Today, Accounts, Transactions, Budget + Categories, Settings, Onboarding
- **Ingestion:** Manual entry + CSV/OFX/QIF import (pluggable provider trait reserves slots for Plaid/SimpleFin in follow-on specs)
- **Agent:** Auto-categorization with confidence, ⌘K command palette with Ask, statistical anomaly detection with LLM-narrated explanations
- **LLM provider:** Pluggable trait. Ollama default (local); Anthropic Claude opt-in (cloud). Selected in Settings.
- **Storage:** SQLite via SQLCipher (encrypted at rest), key in OS keychain
- **Distribution:** Tauri 2 desktop (macOS, Windows, Linux)

### Explicitly deferred to follow-on specs
Recurring (3 views), Goals (5 types), Scenarios (NL what-ifs), Reports (customizable widgets), Rules engine, Plan-Next-Month ritual, Trip mode, Sinking funds, Reimbursable transactions, FIRE calculator, Sankey/YoY visualizations, Plaid/SimpleFin integrations, Mobile (Tauri 2 mobile), Multi-currency UI (FX schema stays in MVP for forward compat).

## 2. Product context

The full product brief is preserved in `design/plutus/chats/chat1.md`. The values that drive design and engineering decisions:

- **Show the signal, not the verdict.** Never moralize about spending. Never gamify saving.
- **Earn trust through restraint.** Reserve emphasis for moments that genuinely warrant attention.
- **Respect privacy.** Local-first. Data does not leave the user's machine unless they explicitly opt in.
- **Be honest about money.** Real balances, real cash flow, real runway. No softening.
- **Onboarding is a feature, not a hurdle.** Plain language. Progressive disclosure. Never expose internals.
- **Power lives one layer deeper.** Rules and automations exist for users who want them; they never clutter the surface presented to a beginner.

Visual identity (carried forward from the prototype): dark-first, near-black surfaces, lime accent (`#C9F950` default), Geist Sans for UI, Geist Mono for tabular numbers, no Instrument Serif. Six accent options, two themes (dark/light), two densities (cozy/compact), privacy mode (⌘. blurs amounts).

## 3. Architecture

### 3.1 Shape

Rust workspace + React/TypeScript frontend, wrapped in Tauri 2.

```
finsight/
├── src-tauri/                       # Tauri shell (thin)
│   ├── tauri.conf.json
│   └── src/main.rs                  # registers crates::app
├── crates/
│   ├── core/                        # domain types, SQLCipher, migrations, repos
│   ├── providers/                   # data ingestion (CSV/OFX/QIF; trait for future Plaid/SimpleFin)
│   ├── agent/                       # LLM provider trait + impls, categorizer, anomaly, palette
│   └── app/                         # Tauri command handlers, event emission, glue
├── ui/                              # React + Vite frontend
│   ├── src/
│   │   ├── api/                     # tauri-specta bindings + Tanstack Query hooks
│   │   ├── state/                   # Zustand stores
│   │   ├── screens/
│   │   ├── components/
│   │   ├── styles/
│   │   └── main.tsx
└── resources/
    └── sample-data/                 # baked Mira & Adam DB for "Try sample"
```

### 3.2 Why this shape

- **Clean swap points.** `CompletionProvider`, `EmbeddingProvider`, and `SyncProvider` are pure traits in their own crates. Swapping Ollama → Anthropic or adding Plaid does not touch `core`, `app`, or the frontend.
- **Independently testable.** Each crate exposes a small public API; no crate depends on Tauri except `app`.
- **Type contract.** `tauri-specta` generates TypeScript types from Rust command signatures. There is one source of truth for the data shapes that cross the IPC boundary.

### 3.3 Crate responsibilities

| Crate | Owns | Depends on |
|---|---|---|
| `core` | DB connection pool, migrations, domain models, repositories, SQLCipher key loading | (none) |
| `providers` | `SyncProvider` trait, `CsvProvider` impl, OFX/QIF parsers | `core` (types only) |
| `agent` | `CompletionProvider` + `EmbeddingProvider` traits, `OllamaProvider`, `AnthropicProvider`, categorizer, anomaly detector, palette query handler | `core` (repos), `reqwest` |
| `app` | Tauri commands, event emission, lifecycle wiring | all of the above + `tauri` |

## 4. Data layer

### 4.1 Database

- **Engine:** SQLite (latest) compiled with SQLCipher.
- **Driver:** `rusqlite` with the `sqlcipher` feature, plus `r2d2` for connection pooling. (`sqlx` was considered and rejected: `sqlx` builds against vanilla `libsqlite3-sys`; getting it to link `libsqlcipher` is fragile and breaks on `sqlx` updates. `rusqlite + sqlcipher` is the path that actually works across mac/Windows/Linux. We lose compile-time query macros but gain a working build.)
- **Async bridge:** queries run on a `tokio::task::spawn_blocking` boundary inside the repo layer so the synchronous `rusqlite` API does not block the Tauri runtime.
- **At-rest encryption:** SQLCipher with a 32-byte random key per install. Key stored in OS keychain via `keyring-rs` (Keychain on macOS, Credential Manager on Windows, Secret Service on Linux).
- **Keychain failure path:** if the OS denies keychain access (typical first-run prompt the user can deny), the app surfaces a modal explaining the choice and offers a single retry. A second denial blocks app start with a clear "FinSight needs keychain access to keep your data encrypted" screen — no plaintext fallback path. Documented in Settings → Privacy.
- **Pragmas:** `journal_mode=WAL`, `synchronous=NORMAL`, `mmap_size=268435456` (256MB), `cache_size=-65536` (64MB), `foreign_keys=ON`, `busy_timeout=5000`. `sqlite-vec` extension loaded per-connection via `load_extension` (enabled at pool init; SQLCipher allows this but extension loading is off by default).
- **Migrations:** `refinery` (or hand-rolled migration runner — choice deferred to plan) against `crates/core/migrations/`. Forward-only in production; reversible in dev.

### 4.2 Schema

Money is **always `INTEGER` cents**, never float. Multi-currency uses `amount_cents` + `currency` columns with a daily `fx_rates` snapshot table. Soft delete via `archived_at` so historical reports stay correct.

**Tables:**

| Table | Purpose |
|---|---|
| `accounts` | Owner (joint/mira/adam/…), bank, type, name, last4, currency, color, `archived_at` |
| `account_balances` | `(account_id, as_of_date, balance_cents)` — one row per day per account; sparkline = `ORDER BY as_of_date` |
| `assets` | Manual assets (home, vehicles, crypto). `value_cents` (fiat-equivalent net worth contribution), `quantity_scaled` (INTEGER, 8-decimal scaled for crypto / fractional units; NULL for non-fractional), `unit` (e.g. `BTC`, `ETH`, NULL for fiat-valued), `last_updated_at`, note. Net-worth math reads `value_cents`; `quantity_scaled + unit` are display-only for the asset card. |
| `liabilities` | Mortgage/loan/card. `balance_cents`, `apr_bps`, `monthly_payment_cents`, `payoff_date` |
| `category_groups` | Fixed / Daily / Lifestyle / Wellbeing |
| `categories` | `id`, `group_id`, label, color, icon, `archived_at` |
| `merchants` | `canonical_name`, `color`, `initials` (auto-computed display fallback). MVP does **not** ship a logo pack and does **not** fetch logos at render time (local-first). Transaction list renders a colored square with merchant initials. A future spec may add a bundled logo pack. |
| `transactions` | `account_id`, `posted_at`, `amount_cents` (signed; negative = outflow), `merchant_raw`, `merchant_id`, `category_id`, `status` (cleared/pending/manual), `notes`, `ai_confidence`, `ai_explanation`, `is_anomaly` |
| `transaction_splits` | `txn_id`, `category_id`, `amount_cents`, `note` |
| `transaction_attachments` | `txn_id`, `blob` (the file bytes, stored inline as BLOB), `mime`, `size_bytes`, `filename`. Attachments are stored inside the encrypted DB rather than on the filesystem so they inherit SQLCipher encryption. MVP caps individual attachments at 10MB and total attachments at 100MB; larger limits and on-disk encrypted storage come later if needed. |
| `transaction_tags` | `(txn_id, tag)` — e.g. `trip:italy-2026`, `reimbursable:work` |
| `categorizations` | Append-only audit: `txn_id`, `category_id`, `source` (rule/vector/agent/user), `confidence`, `at` |
| `budgets` | `month` (YYYY-MM), `mode` (envelope/tracking) |
| `budget_envelopes` | `budget_id`, `category_id`, `budgeted_cents`, `carryover_cents` |
| `rules` | `name`, `conditions` (JSON), `actions` (JSON), `enabled`, `source` (user/agent-proposed) |
| `agent_insights` | `kind`, `headline`, `summary`, `reasoning_chain` (JSON), `confidence`, `model`, `generated_at`, `reviewed_at`, `dismissed_at` |
| `agent_memory` | `key`, `value` (TEXT), `source` (correction/feedback/preference), `updated_at` |
| `imports` | `source`, `filename`, `started_at`, `finished_at`, `row_count`, `error` |
| `audit_log` | Append-only; what the agent changed, what the user changed |
| `settings` | Single-row table, `value` (JSON) — UI tweaks, LLM provider config, privacy prefs |
| `fx_rates` | `(currency, as_of_date, rate_to_base)` — daily snapshot, base = USD for MVP |

**Virtual tables:**
- `transactions_fts` — FTS5 over `merchant_raw + notes + concatenated tags`, kept in sync via AFTER triggers on `transactions`, `transaction_tags`
- `transactions_vec` — `sqlite-vec` virtual table; one row per transaction with an embedding generated when the txn is categorized

**Indices:**
- `transactions(posted_at DESC, account_id)` — timeline & account drilldown
- `transactions(category_id, posted_at)` — category breakdowns
- `transactions(merchant_id)` — merchant rollup
- `transactions(is_anomaly) WHERE is_anomaly = 1` — partial index for the anomaly feed
- `account_balances(account_id, as_of_date DESC)` — sparkline lookup
- `categorizations(txn_id, at DESC)` — audit history

### 4.3 Non-obvious decisions

- **`transactions.merchant_raw` is immutable.** `merchant_id` is the resolved canonical. Re-resolving never destroys the original string from the bank.
- **One `categorizations` audit table** records every category assignment with source. The agent can learn from user corrections without overwriting them.
- **`settings` is a single-row JSON column**, not EAV. Small surface, atomic writes, no schema churn for new tweaks.
- **`agent_insights.reasoning_chain` is JSON** because the design's reasoning-trace UI needs ordered steps with sources — this is the natural shape.

## 5. Agent workflow

### 5.1 LLM provider traits

Completions and embeddings are split into two traits because Anthropic does not expose an embeddings API (its docs route embeddings to Voyage). Keeping them separate also lets a user run completions against Anthropic while embeddings stay local.

```rust
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    async fn complete(&self, req: CompletionRequest) -> Result<CompletionResponse>;
    fn model_id(&self) -> &str;
    fn capabilities(&self) -> Capabilities; // supports_json_mode, max_context_tokens, …
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn model_id(&self) -> &str;
    fn dimensions(&self) -> usize;
}
```

`CompletionRequest` includes a `response_format: Option<JsonSchema>` field. `OllamaProvider` honors it via JSON mode; `AnthropicProvider` honors it via tool-use forced output or response prefill.

**Default configuration:**
- Completion: `OllamaProvider` pointed at `http://localhost:11434`, model `llama3.1:8b-instruct-q4_K_M` (configurable in Settings)
- Embedding: `OllamaProvider` (same endpoint), model `nomic-embed-text` (137M params, 768-dim, well-supported by Ollama, decent for short text)

`AnthropicProvider` is `CompletionProvider`-only. If selected for completions, the user keeps an Ollama embedding model installed for vector NN; we surface this in Settings ("Embeddings always use a local model — configure here"). The embedding model is small (~270MB) so this is reasonable even for users who otherwise prefer cloud completions.

**Provider selection** at agent-construction time from `settings`. Switching providers is a config change + restart of the agent task; no app restart needed.

**Onboarding pulls the embedding model on first run** so categorization works out of the box. The "Try with sample data" path skips this (sample DB ships with embeddings pre-computed).

**Failure handling:** if neither provider is reachable, the agent surfaces a quiet banner ("Local model not detected — install Ollama, or configure in Settings") and the categorizer falls back to `needs_review` for new transactions. The app remains fully functional without an LLM; the agent features simply pause.

### 5.2 Categorization

**Trigger:** transaction inserted (manual or CSV import). Async; never blocks the UI.

**Pipeline:**
1. **Rules first.** Check user-defined rules; on match, assign category with source `rule`.
2. **Vector nearest-neighbor.** Embed the transaction's `merchant_raw + amount + day-of-week`. Query `transactions_vec` for the 3 closest already-categorized transactions. If top-1 similarity > 0.92 AND top-3 majority agrees, assign with source `vector` and confidence = similarity.
3. **LLM batch classify.** Otherwise, batch up to 20 uncategorized transactions into one completion. Strict JSON schema:
   ```json
   {
     "results": [
       { "txn_id": "...", "category_id": "...", "confidence": 0.0, "rationale": "..." }
     ]
   }
   ```
   Schema-validate. On parse failure, retry once with a stricter system prompt. On second failure, mark all txns `needs_review` (no silent garbage).
4. **Write-back** with source `agent`. Transactions with confidence < 0.6 surface in the Today screen's "Needs a glance" row.
5. **Embed and store** the transaction in `transactions_vec` after classification (regardless of source). Embedding cache key = hash of `merchant_raw + rounded amount` so identical-looking txns reuse vectors.
6. **Emit Tauri event** `categorization.progress { done, total }` so the frontend can show a quiet indicator.

**Prompt content** includes: the user's category list (id + label + group + a one-line hint), a small set of recently categorized examples drawn from the user's own data (in-context learning), and the batch of transactions to classify. No chain-of-thought; the model returns rationale only on request.

### 5.3 Anomaly detection

**Detection is statistical. The LLM only writes the prose.**

Trigger: on app launch if the last scan completed more than 24 hours ago, plus on-demand from the Today screen's "Re-run scan" action. Desktop apps don't reliably run at fixed times, so we hook the launch event instead of a literal scheduler.

1. For each `(account_id, category_id)` pair, compute trailing 12-month median and MAD (median absolute deviation).
2. If this month's running total exceeds `median + 3·MAD`, flag the pair.
3. For each transaction in a flagged category, compute its z-score within the category over 12 months; flag any with `|z| > 3`.
4. For each flag, the LLM is called **once** with the relevant context (the transaction, the category's recent history, the merchant pattern) to produce a one-sentence explanation and a suggested follow-up.
5. Result lands in `agent_insights` with `kind = 'anomaly'`, full reasoning chain populated with the stats that drove the flag, and the LLM's prose.

This honors the design's "Insights = reasoning trace" UI: deterministic detection (the actual signals the agent had), LLM-narrated explanation (what the agent thinks it means).

### 5.4 ⌘K Ask

The hardest agent surface. Pattern: **structured retrieval → LLM synthesis with grounded context.**

1. **Intent classification.** Try a small regex/heuristic match first (e.g., `over \$\d+`, `since`, `category:`, `account:`). If not matched, a quick LLM call picks one of: `query` (returns a filtered list with optional viz), `summary` (returns prose + viz), `action` (proposes a state change), `navigate` (jump to a screen).
2. **Retrieval.** For `query` and `summary`: run a structured SQLite query (or a vec search via embeddings for semantic asks). Cap results to a context-safe size (e.g., 50 transactions max).
3. **Synthesis.** LLM receives the retrieved data + the original question, returns:
   ```json
   {
     "answer_prose": "...",
     "viz": { "kind": "list|bigNumber|bars|progress", "data": { /* per-kind */ } },
     "actions": [ { "kind": "navigate_to", "args": { "route": "/transactions" } } ]
   }
   ```
4. **Actions** are pre-defined, whitelisted commands (`navigate_to`, `create_rule_proposal`, `mark_anomaly_reviewed`, etc.). The LLM proposes; the user confirms by clicking. The LLM cannot execute arbitrary writes.

### 5.5 Agent task lifecycle

A single long-lived `tokio::spawn` task runs at app start and consumes an `mpsc::Receiver<AgentJob>` queue. Tauri commands enqueue jobs (categorize-batch, anomaly-scan, ask-question) and receive results either synchronously (via `oneshot`) for ⌘K Ask, or via emitted events (categorization, anomaly) for long-running work.

On provider config change, the agent task drops its current providers, rebuilds them from the new settings, and continues processing the queue. No app restart.

On unrecoverable provider error (e.g., Ollama process killed mid-batch), the in-flight job is marked failed in `imports`/`agent_insights` and the task continues. The frontend gets an `agent.error` event.

### 5.6 Agent boundaries

- No agent-initiated writes to the DB. Every change requires user confirmation.
- No streaming token-by-token in MVP — responses arrive whole, easier to validate.
- No multi-turn chat. Each ⌘K query is independent; memory is structured (`agent_memory`), not conversational.
- No tool-calling loops. One model call per surface, validated, done.

## 6. Frontend

### 6.1 Stack

- **Vite + React 18 + TypeScript (strict)**
- **Routing:** `react-router-dom` v6
- **Server state:** `@tanstack/react-query`
- **Client state:** `zustand` (palette, route, tweaks, privacy, toasts)
- **Forms:** `react-hook-form` + `zod`
- **Charts:** hand-rolled SVG primitives ported from the prototype (no Recharts/Visx dependency; the design idiom is custom gradients/glows)
- **Type generation:** `tauri-specta` — Rust command signatures compile to typed TS bindings
- **Styling:** ported `design/plutus/project/styles.css`, with CSS variables already factored for theme/accent/density/privacy

### 6.2 IPC pattern

Tauri commands are organized by domain, named with verbs. Representative set (full list in implementation plan):

```
list_accounts() -> Vec<AccountSummary>
get_account(id) -> AccountDetail
create_manual_account(input) -> Account
archive_account(id) -> ()

list_transactions(filter: TxnFilter) -> Page<Transaction>
get_transaction(id) -> TransactionDetail
update_transaction_category(id, category_id, source: "user") -> ()
split_transaction(id, splits) -> ()
import_csv(path, account_id, mapping) -> ImportSummary

get_budget(month) -> BudgetView
set_envelope(month, category_id, cents) -> ()

recategorize_transaction(id) -> CategorizationResult
ask_agent(question) -> AgentAnswer
rerun_anomaly_scan() -> ScanSummary

get_settings() -> Settings
update_settings(patch) -> ()
set_agent_providers(config: AgentProviderConfig) -> ()  // sets completion and embedding providers
```

**Events** (Rust → frontend):
- `categorization.progress { done, total }`
- `import.progress { rows_done, rows_total }`
- `insights.updated` — frontend invalidates Insights query
- `agent.activity { kind, text }` — drives Today's "agent activity" stream

### 6.3 Data-fetching conventions

- One Tanstack Query hook per command (generated from `tauri-specta` types), in `ui/src/api/`.
- Mutations invalidate related query keys explicitly (no global wipes). E.g. `updateTransactionCategory` invalidates `transactions`, `categories`, `budget`, `today-summary`.
- Optimistic updates for category edits and splits — UI snaps immediately; rollback on failure via toast.
- Tauri events trigger targeted invalidations.

### 6.4 Onboarding (4 steps)

1. **Welcome.** Short prose ("a quiet way to understand your money"), single primary action: "Get started." Tertiary: "Try with sample data."
2. **Connect your money.** Two cards: "Import a statement" (CSV/OFX/QIF picker → column mapping screen) or "Add manually" (account form). Repeatable — user can add more before continuing.
3. **Confirm your categories.** Pre-populated category list (10 starter categories grouped Fixed / Daily / Lifestyle / Wellbeing). User can rename, delete, add. Skippable.
4. **Set up the agent.** Detect Ollama; if found, pick a completion model from installed list and confirm `nomic-embed-text` is available (offer to pull it). If not found, show "Install Ollama" instructions + "Configure later" link. Skippable (agent stays paused until configured).

Finishes with "→ Today." Onboarding is re-launchable from Settings.

### 6.5 CSV import mapping

```rust
pub struct CsvImportMapping {
    pub posted_at_col: usize,                 // required
    pub amount_col: usize,                    // required; positive/negative convention picked below
    pub merchant_col: usize,                  // required
    pub notes_col: Option<usize>,
    pub category_col: Option<usize>,          // some banks export a category; used as hint, not authoritative
    pub balance_col: Option<usize>,           // some banks include running balance per row
    pub date_format: String,                  // strftime-style, e.g. "%Y-%m-%d"
    pub amount_convention: AmountConvention,  // PositiveIsInflow | NegativeIsInflow | SeparateDebitCredit { debit_col, credit_col }
    pub skip_header_rows: usize,              // typically 1
    pub remembered_as: Option<String>,        // when set, saved to settings keyed by account_id so re-imports skip the mapping step
}
```

The mapping UI presents the first 10 rows of the CSV in a table, lets the user assign each required field by clicking column headers, and lives at `/onboarding/import/map` plus `/transactions/import` for ongoing imports.

### 6.6 Sample data path

- `resources/sample-data/sample.sqlcipher` baked into the bundle, encrypted with a known test key.
- Onboarding "Try with sample data" copies the file to app data dir, writes the test key to keychain, reopens the pool.
- "Reset to my own data" wipes the file, generates a fresh key, runs migrations on an empty DB.

## 7. Cross-cutting concerns

### 7.1 Accessibility

- Keyboard-first navigation across sidebar, lists, palette, drawers (matches the design brief's non-negotiable accessibility requirement).
- Focus traps in palette + modal drawers.
- Reduced-motion preference honored: animations skip transforms; transitions become instant.
- High-contrast theme: dark theme already meets WCAG AA for text; light theme verified during Phase 6.
- Font scaling: `rem`-based throughout; respects browser zoom.
- Screen-reader: semantic landmarks (`<nav>`, `<main>`, `<aside>`), `aria-live="polite"` for the agent-activity stream, `aria-label` on icon-only controls.

### 7.2 Privacy

- All data lives in `~/<app-data>/finsight/data.sqlcipher`. Key in OS keychain. Nothing leaves the machine.
- Anthropic provider, if configured, is the **only** outbound network destination, and only when the user invokes ⌘K, categorization, or anomaly explanation while that provider is selected. Settings includes a clear indicator.
- Privacy mode (⌘.) replaces every numeric amount with a blurred placeholder. Toggleable globally; state in `settings.privacy`.
- "Screen-share mode" (deferred to follow-on spec) is foreshadowed by the privacy primitive.

### 7.3 Error handling

- All Tauri commands return `Result<T, AppError>`. `AppError` is a flat enum with `code`, `message`, `details`. The frontend renders an error toast with the message; details are logged.
- Long-running operations (CSV import, batch categorization) emit `*.error` events alongside `*.progress` events. Frontend shows an inline error in the progress UI rather than a global toast.
- LLM-provider unavailability is not an error — it's a known state surfaced as a quiet banner.

### 7.4 Logging

- `tracing` crate with file rotation under app data dir. INFO by default; DEBUG via env var.
- No transaction amounts or merchant names in logs at INFO level.
- A "Diagnostics" button in Settings copies the last N lines to clipboard.

### 7.5 Testing

- **`core`:** unit tests on repositories using an in-memory SQLite DB. Migration tests assert forward-only application.
- **`providers`:** parser tests with fixture CSV/OFX/QIF files covering 8–10 real-world bank exports.
- **`agent`:** trait-level tests using `MockCompletionProvider` + `MockEmbeddingProvider` that return canned responses; integration tests against Ollama gated by an env var.
- **`app`:** Tauri command tests via the Tauri test harness.
- **Frontend:** Vitest for hooks/utilities; Playwright for one E2E smoke (onboarding → import sample → see Today).
- **CI:** `cargo test`, `pnpm test`, `pnpm typecheck`, `pnpm lint`, `cargo clippy --all-targets -- -D warnings`.

## 8. Build order (walking skeleton)

The failure mode here is "every crate is gorgeous, nothing renders for three weeks." The build sequence is one thin vertical slice first, then thicken.

**Effort estimates below are focused-work weeks for someone fluent in both Rust and React; not calendar weeks.**

### Phase 0 — Bootstrap (1–2 effort-days)
- Cargo workspace, Tauri 2 init, Vite+React+TS scaffold
- **`rusqlite + sqlcipher` build green on mac/Windows/Linux CI matrix before any feature code.** This is the biggest single risk; de-risk on day one. `sqlite-vec` extension load also verified here.
- `tauri-specta` codegen pipeline
- Sidebar shell + routing + theme/accent/density wiring (ports prototype's `app.jsx` chrome)
- All routes render a stub panel

**Exit:** `pnpm tauri dev` opens a window, sidebar navigates 7 empty routes, theme tweaks work, a smoke test inserts and reads a row from an encrypted DB on all three platforms.

### Phase 1 — Walking skeleton (~1 effort-week)
- `accounts`, `transactions`, `categories` tables migrated
- Hard-coded seed: 1 account, 3 transactions, 4 categories
- `list_accounts`, `list_transactions` commands
- Today renders the runway number from the seed
- Transactions renders the list with merchant + amount

**Exit:** Data flows Rust → SQLCipher → command → query hook → render.

### Phase 2 — Ingest path (~1 effort-week)
- CSV import provider + `import_csv` command + column-mapping UI
- Onboarding: Welcome → Try sample / Import CSV → confirm mapping → finish
- Sample DB ship path working
- Manual account create + manual transaction add

**Exit:** A new user can install, choose import-or-sample, and see their data.

### Phase 3 — Agent foundation (~1 effort-week)
- `CompletionProvider` + `EmbeddingProvider` traits + `OllamaProvider` impl of both
- Categorizer pipeline: rules → vec NN → LLM batch, with progress events
- Settings: LLM provider config (Ollama URL test + model picker)
- Today's "agent activity" stream subscribed to events
- Low-confidence "Needs a glance" surface on Today

**Exit:** Importing a CSV triggers background categorization with visible progress; category labels with confidence land.

### Phase 4 — Budget + Categories (~1 effort-week)
- `budgets`, `budget_envelopes` migrations
- Budget screen: envelope cards grouped by Fixed/Daily/Lifestyle/Wellbeing (2026 card design from chat1)
- Categories screen: monthly breakdown + stream chart
- Envelope/tracking mode toggle persists

**Exit:** Spending in a category updates the envelope; under/over states render correctly.

### Phase 5 — Agent surfaces (~1 effort-week)
- Anomaly detection (stats + LLM prose)
- ⌘K palette: navigate + Ask (retrieval → synthesis)
- Insights screen with reasoning-trace cards

**Exit:** ⌘K answers "what did I spend on dining in May" with prose + bar viz; anomalies show on Today.

### Phase 6 — Polish + accessibility (3–5 effort-days)
- Privacy mode (⌘.) blurs amounts everywhere
- Keyboard nav across sidebar + lists
- Focus traps
- Reduced-motion honored
- Theme/density/accent tweaks panel
- Toast system

**Exit:** Keyboard-only flow through all MVP screens works; accessibility audit passes.

## 9. Risks

1. **SQLCipher build across mac/Windows/Linux** is the single biggest Phase 0 risk. `rusqlite + sqlcipher` is the chosen path; verify the CI matrix on all three targets on day one before any feature work.
2. **`sqlite-vec` extension loading** requires `load_extension` which is off by default. Enable per-connection at pool init. Verify it loads against the SQLCipher build (not just stock SQLite).
3. **Ollama not installed** is the common case for new users. Onboarding's step 4 handles this with a clear "Install Ollama" CTA + "Configure later" link. Categorization gracefully pauses with `needs_review` for new transactions until a provider is configured.
4. **CSV column mapping** — banks export wildly different schemas. The mapping UI (Section 6.5) must be flexible and remember mappings per account.
5. **Embedding generation cost** — embedding thousands of imported transactions takes time. Run in background with progress events; never block the UI. Cache embeddings by `hash(merchant_raw + rounded_amount)` so re-imports skip work.
6. **First-run Ollama embedding-model pull** is ~270MB for `nomic-embed-text`. Onboarding step 4 surfaces this with a progress indicator; user can skip and the agent stays paused.
7. **Keychain denial on macOS first launch** — handled by retry + hard-block flow (Section 4.1). Document the choice in Settings → Privacy.
8. **Tauri 2 maturity** — pin to a specific minor and test plugin compatibility (especially `tauri-plugin-store` and the keychain plugin) before depending on it.
9. **Mobile target shape** is out of scope but the layout system (CSS variables + responsive breakpoints already in the prototype) should not paint us into a desktop-only corner.

## 10. Out of scope (follow-on specs)

Each of these becomes its own spec → plan → implementation cycle:

- Recurring (3 views: calendar, list, subscription audit)
- Goals (5 kinds: save-by-date, build-balance, debt-payoff, spending-cap, sinking-funds)
- Scenarios (natural-language what-ifs)
- Reports (customizable widgets, Sankey, YoY, FIRE, savings rate, custom dashboards)
- Rules engine (regex matchers, OR conditions, webhook actions)
- Plan-Next-Month ritual (5-step monthly ritual)
- Trip mode + reimbursable transactions
- Plaid + SimpleFin sync providers
- Anthropic provider (the trait exists; this spec adds the UI + key handling + retry/streaming UX)
- Multi-currency UI
- Tauri 2 mobile (iOS, Android)
- iOS/Android home-screen widgets
- API access for power users
- Sync server (optional, self-hosted)
