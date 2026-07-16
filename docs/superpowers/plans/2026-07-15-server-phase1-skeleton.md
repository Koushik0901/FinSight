# FinSight Server — Phase 1 Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract FinSight's Tauri command surface into a transport-agnostic `finsight-api` crate and stand up a `finsight-server` axum binary + browser HTTP/SSE shim, so the full app runs in a plain browser against `http://localhost:8674` with a single hardcoded user.

**Architecture:** New tauri-free crate `finsight-api` holds `ApiState` (db + agent + provider + sync scheduler + data dir), the `AppError` type, a `FrameSink` event-emission trait, and every command body as a plain `pub async fn(&ApiState, args) -> AppResult<T>`. `finsight-app` keeps thin `#[tauri::command]` wrappers (so `bindings.ts` regenerates byte-identical). New crate `finsight-server` (axum) exposes `POST /api/rpc/{cmd}` (a match-dispatcher over the same functions), `GET /api/events` (SSE fan-out of what Tauri used to `emit`), `/api/health`, and serves `ui/dist`. The UI installs an HTTP-backed `__TAURI_INTERNALS__` shim (same trick as `ui/src/dev/mockBackend.ts`) whenever it runs in a browser without Tauri and without `?mock`.

**Tech Stack:** Rust (axum 0.8, tower-http, tokio broadcast), existing crates (finsight-core/agent/providers untouched), React + Vite UI, generated `bindings.ts` (unchanged), vitest.

**Spec:** `docs/superpowers/specs/2026-07-15-server-architecture-design.md` (this plan = spec Phase 1 only; Phases 2–4 get their own plans after this lands).

---

## Ground rules (read first)

- **Run every `cargo` command via PowerShell, NOT Git Bash.** rusqlite's vendored-OpenSSL build needs Strawberry Perl (`C:\Strawberry\perl\bin\perl.exe`, on PowerShell's PATH); Git Bash's MSYS perl lacks `Locale::Maketext::Simple` and the build dies with an OpenSSL "perl reported failure" error. `npm`/`npx`/`pnpm` are fine from either shell.
- **Worktree gotchas:** `node_modules` and `samples/` are already set up in this worktree. Never run two `cargo test` invocations in parallel (Windows link error 1104).
- **Baseline (controller-verified green):** Rust `cargo test --workspace` and frontend `424 tests / 82 files` + `tsc --noEmit` clean. "Count unchanged" means against these.
- **Green bar:** the whole plan must keep `cargo test --workspace` and `cd ui && npx vitest run` green after every task. Baseline: 509 Rust / 424 frontend / 0 TS errors.
- **Bindings invariant:** after any task that touches command wrappers, `cargo run -p finsight-tauri --bin export_bindings` must produce **zero diff** in `ui/src/api/bindings.ts` (`git diff --exit-code ui/src/api/bindings.ts`). Phase 1 changes no command names, signatures, or doc comments.
- **The signature transformation** (used in Tasks 3–7). Every moved command changes exactly like this:

  Before (in `crates/finsight-app/src/commands/accounts.rs`):
  ```rust
  #[tauri::command]
  #[specta::specta]
  pub async fn list_accounts(state: tauri::State<'_, AppState>) -> AppResult<Vec<AccountSummary>> {
      let db = (*state.db).clone();
      let result = run(&db, accounts::list_summaries).await.map_err(AppError::from)?;
      Ok(result)
  }
  ```

  After — body moves to `crates/finsight-api/src/commands/accounts.rs`:
  ```rust
  pub async fn list_accounts(state: &ApiState) -> AppResult<Vec<AccountSummary>> {
      let db = (*state.db).clone();
      let result = run(&db, accounts::list_summaries).await.map_err(AppError::from)?;
      Ok(result)
  }
  ```

  …and `crates/finsight-app/src/commands/accounts.rs` keeps a thin wrapper **with the original doc comment and attributes**:
  ```rust
  #[tauri::command]
  #[specta::specta]
  pub async fn list_accounts(state: tauri::State<'_, AppState>) -> AppResult<Vec<AccountSummary>> {
      finsight_api::commands::accounts::list_accounts(&state.api).await
  }
  ```

  Rules:
  - `state: tauri::State<'_, AppState>` → `state: &ApiState`; wrapper passes `&state.api`.
  - All other params/return types unchanged. Doc comments live on the **wrapper** (that's what specta exports).
  - Types defined in a command module (request/response structs with `#[derive(Type)]`, enums like `CompletionProviderConfig`) move with the module; the finsight-app module re-exports them (`pub use finsight_api::commands::agent::CompletionProviderConfig;`) so `lib.rs`/tests keep compiling.
  - Module-level unit tests (`#[cfg(test)] mod tests`) move with the bodies. Fixture paths like `../../ui/src/...` still resolve (same crate depth).
  - **AppHandle commands — the controller has already inventoried these from source (do NOT re-derive; use this table).** Every command taking `tauri::AppHandle`, and its correct disposition:

    | Command | File | AppHandle used for | Phase 1 disposition |
    |---|---|---|---|
    | `export_account_csv` | accounts.rs | file-save dialog (`DialogExt`) | **UNSUPPORTED** (501; web flow in P3) |
    | `export_transactions_csv` | transactions.rs | file-save dialog | **UNSUPPORTED** |
    | `export_search_transactions_csv` | transactions.rs | file-save dialog | **UNSUPPORTED** |
    | `export_all_data_json` | settings.rs | file-save dialog | **UNSUPPORTED** |
    | `export_all_data_csv` | settings.rs | file-save dialog | **UNSUPPORTED** |
    | `get_data_health`, `create_manual_backup`, `stage_restore_backup`, `cancel_staged_restore` | data_health.rs | only the `app_data_dir(&app)` helper (data dir) | **MOVE** using `state.data_dir` (Task 4) — no dialog |
    | `apply_next_month_plan` | budget.rs | only `notifications::check_and_fire(&app, …)` after the writes | **MOVE** (Task 4); the fire-and-forget notification stays in the **Tauri wrapper**, not the api fn — the server has no native notifications in P1 |
    | `import_csv` | import.rs | `app.emit("import-progress")` + `app.emit("import-complete")` progress events | **MOVE via `FrameSink`** (Task 4) — this is the same emit pattern as copilot streaming, NOT a dialog. Its two events must flow over SSE. |
    | `stream_copilot_message` | copilot_chat.rs | frame emits | **MOVE via `FrameSink`** (Task 6) |

    The **`UNSUPPORTED` set is exactly the 5 export commands above** — nothing else. Tasks 3–5 must still run `grep -rn "AppHandle" crates/finsight-app/src/commands/` and report any command NOT in this table back to the controller before proceeding (guards against a signature that changed since this inventory). If a stayed-behind command shares pure helpers (e.g. `csv_escape`) with moved code, move the helper to finsight-api and import it.
  - **Arg-key convention (load-bearing for the Task 10 guard):** every moved command keeps its exact parameter names. The dispatcher (Task 9) will read each argument via `arg(&p, "<camelCaseKey>")` where the key is the Rust param name camelCased (`balance_cents` → `"balanceCents"`, `import_id` → `"importId"`, single-word names unchanged). This matches what `bindings.ts` emits (`TAURI_INVOKE("set_account_balance", { id, balanceCents })`). Do not rename params during the move.

---

### Task 1: `finsight-api` crate skeleton (state + error)

**Files:**
- Create: `crates/finsight-api/Cargo.toml`, `crates/finsight-api/src/lib.rs`, `crates/finsight-api/src/error.rs`
- Modify: `Cargo.toml` (workspace members), `crates/finsight-app/Cargo.toml` (add dep), `crates/finsight-app/src/error.rs` (become re-export), `crates/finsight-app/src/lib.rs` (AppState wraps ApiState)

- [ ] **Step 1: Create the crate**

`crates/finsight-api/Cargo.toml`:
```toml
[package]
name = "finsight-api"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
finsight-core = { path = "../finsight-core" }
finsight-agent = { path = "../finsight-agent" }
finsight-providers = { path = "../finsight-providers" }
serde.workspace = true
serde_json.workspace = true
specta.workspace = true
tokio.workspace = true
chrono.workspace = true
uuid.workspace = true
rand.workspace = true
base64.workspace = true
rust_decimal.workspace = true
url.workspace = true
reqwest.workspace = true

[dev-dependencies]
tempfile.workspace = true
rstest.workspace = true
```
(Add further workspace deps only when a moved module actually needs them — check finsight-app's Cargo.toml for the candidates.)

Add `"crates/finsight-api"` to `members` in the root `Cargo.toml`.

- [ ] **Step 2: Move `AppError` into finsight-api**

Move `crates/finsight-app/src/error.rs` → `crates/finsight-api/src/error.rs` **minus** the `impl From<tauri::Error> for AppError` block (finsight-api must stay tauri-free). Replace `crates/finsight-app/src/error.rs` with:

```rust
pub use finsight_api::error::{AppError, AppResult};

/// finsight-api owns AppError and must stay tauri-free, so the old
/// `From<tauri::Error>` impl is now this helper (orphan rule forbids the impl here).
pub fn tauri_err(e: tauri::Error) -> AppError {
    AppError::new("tauri", e.to_string())
}
```
Fix every `?` site that relied on `From<tauri::Error>` with `.map_err(crate::error::tauri_err)` (compiler finds them; they're all in dialog/data_health code).

- [ ] **Step 3: Define `ApiState` and rewire `AppState`**

`crates/finsight-api/src/lib.rs`:
```rust
pub mod commands; // starts empty; filled by Tasks 3-7
pub mod error;
pub mod sink;     // FrameSink trait — defined in Step 3b below (import_csv in Task 4
                  // and copilot_chat in Task 6 both depend on it existing early)

use finsight_agent::{agent::{AgentHandle, EventCallback}, CompletionProvider};
use finsight_core::Db;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Transport-agnostic application state: everything a command needs,
/// with no Tauri types. Both the Tauri app and finsight-server own one.
pub struct ApiState {
    pub db: Arc<Db>,
    pub agent: AgentHandle,
    pub agent_provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>>,
    pub data_dir: PathBuf,
}

impl ApiState {
    pub fn new(db: Db, data_dir: PathBuf, on_event: EventCallback) -> Self {
        let provider: Arc<RwLock<Option<Arc<dyn CompletionProvider>>>> =
            Arc::new(RwLock::new(None));
        let agent = AgentHandle::spawn(db.clone(), Arc::clone(&provider), on_event);
        Self { db: Arc::new(db), agent, agent_provider: provider, data_dir }
    }
}
```

In `crates/finsight-app/src/lib.rs`, `AppState` becomes a wrapper (sync_scheduler moves later, keep it here for now):
```rust
pub struct AppState {
    pub api: Arc<finsight_api::ApiState>,
    pub sync_scheduler: SyncScheduler,
}
```
`AppState::new(db, data_dir, on_event)` builds `ApiState::new(...)` + `SyncScheduler::new(db)`. In `configure_app`'s setup, pass `app_data_dir` as `data_dir`. Every existing command body that used `state.db` / `state.agent` / `state.agent_provider` now reads `state.api.db` etc. — do a mechanical find/replace within `crates/finsight-app/src/commands/` (`state.db` → `state.api.db`, `state.agent` → `state.api.agent`, `state.agent_provider` → `state.api.agent_provider`). `state.sync_scheduler` is unchanged.

- [ ] **Step 3b: Define the `FrameSink` trait now** (`crates/finsight-api/src/sink.rs`)

The trait is tiny and standalone, and TWO later tasks depend on it existing (`import_csv` in Task 4, `copilot_chat` in Task 6), so define it here rather than in Task 6:
```rust
use std::sync::Arc;

/// Transport-agnostic replacement for `tauri::AppHandle::emit`. The Tauri app
/// emits window events; finsight-server pushes into a broadcast channel → SSE.
pub trait FrameSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

/// A no-op sink: for command paths that emit but where the caller doesn't care
/// (and as a safe default). Also handy in unit tests that ignore emissions.
pub struct NullSink;
impl FrameSink for NullSink {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
}

/// Test/collector sink — records every (event, payload) in order.
pub struct VecSink(pub std::sync::Mutex<Vec<(String, serde_json::Value)>>);
impl VecSink {
    pub fn new() -> Arc<Self> { Arc::new(Self(std::sync::Mutex::new(Vec::new()))) }
}
impl FrameSink for VecSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.0.lock().unwrap().push((event.to_string(), payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vec_sink_collects_events_in_order() {
        let sink = VecSink::new();
        sink.emit("import-progress", serde_json::json!({"rows_done": 1}));
        sink.emit("import-complete", serde_json::json!({"ok": true}));
        let got = sink.0.lock().unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "import-progress");
        assert_eq!(got[1].1["ok"], true);
    }
}
```
Run `cargo test -p finsight-api sink` → PASS.

- [ ] **Step 4: Verify green + zero bindings diff**

Run: `cargo test --workspace` → all pass (count unchanged).
Run: `cargo run -p finsight-tauri --bin export_bindings && git diff --exit-code ui/src/api/bindings.ts` → exit 0.

- [ ] **Step 5: Commit**
```bash
git add -A && git commit -m "refactor(server): introduce tauri-free finsight-api crate with ApiState + AppError"
```

---

### Task 2: Move `SyncScheduler` and provider helpers into finsight-api

**Files:**
- Move: `crates/finsight-app/src/sync_scheduler.rs` → `crates/finsight-api/src/sync_scheduler.rs`
- Modify: `crates/finsight-app/src/lib.rs`, `crates/finsight-api/src/lib.rs`

- [ ] **Step 1: Move sync_scheduler**

The file's only tauri usage is `tauri::async_runtime::spawn` / `JoinHandle` (line ~85–91). Replace with `tokio::spawn` / `tokio::task::JoinHandle<()>` — behavior identical (both run on a Tokio runtime). Add `pub mod sync_scheduler;` to finsight-api's lib.rs; in finsight-app replace the module with `pub use finsight_api::sync_scheduler;`. Move the `sync_scheduler` field from `AppState` into `ApiState` (constructed in `ApiState::new`); finsight-app setup calls `app.state::<AppState>().api.sync_scheduler.start()`.

- [ ] **Step 2: Move the provider-construction helpers**

Move from `crates/finsight-app/src/lib.rs` to `crates/finsight-api/src/provider.rs` (new module): `migrate_provider_settings`, `load_provider_from_settings`, `build_provider_from_config`, `load_completion_provider_config`, `build_copilot_router_from_settings`. They only use finsight-core/agent/providers + keychain — tauri-free already. `load_completion_provider_config` references `commands::agent::CompletionProviderConfig`, which hasn't moved yet — for now have finsight-app keep a thin `pub fn load_completion_provider_config(db)` delegating after Task 5 moves the type; if the compile ordering fights you, move just this one helper together with Task 5 instead and leave a `// moves in Task 5` note. finsight-app lib.rs re-exports: `pub use finsight_api::provider::{migrate_provider_settings, load_provider_from_settings, ...};` (integration tests import them from finsight-app today).

- [ ] **Step 3: Verify + commit**

Run: `cargo test --workspace` → green. `git add -A && git commit -m "refactor(server): move sync scheduler + provider helpers to finsight-api"`

---

### Tasks 3–5: Move command modules (the mechanical bulk)

Apply the **signature transformation** from Ground Rules to every command listed, one task per group. For each task: create the finsight-api module, move bodies + module-private helpers + `#[cfg(test)]` tests, leave wrappers, keep desktop-only commands behind.

**Per-task verification (identical for 3, 4, 5):**
1. `cargo test -p finsight-api -p finsight-app` → green (moved tests now run in finsight-api).
2. `cargo run -p finsight-tauri --bin export_bindings && git diff --exit-code ui/src/api/bindings.ts` → exit 0.
3. Commit: `git add -A && git commit -m "refactor(server): move <group> command bodies to finsight-api"`

### Task 3: Core-data group

**Files:** move bodies from `crates/finsight-app/src/commands/{accounts,categories,transactions,onboarding,meta,investments}.rs` to same-named files under `crates/finsight-api/src/commands/`.

- [ ] accounts: `list_accounts`, `create_account`, `update_account`, `archive_account`, `set_account_balance`, `list_account_balance_history`, `list_account_balance_sparklines` — **stays:** `export_account_csv` (dialog)
- [ ] categories: `update_category_color`, `create_category`, `rename_category`, `archive_category`, `set_category_guidance`
- [ ] transactions: `list_transactions`, `create_transaction`, `update_transaction`, `delete_transaction`, `create_rule`, `set_transaction_owner`, `list_categories`, `set_category_spending_type`, `get_spending_breakdown`, `list_categories_with_spending`, `list_rules_with_categories`, `toggle_rule`, `get_transaction_count`, `set_transaction_flags`, `set_transaction_transfer`, `apply_transfer_verdict_to_similar`, `get_transaction_splits`, `set_transaction_splits` — **stays:** `export_transactions_csv`, `export_search_transactions_csv` (dialog)
- [ ] onboarding: `get_onboarding_state`, `mark_onboarding_complete`, `reset_onboarding_completion`, `commit_starter_categories`, `probe_ollama`, `save_llm_provider`
- [ ] meta: `app_ready` — **verified:** takes no args, returns `AppReady { version }`. Moves normally; the server dispatcher returns the real value (the UI reads `version`), NOT a no-op.
- [ ] investments: `list_account_positions`, `get_investment_summary`
- [ ] Run per-task verification; commit.

### Task 4: Planning & wellness group

**Files:** move bodies from `crates/finsight-app/src/commands/{budget,recurring,reports,spending,metrics,scenarios,journey,inbox,assets,household,insights,planned_transactions,data_health,import,settings}.rs`.

**Define `TauriFrameSink` in this task** (first user is `import_csv`), in `crates/finsight-app/src/commands/mod.rs`:
```rust
pub struct TauriFrameSink(pub tauri::AppHandle);
impl finsight_api::sink::FrameSink for TauriFrameSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        use tauri::Emitter;
        let _ = self.0.emit(event, payload);
    }
}
```
Task 6 reuses it for `stream_copilot_message`.

- [ ] budget: `list_budget_envelopes`, `set_budget`, `list_goals`, `create_goal`, `update_goal_balance`, `contribute_to_goal`, `list_goal_contributions`, `archive_goal`, `project_goal_growth`, `update_goal_monthly`, `update_goal_purpose`, `get_plan_next_month_data`, `apply_next_month_plan`, `list_budget_history`. **`apply_next_month_plan` (verified):** its `AppHandle` is used ONLY for a fire-and-forget `notifications::check_and_fire(&app, &db)` after the budget writes. Move the budget-writing body to `apply_next_month_plan(state: &ApiState, assignments)`; the Tauri wrapper calls the api fn and THEN spawns the notification (server has no native notifications in P1, so the server dispatcher just calls the api fn). NOT UNSUPPORTED.
- [ ] recurring: `list_recurring` · reports: `get_report_data`, `get_month_totals`, `get_savings_rate_history`, `create_monthly_review`, `list_monthly_reviews` · spending: `get_spending_path_back`, `set_spending_annotation` · metrics: `get_financial_metrics`, `household_net_worth_breakdown`, `set_financial_assumptions` · scenarios: `run_scenario`, `save_scenario`, `list_scenario_history`, `delete_scenario` · journey: `get_journey_status` · inbox: `get_action_items`
- [ ] assets: `list_manual_assets`, `create_manual_asset`, `update_manual_asset`, `delete_manual_asset`, `record_net_worth_snapshot`, `list_net_worth_history`, `compute_debt_payoff`, `get_uncelebrated_milestones`
- [ ] household: `list_household_members`, `create_household_member`, `set_self_member`, `delete_household_member`, `list_account_owners`, `set_account_owners`, `set_account_owner_shares`, `list_asset_owners`, `set_asset_owners`
- [ ] insights: `list_agent_memory`, `forget_agent_memory`, `get_financial_health_score` · planned_transactions: all 5
- [ ] data_health: `get_data_health`, `create_manual_backup`, `stage_restore_backup`, `cancel_staged_restore` — replace the `fn app_data_dir(app: &tauri::AppHandle)` helper with `state.data_dir` (that's why ApiState carries it)
- [ ] import: `preview_csv_columns`, `prepare_csv_import`, `import_csv`, `get_saved_csv_mapping`, `list_unfinished_imports`, `discard_unfinished_import`. **`import_csv` (verified):** its `AppHandle` is used for `app.emit("import-progress", …)` and `app.emit("import-complete", …)` — the SAME emit pattern as copilot streaming, NOT a dialog. Move it to `import_csv(state: &ApiState, sink: Arc<dyn FrameSink>, path, account_id, mapping)` and route both emits through `sink.emit(...)` (keep the exact event names `"import-progress"` / `"import-complete"` and payload shapes so UI listeners and the SSE stream work unchanged). The Tauri wrapper passes `TauriFrameSink(app)`; the server dispatcher passes a `BroadcastSink` (Task 9). NOT UNSUPPORTED. The other import commands are plain moves.
- [ ] settings: `get_currency`, `set_currency`, `delete_all_data`, `get_notifications_enabled`, `set_notifications_enabled`, `get_auto_categorize_enabled`, `set_auto_categorize_enabled` — **stays UNSUPPORTED (file-save dialog):** `export_all_data_json`, `export_all_data_csv`
- [ ] Run per-task verification; commit.

### Task 5: Agent & integrations group

**Files:** move bodies from `crates/finsight-app/src/commands/{agent,copilot,recipes,simplefin}.rs`. `agent.rs` is 3000+ lines with a large test suite — move it whole (bodies, types like `CompletionProviderConfig` and `AgentResponseBlock`, tests, the Rust↔Zod parity corpus test); the fixture `include_str!("../../ui/...")` paths keep working at the same crate depth. Re-export moved public types from the finsight-app module so `lib.rs` and UI-facing type names don't churn.

- [ ] agent: `set_completion_provider`, `get_completion_provider`, `save_provider_api_key`, `list_provider_models`, `test_completion_provider`, `get_needs_review_count`, `trigger_categorize`, `recompute_anomalies`, `set_anomaly_dismissed`, `trigger_recategorize_low_confidence`, `get_agent_status`, `ask_agent`, `list_rule_proposals`, `accept_rule_proposal`, `decline_rule_proposal`, `list_recent_agent_activity`
- [ ] copilot: `list_agent_sessions`, `create_agent_session`, `close_agent_session`, `list_action_bundles`, `get_action_bundle`, `approve_action_item`, `reject_action_item`, `list_execution_log`, `execute_action_bundle`
- [ ] recipes: `list_recipes`, `create_recipe`, `update_recipe`, `pause_recipe`, `resume_recipe`, `delete_recipe`, `trigger_recipe`, `list_recipe_runs`
- [ ] simplefin: all 21 commands (`save_simplefin_setup_token` … `dismiss_import_candidate`)
- [ ] Finish the `load_completion_provider_config` move deferred from Task 2 if it was deferred.
- [ ] Run per-task verification; commit.

---

### Task 6: `FrameSink` trait + copilot_chat extraction

**Note:** the `FrameSink` trait + `VecSink`/`NullSink` were already defined in Task 1 Step 3b, and `import_csv` was already converted to it in Task 4. This task converts the remaining, larger emit path: `copilot_chat`.

**Files:**
- Move: `crates/finsight-app/src/commands/copilot_chat.rs` bodies → `crates/finsight-api/src/commands/copilot_chat.rs`
- Modify: `crates/finsight-app/src/commands/copilot_chat.rs` (wrappers + reuse the `TauriFrameSink` created in Task 4)

- [ ] **Step 1: Confirm the trait exists** — `cargo test -p finsight-api sink` is already green from Task 1. (No new trait definition here.)

- [ ] **Step 2 (was Step 3): Move copilot_chat onto the sink**

In the moved `finsight-api/src/commands/copilot_chat.rs`: every `app: tauri::AppHandle` / `app: &tauri::AppHandle` parameter becomes `sink: Arc<dyn FrameSink>` / `sink: &Arc<dyn FrameSink>`, and the single choke point changes:
```rust
fn emit_copilot_frame(sink: &Arc<dyn FrameSink>, frame: CopilotStreamFrame) {
    sink.emit("copilot-stream-frame", serde_json::to_value(frame).expect("frame serializes"));
}
```
(`emit_stream_error` and the fn at old line ~1167 get the same treatment.) All existing copilot_chat tests move; where they constructed a mock AppHandle or asserted on emissions, use `VecSink`.

The finsight-app wrapper keeps the public command signature, reusing the `TauriFrameSink` defined in Task 4 (`crates/finsight-app/src/commands/mod.rs` or wherever Task 4 placed it — do NOT redefine it):
```rust
// TauriFrameSink already exists from Task 4:
//   pub struct TauriFrameSink(pub tauri::AppHandle);
//   impl finsight_api::sink::FrameSink for TauriFrameSink { emit → app.emit }

#[tauri::command]
#[specta::specta]
pub async fn stream_copilot_message(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    /* ...original args... */
) -> AppResult</* original */> {
    let sink: Arc<dyn finsight_api::sink::FrameSink> = Arc::new(crate::commands::TauriFrameSink(app));
    finsight_api::commands::copilot_chat::stream_copilot_message(&state.api, sink, /* args */).await
}
```
Also move the remaining copilot_chat commands (`list_conversations`, `get_conversation_messages`, `delete_conversation`, `create_conversation`, `edit_conversation_user_message`, `delete_conversation_messages_after`) with the standard transformation.

- [ ] **Step 3: Verify** — `cargo test --workspace` green; export_bindings zero-diff.
- [ ] **Step 4: Commit** — `git commit -am "refactor(server): copilot_chat is transport-agnostic via FrameSink"`

---

### Task 7: Retire finsight-app's direct state fields

- [ ] **Step 1:** Sweep `crates/finsight-app/src/lib.rs` setup: the `on_event` EventCallback closure and startup cascade are unchanged (they already use plain `db`), but confirm nothing in finsight-app still reaches into moved internals except via `state.api` and wrappers. `cargo test --workspace` green; commit `refactor(server): finsight-app is a thin wrapper layer`.

---

### Task 8: `finsight-server` crate — startup + health

**Files:**
- Create: `crates/finsight-server/Cargo.toml`, `crates/finsight-server/src/main.rs`, `crates/finsight-server/src/state.rs`, `crates/finsight-server/src/router.rs`
- Modify: root `Cargo.toml` (member)

- [ ] **Step 1: Crate manifest**

```toml
[package]
name = "finsight-server"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
finsight-api = { path = "../finsight-api" }
finsight-core = { path = "../finsight-core" }
finsight-agent = { path = "../finsight-agent" }
axum = { version = "0.8", features = ["json"] }
tower-http = { version = "0.6", features = ["fs", "trace", "cors"] }
tokio.workspace = true
tokio-stream = { version = "0.1", features = ["sync"] }
serde.workspace = true
serde_json.workspace = true
rand.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
chrono.workspace = true

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
tempfile.workspace = true
```
**Guard:** `cargo tree -p finsight-server -i tauri` must report "nothing depends on tauri" — the server build must never pull tauri/wry (Docker builds on Linux would need webkit2gtk otherwise).

- [ ] **Step 2: Failing test — health endpoint** (`crates/finsight-server/src/router.rs` tests)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    pub(crate) async fn test_state() -> std::sync::Arc<crate::state::ServerState> {
        let dir = tempfile::tempdir().unwrap();
        // Leak the tempdir so the DB outlives the test body.
        let path = dir.into_path();
        crate::state::ServerState::bootstrap(&path).await.unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = build_router(test_state().await);
        let res = app
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
```
Run: `cargo test -p finsight-server` → FAIL (nothing exists yet).

- [ ] **Step 3: Implement `ServerState::bootstrap` + router + main**

`state.rs`:
```rust
use finsight_api::ApiState;
use finsight_agent::agent::{AgentEvent, EventCallback};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::broadcast;

/// One event as the UI's Tauri-event shim expects it: `{ event, payload }`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OutboundEvent {
    pub event: String,
    pub payload: serde_json::Value,
}

pub struct ServerState {
    pub api: Arc<ApiState>,
    pub events: broadcast::Sender<OutboundEvent>,
}

/// Phase 1 key management: hex keyfile in the data dir (Phase 2 replaces this
/// with per-user password-wrapped keys). NOT the OS keychain: must work headless.
fn load_or_create_keyfile(data_dir: &Path) -> std::io::Result<String> {
    let path = data_dir.join("db.key");
    if path.exists() {
        return Ok(std::fs::read_to_string(&path)?.trim().to_string());
    }
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let key: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(&path, &key)?;
    Ok(key)
}

impl ServerState {
    pub async fn bootstrap(data_dir: &Path) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(data_dir)?;
        let key = load_or_create_keyfile(data_dir)?;
        let db = finsight_core::Db::open(&data_dir.join("data.sqlcipher"), &key)?;
        finsight_core::db::run_migrations(&db)?;
        finsight_api::provider::migrate_provider_settings(&db)?;

        let (tx, _) = broadcast::channel::<OutboundEvent>(256);
        let etx = tx.clone();
        let on_event: EventCallback = Arc::new(move |event: AgentEvent| {
            // Same names configure_app uses, so the UI listeners work unchanged.
            let name = match &event {
                AgentEvent::CategorizationProgress { .. } => "categorization.progress",
                AgentEvent::CategorizationComplete { .. } => "categorization.complete",
                AgentEvent::Error { .. } => "agent.error",
            };
            let _ = etx.send(OutboundEvent {
                event: name.to_string(),
                payload: serde_json::to_value(&event).unwrap_or_default(),
            });
        });

        let api = Arc::new(ApiState::new(db.clone(), data_dir.to_path_buf(), on_event));
        if let Some(p) = finsight_api::provider::load_provider_from_settings(&db) {
            api.agent.set_provider(p);
        }
        Ok(Arc::new(Self { api, events: tx }))
    }
}
```
(If `anyhow` isn't already a workspace dep of this crate, add it. If `Db::open`'s key parameter shape differs — check `finsight_core::keychain::load_or_create_key`'s return type and mirror it exactly.)

`router.rs`:
```rust
use crate::state::ServerState;
use axum::{routing::get, Json, Router};
use std::sync::Arc;

pub fn build_router(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/api/health", get(|| async { Json(serde_json::json!({"status":"ok"})) }))
        .with_state(state)
}
```

`main.rs`:
```rust
mod dispatch; // Task 9 (create as empty module now)
mod router;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    ).init();
    let data_dir = std::path::PathBuf::from(
        std::env::var("FINSIGHT_DATA_DIR").unwrap_or_else(|_| "./data".into()),
    );
    let port: u16 = std::env::var("FINSIGHT_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8674);
    let state = state::ServerState::bootstrap(&data_dir).await?;
    let app = router::build_router(state);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("finsight-server listening on http://localhost:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
```
NOTE: the desktop setup's startup cascade (integrity check, pre-migration backup, builtin categorization, transfer pairing, snapshots) is deliberately NOT replicated in Phase 1 bootstrap — record it as a Phase 2 item in the plan-completion notes.

- [ ] **Step 4: Run** — `cargo test -p finsight-server` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(server): finsight-server crate with bootstrap + /api/health"`

---

### Task 9: RPC dispatcher

**Files:**
- Create: `crates/finsight-server/src/dispatch.rs`
- Modify: `crates/finsight-server/src/router.rs`

- [ ] **Step 1: Failing test first** (in `dispatch.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn rpc_list_accounts_roundtrip() {
        let state = crate::router::tests::test_state().await;
        let app = crate::router::build_router(state);
        let res = app.oneshot(
            Request::post("/api/rpc/list_accounts")
                .header("content-type", "application/json")
                .body(Body::from("{}")).unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.is_array()); // empty DB → []
    }

    #[tokio::test]
    async fn rpc_create_then_list_account() {
        let state = crate::router::tests::test_state().await;
        let app = crate::router::build_router(state);
        let input = serde_json::json!({ "input": {
            // fill required NewAccount fields per finsight_core::models::NewAccount
            "owner": "You", "bank": "Test Bank", "type": "Checking",
            "name": "RPC Test", "currency": "USD", "source": "manual"
        }});
        let res = app.clone().oneshot(
            Request::post("/api/rpc/create_account")
                .header("content-type", "application/json")
                .body(Body::from(input.to_string())).unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app.oneshot(
            Request::post("/api/rpc/list_accounts")
                .header("content-type", "application/json")
                .body(Body::from("{}")).unwrap(),
        ).await.unwrap();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unknown_command_is_404_with_app_error_body() {
        let state = crate::router::tests::test_state().await;
        let app = crate::router::build_router(state);
        let res = app.oneshot(
            Request::post("/api/rpc/not_a_command")
                .header("content-type", "application/json")
                .body(Body::from("{}")).unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["code"], "rpc.unknown_command");
    }

    #[tokio::test]
    async fn unsupported_command_is_501() {
        let state = crate::router::tests::test_state().await;
        let app = crate::router::build_router(state);
        let res = app.oneshot(
            Request::post("/api/rpc/export_all_data_csv")
                .header("content-type", "application/json")
                .body(Body::from("{}")).unwrap(),
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
```
(NewAccount's exact required fields: check `finsight_core::models::NewAccount` and adjust the JSON — the test must construct a valid input.)
Run: `cargo test -p finsight-server` → FAIL.

- [ ] **Step 2: Implement the dispatcher**

Structure (in `dispatch.rs`):
```rust
use crate::state::{OutboundEvent, ServerState};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use finsight_api::error::AppError;
use finsight_api::sink::FrameSink;
use std::sync::Arc;

/// Sink that fans command-emitted events out to every SSE subscriber.
pub struct BroadcastSink(pub tokio::sync::broadcast::Sender<OutboundEvent>);
impl FrameSink for BroadcastSink {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        let _ = self.0.send(OutboundEvent { event: event.to_string(), payload });
    }
}

/// Argument keys arrive in camelCase (what bindings.ts sends — Tauri converted
/// them to snake_case for us before; here we read them by their camelCase name).
fn arg<T: serde::de::DeserializeOwned>(p: &serde_json::Value, name: &str) -> Result<T, AppError> {
    let v = p.get(name).cloned().unwrap_or(serde_json::Value::Null);
    serde_json::from_value(v)
        .map_err(|e| AppError::new("rpc.bad_arg", format!("argument `{name}`: {e}")))
}

fn ok<T: serde::Serialize>(v: T) -> Result<serde_json::Value, AppError> {
    serde_json::to_value(v).map_err(|e| AppError::new("rpc.serialize", e.to_string()))
}

/// Desktop-only commands (native file dialogs). Kept explicit so the parity
/// test (Task 10) proves SUPPORTED ∪ UNSUPPORTED == everything bindings.ts calls.
/// Phase 3 gives these real upload/download flows.
// EXACTLY these 5 file-save-dialog exports (controller-verified from source).
// import_csv and apply_next_month_plan are NOT here — they move (see Task 4).
pub const UNSUPPORTED: &[&str] = &[
    "export_account_csv",
    "export_transactions_csv",
    "export_search_transactions_csv",
    "export_all_data_json",
    "export_all_data_csv",
];

pub async fn rpc(
    State(st): State<Arc<ServerState>>,
    Path(cmd): Path<String>,
    Json(p): Json<serde_json::Value>,
) -> Response {
    if UNSUPPORTED.contains(&cmd.as_str()) {
        return (StatusCode::NOT_IMPLEMENTED, Json(AppError::new(
            "rpc.unsupported", format!("`{cmd}` needs the desktop app (Phase 3 adds a web flow)"),
        ))).into_response();
    }
    match dispatch(&st, &cmd, p).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) if e.code == "rpc.unknown_command" => (StatusCode::NOT_FOUND, Json(e)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(e)).into_response(),
    }
}

async fn dispatch(st: &Arc<ServerState>, cmd: &str, p: serde_json::Value)
    -> Result<serde_json::Value, AppError>
{
    use finsight_api::commands as c;
    let api = &st.api;
    match cmd {
        // ── accounts ──
        "list_accounts" => ok(c::accounts::list_accounts(api).await?),
        "create_account" => ok(c::accounts::create_account(api, arg(&p, "input")?).await?),
        "update_account" => ok(c::accounts::update_account(api, arg(&p, "id")?, arg(&p, "patch")?).await?),
        "archive_account" => ok(c::accounts::archive_account(api, arg(&p, "id")?).await?),
        "set_account_balance" => ok(c::accounts::set_account_balance(api, arg(&p, "id")?, arg(&p, "balanceCents")?).await?),
        // ── meta: returns the real AppReady { version } (UI reads it) ──
        "app_ready" => ok(c::meta::app_ready().await?),
        // ── emit-path commands: construct a BroadcastSink; note the sink is NOT a
        //    bindings arg, so the real args below ARE still arg-checked by Task 10 ──
        "stream_copilot_message" => {
            let sink: Arc<dyn FrameSink> = Arc::new(BroadcastSink(st.events.clone()));
            ok(c::copilot_chat::stream_copilot_message(api, sink, /* arg(&p,"...")? per signature */).await?)
        }
        "import_csv" => {
            let sink: Arc<dyn FrameSink> = Arc::new(BroadcastSink(st.events.clone()));
            ok(c::import::import_csv(api, sink, arg(&p, "path")?, arg(&p, "accountId")?, arg(&p, "mapping")?).await?)
        }
        // ── …one arm per remaining SUPPORTED command, same mechanical pattern.
        //    Arg key = Rust param name in camelCase (`balance_cents` → "balanceCents",
        //    single-word params unchanged). Task 10 enforces BOTH: (a) a missed
        //    arm = red parity test, and (b) every `arg(&p, "key")` matches the
        //    camelCase key bindings.ts actually sends — so a typo'd key is a red
        //    test at `cargo test`, NOT a latent 500 discovered in production. ──
        _ => Err(AppError::new("rpc.unknown_command", format!("unknown command `{cmd}`"))),
    }
}
```
Write out ALL remaining arms — every command registered in `build_specta_builder()` (the full list is in `crates/finsight-app/src/lib.rs:174-353`) except the UNSUPPORTED set. This is ~160 one-line arms; tedious, mechanical, enforced complete by Task 10.

**HARD CONVENTION (Task 10 depends on it):** read every argument via `arg(&p, "<key>")` and nothing else — no inline `p.get(...)`, no destructuring, no renamed locals for the key. The key string must be the literal the test can regex out. Sink-constructing commands (`stream_copilot_message`, `import_csv`) build the sink locally but STILL read every one of their bindings args via `arg(&p, "...")` — the sink is not a bindings arg, so these commands are fully arg-checked and need NO exemption. `app_ready` takes no args and reads none → it matches the empty set naturally. **Therefore `ARG_CHECK_EXEMPT` (Task 10) should be empty**; adding any entry requires a written justification and controller sign-off. Fill `stream_copilot_message`'s arm with `arg(&p, "...")` for each non-sink param per its actual signature (check `copilot_chat.rs`).

Wire into `router.rs`:
```rust
.route("/api/rpc/{cmd}", axum::routing::post(crate::dispatch::rpc))
```

- [ ] **Step 3: Run** — `cargo test -p finsight-server` → PASS.
- [ ] **Step 4: Commit** — `git commit -am "feat(server): POST /api/rpc/{cmd} dispatcher over finsight-api"`

---

### Task 10: Bindings↔dispatcher parity test (routing AND arg-keys)

This is the plan's most important guard. It turns two otherwise-eyeball correctness surfaces — "is every command routed?" and "is every one of ~160 camelCase arg-keys right?" — into red-bar-on-drift invariants at `cargo test` time. Neither the compiler, the zero-diff-bindings check, nor human review covers the arg-keys; only this test does.

**Files:** Create `crates/finsight-server/src/lib.rs` (expose modules), `crates/finsight-server/tests/parity.rs`; Modify `dispatch.rs` (export `SUPPORTED` + `ARG_CHECK_EXEMPT`), `main.rs` (use the lib).

- [ ] **Step 1: lib+bin restructure.** Add `crates/finsight-server/src/lib.rs` with `pub mod dispatch; pub mod router; pub mod state; pub mod events;`. Change `main.rs` to `use finsight_server::{router, state};` (drop the `mod` decls). Integration tests under `tests/` can now `use finsight_server::...`. `cargo test -p finsight-server` still green.

- [ ] **Step 2:** In `dispatch.rs`, add adjacent to the match:
```rust
/// Every command with a match arm in `dispatch()`. Keep in the SAME ORDER as the
/// match so review sees drift. The parity test proves this == (bindings − UNSUPPORTED).
pub const SUPPORTED: &[&str] = &[ "list_accounts", "create_account", /* …every arm… */ ];

/// Commands whose dispatch legitimately does not read every bindings arg via
/// `arg(&p, "key")`. Should be EMPTY: sink-constructing commands
/// (`stream_copilot_message`, `import_csv`) still arg-check their real args (the
/// sink is not a bindings arg), and `app_ready` has no args. Any entry here needs
/// a written justification + controller sign-off.
pub const ARG_CHECK_EXEMPT: &[&str] = &[];
```

- [ ] **Step 3: Write the failing test** (`crates/finsight-server/tests/parity.rs`)
```rust
use std::collections::{BTreeMap, BTreeSet};

/// Parse `bindings.ts` into cmd → set(camelCase arg keys). Matches the two shapes
/// tauri-specta emits: `TAURI_INVOKE("cmd")` and `TAURI_INVOKE("cmd", { a, b })`.
fn parse_bindings() -> BTreeMap<String, BTreeSet<String>> {
    let src = include_str!("../../../ui/src/api/bindings.ts");
    let mut out = BTreeMap::new();
    for chunk in src.split("TAURI_INVOKE(\"").skip(1) {
        let cmd = chunk.split('"').next().unwrap().to_string();
        // Args object (if any) is the `{ ... }` before the first `)` after the name.
        let head = &chunk[..chunk.find(')').unwrap_or(chunk.len())];
        let mut keys = BTreeSet::new();
        if let (Some(o), Some(c)) = (head.find('{'), head.rfind('}')) {
            for raw in head[o + 1..c].split(',') {
                // shorthand `{ id, balanceCents }` OR `{ id: id }` — take the key.
                let k = raw.split(':').next().unwrap().trim();
                if !k.is_empty() { keys.insert(k.to_string()); }
            }
        }
        out.insert(cmd, keys);
    }
    out
}

/// Parse `dispatch.rs` into cmd → set(keys read via `arg(&p, "key")`) by walking
/// each `"cmd" =>` match arm up to the next arm.
fn parse_dispatch_arg_keys() -> BTreeMap<String, BTreeSet<String>> {
    let src = include_str!("../src/dispatch.rs");
    // Everything after `match cmd {` (skip the const arrays above it).
    let body = &src[src.find("match cmd {").expect("match cmd block")..];
    let mut out = BTreeMap::new();
    // Arm headers look like:  "list_accounts" =>
    let arm_re = regex::Regex::new(r#""([a-z0-9_]+)"\s*=>"#).unwrap();
    let key_re = regex::Regex::new(r#"arg\(&p,\s*"([A-Za-z0-9_]+)"\)"#).unwrap();
    let arms: Vec<(usize, String)> = arm_re
        .captures_iter(body)
        .map(|c| (c.get(0).unwrap().start(), c[1].to_string()))
        .collect();
    for (i, (start, cmd)) in arms.iter().enumerate() {
        let end = arms.get(i + 1).map(|(s, _)| *s).unwrap_or(body.len());
        let mut keys = BTreeSet::new();
        for k in key_re.captures_iter(&body[*start..end]) {
            keys.insert(k[1].to_string());
        }
        out.insert(cmd.clone(), keys);
    }
    out
}

#[test]
fn every_binding_command_is_routed_or_explicitly_unsupported() {
    let wanted: BTreeSet<String> = parse_bindings().keys().cloned().collect();
    assert!(wanted.len() > 100, "bindings parse looks broken: {}", wanted.len());
    let routed: BTreeSet<String> = finsight_server::dispatch::SUPPORTED
        .iter().chain(finsight_server::dispatch::UNSUPPORTED)
        .map(|s| s.to_string()).collect();
    let missing: Vec<_> = wanted.difference(&routed).collect();
    let stale: Vec<_> = routed.difference(&wanted).collect();
    assert!(missing.is_empty(), "bindings.ts commands with no server route: {missing:?}");
    assert!(stale.is_empty(), "server routes for commands not in bindings.ts: {stale:?}");
}

/// THE arg-key guard: for every SUPPORTED command (minus exemptions), the keys the
/// dispatcher reads via `arg(&p, "…")` must EXACTLY equal the keys bindings.ts sends.
/// Catches `balance_cents` vs `balanceCents`, missing args, and typos — at test time.
#[test]
fn dispatcher_arg_keys_match_bindings_exactly() {
    let bindings = parse_bindings();
    let dispatch = parse_dispatch_arg_keys();
    let exempt: BTreeSet<&str> = finsight_server::dispatch::ARG_CHECK_EXEMPT.iter().copied().collect();
    let mut problems = Vec::new();
    for cmd in finsight_server::dispatch::SUPPORTED {
        if exempt.contains(cmd) { continue; }
        let want = bindings.get(*cmd)
            .unwrap_or_else(|| panic!("SUPPORTED command `{cmd}` absent from bindings.ts"));
        let got = dispatch.get(*cmd).cloned().unwrap_or_default();
        if &got != want {
            problems.push(format!(
                "  {cmd}: bindings sends {want:?} but dispatcher reads {got:?}"
            ));
        }
    }
    assert!(problems.is_empty(),
        "dispatcher arg-key mismatches (fix the arg(&p, \"…\") keys):\n{}", problems.join("\n"));
}
```
Add `regex = "1"` to `finsight-server` `[dev-dependencies]`.

- [ ] **Step 4:** `cargo test -p finsight-server --test parity` → run it. Both tests must pass ONLY when the dispatcher is complete AND every arg-key is correct. Fix every mismatch it reports (these are real bugs — a red bar here means a command would have 500'd in production). Re-run until green.
- [ ] **Step 5: Controller checkpoint (do not skip):** the implementer reports the final `UNSUPPORTED` list; the controller reconciles it against the AppHandle grep results collected in Tasks 3–5 before accepting the task.
- [ ] **Step 6: Commit** — `git commit -am "test(server): bindings↔dispatcher parity + arg-key guard"`

---

### Task 11: SSE `/api/events`

**Files:** Create `crates/finsight-server/src/events.rs`; Modify `router.rs`

- [ ] **Step 1: Failing test**
```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn broadcast_event_reaches_sse_subscriber() {
        let state = crate::router::tests::test_state().await;
        let mut rx = state.events.subscribe();
        state.events.send(crate::state::OutboundEvent {
            event: "copilot-stream-frame".into(),
            payload: serde_json::json!({"type":"text","delta":"hi"}),
        }).unwrap();
        let got = rx.recv().await.unwrap();
        assert_eq!(got.event, "copilot-stream-frame");
        let line = crate::events::sse_data(&got);
        assert!(line.contains("\"event\":\"copilot-stream-frame\""));
    }
}
```

- [ ] **Step 2: Implement**
```rust
use crate::state::{OutboundEvent, ServerState};
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

/// One SSE `data:` line: `{"event": name, "payload": ...}` — the shim
/// dispatches on `event`, mirroring Tauri's listen(event) semantics.
pub fn sse_data(ev: &OutboundEvent) -> String {
    serde_json::to_string(ev).unwrap_or_else(|_| "{}".into())
}

pub async fn events(State(st): State<Arc<ServerState>>)
    -> Sse<impl Stream<Item = Result<Event, Infallible>>>
{
    let rx = st.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|ev| match ev {
        Ok(ev) => Some(Ok(Event::default().data(sse_data(&ev)))),
        Err(_lagged) => None, // dropped frames are acceptable; see spec reconnect rule
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
```
Add `futures-util = "0.3"` to Cargo.toml. Route: `.route("/api/events", get(crate::events::events))`.

- [ ] **Step 3:** `cargo test -p finsight-server` → PASS. Commit `feat(server): SSE /api/events fan-out`.

---

### Task 12: Serve the built UI

**Files:** Modify `router.rs`, `main.rs`

- [ ] **Step 1:** Serve static files with SPA fallback:
```rust
use tower_http::services::{ServeDir, ServeFile};
// in build_router, after the /api routes:
let ui_dir = std::env::var("FINSIGHT_UI_DIR").unwrap_or_else(|_| "ui/dist".into());
let index = std::path::Path::new(&ui_dir).join("index.html");
router.fallback_service(ServeDir::new(&ui_dir).fallback(ServeFile::new(index)))
```
(Make `build_router` take the ui_dir as a parameter so tests can pass a tempdir; test: write `index.html` into a tempdir, GET `/some/spa/route` → 200 with that content, GET `/api/health` still routes to the API.)

- [ ] **Step 2:** Test, then commit `feat(server): serve ui/dist with SPA fallback`.

---

### Task 13: Browser HTTP/SSE shim in the UI

**Files:**
- Create: `ui/src/api/httpBackend.ts`, `ui/src/api/httpBackend.test.ts`
- Modify: `ui/src/main.tsx`

- [ ] **Step 1: Failing tests** (`ui/src/api/httpBackend.test.ts`)

```typescript
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { installHttpBackend } from "./httpBackend";

type AnyRec = Record<string, unknown>;
const w = window as unknown as AnyRec;

describe("httpBackend shim", () => {
  beforeEach(() => {
    delete w.__TAURI_INTERNALS__;
    vi.stubGlobal("EventSource", class {
      static last: unknown;
      onmessage: ((e: { data: string }) => void) | null = null;
      constructor(public url: string) { (this.constructor as AnyRec).last = this; }
      close() {}
    });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("routes invoke to POST /api/rpc/{cmd} and returns parsed JSON", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify([{ id: "a1" }]), { status: 200 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as { invoke: (c: string, a?: AnyRec) => Promise<unknown> };
    const out = await internals.invoke("list_accounts", {});
    expect(fetch).toHaveBeenCalledWith("/api/rpc/list_accounts", expect.objectContaining({
      method: "POST",
      headers: expect.objectContaining({ "content-type": "application/json" }),
      body: "{}",
    }));
    expect(out).toEqual([{ id: "a1" }]);
  });

  it("throws the parsed AppError object (not an Error) on non-2xx", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify({ code: "core.db", message: "boom" }), { status: 500 })));
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as { invoke: (c: string, a?: AnyRec) => Promise<unknown> };
    // bindings.ts does `if (e instanceof Error) throw e; else return {status:"error", error:e}`
    // so the thrown value MUST be the plain AppError object.
    await expect(internals.invoke("list_accounts", {})).rejects.toEqual({ code: "core.db", message: "boom" });
  });

  it("dispatches SSE frames to listeners registered via plugin:event|listen", async () => {
    vi.stubGlobal("fetch", vi.fn());
    installHttpBackend();
    const internals = w.__TAURI_INTERNALS__ as {
      invoke: (c: string, a?: AnyRec) => Promise<unknown>;
      transformCallback: (cb: unknown) => number;
    };
    const received: unknown[] = [];
    const handler = internals.transformCallback((e: unknown) => received.push(e));
    await internals.invoke("plugin:event|listen", { event: "copilot-stream-frame", handler });
    const es = (globalThis.EventSource as unknown as AnyRec).last as {
      onmessage: (e: { data: string }) => void;
    };
    es.onmessage({ data: JSON.stringify({ event: "copilot-stream-frame", payload: { type: "text", delta: "hi" } }) });
    expect(received).toHaveLength(1);
    expect((received[0] as AnyRec).payload).toEqual({ type: "text", delta: "hi" });
  });
});
```
Run: `cd ui && npx vitest run src/api/httpBackend.test.ts` → FAIL (module missing).

- [ ] **Step 2: Implement `httpBackend.ts`** — model directly on `mockBackend.ts`'s internals-installation block (lines ~547–590):

```typescript
/**
 * PRODUCTION browser transport: installs an HTTP-backed `__TAURI_INTERNALS__`
 * so the generated bindings.ts works unchanged against finsight-server.
 * - invoke(cmd, args)        → POST /api/rpc/{cmd}
 * - plugin:event|listen      → registry + one shared EventSource(/api/events)
 * Mirrors ui/src/dev/mockBackend.ts (the proven shape for this trick).
 */
type AnyRec = Record<string, unknown>;

export function installHttpBackend(): void {
  const w = window as unknown as AnyRec;
  if (w.__TAURI_INTERNALS__) return; // never shadow a real Tauri runtime

  let cbSeq = 0;
  // event name → callback ids; SSE frames fan out to window[`_${id}`]
  const listeners = new Map<string, Set<number>>();
  let es: EventSource | null = null;

  function ensureEventSource() {
    if (es) return;
    es = new EventSource("/api/events");
    es.onmessage = (msg) => {
      const { event, payload } = JSON.parse(msg.data) as { event: string; payload: unknown };
      for (const id of listeners.get(event) ?? []) {
        const cb = w[`_${id}`] as ((e: unknown) => void) | undefined;
        // Shape mirrors @tauri-apps/api v2 event delivery: {event, id, payload}
        cb?.({ event, id, payload });
      }
    };
  }

  const invoke = async (cmd: string, args?: AnyRec): Promise<unknown> => {
    if (cmd.startsWith("plugin:")) {
      if (cmd === "plugin:event|listen") {
        const { event, handler } = (args ?? {}) as { event: string; handler: number };
        if (!listeners.has(event)) listeners.set(event, new Set());
        listeners.get(event)!.add(handler);
        ensureEventSource();
        return handler; // unlisten id
      }
      if (cmd === "plugin:event|unlisten") {
        const { event, eventId } = (args ?? {}) as { event: string; eventId: number };
        listeners.get(event)?.delete(eventId);
        return null;
      }
      return null; // other plugin traffic (dialog, notification) resolves harmlessly
    }
    const res = await fetch(`/api/rpc/${cmd}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(args ?? {}),
    });
    const body = res.status === 204 ? null : await res.json();
    // Throw the plain AppError object so bindings.ts's catch returns
    // {status:"error", error} exactly as it does under real Tauri.
    if (!res.ok) throw body;
    return body;
  };

  w.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (cb: unknown) => {
      const id = ++cbSeq;
      w[`_${id}`] = cb;
      return id;
    },
    unregisterCallback: () => {},
    unregisterListener: () => {},
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
  };
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
}
```
During implementation, verify the exact `plugin:event|listen` / `unlisten` arg names against the installed `@tauri-apps/api` (^2.0.0) source in `ui/node_modules/@tauri-apps/api/event.js` — adjust `handler`/`eventId` keys if the version differs.

- [ ] **Step 3: Wire into `main.tsx` boot()** — extend the existing block (lines ~47–57):
```typescript
async function boot() {
  if (typeof window !== "undefined") {
    const params = new URLSearchParams(window.location.search);
    const w = window as unknown as { __TAURI_INTERNALS__?: unknown };
    if (import.meta.env.DEV && params.has("mock") && !w.__TAURI_INTERNALS__) {
      const { installMockBackend } = await import("./dev/mockBackend");
      installMockBackend(params.get("mock"));
    } else if (!w.__TAURI_INTERNALS__) {
      // No Tauri, no mock → we're being served by finsight-server (or vite
      // proxying to it). Install the production HTTP/SSE transport.
      const { installHttpBackend } = await import("./api/httpBackend");
      installHttpBackend();
    }
  }
  renderApp();
}
```

- [ ] **Step 4: Run** — `cd ui && npx vitest run` → all green (424 + new). `npx tsc --noEmit` → 0 errors.
- [ ] **Step 5: Commit** — `git commit -am "feat(ui): HTTP/SSE transport shim for server mode"`

---

### Task 14: Dev ergonomics + docs

**Files:** Modify `ui/vite.config.ts`, `CLAUDE.md`

- [ ] **Step 1:** Vite proxy so `cd ui && npm run dev` + `cargo run -p finsight-server` compose:
```typescript
server: {
  port: 5173,
  strictPort: false,
  proxy: {
    "/api": { target: "http://localhost:8674", changeOrigin: false },
  },
},
```
(EventSource over the proxy works; `/api/events` needs no special config.)

- [ ] **Step 2:** CLAUDE.md — add to Commands:
```bash
# Server mode (Immich-style): API + SSE + serves ui/dist on :8674
# Data dir defaults to ./data (FINSIGHT_DATA_DIR to override; keyfile db.key inside)
cargo run -p finsight-server

# Browser dev against the server: run the server, then `cd ui && npm run dev`
# (vite proxies /api → :8674; no ?mock needed)
```
And a short "Server architecture" paragraph pointing at the spec + finsight-api/finsight-server crates.

- [ ] **Step 3:** Commit — `git commit -am "chore(server): vite /api proxy + CLAUDE.md server docs"`

---

### Task 15: End-to-end verification (exit criterion)

- [ ] **Step 1:** Full green bar: `cargo test --workspace` then `cd ui && npx vitest run && npx tsc --noEmit`. Bindings zero-diff check one final time.
- [ ] **Step 2:** `cd ui && npm run build` → dist exists.
- [ ] **Step 3:** Launch `cargo run -p finsight-server` with `FINSIGHT_DATA_DIR` pointed at a scratch dir; open `http://localhost:8674` in the browser preview and verify, in order:
  1. App boots (no `__TAURI` errors in console; `app_ready` no-op succeeds; onboarding renders on the empty DB).
  2. Create an account via the UI → appears in the accounts list (RPC write+read).
  3. Add a manual transaction → shows on Transactions screen.
  4. Open Copilot, send a message → **SSE streaming frames render** (requires configuring an LLM provider via Settings first; if no key available, verify the frames error path renders gracefully and note it).
  5. `GET /api/health` returns `{"status":"ok"}`.
  6. An UNSUPPORTED command (Settings → export) surfaces a readable error toast, not a crash.
  7. **Exercise a multi-word-arg command that the parity test guards but the smoke flow above doesn't** — e.g. Goals → contribute to a goal (`contribute_to_goal`), or set an account balance (`set_account_balance`, arg `balanceCents`). Confirm it succeeds (no `rpc.bad_arg`). This is belt-and-suspenders on top of the Task 10 arg-key test.
- [ ] **Step 4:** Record results (screenshots/log excerpts) in the PR description; note the deferred items: desktop startup cascade parity (Phase 2), dialog import/export web flows (Phase 3), auth (Phase 2).
- [ ] **Step 5:** Final commit; use superpowers:finishing-a-development-branch to merge/PR.

---

## Deferred to later phases (explicitly NOT in this plan)

- Auth, sessions, multi-user, key wrapping (Phase 2 — keyfile is a stopgap).
- Desktop startup cascade (integrity check, pre-migration backup, categorization/pairing/snapshot refresh) in server bootstrap (Phase 2).
- Web upload/download flows for CSV import/export dialog commands (Phase 3).
- PWA manifest/service worker/offline cache, Docker, deployment docs (Phase 3).
- Thin Tauri shell; deleting the wrapper command surface (Phase 4).
- **Long-held `stream_copilot_message` POST:** in Phase 1 the RPC POST stays open for the whole Copilot run (up to the 180s agent loop ceiling) while frames arrive on the side-channel SSE. Fine on localhost. Reverse proxies (Caddy/Traefik) commonly cut idle requests at 30–60s, so **before Phase 3 goes behind a proxy**, revisit this — either return immediately and let the client rely purely on SSE + conversation-refetch (matches the spec's "runs survive disconnects" rule), or stream the HTTP response in chunks. Flag in the Phase 1 PR notes.
