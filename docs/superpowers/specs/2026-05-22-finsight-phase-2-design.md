# FinSight — Phase 2 Design Document

**Date:** 2026-05-22
**Status:** Approved (brainstorming complete; ready for implementation plan)
**Builds on:** Phase 0+1 (commit `e370d3c`)
**Parent spec:** [2026-05-22-finsight-mvp-design.md](./2026-05-22-finsight-mvp-design.md)

## 1. Summary

Phase 2 turns the walking-skeleton foundation into a usable first-run experience. A new user installs FinSight, picks one of three onboarding paths, and ends up at Today with real data:

- **Try with sample data** — generates a procedural Mira & Adam household (6 accounts, ~250 transactions across 12 months).
- **Import a statement** — picks a CSV file, assigns columns to fields in a mapping dialog, imports with deduplication.
- **Add manually** — uses the same drawers the Accounts/Transactions screens will surface to power users.

Either path lands the user on `/today` with non-empty state. The wizard also walks them through confirming category names and pointing the agent at Ollama (or skipping that for later).

### In scope
- **CSV import provider** (`finsight-providers::csv`) — pure parser + column-mapping persistence + per-account remembered mapping.
- **Sample household generator** (`finsight-core::sample::seed_household`) — deterministic procedural generator, replaces the Phase 1 `walking_skeleton` in the app startup chain.
- **Onboarding wizard** — 4-step React flow with auto-redirect from app start when `accounts` is empty.
- **Manual entry drawers** — `AccountDrawer`, `TransactionDrawer` shared between Onboarding step 2 and the Accounts/Transactions screens.
- **Tauri commands** for manual creates, CSV preview, CSV import, sample-household seeding, and onboarding-state probing.
- **V002 migration** adding `imports` + `csv_import_mappings` tables.

### Explicitly out of scope (each gets its own follow-on spec when needed)
- **OFX / QIF** — separate spec when user demand surfaces. CSV covers ~95% of real-world bank exports.
- **Plaid / SimpleFin** sync providers — Phase 4+. The `SyncProvider` trait is in place from Phase 0.
- **CSV export / round-trip portability** — Phase 6 polish.
- **Editing existing accounts/transactions** — Phase 3 (couples with the categorizer's writes; design together).
- **Ollama `pull` orchestration** — surface the command, don't run it from the app.
- **Audit-log writes from imports and manual creates** — table exists from V001; first writers land in Phase 3 when the audit story gets built end-to-end.
- **Re-import warning when the exact same filename re-appears** — defer.
- **Multi-account CSV** (one file → many accounts) — single-account per import in Phase 2.

## 2. Backend additions

### 2.1 New files

```
crates/finsight-core/src/
├── sample.rs                # NEW — generates Mira & Adam household
├── lib.rs                   # MODIFY — pub mod sample
└── repos/imports.rs         # NEW — insert/list/finish import rows

crates/finsight-providers/src/
├── lib.rs                   # MODIFY — re-exports CsvProvider, ImportSummary
├── error.rs                 # NEW — ProviderError
├── provider.rs              # NEW — flesh out SyncProvider trait (file name 'trait' is reserved)
└── csv/
    ├── mod.rs               # NEW — CsvProvider, CsvImportMapping, CsvPreview
    ├── parse.rs             # NEW — pure CSV row → NewTransaction
    └── mapping.rs           # NEW — CsvImportMapping persistence + dedup hash
```

The Phase 1 `walking_skeleton` seed in `finsight-core::seed` remains, but is **removed from the app startup chain**. `sample_household` replaces it as the "intentional demo path" triggered by the user.

### 2.2 V002 migration

`crates/finsight-core/migrations/V002__import_history.sql`:

```sql
CREATE TABLE imports (
  id            TEXT PRIMARY KEY,
  source        TEXT NOT NULL,       -- 'csv' | 'manual' | 'sample'
  filename      TEXT,                -- NULL for manual/sample
  account_id    TEXT REFERENCES accounts(id),
  started_at    TEXT NOT NULL,
  finished_at   TEXT,                -- NULL until run completes
  row_count     INTEGER NOT NULL DEFAULT 0,
  duplicates    INTEGER NOT NULL DEFAULT 0,
  error         TEXT
);
CREATE INDEX idx_imports_unfinished ON imports(finished_at) WHERE finished_at IS NULL;

CREATE TABLE csv_import_mappings (
  account_id    TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  mapping_json  TEXT NOT NULL,
  last_used_at  TEXT NOT NULL
);
```

Both migrations are forward-only. The `imports` table is referenced by the "an import didn't finish" recovery banner; `csv_import_mappings` lets re-imports skip the column-mapping step per-account.

### 2.3 Tauri commands (added to `finsight-app/src/commands/`)

```rust
// commands/accounts.rs
create_account(input: NewAccountInput) -> AccountSummary

// commands/transactions.rs
create_transaction(input: NewTransactionInput) -> Transaction

// commands/import.rs  (NEW module)
preview_csv_columns(path: String, skip_header_rows: u32) -> CsvPreview
import_csv(path: String, account_id: String, mapping: CsvImportMapping) -> ImportSummary
list_unfinished_imports() -> Vec<Import>

// commands/onboarding.rs  (NEW module)
get_onboarding_state() -> OnboardingState
seed_sample_household() -> SeedSummary
```

`OnboardingState`:
```rust
pub struct OnboardingState {
    pub account_count: i64,
    pub category_count: i64,
    pub completion_marked: bool,   // settings flag set when wizard finishes
}
```

`CsvPreview`:
```rust
pub struct CsvPreview {
    pub headers: Option<Vec<String>>,   // Some if first row looked like headers
    pub rows: Vec<Vec<String>>,         // first 10 data rows, regardless
    pub detected_delimiter: char,        // ',' or ';' or '\t'
    pub total_rows: u32,                 // up to 10_000; capped to avoid scanning huge files
}
```

`ImportSummary`:
```rust
pub struct ImportSummary {
    pub import_id: String,
    pub rows_imported: u32,
    pub rows_skipped_duplicates: u32,
    pub errors: Vec<RowError>,
}

pub struct RowError {
    pub row_number: u32,       // 1-indexed, includes skipped header rows
    pub reason: String,
}
```

### 2.4 Events emitted

- `import.progress { import_id, rows_done, rows_total }` — emitted every 50 rows during CSV import.
- `import.complete { import_id }` — emitted when the import row's `finished_at` is set. Frontend invalidates `transactions`, `accounts`, `today-summary` queries.
- `onboarding.complete` — emitted when wizard finishes. Frontend invalidates queries.

### 2.5 Duplicate detection

`mapping.rs` computes a duplicate hash per row before insert:

```rust
fn dedup_hash(account_id: &str, posted_at: &DateTime<Utc>, amount_cents: i64, merchant_raw: &str) -> u64 {
    // SipHash via DefaultHasher — deterministic per process, fine for in-memory dedup within an import.
    // For cross-import dedup we use a SQL EXISTS check.
}
```

Cross-import deduplication: before inserting, the importer runs
```sql
SELECT 1 FROM transactions WHERE account_id = ?1 AND posted_at = ?2 AND amount_cents = ?3 AND merchant_raw = ?4
```
If a row exists, increment `rows_skipped_duplicates` and skip insertion. This lets the user re-import overlapping statements safely.

## 3. Frontend additions

### 3.1 Routing changes

`App.tsx` adds an effect:
```tsx
const { data: onboarding } = useOnboardingState();
const navigate = useNavigate();
const location = useLocation();
useEffect(() => {
  if (!onboarding) return;
  const shouldShowOnboarding = onboarding.account_count === 0 && !onboarding.completion_marked;
  if (shouldShowOnboarding && location.pathname !== "/onboarding") {
    navigate("/onboarding", { replace: true });
  }
}, [onboarding, location.pathname]);
```

After the wizard finishes (or after sample-data seeding), the wizard navigates to `/today` and the effect won't redirect because account_count is now > 0.

### 3.2 New components

```
ui/src/
├── components/
│   ├── Drawer.tsx                # generic right-slide drawer (focus trap, ESC, backdrop click)
│   ├── AccountDrawer.tsx
│   ├── TransactionDrawer.tsx
│   ├── FilePicker.tsx            # wraps @tauri-apps/plugin-dialog
│   └── ImportProgress.tsx        # listens to import.progress, shows a quiet pill
├── screens/
│   ├── Onboarding.tsx            # 4-step wizard shell
│   ├── Accounts.tsx              # list + "Add account" → AccountDrawer
│   ├── Transactions.tsx          # MODIFY: + "Import CSV", + "Add transaction"
│   └── onboarding/
│       ├── StepWelcome.tsx
│       ├── StepConnect.tsx       # sample / import / manual branching
│       ├── StepCategories.tsx
│       ├── StepAgent.tsx
│       └── ImportMappingDialog.tsx
├── state/
│   └── onboarding.ts             # Zustand: current step, in-progress mapping draft
└── api/hooks/
    ├── accounts.ts               # ADD useCreateAccount
    ├── transactions.ts           # ADD useCreateTransaction, useImportCsv
    ├── onboarding.ts             # useOnboardingState, useSeedSampleHousehold
    └── csv.ts                    # usePreviewCsvColumns
```

### 3.3 Drawer component contract

```tsx
interface DrawerProps {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  width?: number;  // default 480
}
```

Drawer behavior:
- Slides in from the right.
- Backdrop with `pointer-events: auto`, dimmed background.
- ESC closes; backdrop click closes.
- Focus trap inside the drawer; focus restores to the trigger on close.
- `aria-modal="true"`, `role="dialog"`, `aria-labelledby` pointing to the title.

### 3.4 Onboarding wizard flow

**`Onboarding.tsx` orchestrates state + step rendering:**
```tsx
type OnboardingStep = "welcome" | "connect" | "categories" | "agent";
```

State machine: linear, but the user can revisit completed steps via a step indicator at the top. Cannot skip forward past an incomplete step.

**Step 1 · Welcome**
- Title: "A quiet way to understand your money"
- Short paragraph (3-4 sentences) framing the product
- Primary CTA: `[Get started →]` (advances to Step 2)
- Tertiary link: `Try with sample data` — calls `seed_sample_household`, sets `completion_marked = true` in settings, navigates to `/today`

**Step 2 · Connect your money**
- Three cards:
  1. **Import a statement** — opens FilePicker, then `ImportMappingDialog`. After successful import, shows "1 account, 47 transactions" status. User can repeat.
  2. **Add manually** — opens `AccountDrawer`, then `TransactionDrawer` in a loop until user clicks "Done." Status updates the running count.
  3. **Skip for now** — proceeds to Step 3 (categories are still useful even with no data)
- Right pane shows running tally: "N accounts added, M transactions imported"
- `[Continue →]` enabled once any of the three actions completed (or user clicks "Skip for now")

**Step 3 · Confirm your categories**
- Pre-populated list of 10 starter categories (the same set used by `sample_household`):
  - Fixed: Housing, Utilities, Subscriptions
  - Daily: Groceries, Dining, Transport
  - Lifestyle: Shopping, Travel, Gifts
  - Wellbeing: Health
- Each row: icon, label (inline-editable), group dropdown, delete button
- "+ Add category" button appends a blank row
- `[Use these →]` writes to DB (INSERT OR IGNORE — won't clobber existing categories from sample/import path), advances to Step 4

**Step 4 · Set up the agent**
- On mount: probe `http://localhost:11434/api/tags` with 2s timeout
- **Ollama reachable:**
  - List installed models, show as a dropdown ("Pick a completion model")
  - Check for `nomic-embed-text` in the list
  - If `nomic-embed-text` is missing: show "Run `ollama pull nomic-embed-text` in your terminal, then refresh" + `[Refresh]` button
  - When both ready: `[Use Ollama →]`
- **Ollama not reachable:**
  - "We couldn't find a local model. FinSight uses [Ollama](https://ollama.com) for private agent features."
  - `[Install Ollama →]` link (opens in default browser via Tauri's opener plugin)
  - `[Configure later →]` link advances anyway
- Writes `settings.llm_provider = { kind: "ollama", base_url, completion_model, embedding_model }` or `{ kind: "unconfigured" }`
- Sets `completion_marked = true`
- Emits `onboarding.complete`
- Navigates to `/today`

### 3.5 ImportMappingDialog flow

Triggered by:
- Onboarding Step 2 → "Import a statement" card
- Transactions screen → "Import CSV" button

Flow:
1. `FilePicker.tsx` opens an OS file dialog (CSV filter).
2. On select, parent renders `<ImportMappingDialog path={path} />` as a full-screen overlay (not a drawer — needs more horizontal space for the column preview table).
3. Dialog calls `usePreviewCsvColumns({ path, skip_header_rows: 1 })` → renders preview.
4. Mapping form fields:
   - **Account** — dropdown of existing accounts, or "+ Add new account" inline (opens `AccountDrawer` in a nested overlay)
   - **Skip header rows** — number input, default 1, drives the preview re-fetch
   - For each column in the preview, a dropdown: Date / Amount / Merchant / Notes / Category / Skip
   - **Date format** — segmented control with 6 common formats + "Custom" text input
   - **Amount sign convention** — radio: "Negative = outflow" / "Positive = outflow" / "Separate debit/credit columns" (latter enables two extra column dropdowns)
5. Live validation: as the user assigns columns, show a parsed preview ("Row 1: −$8.42, Safeway, 2026-05-19") for the first 3 rows below the form.
6. `[Import N transactions]` button — disabled until required fields (account, date column, amount column, merchant column, date format, amount convention) are all set.
7. On click: closes the dialog, opens `ImportProgress` pill in the corner, calls `useImportCsv(...)`.
8. On `import.complete` event: toast "Imported N transactions, skipped M duplicates" with `[View]` button → navigates to /transactions filtered to that import.
9. On error: toast with first 3 row errors + "[Open log]" link to a small drawer listing all errors.

If `csv_import_mappings` has a row for the chosen account, the dialog preloads its values and shows a small "Saved mapping for this account · [Edit]" indicator at the top. Otherwise the user is mapping from scratch.

### 3.6 Manual entry drawers — fields

**AccountDrawer:**
- Bank (text, required)
- Name (text, required) — placeholder "e.g. Joint Checking"
- Type (segmented: Checking / Savings / Credit / Investment / Cash / Other) — default Checking
- Last 4 (text, 4 chars, optional)
- Currency (dropdown of common: USD / EUR / GBP / CAD / AUD) — default USD
- Opening balance (currency input, accepts negative for credit cards)
- Owner (dropdown: from existing owners + "+ New owner" inline text input) — defaults to "joint" if no other accounts; otherwise the most-recent owner

**TransactionDrawer:**
- Account (dropdown, required) — defaults to current filter context if drawer opened from Transactions
- Date (date picker, required) — defaults to today
- Amount (currency input, required) — sign toggle ("Inflow / Outflow") defaults to Outflow
- Merchant (text with autocomplete from existing merchants, required)
- Category (dropdown grouped by category_group, optional) — defaults to "Uncategorized"
- Notes (textarea, optional)

Both drawers use `react-hook-form` + `zod` schemas. Submit calls the corresponding `useCreateAccount` / `useCreateTransaction` mutation; on success invalidates relevant queries and emits a toast.

## 4. Error handling

### 4.1 CSV parse errors

- **Malformed rows** (wrong column count, unparseable date, unparseable amount) are collected into `ImportSummary.errors`, not fatal. Rows that *can* be parsed still import.
- **Empty file / no rows** — surfaced before mapping ("This file is empty"); the dialog doesn't open.
- **Date format mismatch on the entire file** — the live preview will be empty/red. Surfacing happens at preview time, not after a full import wastes the user's time.
- **All-rows-failed import** — `ImportSummary.errors` is populated, `rows_imported = 0`. Toast: "Couldn't import any rows. First error: <reason>." with "View all errors" link.

### 4.2 Duplicate detection

- Silent skip (counted), not surfaced as an error. Toast at end: "Imported 12, skipped 3 duplicates."

### 4.3 Import interrupted

- `imports` row has `finished_at = NULL` when the import task starts; gets set when the task finishes (or errors).
- On app launch, the App component queries `list_unfinished_imports()` once. If any exist, a quiet banner appears: "An import didn't finish last time. [Discard] [View]." Discard just sets `finished_at` + `error = 'discarded'`.
- We do **not** roll back partially-imported rows. They are already deduplicated against, so re-importing the same file is safe and idempotent.

### 4.4 Onboarding interrupted

- State lives in the DB, not in Zustand persistence. If the user quits mid-wizard:
  - `accounts` count > 0 → wizard won't auto-show; user can manually navigate to `/onboarding` to continue if they want
  - `accounts` count == 0 AND `completion_marked` false → wizard auto-shows again on next launch
  - `completion_marked` true (set by clicking "Use these" in Step 3 or finishing Step 4) → wizard never auto-shows again; available via Settings link

### 4.5 Ollama probe

- 2-second `fetch` timeout on `http://localhost:11434/api/tags`. Tauri command wraps it.
- Any error (timeout, refused, malformed JSON) is treated as "not reachable" — the Ollama-unreachable branch of Step 4 renders.
- No partial states: either the user picks a model, or they skip.

### 4.6 Permission errors

- File picker denied or path unreadable → toast "Couldn't read the file. Please try again."
- Disk write errors during import → import marked failed, `error` column populated, banner shown on next launch.

## 5. Testing

### 5.1 Rust

- **`finsight-core::sample`** — unit test asserts the seed is deterministic (same RNG seed → same row count and same first row's `id`/`merchant_raw`). Integration test: open empty encrypted DB, run migrations, run `seed_household`, assert 6 accounts + 250+ transactions.
- **`finsight-core::repos::imports`** — CRUD test for insert/finish/list_unfinished.
- **`finsight-providers::csv::parse`** — pure function, ≥10 unit tests against fixture rows covering: standard CSV, semicolon-delimited, quoted commas, MM/DD/YYYY dates, separate debit/credit columns, signed-positive convention, missing optional columns.
- **`finsight-providers::csv::CsvProvider::import`** — integration test against a fixture file: import once, count rows; import again, assert all skipped as duplicates.
- **`finsight-app::commands::import`** — happy-path command test + error-path (nonexistent file).

Test fixtures live at `crates/finsight-providers/tests/fixtures/csv/`:
- `chase-checking.csv`
- `amex-card.csv`
- `mercury-checking.csv`
- `mint-export.csv`
- `personal-capital.csv`
- `simple-semicolon.csv`

### 5.2 Frontend

- **`Drawer.test.tsx`** — opens, ESC closes, backdrop click closes, focus trap, focus restoration.
- **`AccountDrawer.test.tsx`** — form validation, submit calls mutation.
- **`Onboarding.test.tsx`** — Welcome step renders, "Try sample" path calls hook, advances to Step 2.
- **`ImportMappingDialog.test.tsx`** — preview rendering, column assignment, validation gating the Import button, parsed preview live-updates.

Tauri invoke remains mocked at the `@tauri-apps/api/core` boundary as established in Phase 1.

### 5.3 CI

No changes to CI — Phase 1's matrix already runs `cargo test --workspace`, `pnpm test`, fmt, clippy, tauri debug build across all three OSes. Phase 2 tests slot in automatically.

## 6. Build order

Effort estimates assume a developer fluent in both Rust and React. Not calendar weeks.

### Phase 2.0 — Backend foundations (~1 effort-week)
- V002 migration (`imports`, `csv_import_mappings`)
- `finsight-core::sample::seed_household` with determinism test
- `finsight-core::repos::imports`
- `finsight-app::commands::accounts::create_account` + `transactions::create_transaction` + `onboarding::*`
- Regenerate `bindings.ts`
- All tests + clippy + fmt green

**Exit:** `cargo run --bin finsight` with a deleted DB → setup chain skips seed → empty DB. `seed_sample_household` invocable via the dev console.

### Phase 2.1 — CSV provider (~1 effort-week)
- `finsight-providers::error::ProviderError`
- `finsight-providers::csv::parse::parse_row` with ≥10 fixture tests
- `finsight-providers::csv::CsvProvider` (preview + import + persisted mapping)
- `finsight-app::commands::import::preview_csv_columns`, `import_csv`, `list_unfinished_imports`
- `import.progress` + `import.complete` events
- Cross-import dedup via SQL EXISTS check

**Exit:** importing a Chase CSV via a manual `tauri invoke` from the dev console produces real transactions visible on the Transactions screen.

### Phase 2.2 — Onboarding shell + Welcome + Sample (~3-4 effort-days)
- `App.tsx` auto-redirect effect
- `useOnboardingState` + `useSeedSampleHousehold` hooks
- `Onboarding.tsx` shell with 4-step indicator
- `StepWelcome.tsx` with both CTAs wired
- `Settings.tsx` gets a "Re-run onboarding" button (still mostly stub otherwise)

**Exit:** fresh install → onboarding auto-opens → clicking "Try with sample data" lands on `/today` with the Mira & Adam household visible.

### Phase 2.3 — Manual entry drawers (~3-4 effort-days)
- `Drawer.tsx` with focus management
- `AccountDrawer.tsx`, `TransactionDrawer.tsx` with `react-hook-form` + `zod`
- Wired into `Accounts.tsx` and `Transactions.tsx`
- Wired into Onboarding Step 2 via `StepConnect.tsx`'s "Add manually" card
- Tests for Drawer + form validation

**Exit:** user can add accounts and transactions from the Accounts/Transactions screens AND from Step 2 of Onboarding.

### Phase 2.4 — Import flow UI (~1 effort-week)
- `FilePicker.tsx` wrapping `@tauri-apps/plugin-dialog`
- `usePreviewCsvColumns`, `useImportCsv` hooks
- `ImportMappingDialog.tsx` with live parsed preview
- `ImportProgress.tsx` listening to `import.progress` event
- Wired into Onboarding Step 2 + Transactions "Import CSV" button
- Unfinished-import banner on `App.tsx`

**Exit:** importing a CSV through the UI produces real transactions visible on `/transactions` with deduplication working across re-imports.

### Phase 2.5 — Categories + Agent steps + polish (~3-4 effort-days)
- `StepCategories.tsx` with inline-edit + add/delete
- `StepAgent.tsx` with Ollama probe (`/api/tags` via Tauri command for CORS-safety)
- Settings stores `llm_provider` config
- Accessibility audit on new components (focus management, ARIA, keyboard nav)
- Toast notifications wired for import complete, account created, etc.

**Exit:** all four onboarding steps work end-to-end on a fresh install.

## 7. Risks

1. **CSV parsing edge cases** — banks export wildly different formats. The 6 fixture files cover common cases but the long tail will surface real bugs in user testing. Mitigation: the row-level error reporting in `ImportSummary` lets users see exactly which rows failed and why.
2. **Ollama probe latency on offline machines** — a 2s timeout is reasonable but feels long. Mitigation: render the loading state with a "Checking for Ollama…" message.
3. **Drawer focus trap edge cases** — react-focus-lock can conflict with inputs inside modal dialogs. Mitigation: test thoroughly with the nested "AccountDrawer inside ImportMappingDialog" case.
4. **Determinism of `sample_household`** — RNG seeded from a constant string. If `rand` updates its algorithm, deterministic test assertions will break. Mitigation: pin `rand`'s minor version and use `ChaCha20Rng` explicitly (not `thread_rng`).
5. **Migrations against the Phase 1 walking-skeleton DB** — V002 must apply cleanly even if `accounts` already has rows from Phase 1's auto-seed. Mitigation: V002 only adds tables; no schema changes to existing tables. Test by booting a Phase 1 build to seed, then running V002.

## 8. Open follow-ups (tracked for later phases)

- Inline styles in `Today.tsx` + `Transactions.tsx` still need porting to CSS classes (carried from Phase 1).
- `TweaksPanel.tsx` UI surface still missing (theme engine exists but user has no way to flip it).
- Mixed snake_case/camelCase across tauri-specta types — should pick one and apply consistently.
- `cargo install tauri-cli` was not added to CI; relies on the `@tauri-apps/cli` npm package via pnpm. Worth a note in CONTRIBUTING.
