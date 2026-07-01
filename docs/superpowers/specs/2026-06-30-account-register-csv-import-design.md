# Account Register CSV Import Button

**Date:** 2026-06-30  
**Status:** Approved  
**Scope:** Add an Import button to the account register page that lets users import a CSV statement directly into the current account, with the account pre-selected in the mapping dialog.

## Context

The app already supports CSV import via `ui/src/screens/onboarding/ImportMappingDialog.tsx`, which is used during onboarding in `StepConnect.tsx`. The dialog accepts a `defaultAccountId` prop and uses `useImportCsv` to import rows into the chosen account.

With the new account-first navigation, each account has its own register page at `/accounts/:id/transactions`. Users need a way to import statements into that account without going through onboarding again.

## Goals

1. Add an **Import** button to the account register page header.
2. Clicking Import opens the system file picker for a CSV.
3. After a file is selected, open `ImportMappingDialog` with the current account pre-selected.
4. Move `ImportMappingDialog` out of `screens/onboarding/` so it can be reused outside onboarding.
5. After import, the transaction list refreshes automatically and the dialog closes.

## Non-goals

- No changes to the mapping dialog's internal column-mapping logic.
- No new backend endpoints or import formats.
- No persistence of "last used mapping" beyond what the dialog already does.

## Design

### File Move

Move `ImportMappingDialog` to the shared components directory:

- **From:** `ui/src/screens/onboarding/ImportMappingDialog.tsx`
- **To:** `ui/src/components/ImportMappingDialog.tsx`

Update all imports:
- `ui/src/screens/onboarding/StepConnect.tsx`
- `ui/src/screens/AccountTransactions.tsx`

### Account Register Page Changes

In `ui/src/screens/AccountTransactions.tsx`:

1. Add imports:
   ```tsx
   import { open as openDialog } from "@tauri-apps/plugin-dialog";
   import ImportMappingDialog from "../components/ImportMappingDialog";
   ```

2. Add local state:
   ```tsx
   const [csvPath, setCsvPath] = useState<string | null>(null);
   ```

3. Add an Import button in the header action row (next to Export and Add manual):
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

4. Render `ImportMappingDialog` when `csvPath` is set:
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

### Data Flow

```
User clicks Import
  → openDialog({ filters: [{ name: "CSV", extensions: ["csv"] }] })
  → user selects CSV path
  → AccountTransactions sets csvPath
  → ImportMappingDialog renders with defaultAccountId={account.id}
  → user maps columns and submits
  → useImportCsv mutates, importing into the pre-selected account
  → useImportCsv onSuccess invalidates ["transactions"] queries
  → AccountTransactions refetches automatically
  → onImported closes the dialog
```

### Error Handling

- If the user cancels the file picker, `selected` is not a string and nothing happens.
- If `isTauriRuntime()` is false (browser preview), show a toast explaining that import requires the desktop app.
- Import errors are displayed inside `ImportMappingDialog` as they are today.

### Testing

1. **Move test file:** Move `ui/src/test/ImportMappingDialog.test.tsx` to `ui/src/components/ImportMappingDialog.test.tsx` and update its import path.

2. **Update `AccountTransactions.test.tsx`:**
   - Mock `@tauri-apps/plugin-dialog` so `open` returns a fake path.
   - Click the Import button.
   - Assert that `ImportMappingDialog` renders with the current account pre-selected.
   - Mock `ImportMappingDialog` itself if testing its internals would require too many hooks.

3. **Update `StepConnect.test.tsx` (if it exists):**
   - Update the import path for `ImportMappingDialog`.

## Risks

- Moving the dialog changes its import path in onboarding; all existing tests must be updated.
- The Import button is only functional in the Tauri desktop runtime; in browser preview it shows a helpful toast.
