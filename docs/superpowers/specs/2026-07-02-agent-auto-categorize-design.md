# Design: Auto-categorize new transactions (Settings → Agent)

## Background

The design-conformance audit's Tier B item 3 originally assumed a real "categorize on import" behavior already existed and just needed a Settings toggle wired to it. That assumption was wrong: `AgentJob::CategorizeImport { import_id }` is defined in `crates/finsight-agent/src/agent.rs` but is **never dispatched anywhere** — not from CSV import, not from any SimpleFin sync path. There is no existing auto-categorize-on-import behavior to gate.

Decision (confirmed with user): build the real behavior — automatic LLM categorization fires after new transactions land — gated by a new Settings toggle, **default ON** (matches the original mockup).

## Scope

**In scope — user-initiated entry points, both routed through `AppState` (has `agent: AgentHandle`):**
1. `import_csv` (`crates/finsight-app/src/commands/import.rs`) — after a successful import with `rows_done > 0`, dispatch `AgentJob::CategorizeImport { import_id }` if the toggle is on.
2. SimpleFin manual sync / initial import — `sync_simplefin_account`, `import_simplefin_accounts` (both call `sync_local_account` in `crates/finsight-app/src/commands/simplefin.rs`). SimpleFin transactions aren't tagged with a CSV-style `import_id`, so on `summary.added > 0` dispatch `AgentJob::CategorizeAll` (already excludes previously-categorized transactions) if the toggle is on.

**Out of scope (explicit, documented limitation, not silently dropped):**
- Background scheduled SimpleFin sync (`SyncScheduler` in `crates/finsight-app/src/sync_scheduler.rs`) — it owns only a `Db`, no `AgentHandle`, and threading one in means changing its construction signature and startup wiring in `lib.rs`. Auto-categorizing silently in the background is also a materially different UX moment (no user action in the loop) from the two in-scope cases. Left as a known follow-up, noted in the audit doc when this item closes.

## Backend

- `crates/finsight-app/src/commands/settings.rs`: add `AUTO_CATEGORIZE_ENABLED_KEY = "agent.auto_categorize_enabled"`, `get_auto_categorize_enabled` (default `true`) / `set_auto_categorize_enabled`, following the exact `get_notifications_enabled`/`set_notifications_enabled` pattern already in the file.
- `import.rs::import_csv`: after `app.emit("import-complete", ...)`, check the setting; if on and `summary.rows_done > 0`, `state.agent.tx.try_send(AgentJob::CategorizeImport { import_id })` (best-effort — `try_send` failure just skips, same as existing manual triggers do on a full queue).
- `simplefin.rs`: `sync_local_account` currently takes `db: &Db` only, no `AppState`/`AgentHandle`. Add an `agent: Option<&AgentHandle>` parameter (or thread `AppState` through) so `sync_simplefin_account` and `import_simplefin_accounts` can pass `Some(&state.agent)`; after a successful commit with `summary.added > 0`, check the setting and dispatch `AgentJob::CategorizeAll` if on.

## Frontend

- `ui/src/api/hooks/settings.ts`: add `useAutoCategorizeEnabled` / `useSetAutoCategorizeEnabled`, mirroring `useNotificationsEnabled`/`useSetNotificationsEnabled` exactly (same `queryKey` invalidation pattern).
- `ui/src/screens/Settings.tsx`: add `"agent"` to `SECTIONS`, positioned between `"privacy"` and `"appearance"` (matches the original mockup ordering). New `<Section id="agent" title="Agent" description="...">` with one `s-row` + `Tog`, following the exact notifications-toggle-row template already in the file. Label: "Auto-categorize new transactions." Description: "Automatically categorize transactions after each import or sync, using your configured AI provider."
- `ui/src/screens/Rules.tsx`: soften the Trust dial card's overclaim. Change `"Adjust how much the agent acts without asking. You can change this per category in Settings."` → `"Adjust how much the agent acts without asking. Auto-categorization is controlled in Settings."` (drops the false "per category" claim — the control is global).

## Explicitly not building

- No per-category autonomy control (doesn't exist; Rules.tsx copy is being corrected to stop implying it does).
- No gating of `trigger_categorize` (manual "Re-categorize all" button) or `trigger_recategorize_low_confidence` — those stay manual, unconditional, regardless of the toggle. The toggle only governs *automatic* categorization triggered by new data arriving.
- No background-scheduled-sync gating (see Out of scope above).

## Test plan

- Rust: `get_auto_categorize_enabled` default-true round trip; `import_csv` dispatches `CategorizeImport` when toggle on and skips when off (mock/inspect via existing agent test patterns); same for the SimpleFin sync dispatch path.
- Frontend: hook tests for the new settings hooks (mirroring existing notifications hook tests if any exist); Settings.tsx render test asserting the new Agent section and toggle; Rules.tsx test asserting the corrected copy string.
