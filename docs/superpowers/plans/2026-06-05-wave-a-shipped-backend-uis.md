# Wave A — UI for Shipped Backends Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the six pending Wave A frontend UIs (§3a, §4a, §4b, §5d, §11a, §13b) whose backends are already merged, plus one small backend edit so "net worth" is consistent across the chart, the Accounts header, and the Today hero.

**Architecture:** Six self-contained frontend additions plus one repo edit. A single `netWorth = accounts + manualAssets − liabilities` value is centralized in a `useNetWorth()` hook consumed by the Today hero and Accounts header; the backend snapshot that feeds the §3a chart is updated to compute the same formula. Each task is independently committable and ordered so every hook's dependencies exist before it is used.

**Tech Stack:** Rust (rusqlite) backend; React 18 + TypeScript + Vite; tanstack-query hooks; react-hook-form + zod for drawer forms; sonner toasts; vitest + @testing-library/react for frontend tests. Design tokens in `ui/src/styles/tokens.css` + `app.css`.

---

## Wave A facts the implementer must know

- **No `export_bindings` run is needed for Wave A.** The only backend edit (`record_today`) is an internal repo function, **not** a `#[tauri::command]`. No command signatures change, so `ui/src/api/bindings.ts` does not regenerate. (CLAUDE.md's "regenerate after any Rust change" rule does not apply here because no command surface changes.)
- **Snapshots only re-record on app start.** After the Task 1 backend change, the §3a chart's last point keeps its old (bank-only) value until the next app launch records a fresh snapshot. The *live* headline (`useNetWorth`) updates immediately. A momentary gap between the chart's last point and the headline is expected — do **not** treat it as a bug.
- **Field-casing trap (CLAUDE.md flags this).** The `Transaction` type is **snake_case** (`is_reimbursable`, `is_split`, `merchant_raw`, `amount_cents`). The new Wave A types are **camelCase** (`valueCents`, `assetType`, `balanceCents`, `aprPct`, `whenLabel`, `payoffDate`). Task 6 (§5d) code uses snake_case; all other tasks use camelCase. Mixing them silently fails `tsc --noEmit`.
- **Verified backend types** (from `ui/src/api/bindings.ts`):
  - `ManualAsset = { id, name, assetType, valueCents, currency, notes|null, createdAt, updatedAt }`
  - `NewManualAsset = { name, assetType, valueCents, currency, notes|null }`
  - `ManualAssetPatch = { name|null, assetType|null, valueCents|null, currency|null, notes|null }`
  - `Liability = { id, name, liabilityType, balanceCents, limitCents|null, aprPct|null, payoffDate|null, currency, createdAt, updatedAt }`
  - `NewLiability = { name, liabilityType, balanceCents, limitCents|null, aprPct|null, payoffDate|null, currency }`
  - `LiabilityPatch = { name|null, liabilityType|null, balanceCents|null, limitCents|null, aprPct|null, payoffDate|null, currency|null }`
  - `RuleProposal = { id, whenLabel, description, pattern, categoryId, status, createdAt }`
  - `AgentMemory = { id, kind, description, merchantKey|null, createdAt }`
  - `NetWorthPoint = { date, totalCents }`
- **Verified commands:** `recordNetWorthSnapshot()`, `listNetWorthHistory(days)`, `listManualAssets()`, `createManualAsset(input)`, `updateManualAsset(id, patch)`, `deleteManualAsset(id)`, `listLiabilities()`, `createLiability(input)`, `updateLiability(id, patch)`, `deleteLiability(id)`, `listRuleProposals()`, `acceptRuleProposal(id)`, `declineRuleProposal(id)`, `listAgentMemory()`, `forgetAgentMemory(id)`, `setTransactionFlags(id, isReimbursable, isSplit)`. All return `{ status: "ok", data } | { status: "error", error }`.
- **Verified existing helpers/patterns:**
  - Hooks live in `ui/src/api/hooks/*`, import from `../client`, unwrap with `if (result.status === "error") throw new Error(result.error.message)`, mutations call `qc.invalidateQueries({ queryKey: [...] })`. (Reference: `ui/src/api/hooks/transactions.ts`.)
  - Drawer form pattern: `ui/src/components/AccountDrawer.tsx` (Drawer + useForm + zodResolver + edit/create + delete-confirm).
  - `Drawer` props: `{ open, onClose, title, children, width? }` (`ui/src/components/Drawer.tsx`).
  - Test pattern: `import { createWrapper } from "../test-utils"`, `vi.mock("../api/hooks/...")`, render with `{ wrapper: createWrapper() }`. (Reference: `ui/src/screens/Scenarios.test.tsx`.)

## File Structure

| File | Responsibility | Task |
|------|----------------|------|
| `crates/finsight-core/src/repos/net_worth.rs` (modify) | `record_today` folds in assets − liabilities | 1 |
| `ui/src/api/hooks/assets.ts` (create) | Manual-asset + liability query/mutation hooks | 2, 3 |
| `ui/src/components/AssetDrawer.tsx` (create) | Create/edit/delete a manual asset | 2 |
| `ui/src/components/LiabilityDrawer.tsx` (create) | Create/edit/delete a liability | 3 |
| `ui/src/api/hooks/networth.ts` (create) | `useNetWorthHistory(days)`, `useNetWorth()` | 4 |
| `ui/src/screens/Accounts.tsx` (modify) | Assets section, liabilities section, net-worth header | 2, 3, 4 |
| `ui/src/screens/Today.tsx` (modify) | Hero uses `useNetWorth`; net-worth chart | 4, 5 |
| `ui/src/components/NetWorthChart.tsx` (create) | SVG area chart from `NetWorthPoint[]` | 5 |
| `ui/src/api/hooks/transactions.ts` (modify) | `useSetTransactionFlags` | 6 |
| `ui/src/components/TransactionDrawer.tsx` (modify) | Reimbursable/split toggles | 6 |
| `ui/src/screens/Transactions.tsx` (modify) | Flag chips on table rows | 6 |
| `ui/src/api/hooks/proposals.ts` (create) | `useRuleProposals`, accept/decline | 7 |
| `ui/src/screens/Rules.tsx` (modify) | Agent-proposals card | 7 |
| `ui/src/api/hooks/agentMemory.ts` (create) | `useAgentMemory`, `useForgetAgentMemory` | 8 |
| `ui/src/screens/Insights.tsx` (modify) | Agent-memory section + deferred-undo forget | 8 |

**Design deviations from the prototypes (intentional, do not "fix"):**
- `ManualAsset` has no trend/`delta90d` field → show "updated {date}" instead of the design's ↑/↓ arrow.
- `Icons.tsx` has no house/car/chart/currency glyphs → asset/liability rows use a 28×28 letter tile (first letter of the type) instead of per-type icons. No new icons are added in Wave A.
- `AgentMemory`'s text field is `description` (the prototype called it `learned`) → use `description`.

---

## Task 1: Backend — fold assets/liabilities into the net-worth snapshot

**Files:**
- Modify: `crates/finsight-core/src/repos/net_worth.rs:23-29` (`record_today`)
- Test: `crates/finsight-core/src/repos/net_worth.rs` (tests module, ~line 45)

- [ ] **Step 1: Write the failing test**

Add this test inside the existing `mod tests` block in `crates/finsight-core/src/repos/net_worth.rs` (after `record_snapshot_upserts_one_row_per_day`). It inserts one asset and one liability (no accounts → accounts sum is 0) and asserts the snapshot equals `assets − liabilities`:

```rust
    #[test]
    fn record_today_folds_assets_and_liabilities() {
        use crate::models::{NewLiability, NewManualAsset};
        use crate::repos::{liabilities, manual_assets};

        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();

        manual_assets::create(&mut conn, NewManualAsset {
            name: "House".into(), asset_type: "property".into(),
            value_cents: 50_000_000, currency: "USD".into(), notes: None,
        }).unwrap();
        liabilities::create(&mut conn, NewLiability {
            name: "Mortgage".into(), liability_type: "mortgage".into(),
            balance_cents: 30_000_000, limit_cents: Some(35_000_000),
            apr_pct: Some(5.5), payoff_date: None, currency: "USD".into(),
        }).unwrap();

        record_today(&mut conn).unwrap();

        let hist = list_history(&mut conn, 30).unwrap();
        assert_eq!(hist.len(), 1);
        // 0 accounts + 50,000,000 assets − 30,000,000 liabilities
        assert_eq!(hist[0].total_cents, 20_000_000);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --lib repos::net_worth::tests::record_today_folds_assets_and_liabilities`
Expected: FAIL — current `record_today` sums accounts only, so `total_cents` is `0`, not `20_000_000`.

- [ ] **Step 3: Update `record_today`**

Replace the body of `record_today` (`crates/finsight-core/src/repos/net_worth.rs:23-29`) with:

```rust
/// Sum account balances + manual assets − liabilities, then upsert today's
/// snapshot. Keeps the recorded net worth consistent with the headline shown
/// on the Today/Accounts screens.
pub fn record_today(conn: &mut Connection) -> CoreResult<()> {
    let accounts: i64 = accounts::list_summaries(conn)?
        .iter()
        .map(|a| a.balance_cents)
        .sum();
    let assets: i64 = crate::repos::manual_assets::list(conn)?
        .iter()
        .map(|a| a.value_cents)
        .sum();
    let liabilities: i64 = crate::repos::liabilities::list(conn)?
        .iter()
        .map(|l| l.balance_cents)
        .sum();
    record_snapshot(conn, accounts + assets - liabilities)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p finsight-core --lib repos::net_worth`
Expected: PASS — both `record_snapshot_upserts_one_row_per_day` and `record_today_folds_assets_and_liabilities`.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/repos/net_worth.rs
git commit -m "feat(core): fold manual assets and liabilities into net-worth snapshot"
```

---

## Task 2: §4a — Manual assets hooks, AssetDrawer, and Accounts section

**Files:**
- Create: `ui/src/api/hooks/assets.ts`
- Create: `ui/src/components/AssetDrawer.tsx`
- Modify: `ui/src/screens/Accounts.tsx`
- Test: `ui/src/screens/Accounts.test.tsx` (create)

- [ ] **Step 1: Write the failing test**

Create `ui/src/screens/Accounts.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import Accounts from "./Accounts";
import { createWrapper } from "../test-utils";

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [], isLoading: false, error: null })),
}));

vi.mock("../api/hooks/assets", () => ({
  useManualAssets: vi.fn(() => ({ data: [
    { id: "a1", name: "House", assetType: "property", valueCents: 50000000, currency: "USD", notes: null, createdAt: "2026-06-01T00:00:00Z", updatedAt: "2026-06-01T00:00:00Z" },
  ], isLoading: false })),
  useCreateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useLiabilities: vi.fn(() => ({ data: [] })),
  useCreateLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("Accounts — manual assets", () => {
  it("renders the manual assets section with an asset row", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Manual assets")).toBeInTheDocument();
    expect(screen.getByText("House")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/screens/Accounts.test.tsx`
Expected: FAIL — `Accounts` does not import `../api/hooks/assets` and renders no "Manual assets" text (and the mocked module does not yet exist).

- [ ] **Step 3: Create the assets hooks (manual-asset portion)**

Create `ui/src/api/hooks/assets.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type ManualAsset, type NewManualAsset, type ManualAssetPatch,
} from "../client";

export function useManualAssets() {
  return useQuery<ManualAsset[]>({
    queryKey: ["manual-assets"],
    queryFn: async () => {
      const result = await commands.listManualAssets();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewManualAsset) => {
      const result = await commands.createManualAsset(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["manual-assets"] });
      qc.invalidateQueries({ queryKey: ["net-worth"] });
    },
  });
}

export function useUpdateManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: ManualAssetPatch }) => {
      const result = await commands.updateManualAsset(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["manual-assets"] });
      qc.invalidateQueries({ queryKey: ["net-worth"] });
    },
  });
}

export function useDeleteManualAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.deleteManualAsset(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["manual-assets"] });
      qc.invalidateQueries({ queryKey: ["net-worth"] });
    },
  });
}
```

- [ ] **Step 4: Create `AssetDrawer`**

Create `ui/src/components/AssetDrawer.tsx` (modeled on `AccountDrawer.tsx`):

```tsx
import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import {
  useCreateManualAsset, useUpdateManualAsset, useDeleteManualAsset,
} from "../api/hooks/assets";
import type { ManualAsset } from "../api/bindings";

const ASSET_TYPES = ["cash", "property", "vehicle", "investment", "crypto", "other"] as const;

const schema = z.object({
  name: z.string().min(1, "Required"),
  assetType: z.enum(ASSET_TYPES),
  value_dollars: z.coerce.number().nonnegative("Must be ≥ 0"),
  notes: z.string().optional(),
});
type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  asset?: ManualAsset;
}

export default function AssetDrawer({ open, onClose, asset }: Props) {
  const isEdit = !!asset;
  const create = useCreateManualAsset();
  const update = useUpdateManualAsset();
  const del = useDeleteManualAsset();
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", assetType: "cash", value_dollars: 0, notes: "" },
  });

  useEffect(() => {
    if (asset) {
      reset({
        name: asset.name,
        assetType: asset.assetType as typeof ASSET_TYPES[number],
        value_dollars: asset.valueCents / 100,
        notes: asset.notes ?? "",
      });
    } else {
      reset({ name: "", assetType: "cash", value_dollars: 0, notes: "" });
    }
    setDeleteConfirm(false);
  }, [asset?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  async function onSubmit(values: FormValues) {
    try {
      const valueCents = Math.round(values.value_dollars * 100);
      if (isEdit && asset) {
        await update.mutateAsync({
          id: asset.id,
          patch: {
            name: values.name, assetType: values.assetType, valueCents,
            currency: null, notes: values.notes || null,
          },
        });
      } else {
        await create.mutateAsync({
          name: values.name, assetType: values.assetType, valueCents,
          currency: "USD", notes: values.notes || null,
        });
      }
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not save asset");
    }
  }

  async function handleDelete() {
    if (!deleteConfirm) { setDeleteConfirm(true); return; }
    if (!asset) return;
    try { await del.mutateAsync(asset.id); onClose(); }
    catch { setDeleteConfirm(false); }
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit asset" : "Add manual asset"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Name
          <input {...register("name")} placeholder="e.g. Home" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        <label> Type
          <select {...register("assetType")}>
            {ASSET_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </label>
        <label> Value ($)
          <input type="number" step="0.01" {...register("value_dollars")} aria-invalid={!!errors.value_dollars} />
          {errors.value_dollars && <span className="err">{errors.value_dollars.message}</span>}
        </label>
        <label> Notes <input {...register("notes")} /></label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : (isEdit ? "Save changes" : "Add asset")}
          </button>
        </div>
      </form>
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button type="button" className="danger" onClick={handleDelete} disabled={del.isPending}>
            {deleteConfirm ? "Confirm delete?" : "Delete asset"}
          </button>
          {deleteConfirm && (
            <button type="button" onClick={() => setDeleteConfirm(false)} style={{ marginLeft: 8 }}>Cancel</button>
          )}
        </div>
      )}
    </Drawer>
  );
}
```

- [ ] **Step 5: Add the Manual assets section to `Accounts.tsx`**

In `ui/src/screens/Accounts.tsx`: add imports at the top and render the section after the accounts table (before the closing `</div>` of `screen-accounts`, alongside the existing `AccountDrawer`s). Add these imports:

```tsx
import { useManualAssets } from "../api/hooks/assets";
import AssetDrawer from "../components/AssetDrawer";
import type { ManualAsset } from "../api/client";
```

Inside the component, add state and data near the existing hooks:

```tsx
  const { data: assets = [] } = useManualAssets();
  const [assetAddOpen, setAssetAddOpen] = useState(false);
  const [editAsset, setEditAsset] = useState<ManualAsset | null>(null);
```

Add this JSX just before `<AccountDrawer open={addOpen} ... />`:

```tsx
      <section style={{ marginTop: 40 }}>
        <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Manual assets</h2>
          <button onClick={() => setAssetAddOpen(true)}>+ Add manual asset</button>
        </header>
        {assets.length === 0 ? (
          <div className="stub">No manual assets yet.</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column" }}>
            {assets.map((a) => (
              <div
                key={a.id}
                role="button"
                tabIndex={0}
                onClick={() => setEditAsset(a)}
                onKeyDown={(e) => { if (e.key === "Enter") setEditAsset(a); }}
                aria-label={`Edit ${a.name}`}
                style={{ display: "flex", alignItems: "center", gap: 12, padding: "12px 0", borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
              >
                <span style={{ width: 28, height: 28, borderRadius: 7, background: "var(--surface-2)", display: "flex", alignItems: "center", justifyContent: "center", fontSize: 13, textTransform: "uppercase", flexShrink: 0 }}>
                  {a.assetType.charAt(0)}
                </span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 14 }}>{a.name}</div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {a.assetType} · updated {new Date(a.updatedAt).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                  </div>
                </div>
                <span className="money" style={{ fontFamily: "var(--mono)", fontSize: 14 }}>{formatMoney(a.valueCents)}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      <AssetDrawer open={assetAddOpen} onClose={() => setAssetAddOpen(false)} />
      <AssetDrawer open={editAsset !== null} onClose={() => setEditAsset(null)} asset={editAsset ?? undefined} />
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cd ui && npx vitest run src/screens/Accounts.test.tsx`
Expected: PASS — "Manual assets" and "House" both render.

- [ ] **Step 7: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add ui/src/api/hooks/assets.ts ui/src/components/AssetDrawer.tsx ui/src/screens/Accounts.tsx ui/src/screens/Accounts.test.tsx
git commit -m "feat(ui): manual assets section on Accounts (§4a)"
```

---

## Task 3: §4b — Liabilities hooks, LiabilityDrawer, and Accounts section

**Files:**
- Modify: `ui/src/api/hooks/assets.ts` (add liability hooks)
- Create: `ui/src/components/LiabilityDrawer.tsx`
- Modify: `ui/src/screens/Accounts.tsx`
- Test: `ui/src/screens/Accounts.test.tsx` (add a case)

- [ ] **Step 1: Write the failing test**

Add a liability to the existing `useLiabilities` mock and a new test case in `ui/src/screens/Accounts.test.tsx`. Change the `useLiabilities` mock line to:

```tsx
  useLiabilities: vi.fn(() => ({ data: [
    { id: "l1", name: "Mortgage", liabilityType: "mortgage", balanceCents: 30000000, limitCents: 35000000, aprPct: 5.5, payoffDate: "2045-01-01", currency: "USD", createdAt: "2026-06-01T00:00:00Z", updatedAt: "2026-06-01T00:00:00Z" },
  ] })),
```

Add this test case inside the `describe` block:

```tsx
  it("renders the liabilities section with a liability row", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Liabilities")).toBeInTheDocument();
    expect(screen.getByText("Mortgage")).toBeInTheDocument();
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/screens/Accounts.test.tsx`
Expected: FAIL — no "Liabilities" / "Mortgage" rendered yet.

- [ ] **Step 3: Add liability hooks to `assets.ts`**

Append to `ui/src/api/hooks/assets.ts`. First extend the import:

```ts
import {
  commands,
  type ManualAsset, type NewManualAsset, type ManualAssetPatch,
  type Liability, type NewLiability, type LiabilityPatch,
} from "../client";
```

Then add:

```ts
export function useLiabilities() {
  return useQuery<Liability[]>({
    queryKey: ["liabilities"],
    queryFn: async () => {
      const result = await commands.listLiabilities();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateLiability() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewLiability) => {
      const result = await commands.createLiability(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["liabilities"] });
      qc.invalidateQueries({ queryKey: ["net-worth"] });
    },
  });
}

export function useUpdateLiability() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, patch }: { id: string; patch: LiabilityPatch }) => {
      const result = await commands.updateLiability(id, patch);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["liabilities"] });
      qc.invalidateQueries({ queryKey: ["net-worth"] });
    },
  });
}

export function useDeleteLiability() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.deleteLiability(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["liabilities"] });
      qc.invalidateQueries({ queryKey: ["net-worth"] });
    },
  });
}
```

- [ ] **Step 4: Create `LiabilityDrawer`**

Create `ui/src/components/LiabilityDrawer.tsx`:

```tsx
import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import {
  useCreateLiability, useUpdateLiability, useDeleteLiability,
} from "../api/hooks/assets";
import type { Liability } from "../api/bindings";

const LIABILITY_TYPES = ["mortgage", "loan", "credit-card", "other"] as const;

const schema = z.object({
  name: z.string().min(1, "Required"),
  liabilityType: z.enum(LIABILITY_TYPES),
  balance_dollars: z.coerce.number().nonnegative("Must be ≥ 0"),
  limit_dollars: z.coerce.number().nonnegative().optional(),
  apr_pct: z.coerce.number().nonnegative().optional(),
  payoff_date: z.string().optional(),
});
type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  liability?: Liability;
}

export default function LiabilityDrawer({ open, onClose, liability }: Props) {
  const isEdit = !!liability;
  const create = useCreateLiability();
  const update = useUpdateLiability();
  const del = useDeleteLiability();
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", liabilityType: "loan", balance_dollars: 0 },
  });

  useEffect(() => {
    if (liability) {
      reset({
        name: liability.name,
        liabilityType: liability.liabilityType as typeof LIABILITY_TYPES[number],
        balance_dollars: liability.balanceCents / 100,
        limit_dollars: liability.limitCents != null ? liability.limitCents / 100 : undefined,
        apr_pct: liability.aprPct ?? undefined,
        payoff_date: liability.payoffDate ?? undefined,
      });
    } else {
      reset({ name: "", liabilityType: "loan", balance_dollars: 0 });
    }
    setDeleteConfirm(false);
  }, [liability?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  async function onSubmit(values: FormValues) {
    try {
      const balanceCents = Math.round(values.balance_dollars * 100);
      const limitCents = values.limit_dollars != null && !Number.isNaN(values.limit_dollars)
        ? Math.round(values.limit_dollars * 100) : null;
      const aprPct = values.apr_pct != null && !Number.isNaN(values.apr_pct) ? values.apr_pct : null;
      const payoffDate = values.payoff_date || null;
      if (isEdit && liability) {
        await update.mutateAsync({
          id: liability.id,
          patch: {
            name: values.name, liabilityType: values.liabilityType, balanceCents,
            limitCents, aprPct, payoffDate, currency: null,
          },
        });
      } else {
        await create.mutateAsync({
          name: values.name, liabilityType: values.liabilityType, balanceCents,
          limitCents, aprPct, payoffDate, currency: "USD",
        });
      }
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not save liability");
    }
  }

  async function handleDelete() {
    if (!deleteConfirm) { setDeleteConfirm(true); return; }
    if (!liability) return;
    try { await del.mutateAsync(liability.id); onClose(); }
    catch { setDeleteConfirm(false); }
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit liability" : "Add liability"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Name
          <input {...register("name")} placeholder="e.g. Mortgage" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        <label> Type
          <select {...register("liabilityType")}>
            {LIABILITY_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </label>
        <label> Balance ($)
          <input type="number" step="0.01" {...register("balance_dollars")} aria-invalid={!!errors.balance_dollars} />
          {errors.balance_dollars && <span className="err">{errors.balance_dollars.message}</span>}
        </label>
        <label> Credit limit / original ($) <input type="number" step="0.01" {...register("limit_dollars")} /></label>
        <label> APR (%) <input type="number" step="0.01" {...register("apr_pct")} /></label>
        <label> Payoff date <input type="date" {...register("payoff_date")} /></label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : (isEdit ? "Save changes" : "Add liability")}
          </button>
        </div>
      </form>
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button type="button" className="danger" onClick={handleDelete} disabled={del.isPending}>
            {deleteConfirm ? "Confirm delete?" : "Delete liability"}
          </button>
          {deleteConfirm && (
            <button type="button" onClick={() => setDeleteConfirm(false)} style={{ marginLeft: 8 }}>Cancel</button>
          )}
        </div>
      )}
    </Drawer>
  );
}
```

- [ ] **Step 5: Add the Liabilities section to `Accounts.tsx`**

Add imports:

```tsx
import { useLiabilities } from "../api/hooks/assets";
import LiabilityDrawer from "../components/LiabilityDrawer";
import type { Liability } from "../api/client";
```

Add state/data near the other hooks:

```tsx
  const { data: liabilities = [] } = useLiabilities();
  const [liabAddOpen, setLiabAddOpen] = useState(false);
  const [editLiab, setEditLiab] = useState<Liability | null>(null);
```

Add this JSX just after the Manual assets `</section>`:

```tsx
      <section style={{ marginTop: 40 }}>
        <header style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 12 }}>
          <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Liabilities</h2>
          <button onClick={() => setLiabAddOpen(true)}>+ Add liability</button>
        </header>
        {liabilities.length === 0 ? (
          <div className="stub">No liabilities yet.</div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column" }}>
            {liabilities.map((l) => {
              const pct = l.limitCents && l.limitCents > 0
                ? Math.min(100, (l.balanceCents / l.limitCents) * 100) : null;
              return (
                <div
                  key={l.id}
                  role="button"
                  tabIndex={0}
                  onClick={() => setEditLiab(l)}
                  onKeyDown={(e) => { if (e.key === "Enter") setEditLiab(l); }}
                  aria-label={`Edit ${l.name}`}
                  style={{ padding: "12px 0", borderTop: "1px solid var(--hairline)", cursor: "pointer" }}
                >
                  <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: 14 }}>{l.name}</div>
                      <div className="muted" style={{ fontSize: 12 }}>
                        <span className="chip" style={{ marginRight: 8 }}>{l.liabilityType}</span>
                        {l.aprPct != null && <>{l.aprPct}% APR</>}
                        {l.payoffDate && <> · payoff {new Date(l.payoffDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}</>}
                      </div>
                    </div>
                    <span className="money" style={{ fontFamily: "var(--mono)", fontSize: 14, color: "var(--negative)" }}>
                      {formatMoney(l.balanceCents)}
                    </span>
                  </div>
                  {pct != null && (
                    <div style={{ height: 4, background: "var(--surface-2)", borderRadius: 999, marginTop: 8 }}>
                      <div style={{ width: `${pct}%`, height: "100%", background: "var(--negative)", borderRadius: 999 }} />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </section>

      <LiabilityDrawer open={liabAddOpen} onClose={() => setLiabAddOpen(false)} />
      <LiabilityDrawer open={editLiab !== null} onClose={() => setEditLiab(null)} liability={editLiab ?? undefined} />
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cd ui && npx vitest run src/screens/Accounts.test.tsx`
Expected: PASS — both the assets and liabilities cases.

- [ ] **Step 7: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add ui/src/api/hooks/assets.ts ui/src/components/LiabilityDrawer.tsx ui/src/screens/Accounts.tsx ui/src/screens/Accounts.test.tsx
git commit -m "feat(ui): liabilities section on Accounts (§4b)"
```

---

## Task 4: Net-worth hooks + consistent headline on Accounts and Today

**Files:**
- Create: `ui/src/api/hooks/networth.ts`
- Modify: `ui/src/screens/Accounts.tsx` (net-worth header)
- Modify: `ui/src/screens/Today.tsx` (hero uses `useNetWorth`)
- Test: `ui/src/screens/Accounts.test.tsx` (add a case)

> Order note: `useNetWorth` derives from the manual-asset and liability list queries created in Tasks 2–3 plus the existing `useAccounts`. Those all exist now, so this hook compiles.

- [ ] **Step 1: Write the failing test**

Add to `ui/src/screens/Accounts.test.tsx`. First, make the `useAccounts` mock return one account so the header math is non-trivial:

```tsx
vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [
    { id: "acc1", name: "Checking", bank: "Bank", type: "Checking", balance_cents: 10000000, currency: "USD", color: "#3B82F6" },
  ], isLoading: false, error: null })),
}));
```

Add this test case (with the asset 50,000,000 and liability 30,000,000 already mocked: 10,000,000 + 50,000,000 − 30,000,000 = 30,000,000 → "$300,000.00"):

```tsx
  it("shows a net-worth header of accounts + assets − liabilities", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Net worth")).toBeInTheDocument();
    expect(screen.getByText("$300,000.00")).toBeInTheDocument();
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/screens/Accounts.test.tsx`
Expected: FAIL — no "Net worth" header yet.

- [ ] **Step 3: Create `networth.ts`**

Create `ui/src/api/hooks/networth.ts`:

```ts
import { useQuery } from "@tanstack/react-query";
import { commands, type NetWorthPoint } from "../client";
import { useManualAssets, useLiabilities } from "./assets";
import { useAccounts } from "./accounts";

/** Net-worth snapshot history for the §3a chart. */
export function useNetWorthHistory(days: number) {
  return useQuery<NetWorthPoint[]>({
    queryKey: ["networth-history", days],
    queryFn: async () => {
      const result = await commands.listNetWorthHistory(days);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

/** Live net worth = accounts + manual assets − liabilities. */
export function useNetWorth(): number {
  const { data: accounts = [] } = useAccounts();
  const { data: assets = [] } = useManualAssets();
  const { data: liabilities = [] } = useLiabilities();
  const accountCents = accounts.reduce((s, a) => s + a.balance_cents, 0);
  const assetCents = assets.reduce((s, a) => s + a.valueCents, 0);
  const liabilityCents = liabilities.reduce((s, l) => s + l.balanceCents, 0);
  return accountCents + assetCents - liabilityCents;
}
```

- [ ] **Step 4: Add the net-worth header to `Accounts.tsx`**

Add the import:

```tsx
import { useNetWorth } from "../api/hooks/networth";
```

Inside the component, after the existing hooks:

```tsx
  const netWorth = useNetWorth();
```

Replace the existing `<header className="screen-header" ...>` block so the title row sits above a net-worth figure. Change the header to:

```tsx
      <header className="screen-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 24 }}>
        <div>
          <div className="eyebrow" style={{ marginBottom: 6 }}>Net worth</div>
          <div className="figure money" style={{ fontSize: 40, lineHeight: 1, color: netWorth >= 0 ? "var(--ink)" : "var(--negative)" }}>
            {formatMoney(netWorth)}
          </div>
          <h1 style={{ fontSize: 20, fontWeight: 600, margin: "12px 0 0" }}>Accounts</h1>
        </div>
        <button className="primary" onClick={() => setAddOpen(true)}>+ Add account</button>
      </header>
```

- [ ] **Step 5: Point the Today hero at `useNetWorth`**

In `ui/src/screens/Today.tsx`: add the import and replace the local net-worth calculation so all surfaces agree. Add:

```tsx
import { useNetWorth } from "../api/hooks/networth";
```

Replace line 71 (`const netWorth = accounts.reduce((s, a) => s + a.balance_cents, 0);`) with:

```tsx
  const netWorth = useNetWorth();
```

Update the hero subtitle (lines 101-103) to describe net worth more accurately:

```tsx
          <div className="muted" style={{ fontSize: 16 }}>
            net worth · {accounts.length} account{accounts.length !== 1 ? "s" : ""} + assets − liabilities
          </div>
```

- [ ] **Step 6: Run the test + the existing Today test**

Run: `cd ui && npx vitest run src/screens/Accounts.test.tsx src/test/Today.test.tsx`
Expected: PASS. (If `src/test/Today.test.tsx` mocks `../api/hooks/accounts` but not the new `../api/hooks/networth`, add a `vi.mock("../api/hooks/networth", () => ({ useNetWorth: () => 0, useNetWorthHistory: () => ({ data: [] }) }))` to that test file and keep its existing assertions green.)

- [ ] **Step 7: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add ui/src/api/hooks/networth.ts ui/src/screens/Accounts.tsx ui/src/screens/Today.tsx ui/src/screens/Accounts.test.tsx ui/src/test/Today.test.tsx
git commit -m "feat(ui): consistent net-worth headline on Accounts and Today"
```

---

## Task 5: §3a — Net-worth area chart on Today

**Files:**
- Create: `ui/src/components/NetWorthChart.tsx`
- Modify: `ui/src/screens/Today.tsx`
- Test: `ui/src/components/NetWorthChart.test.tsx` (create)

- [ ] **Step 1: Write the failing test**

Create `ui/src/components/NetWorthChart.test.tsx`:

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import NetWorthChart from "./NetWorthChart";

const POINTS = [
  { date: "2026-01-15", totalCents: 100000 },
  { date: "2026-02-15", totalCents: 150000 },
  { date: "2026-03-15", totalCents: 140000 },
];

describe("NetWorthChart", () => {
  it("renders an SVG path when there are ≥2 points", () => {
    const { container } = render(<NetWorthChart points={POINTS} />);
    expect(container.querySelector("path")).toBeTruthy();
  });

  it("shows a building-history stub with fewer than 2 points", () => {
    render(<NetWorthChart points={[{ date: "2026-03-15", totalCents: 140000 }]} />);
    expect(screen.getByText(/still building/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/components/NetWorthChart.test.tsx`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Create `NetWorthChart`**

Create `ui/src/components/NetWorthChart.tsx` (area chart mirroring `Reports.tsx`'s `NetLine` style with a gradient fill):

```tsx
import { useId } from "react";
import type { NetWorthPoint } from "../api/client";

function fmt(cents: number) {
  return new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(cents / 100);
}

export default function NetWorthChart({ points }: { points: NetWorthPoint[] }) {
  const gradId = useId();

  if (points.length < 2) {
    return <div className="stub">Net worth history is still building. Check back after a few days.</div>;
  }

  const values = points.map((p) => p.totalCents);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const stepX = 100 / (points.length - 1);

  // y maps value→[34 (bottom) .. 4 (top)] within a 0..40 viewBox.
  const yOf = (v: number) => 34 - ((v - min) / range) * 30;

  const linePts = points.map((p, i) => ({ x: i * stepX, y: yOf(p.totalCents) }));
  const lineD = linePts.map((pt, i) => `${i === 0 ? "M" : "L"}${pt.x.toFixed(1)},${pt.y.toFixed(1)}`).join(" ");
  const areaD = `${lineD} L100,40 L0,40 Z`;
  const last = linePts[linePts.length - 1]!;
  const lastVal = values[values.length - 1]!;

  return (
    <div style={{ background: "var(--surface)", border: "1px solid var(--line)", borderRadius: "var(--radius-lg)", padding: "20px 4px 12px" }}>
      <div style={{ padding: "0 18px 12px" }}>
        <div className="eyebrow">Net worth</div>
        <div className="figure money num" style={{ fontSize: 24, marginTop: 4 }}>{fmt(lastVal)}</div>
      </div>
      <svg viewBox="0 0 100 40" preserveAspectRatio="none" style={{ width: "100%", height: 140, display: "block" }}>
        <defs>
          <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.34" />
            <stop offset="60%" stopColor="var(--accent)" stopOpacity="0.06" />
            <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
          </linearGradient>
        </defs>
        <path d={areaD} fill={`url(#${gradId})`} stroke="none" />
        <path d={lineD} fill="none" stroke="var(--accent)" strokeWidth="1.2" />
        <circle cx={last.x.toFixed(1)} cy={last.y.toFixed(1)} r="1.6" fill="var(--accent)" />
      </svg>
      <div style={{ display: "flex", padding: "4px 4px 0", justifyContent: "space-between" }}>
        {points.map((p, i) => (
          (i === 0 || i === points.length - 1 || i === Math.floor(points.length / 2)) ? (
            <span key={p.date} style={{ fontSize: 11, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
              {new Date(p.date).toLocaleDateString("en-US", { month: "short" })}
            </span>
          ) : null
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd ui && npx vitest run src/components/NetWorthChart.test.tsx`
Expected: PASS — both cases.

- [ ] **Step 5: Wire the chart into Today with a range toolbar**

In `ui/src/screens/Today.tsx`: add imports and render the chart with a range toolbar above the stat row. Add:

```tsx
import { useState } from "react";
import { useNetWorthHistory } from "../api/hooks/networth";
import NetWorthChart from "../components/NetWorthChart";
```

Add range state and data inside the component (after the existing hooks):

```tsx
  const RANGES = [
    { key: "1M", days: 30 }, { key: "3M", days: 90 }, { key: "6M", days: 180 },
    { key: "1Y", days: 365 }, { key: "All", days: 36500 },
  ] as const;
  const [range, setRange] = useState<typeof RANGES[number]["key"]>("6M");
  const days = RANGES.find((r) => r.key === range)!.days;
  const { data: nwHistory = [] } = useNetWorthHistory(days);
```

Render this block immediately after the closing `</div>` of the "Date header" block (i.e., right before the `{/* 4-stat row */}` comment):

```tsx
      {/* Net-worth chart */}
      <div style={{ marginBottom: 20 }}>
        <div className="toolbar" style={{ marginBottom: 10, display: "inline-flex" }}>
          {RANGES.map((r) => (
            <button key={r.key} className={range === r.key ? "on" : ""} onClick={() => setRange(r.key)}>
              {r.key}
            </button>
          ))}
        </div>
        <NetWorthChart points={nwHistory} />
      </div>
```

- [ ] **Step 6: Run the Today test (and fix the new hook mock if needed)**

Run: `cd ui && npx vitest run src/test/Today.test.tsx`
Expected: PASS. The `vi.mock("../api/hooks/networth", ...)` added in Task 4 must now also provide `useNetWorthHistory: () => ({ data: [] })` (it already does per Task 4 Step 6). With `data: []`, the chart renders the "still building" stub — fine for the existing assertions.

- [ ] **Step 7: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add ui/src/components/NetWorthChart.tsx ui/src/components/NetWorthChart.test.tsx ui/src/screens/Today.tsx
git commit -m "feat(ui): net-worth area chart with range selector on Today (§3a)"
```

---

## Task 6: §5d — Reimbursable / split flags

**Files:**
- Modify: `ui/src/api/hooks/transactions.ts` (add `useSetTransactionFlags`)
- Modify: `ui/src/components/TransactionDrawer.tsx` (toggles)
- Modify: `ui/src/screens/Transactions.tsx` (chips)
- Test: `ui/src/components/TransactionDrawer.test.tsx` (add a case)

> Casing reminder: `Transaction` is **snake_case** — use `transaction.is_reimbursable`, `transaction.is_split`, `t.merchant_raw`.

- [ ] **Step 1: Write the failing test**

Add a case to `ui/src/components/TransactionDrawer.test.tsx`. Inspect the existing file's mocks first; it already mocks `../api/hooks/transactions`. Add `useSetTransactionFlags: vi.fn(() => ({ mutateAsync: setFlags, isPending: false }))` to that mock (declare `const setFlags = vi.fn()` at top), and pass a `transaction` prop with `is_reimbursable: false, is_split: false`. Then:

```tsx
  it("toggles the reimbursable flag", async () => {
    // render TransactionDrawer with open + a transaction whose is_reimbursable=false
    fireEvent.click(screen.getByRole("button", { name: /reimbursable/i }));
    await waitFor(() => expect(setFlags).toHaveBeenCalled());
  });
```

(Match the existing test file's render setup for `TransactionDrawer`; it already constructs a `transaction` object — extend that object with `is_reimbursable: false, is_split: false`.)

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/components/TransactionDrawer.test.tsx`
Expected: FAIL — no "Reimbursable" button, `useSetTransactionFlags` undefined.

- [ ] **Step 3: Add the hook**

Append to `ui/src/api/hooks/transactions.ts`:

```ts
export function useSetTransactionFlags() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, isReimbursable, isSplit }: { id: string; isReimbursable: boolean; isSplit: boolean }) => {
      const result = await commands.setTransactionFlags(id, isReimbursable, isSplit);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
      qc.invalidateQueries({ queryKey: ["needs-review-count"] });
    },
  });
}
```

- [ ] **Step 4: Add toggles to `TransactionDrawer`**

In `ui/src/components/TransactionDrawer.tsx`: import the hook and render two toggle buttons (edit mode only). Add to the imports from `../api/hooks/transactions`:

```tsx
import { useSetTransactionFlags } from "../api/hooks/transactions";
```

Inside the component:

```tsx
  const setFlags = useSetTransactionFlags();
```

Render this block inside the `{isEdit && (...)}` footer area (above or below the delete button), reading current values from `transaction`:

```tsx
      {isEdit && transaction && (
        <div style={{ marginTop: 16, display: "flex", gap: 8 }}>
          <button
            type="button"
            className={`chip${transaction.is_reimbursable ? " accent" : ""}`}
            aria-pressed={transaction.is_reimbursable}
            onClick={() => setFlags.mutateAsync({ id: transaction.id, isReimbursable: !transaction.is_reimbursable, isSplit: transaction.is_split })}
          >
            Reimbursable
          </button>
          <button
            type="button"
            className={`chip${transaction.is_split ? " accent" : ""}`}
            aria-pressed={transaction.is_split}
            onClick={() => setFlags.mutateAsync({ id: transaction.id, isReimbursable: transaction.is_reimbursable, isSplit: !transaction.is_split })}
          >
            Split
          </button>
        </div>
      )}
```

- [ ] **Step 5: Add chips to the transactions table**

In `ui/src/screens/Transactions.tsx`: in the row that renders each transaction's merchant cell, append flag chips. Locate where `t.merchant_raw` is rendered and add, next to it:

```tsx
                  {t.is_reimbursable && <span className="chip" style={{ marginLeft: 6, fontSize: 10 }}>Reimbursable</span>}
                  {t.is_split && <span className="chip" style={{ marginLeft: 6, fontSize: 10 }}>Split</span>}
```

(If the merchant cell currently renders just `{t.merchant_raw}`, wrap it: `<>{t.merchant_raw}{...chips}</>`.)

- [ ] **Step 6: Run the test to verify it passes**

Run: `cd ui && npx vitest run src/components/TransactionDrawer.test.tsx`
Expected: PASS.

- [ ] **Step 7: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add ui/src/api/hooks/transactions.ts ui/src/components/TransactionDrawer.tsx ui/src/components/TransactionDrawer.test.tsx ui/src/screens/Transactions.tsx
git commit -m "feat(ui): reimbursable/split flags in TransactionDrawer and table (§5d)"
```

---

## Task 7: §11a — Agent proposals card on Rules

**Files:**
- Create: `ui/src/api/hooks/proposals.ts`
- Modify: `ui/src/screens/Rules.tsx`
- Test: `ui/src/screens/Rules.test.tsx` (create)

- [ ] **Step 1: Write the failing test**

Create `ui/src/screens/Rules.test.tsx`:

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Rules from "./Rules";
import { createWrapper } from "../test-utils";

const accept = vi.fn();

vi.mock("../api/hooks/transactions", () => ({
  useRulesWithCategories: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useToggleRule: vi.fn(() => ({ mutateAsync: vi.fn() })),
}));

vi.mock("../api/hooks/proposals", () => ({
  useRuleProposals: vi.fn(() => ({ data: [
    { id: "p1", whenLabel: "3 corrections for Whole Foods", description: "Always categorize Whole Foods as Groceries", pattern: "%whole foods%", categoryId: "groceries", status: "pending", createdAt: "2026-06-01T00:00:00Z" },
  ] })),
  useAcceptRuleProposal: vi.fn(() => ({ mutateAsync: accept, isPending: false })),
  useDeclineRuleProposal: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("Rules — agent proposals", () => {
  it("renders proposals and accepts one", async () => {
    render(<Rules />, { wrapper: createWrapper() });
    expect(screen.getByText("Agent proposals")).toBeInTheDocument();
    expect(screen.getByText("Always categorize Whole Foods as Groceries")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /accept/i }));
    await waitFor(() => expect(accept).toHaveBeenCalledWith("p1"));
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/screens/Rules.test.tsx`
Expected: FAIL — no "Agent proposals" card; `../api/hooks/proposals` module missing.

- [ ] **Step 3: Create `proposals.ts`**

Create `ui/src/api/hooks/proposals.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type RuleProposal } from "../client";

export function useRuleProposals() {
  return useQuery<RuleProposal[]>({
    queryKey: ["rule-proposals"],
    queryFn: async () => {
      const result = await commands.listRuleProposals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useAcceptRuleProposal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.acceptRuleProposal(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rule-proposals"] });
      qc.invalidateQueries({ queryKey: ["rules"] });
    },
  });
}

export function useDeclineRuleProposal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.declineRuleProposal(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["rule-proposals"] });
    },
  });
}
```

- [ ] **Step 4: Render the proposals card in `Rules.tsx`**

Add imports:

```tsx
import { toast } from "sonner"; // already imported — keep single import
import { useRuleProposals, useAcceptRuleProposal, useDeclineRuleProposal } from "../api/hooks/proposals";
```

Inside `Rules()`, after the existing `useRulesWithCategories` line:

```tsx
  const { data: proposals = [] } = useRuleProposals();
  const acceptProposal = useAcceptRuleProposal();
  const declineProposal = useDeclineRuleProposal();
```

Render this card at the bottom of the rules-list column (inside the `<div>` that holds the rules list, after the `{rules.length === 0 ? ... : ...}` block):

```tsx
          {proposals.length > 0 && (
            <div className="card" style={{ marginTop: 28, border: "1px dashed var(--accent)" }}>
              <div className="eyebrow" style={{ marginBottom: 12, color: "var(--accent)" }}>
                <I.Sparkle width="12" height="12" style={{ marginRight: 6 }} />
                Agent proposals · {proposals.length}
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                {proposals.map((p) => (
                  <div key={p.id} style={{ display: "flex", alignItems: "center", gap: 12 }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div className="eyebrow" style={{ marginBottom: 2 }}>{p.whenLabel}</div>
                      <div style={{ fontSize: 14 }}>{p.description}</div>
                    </div>
                    <button
                      className="btn primary"
                      onClick={async () => {
                        try { await acceptProposal.mutateAsync(p.id); toast.success("Rule created"); }
                        catch { toast.error("Could not accept proposal"); }
                      }}
                    >
                      Accept
                    </button>
                    <button
                      className="btn ghost sm"
                      onClick={async () => {
                        try { await declineProposal.mutateAsync(p.id); }
                        catch { toast.error("Could not decline proposal"); }
                      }}
                    >
                      Decline
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd ui && npx vitest run src/screens/Rules.test.tsx`
Expected: PASS.

- [ ] **Step 6: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add ui/src/api/hooks/proposals.ts ui/src/screens/Rules.tsx ui/src/screens/Rules.test.tsx
git commit -m "feat(ui): agent proposals card on Rules (§11a)"
```

---

## Task 8: §13b — Agent memory section on Insights (with deferred-undo forget)

**Files:**
- Create: `ui/src/api/hooks/agentMemory.ts`
- Modify: `ui/src/screens/Insights.tsx`
- Test: `ui/src/screens/Insights.memory.test.tsx` (create)

> The forget action must *actually delete* after a delay, unlike the existing in-memory insight dismiss. The logic below handles per-id timers, optimistic hiding, undo cancellation, and unmount cleanup.

- [ ] **Step 1: Write the failing test**

Create `ui/src/screens/Insights.memory.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import Insights from "./Insights";
import { createWrapper } from "../test-utils";

const forget = vi.fn(() => Promise.resolve());

// Neutralize the data hooks the insight cards rely on.
vi.mock("../api/hooks/accounts", () => ({ useAccounts: () => ({ data: [] }) }));
vi.mock("../api/hooks/budget", () => ({ useBudgetEnvelopes: () => ({ data: [] }), useGoals: () => ({ data: [] }) }));
vi.mock("../api/hooks/transactions", () => ({ useCategoriesWithSpending: () => ({ data: [] }) }));
vi.mock("../api/client", () => ({ commands: {
  getMonthTotals: vi.fn().mockResolvedValue({ status: "ok", data: { incomeCents: 0, expenseCents: 0, netCents: 0, savingsRatePct: 0, txnCount: 0 } }),
  listRecurring: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
} }));

vi.mock("../api/hooks/agentMemory", () => ({
  useAgentMemory: vi.fn(() => ({ data: [
    { id: "m1", kind: "correction", description: "Learned: Trader Joe's is Groceries", merchantKey: "trader joes", createdAt: "2026-06-01T00:00:00Z" },
  ] })),
  useForgetAgentMemory: vi.fn(() => ({ mutateAsync: forget })),
}));

describe("Insights — agent memory", () => {
  beforeEach(() => { vi.useFakeTimers(); forget.mockClear(); });
  afterEach(() => { vi.useRealTimers(); });

  it("forget hides the row and deletes after the delay", () => {
    render(<Insights />, { wrapper: createWrapper() });
    expect(screen.getByText("Learned: Trader Joe's is Groceries")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /forget/i }));
    expect(screen.queryByText("Learned: Trader Joe's is Groceries")).not.toBeInTheDocument();
    expect(forget).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(5000); });
    expect(forget).toHaveBeenCalledWith("m1");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd ui && npx vitest run src/screens/Insights.memory.test.tsx`
Expected: FAIL — no memory row / "Forget" button; `../api/hooks/agentMemory` missing.

- [ ] **Step 3: Create `agentMemory.ts`**

Create `ui/src/api/hooks/agentMemory.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AgentMemory } from "../client";

export function useAgentMemory() {
  return useQuery<AgentMemory[]>({
    queryKey: ["agent-memory"],
    queryFn: async () => {
      const result = await commands.listAgentMemory();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useForgetAgentMemory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.forgetAgentMemory(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}
```

- [ ] **Step 4: Add the agent-memory section + deferred-undo logic to `Insights.tsx`**

Add imports:

```tsx
import { useRef, useEffect } from "react";
import { useAgentMemory, useForgetAgentMemory } from "../api/hooks/agentMemory";
```

Inside `Insights()`, add this state/logic (near the existing `dismissed` state):

```tsx
  const { data: memory = [] } = useAgentMemory();
  const forgetMemory = useForgetAgentMemory();
  const [pendingForget, setPendingForget] = useState<Set<string>>(new Set());
  const forgetTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  // Clear any in-flight timers on unmount (don't fire them).
  useEffect(() => {
    const timers = forgetTimers.current;
    return () => { timers.forEach((t) => clearTimeout(t)); timers.clear(); };
  }, []);

  const handleForget = (m: { id: string; description: string }) => {
    setPendingForget((s) => new Set([...s, m.id]));
    const timer = setTimeout(async () => {
      forgetTimers.current.delete(m.id);
      try { await forgetMemory.mutateAsync(m.id); }
      catch {
        setPendingForget((s) => { const n = new Set(s); n.delete(m.id); return n; });
        toast.error("Could not forget that memory");
      }
    }, 5000);
    forgetTimers.current.set(m.id, timer);
    toast("Memory forgotten", {
      description: m.description.slice(0, 60),
      action: {
        label: "Undo",
        onClick: () => {
          const t = forgetTimers.current.get(m.id);
          if (t) { clearTimeout(t); forgetTimers.current.delete(m.id); }
          setPendingForget((s) => { const n = new Set(s); n.delete(m.id); return n; });
        },
      },
    });
  };

  const visibleMemory = memory.filter((m) => !pendingForget.has(m.id));
```

Render this section just before the final `</div>` that closes `<div className="screen">` (after the insight cards block):

```tsx
      {visibleMemory.length > 0 && (
        <div style={{ marginTop: 40 }}>
          <div className="eyebrow" style={{ marginBottom: 12 }}>What the agent has learned</div>
          <div style={{ display: "flex", flexDirection: "column" }}>
            {visibleMemory.map((m) => (
              <div key={m.id} style={{ display: "flex", alignItems: "center", gap: 12, padding: "10px 0", borderTop: "1px solid var(--hairline)" }}>
                <div style={{ flex: 1, minWidth: 0, fontSize: 14 }}>{m.description}</div>
                <button className="btn ghost sm" onClick={() => handleForget(m)} aria-label={`Forget: ${m.description}`}>
                  Forget
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd ui && npx vitest run src/screens/Insights.memory.test.tsx`
Expected: PASS — row hides immediately, `forget` fires only after `advanceTimersByTime(5000)`.

- [ ] **Step 6: Add an undo-cancels test**

Add a second case to `ui/src/screens/Insights.memory.test.tsx`:

```tsx
  it("undo cancels the deferred delete", () => {
    render(<Insights />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /forget/i }));
    // sonner renders the Undo action button.
    fireEvent.click(screen.getByRole("button", { name: /^undo$/i }));
    act(() => { vi.advanceTimersByTime(5000); });
    expect(forget).not.toHaveBeenCalled();
  });
```

Run: `cd ui && npx vitest run src/screens/Insights.memory.test.tsx`
Expected: PASS. (If sonner's toast action button is not found in jsdom, assert via re-querying the restored row text instead: after Undo + advanceTimers, `screen.getByText("Learned: Trader Joe's is Groceries")` is present and `forget` was not called.)

- [ ] **Step 7: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add ui/src/api/hooks/agentMemory.ts ui/src/screens/Insights.tsx ui/src/screens/Insights.memory.test.tsx
git commit -m "feat(ui): agent memory section with deferred-undo forget (§13b)"
```

---

## Task 9: Full-suite verification

**Files:** none (verification only)

- [ ] **Step 1: Run the entire Rust suite**

Run: `cargo test --workspace`
Expected: all tests pass (the prior green bar plus the new `record_today_folds_assets_and_liabilities`). Note: `keychain::tests::set_key_round_trip` is documented as intermittently flaky on Windows — a failure *only* there is pre-existing and unrelated.

- [ ] **Step 2: Run the entire frontend suite**

Run: `cd ui && npx vitest run`
Expected: all tests pass (prior 53 plus the new Accounts, NetWorthChart, TransactionDrawer, Rules, and Insights-memory cases).

- [ ] **Step 3: Type-check the whole frontend**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 4: Manual smoke (optional but recommended)**

Run: `pnpm tauri:dev`. Verify: Accounts shows net-worth header + assets + liabilities sections with working add/edit drawers; Today shows the net-worth chart with a working range toolbar and a headline matching Accounts; a transaction's reimbursable/split toggles persist and show chips; Rules shows any agent proposals with Accept/Decline; Insights shows agent memory with a working Forget + Undo. (Remember: the chart's last point may lag the live headline until the next app launch records a fresh snapshot — expected, not a bug.)

- [ ] **Step 5: Update the TODO checklist**

In `docs/TODO.md`, mark §3a, §4a, §4b, §5d, §11a, §13b as done (change their "🔧 backend done — UI pending" notes to ✅ and update the priority table rows). Commit:

```bash
git add docs/TODO.md
git commit -m "docs: mark Wave A items (§3a, §4a, §4b, §5d, §11a, §13b) complete"
```

---

## Self-Review

**Spec coverage:**
- §3a net-worth chart → Task 5 (+ history hook in Task 4). ✓
- §4a manual assets → Task 2. ✓
- §4b liabilities → Task 3. ✓
- Net-worth consistency decision (backend + headline) → Task 1 + Task 4. ✓
- §5d reimbursable/split flags → Task 6. ✓
- §11a agent proposals → Task 7. ✓
- §13b agent memory + deferred-undo → Task 8. ✓
- Testing (Rust snapshot test, frontend hook/screen tests, tsc) → Tasks 1–9. ✓

**Placeholder scan:** No "TBD"/"add error handling"/"similar to" placeholders; every code step shows complete code. The two test-environment fallbacks (Today mock note in Task 4/5; sonner-undo fallback in Task 8 Step 6) are explicit, not vague.

**Type consistency:** Hook query keys are consistent (`manual-assets`, `liabilities`, `networth-history`, `rule-proposals`, `agent-memory`). Mutation invalidations match their query keys. `useNetWorth` is created in Task 4 before its use in Accounts/Today; `useNetWorthHistory` (Task 4) before the chart (Task 5); asset/liability hooks (Tasks 2–3) before `useNetWorth` (Task 4) — no use-before-create. Snake_case (`is_reimbursable`/`is_split`) is confined to Task 6; camelCase elsewhere. Command names and type field names match the verified bindings list at the top.
