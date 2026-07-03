# Auto-categorize new transactions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real "Auto-categorize new transactions" toggle (Settings → Agent, default ON) that automatically dispatches LLM categorization after CSV imports and SimpleFin syncs, since no such automatic behavior exists today.

**Architecture:** A new settings KV key (`agent.auto_categorize_enabled`) gates two existing-but-previously-unused agent job dispatches: `AgentJob::CategorizeImport` after `import_csv` succeeds, and `AgentJob::CategorizeAll` after a SimpleFin sync adds transactions. `sync_local_account` (SimpleFin) currently takes only `db: &Db`; it gains an `agent_tx: Option<mpsc::Sender<AgentJob>>` parameter so its three callers can pass `state.agent.tx.clone()`.

**Tech Stack:** Rust (rusqlite, tokio mpsc), React/TypeScript (tanstack-query), existing `settings.rs` KV pattern, existing `Tog`/`Section` Settings.tsx components.

---

### Task 1: Backend settings key + command pair

**Files:**
- Modify: `crates/finsight-app/src/commands/settings.rs`
- Test: `crates/finsight-app/src/commands/settings.rs` (inline `#[cfg(test)]` module — check if one exists first; if not, add one at the bottom of the file)

- [ ] **Step 1: Check for an existing test module in settings.rs**

Run: `grep -n "mod tests" crates/finsight-app/src/commands/settings.rs`

If none exists, you'll add one in Step 2. If one exists, add the new tests into it.

- [ ] **Step 2: Write the failing test**

This crate's established pattern for command tests (see `crates/finsight-app/src/commands/agent.rs:1606-1618`) is a `TempDir` + SQLCipher `Db::open` + `run_migrations` helper, not an in-memory DB. Add this at the end of `settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, repos::run, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("settings.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[tokio::test]
    async fn auto_categorize_enabled_defaults_true() {
        let (_dir, db) = fresh_db();
        let val: bool = run(&db, |conn| {
            let v: Option<bool> = settings::get(conn, AUTO_CATEGORIZE_ENABLED_KEY)?;
            Ok(v.unwrap_or(true))
        })
        .await
        .unwrap();
        assert!(val);
    }

    #[tokio::test]
    async fn auto_categorize_enabled_round_trips() {
        let (_dir, db) = fresh_db();
        run(&db, |conn| settings::set(conn, AUTO_CATEGORIZE_ENABLED_KEY, &false))
            .await
            .unwrap();
        let val: bool = run(&db, |conn| {
            let v: Option<bool> = settings::get(conn, AUTO_CATEGORIZE_ENABLED_KEY)?;
            Ok(v.unwrap_or(true))
        })
        .await
        .unwrap();
        assert!(!val);
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p finsight-app --lib commands::settings::tests::auto_categorize_enabled_defaults_true`
Expected: FAIL — `AUTO_CATEGORIZE_ENABLED_KEY` not found (doesn't exist yet).

- [ ] **Step 4: Add the constant and command pair**

Add near the top of `crates/finsight-app/src/commands/settings.rs`, alongside `CURRENCY_KEY`:

```rust
const AUTO_CATEGORIZE_ENABLED_KEY: &str = "agent.auto_categorize_enabled";
```

Add the command pair (place near other simple get/set command pairs in this file, matching the `get_notifications_enabled`/`set_notifications_enabled` shape exactly — check `crates/finsight-app/src/commands/settings.rs` for that pair's exact location and mirror it):

```rust
#[tauri::command]
#[specta::specta]
pub async fn get_auto_categorize_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let val: Option<bool> = settings::get(conn, AUTO_CATEGORIZE_ENABLED_KEY)?;
        Ok(val.unwrap_or(true))
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_auto_categorize_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, AUTO_CATEGORIZE_ENABLED_KEY, &enabled)
    })
    .await
    .map_err(AppError::from)
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p finsight-app --lib commands::settings::tests`
Expected: PASS (2 tests: `auto_categorize_enabled_defaults_true`, `auto_categorize_enabled_round_trips`)

- [ ] **Step 6: Register the two new commands**

In `crates/finsight-app/src/lib.rs`, find `collect_commands![` and add `commands::settings::get_auto_categorize_enabled, commands::settings::set_auto_categorize_enabled,` next to the existing `commands::settings::get_notifications_enabled, commands::settings::set_notifications_enabled,` entries (grep for `get_notifications_enabled` in `lib.rs` to find the exact line).

- [ ] **Step 7: Regenerate TypeScript bindings**

Run (from repo root): `cargo run -p finsight-tauri --bin export_bindings`
Expected: exits 0, `ui/src/api/bindings.ts` is modified to include `getAutoCategorizeEnabled`/`setAutoCategorizeEnabled`.

- [ ] **Step 8: Commit**

```bash
git add crates/finsight-app/src/commands/settings.rs crates/finsight-app/src/lib.rs ui/src/api/bindings.ts
git commit -m "feat: add auto_categorize_enabled settings command pair"
```

Do NOT stage any other file. This repo's working tree has unrelated in-progress changes — only ever `git add` the exact files listed above.

---

### Task 2: Frontend settings hooks

**Files:**
- Modify: `ui/src/api/hooks/settings.ts`
- Test: `ui/src/api/hooks/settings.test.ts` (create if it doesn't already exist — check first)

- [ ] **Step 1: Check for an existing test file**

Run: `ls ui/src/api/hooks/settings.test.ts 2>/dev/null || echo "none"`

If none exists, this task creates it fresh, testing only the two new hooks (don't retroactively add tests for pre-existing hooks — out of scope).

- [ ] **Step 2: Write the failing test**

If the file doesn't exist, check an existing hook test file for the mocking pattern first — run `grep -rl "commands\." ui/src/api/hooks/*.test.ts | head -1` and open it to copy the exact `vi.mock("../client", ...)` shape used in this codebase. Then write (adapting the mock import path/shape to match what you find):

```ts
import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useAutoCategorizeEnabled, useSetAutoCategorizeEnabled } from "./settings";
import { commands } from "../client";

vi.mock("../client", () => ({
  commands: {
    getAutoCategorizeEnabled: vi.fn(),
    setAutoCategorizeEnabled: vi.fn(),
  },
}));

vi.mock("../../utils/runtime", () => ({
  isTauriRuntime: () => true,
}));

function wrapper({ children }: { children: ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe("useAutoCategorizeEnabled", () => {
  beforeEach(() => vi.clearAllMocks());

  it("returns the enabled value from the backend", async () => {
    vi.mocked(commands.getAutoCategorizeEnabled).mockResolvedValue({ status: "ok", data: true });
    const { result } = renderHook(() => useAutoCategorizeEnabled(), { wrapper });
    await waitFor(() => expect(result.current.data).toBe(true));
  });
});

describe("useSetAutoCategorizeEnabled", () => {
  beforeEach(() => vi.clearAllMocks());

  it("calls setAutoCategorizeEnabled with the new value", async () => {
    vi.mocked(commands.setAutoCategorizeEnabled).mockResolvedValue({ status: "ok", data: undefined });
    const { result } = renderHook(() => useSetAutoCategorizeEnabled(), { wrapper });
    result.current.mutate(false);
    await waitFor(() => expect(commands.setAutoCategorizeEnabled).toHaveBeenCalledWith(false));
  });
});
```

Note: this file needs a `.tsx` extension (not `.ts`) since it contains JSX in `wrapper`. Name it `ui/src/api/hooks/settings.test.tsx`.

- [ ] **Step 3: Run test to verify it fails**

Run: `cd ui && npx vitest run src/api/hooks/settings.test.tsx`
Expected: FAIL — `useAutoCategorizeEnabled` is not exported from `./settings`.

- [ ] **Step 4: Add the two hooks**

Append to `ui/src/api/hooks/settings.ts`, mirroring `useNotificationsEnabled`/`useSetNotificationsEnabled` exactly:

```ts
export function useAutoCategorizeEnabled() {
  return useQuery<boolean>({
    queryKey: ["auto-categorize-enabled"],
    queryFn: async () => {
      const result = await commands.getAutoCategorizeEnabled();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: Infinity,
    enabled: isTauriRuntime(),
  });
}

export function useSetAutoCategorizeEnabled() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (enabled: boolean) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setAutoCategorizeEnabled(enabled);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: ["auto-categorize-enabled"] }),
  });
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd ui && npx vitest run src/api/hooks/settings.test.tsx`
Expected: PASS (2 tests)

- [ ] **Step 6: Commit**

```bash
git add ui/src/api/hooks/settings.ts ui/src/api/hooks/settings.test.tsx
git commit -m "feat: add useAutoCategorizeEnabled hooks"
```

Do NOT stage any other file.

---

### Task 3: Settings.tsx — new Agent section

**Files:**
- Modify: `ui/src/screens/Settings.tsx`
- Test: `ui/src/screens/Settings.test.tsx`

- [ ] **Step 1: Write the failing test**

First run `grep -n "notificationsEnabled\|Notifications enabled" ui/src/screens/Settings.test.tsx` to find the existing notifications-toggle test and copy its mocking setup exactly (same `vi.mock` shape for `../api/hooks/settings`). Add a new test alongside it:

```tsx
it("renders the Agent section with the auto-categorize toggle", async () => {
  render(<Settings />, { wrapper: TestWrapper }); // use whatever render helper this file already uses
  expect(await screen.findByText("Auto-categorize new transactions")).toBeInTheDocument();
  expect(screen.getByRole("switch", { name: "" })).toBeTruthy(); // adjust to match existing toggle query pattern used for notifications
});
```

Before finalizing this step, read the existing notifications-toggle test in `ui/src/screens/Settings.test.tsx` in full and copy its exact render/query/mock idioms rather than guessing — this codebase has an established pattern for these Settings tests that must be matched (mock module paths, `TestWrapper`/`renderWithProviders` helper name, how `useNotificationsEnabled` is mocked to return a value).

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/screens/Settings.test.tsx -t "Agent section"`
Expected: FAIL — text "Auto-categorize new transactions" not found.

- [ ] **Step 3: Add the section**

In `ui/src/screens/Settings.tsx`:

1. Add to the import on line 13:
```tsx
import { useDefaultCurrency, useSetCurrency, useExportJson, useExportCsv, useNotificationsEnabled, useSetNotificationsEnabled, useAutoCategorizeEnabled, useSetAutoCategorizeEnabled } from "../api/hooks/settings";
```

2. Update `SECTIONS` (line 39-48) to insert `"agent"` between `"privacy"` and `"provider"`... wait — the mockup places it between `"privacy"` and `"appearance"`, but `"provider"` sits between those two already. Insert immediately after `"privacy"`:

```tsx
const SECTIONS = [
  ["profile", "Profile"],
  ["privacy", "Privacy & data"],
  ["agent", "Agent"],
  ["provider", "AI Provider"],
  ["appearance", "Appearance"],
  ["connections", "Connections"],
  ["notifications", "Notifications"],
  ["keyboard", "Keyboard"],
  ["about", "About"],
] as const;
```

3. Inside the component body, near line 118-119 (next to the `notificationsEnabled` state), add:

```tsx
const { data: autoCategorizeEnabled = true } = useAutoCategorizeEnabled();
const setAutoCategorizeMutation = useSetAutoCategorizeEnabled();
```

4. Add a new `<Section>` block immediately before the existing `<Section id="provider" ...>` block (find it by its heading "AI Provider"):

```tsx
<Section id="agent" title="Agent" description="Control what the agent does automatically.">
  <div className="s-row"><div><div className="label">Auto-categorize new transactions</div><div className="desc">Automatically categorize transactions after each import or sync, using your configured AI provider.</div></div><div className="muted">{autoCategorizeEnabled ? "Currently on" : "Currently off"}</div><Tog checked={autoCategorizeEnabled} onChange={(value) => setAutoCategorizeMutation.mutate(value)} /></div>
</Section>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd ui && npx vitest run src/screens/Settings.test.tsx`
Expected: PASS, including the new test and all pre-existing Settings tests (no regressions from the `SECTIONS` reorder).

- [ ] **Step 5: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/screens/Settings.tsx ui/src/screens/Settings.test.tsx
git commit -m "feat: add Agent section to Settings with auto-categorize toggle"
```

Do NOT stage any other file.

---

### Task 4: Rules.tsx copy fix

**Files:**
- Modify: `ui/src/screens/Rules.tsx:308`
- Test: `ui/src/screens/Rules.test.tsx`

- [ ] **Step 1: Write the failing test**

Check `ui/src/screens/Rules.test.tsx` for an existing render test of the Trust dial card (grep for "Trust dial" or "per category"). Add or adapt a test:

```tsx
it("does not overclaim a per-category autonomy control", () => {
  render(<Rules />); // match this file's existing render helper
  expect(screen.queryByText(/per category in Settings/i)).not.toBeInTheDocument();
  expect(screen.getByText(/Auto-categorization is controlled in Settings/i)).toBeInTheDocument();
});
```

Match whatever render/wrapper helper the rest of `Rules.test.tsx` already uses.

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/screens/Rules.test.tsx -t "per-category"`
Expected: FAIL — old copy still present.

- [ ] **Step 3: Fix the copy**

In `ui/src/screens/Rules.tsx`, line 308, change:

```tsx
Adjust how much the agent acts without asking. You can change this per category in Settings.
```

to:

```tsx
Adjust how much the agent acts without asking. Auto-categorization is controlled in Settings.
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd ui && npx vitest run src/screens/Rules.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Rules.tsx ui/src/screens/Rules.test.tsx
git commit -m "fix: soften Rules.tsx trust-dial copy to match global (not per-category) auto-categorize control"
```

Do NOT stage any other file.

---

### Task 5: Backend — dispatch CategorizeImport after CSV import

**Files:**
- Modify: `crates/finsight-app/src/commands/import.rs`
- Test: `crates/finsight-app/src/commands/import.rs` (inline test module)

- [ ] **Step 1: Write the failing test**

First check for an existing test module in `import.rs` (`grep -n "cfg(test)" crates/finsight-app/src/commands/import.rs`). If a full `import_csv` integration test already exists there (it likely doesn't, since import needs a real CSV file and a Tauri `AppHandle` mock — check first), skip to a narrower unit test instead: test that `get_auto_categorize_enabled` default is read correctly is already covered in Task 1. For this task, since `import_csv` requires a `tauri::AppHandle` (hard to construct in a unit test), verify the dispatch logic via a small extracted helper function instead of testing `import_csv` end-to-end:

Add this pure helper (testable without Tauri) right above `import_csv` in `import.rs`:

```rust
fn should_auto_categorize(enabled: bool, rows_imported: u32) -> bool {
    enabled && rows_imported > 0
}
```

Test module at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_dispatch_when_disabled() {
        assert!(!should_auto_categorize(false, 5));
    }

    #[test]
    fn does_not_dispatch_when_no_rows_imported() {
        assert!(!should_auto_categorize(true, 0));
    }

    #[test]
    fn dispatches_when_enabled_and_rows_imported() {
        assert!(should_auto_categorize(true, 3));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-app --lib commands::import::tests`
Expected: FAIL — `should_auto_categorize` not defined yet (add it in the next step, then these pass immediately — this is acceptable since the function is trivial; the meaningful verification is Step 4's manual dispatch wiring check via `cargo build`).

- [ ] **Step 3: Add the helper and wire the dispatch**

Add the `should_auto_categorize` helper (Step 1's code) above `import_csv`.

Modify `import_csv` (currently lines 28-70) — insert the dispatch after `app.emit("import-complete", &summary).ok();` (line 61) and before the existing notification-check spawn:

```rust
    let summary = summary?;
    app.emit("import-complete", &summary).ok();

    let auto_categorize_enabled = run(&db, |conn| {
        let val: Option<bool> = finsight_core::settings::get(conn, "agent.auto_categorize_enabled")?;
        Ok(val.unwrap_or(true))
    })
    .await
    .unwrap_or(true);

    if should_auto_categorize(auto_categorize_enabled, summary.rows_imported) {
        let _ = state
            .agent
            .tx
            .try_send(finsight_agent::agent::AgentJob::CategorizeImport {
                import_id: summary.import_id.clone(),
            });
    }

    let notify_app = app.clone();
    let notify_db = (*state.db).clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::notifications::check_and_fire(&notify_app, &notify_db).await;
    });

    Ok(summary)
```

Add `use finsight_core::repos::run;` is already imported (line 3: `use finsight_core::repos::{imports as imports_repo, run};`) — no new import needed for `run`. Add at the top of the file: no new top-level import needed since we're using fully-qualified `finsight_core::settings::get` and `finsight_agent::agent::AgentJob` inline — this avoids colliding with any existing `settings` identifier in this file. Confirm `finsight_agent` is already a dependency of `finsight-app` (check `crates/finsight-app/Cargo.toml` for `finsight-agent = { path = ... }` — it is, since `commands/agent.rs` already imports `finsight_agent::agent::AgentJob`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-app --lib commands::import::tests`
Expected: PASS (3 tests)

Also run: `cargo build -p finsight-app` to confirm the wiring compiles (this is the real verification for this task, since `import_csv` itself isn't unit-testable here).
Expected: exits 0.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/import.rs
git commit -m "feat: dispatch auto-categorize job after CSV import when enabled"
```

Do NOT stage any other file.

---

### Task 6: Backend — dispatch CategorizeAll after SimpleFin sync

**Files:**
- Modify: `crates/finsight-app/src/commands/simplefin.rs`

- [ ] **Step 1: Write the failing test**

Reuse the same pure-helper testing strategy as Task 5, since `sync_local_account` requires network/keychain access and isn't unit-testable directly. Add near the top of `simplefin.rs` (this is the same logic as Task 5's helper — duplicated intentionally since the two call sites live in different files with different summary types; do not introduce a shared crate-level helper for two call sites, that's premature abstraction):

```rust
fn should_auto_categorize(enabled: bool, added: usize) -> bool {
    enabled && added > 0
}
```

Add a test module at the bottom of `simplefin.rs` (check first if one exists — grep `cfg(test)` in this file; if it exists, add these two tests into it):

```rust
#[cfg(test)]
mod auto_categorize_tests {
    use super::*;

    #[test]
    fn skips_when_disabled() {
        assert!(!should_auto_categorize(false, 5));
    }

    #[test]
    fn skips_when_nothing_added() {
        assert!(!should_auto_categorize(true, 0));
    }

    #[test]
    fn dispatches_when_enabled_and_added() {
        assert!(should_auto_categorize(true, 2));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-app --lib commands::simplefin::auto_categorize_tests`
Expected: FAIL — `should_auto_categorize` not defined.

- [ ] **Step 3: Add the helper, thread `agent_tx` through `sync_local_account`, and dispatch**

Add the helper function (Step 1's code) near the top of `simplefin.rs`, after the existing `const` declarations.

Change `sync_local_account`'s signature (currently at line 537-542):

```rust
async fn sync_local_account(
    db: &finsight_core::Db,
    account_id: &str,
    connection_id: String,
    import_pending: bool,
    agent_tx: Option<tokio::sync::mpsc::Sender<finsight_agent::agent::AgentJob>>,
) -> AppResult<SimpleFinImportSummaryWrapper> {
```

Insert the dispatch right before the existing `Ok(SimpleFinImportSummaryWrapper { ... })` return (currently lines 622-627):

```rust
    let auto_categorize_enabled = run(db, |conn| {
        let val: Option<bool> = settings::get(conn, "agent.auto_categorize_enabled")?;
        Ok(val.unwrap_or(true))
    })
    .await
    .unwrap_or(true);

    if should_auto_categorize(auto_categorize_enabled, summary.added) {
        if let Some(tx) = &agent_tx {
            let _ = tx.try_send(finsight_agent::agent::AgentJob::CategorizeAll);
        }
    }

    Ok(SimpleFinImportSummaryWrapper {
        added: summary.added,
        updated: summary.updated,
        skipped: summary.skipped,
        queued_for_review: summary.queued_for_review,
    })
```

Update the three call sites:

1. `sync_simplefin_account` (line ~526-527), change:
```rust
    let summary =
        sync_local_account(&db, &account_id, connection_id, account.import_pending).await?;
```
to:
```rust
    let summary = sync_local_account(
        &db,
        &account_id,
        connection_id,
        account.import_pending,
        Some(state.agent.tx.clone()),
    )
    .await?;
```

2. `import_simplefin_accounts` (line ~500), change:
```rust
        if let Err(e) = sync_local_account(&db, local_id, req.connection_id.clone(), false).await {
```
to:
```rust
        if let Err(e) = sync_local_account(
            &db,
            local_id,
            req.connection_id.clone(),
            false,
            Some(state.agent.tx.clone()),
        )
        .await
        {
```

3. Check for any other callers: `grep -n "sync_local_account(" crates/finsight-app/src/commands/simplefin.rs` — if `sync_scheduler.rs` or anywhere else calls this function directly (rather than through `sync_simplefin_account`/`import_simplefin_accounts`), pass `None` there, since the scheduler has no `AgentHandle` (documented out-of-scope limitation from the design spec).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-app --lib commands::simplefin::auto_categorize_tests`
Expected: PASS (3 tests)

Also run: `cargo build -p finsight-app` to confirm all call sites compile with the new parameter.
Expected: exits 0.

- [ ] **Step 5: Run the full Rust test suite to check for regressions**

Run: `cargo test --workspace`
Expected: all tests pass (baseline count + 6 new tests from Tasks 1, 5, 6).

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-app/src/commands/simplefin.rs
git commit -m "feat: dispatch auto-categorize job after SimpleFin sync when enabled"
```

Do NOT stage any other file.

---

### Task 7: Full-suite verification and live check

**Files:** none (verification only)

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test --workspace`
Expected: green (baseline + new tests from Tasks 1, 5, 6 — no regressions).

- [ ] **Step 2: Run full frontend test suite**

Run: `cd ui && npx vitest run`
Expected: green (baseline + new tests from Tasks 2, 3, 4 — no regressions).

- [ ] **Step 3: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 4: Live verification in the running Tauri app**

Start `pnpm tauri:dev` (or reuse an already-running instance). Navigate to Settings → confirm the "Agent" nav item appears between "Privacy & data" and "AI Provider", clicking it scrolls to a section titled "Agent" with the "Auto-categorize new transactions" toggle defaulting to on. Navigate to Rules → confirm the Trust dial card no longer says "per category in Settings". If a test account/CSV fixture is available, perform a CSV import and confirm categorization kicks in automatically (check for the existing categorization UI feedback, e.g. toast/badge, that already fires for manual "Re-categorize all").

- [ ] **Step 5: Update the master audit doc**

In `docs/superpowers/plans/2026-07-01-design-conformance-deep-audit.md`, mark the "Settings: real Agent settings section" (or however Tier B item 3 is phrased there) as done, with a pointer to this plan and `docs/superpowers/specs/2026-07-02-agent-auto-categorize-design.md`, and a note that background-scheduled-sync auto-categorization was explicitly deferred (not silently dropped).

- [ ] **Step 6: Commit the audit doc update**

```bash
git add docs/superpowers/plans/2026-07-01-design-conformance-deep-audit.md
git commit -m "docs: mark Tier B item 3 (agent auto-categorize) done"
```

Do NOT stage any other file.
