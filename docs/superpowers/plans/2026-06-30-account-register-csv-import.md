# Account Register CSV Import Button Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an Import button to the account register page that opens a CSV file picker and launches `ImportMappingDialog` with the current account pre-selected.

**Architecture:** Move `ImportMappingDialog` from `screens/onboarding/` to `components/` so it can be reused. Add the Import button and file-picker logic to `AccountTransactions.tsx`. Update all imports and tests.

**Tech Stack:** React, TypeScript, Tauri plugin dialog, TanStack Query, vitest.

## Global Constraints

- Move `ImportMappingDialog` to `ui/src/components/ImportMappingDialog.tsx`.
- Add Import button next to Export/Add manual in `AccountTransactions.tsx`.
- Pre-select the current account via `defaultAccountId`.
- Use `@tauri-apps/plugin-dialog` `open()` with CSV filter.
- Show a toast if not running in Tauri runtime.
- No changes to mapping dialog internals.
- Run `cd ui && npx tsc --noEmit` and relevant tests before each commit.

---

### Task 1: Move ImportMappingDialog to shared components

**Files:**
- Move: `ui/src/screens/onboarding/ImportMappingDialog.tsx` → `ui/src/components/ImportMappingDialog.tsx`
- Move: `ui/src/test/ImportMappingDialog.test.tsx` → `ui/src/components/ImportMappingDialog.test.tsx`
- Modify: `ui/src/screens/onboarding/StepConnect.tsx`

**Interfaces:**
- Consumes: Same props as before (`path`, `onClose`, `onImported`, `defaultAccountId?`).
- Produces: `ImportMappingDialog` component exported from `ui/src/components/ImportMappingDialog.tsx`.

- [ ] **Step 1: Move component file**

  ```bash
  git mv ui/src/screens/onboarding/ImportMappingDialog.tsx ui/src/components/ImportMappingDialog.tsx
  ```

- [ ] **Step 2: Move test file**

  ```bash
  git mv ui/src/test/ImportMappingDialog.test.tsx ui/src/components/ImportMappingDialog.test.tsx
  ```

- [ ] **Step 3: Update test import paths**

  In `ui/src/components/ImportMappingDialog.test.tsx`, update the import:
  ```tsx
  import ImportMappingDialog from "./ImportMappingDialog";
  ```

  Update any other relative imports inside the test file from `../../...` to `../...` as needed.

- [ ] **Step 4: Update StepConnect.tsx import**

  In `ui/src/screens/onboarding/StepConnect.tsx`, change:
  ```tsx
  import ImportMappingDialog from "./ImportMappingDialog";
  ```
  to:
  ```tsx
  import ImportMappingDialog from "../../components/ImportMappingDialog";
  ```

- [ ] **Step 5: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/components/ImportMappingDialog.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [ ] **Step 6: Commit**

  ```bash
  git add ui/src/components/ImportMappingDialog.tsx ui/src/components/ImportMappingDialog.test.tsx ui/src/screens/onboarding/StepConnect.tsx
  git commit -m "refactor: move ImportMappingDialog to shared components"
  ```

---

### Task 2: Add Import button to AccountTransactions

**Files:**
- Modify: `ui/src/screens/AccountTransactions.tsx`
- Modify: `ui/src/screens/AccountTransactions.test.tsx`

**Interfaces:**
- Consumes: `open` from `@tauri-apps/plugin-dialog`, `ImportMappingDialog`, `isTauriRuntime`, `useState`.
- Produces: Account register page with working Import button that opens the mapping dialog pre-filled with the current account.

- [ ] **Step 1: Add the failing test**

  In `ui/src/screens/AccountTransactions.test.tsx`, add a test that mocks the file picker and asserts the mapping dialog opens with the right account:

  ```tsx
  import { vi } from "vitest";
  import { render, screen, fireEvent, waitFor } from "@testing-library/react";

  vi.mock("@tauri-apps/plugin-dialog", () => ({
    open: vi.fn(),
  }));

  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  it("opens the import mapping dialog after picking a CSV", async () => {
    (openDialog as ReturnType<typeof vi.fn>).mockResolvedValueOnce("/path/to/export.csv");
    render(
      <MemoryRouter initialEntries={["/accounts/acc-1/transactions"]}>
        <Routes>
          <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
        </Routes>
      </MemoryRouter>
    );
    const importBtn = await screen.findByRole("button", { name: /Import/i });
    fireEvent.click(importBtn);
    await waitFor(() => {
      expect(screen.getByText("Map CSV columns")).toBeInTheDocument();
    });
  });
  ```

- [ ] **Step 2: Run test to verify it fails**

  Run: `cd ui && npx vitest run src/screens/AccountTransactions.test.tsx`
  Expected: FAIL — Import button not found or dialog not rendered.

- [ ] **Step 3: Implement Import button and dialog**

  In `ui/src/screens/AccountTransactions.tsx`:

  Add imports:
  ```tsx
  import { useState } from "react";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import ImportMappingDialog from "../components/ImportMappingDialog";
  import { isTauriRuntime } from "../utils/runtime";
  ```

  Add state:
  ```tsx
  const [csvPath, setCsvPath] = useState<string | null>(null);
  ```

  Add the Import button in the header action row (near Export and Add manual):
  ```tsx
  <button
    className="btn outline sm"
    type="button"
    onClick={async () => {
      if (!isTauriRuntime()) {
        toast.error("CSV import requires the desktop app.");
        return;
      }
      const selected = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "CSV", extensions: ["csv"] }],
      });
      if (typeof selected === "string") setCsvPath(selected);
    }}
  >
    Import
  </button>
  ```

  Render the dialog:
  ```tsx
  {csvPath && account && (
    <ImportMappingDialog
      path={csvPath}
      defaultAccountId={account.id}
      onClose={() => setCsvPath(null)}
      onImported={() => setCsvPath(null)}
    />
  )}
  ```

- [ ] **Step 4: Run tests and type check**

  Run:
  ```bash
  cd ui && npx vitest run src/screens/AccountTransactions.test.tsx
  cd ui && npx tsc --noEmit
  ```
  Expected: Tests pass, no new type errors.

- [ ] **Step 5: Commit**

  ```bash
  git add ui/src/screens/AccountTransactions.tsx ui/src/screens/AccountTransactions.test.tsx
  git commit -m "feat: add CSV import button to account register"
  ```

---

### Task 3: Final verification

**Files:**
- Modify: None (verification only).

- [ ] **Step 1: Run targeted tests**

  Run:
  ```bash
  cd ui && npx vitest run src/screens/AccountTransactions.test.tsx src/components/ImportMappingDialog.test.tsx src/screens/onboarding/StepConnect.test.tsx
  ```
  Expected: All targeted tests pass.

- [ ] **Step 2: Run TypeScript check**

  Run: `cd ui && npx tsc --noEmit`
  Expected: No new type errors.

- [ ] **Step 3: Run Rust check**

  Run: `cargo check --workspace`
  Expected: No errors.

- [ ] **Step 4: Commit any fixes**

  If verification surfaced issues, commit fixes; otherwise this task is verification only.

---

## Self-Review

**Spec coverage:**
- ✅ Move `ImportMappingDialog` to shared components → Task 1
- ✅ Update `StepConnect` import → Task 1
- ✅ Add Import button to `AccountTransactions` → Task 2
- ✅ Pre-select current account → Task 2
- ✅ File picker with CSV filter → Task 2
- ✅ Toast for non-Tauri runtime → Task 2
- ✅ Tests → Tasks 1–2

**Placeholder scan:**
- ✅ No TBD/TODO placeholders
- ✅ All code blocks contain concrete implementation
- ✅ Exact commands provided

**Type consistency:**
- ✅ `ImportMappingDialog` props unchanged
- ✅ `openDialog` import and usage match Tauri plugin API
