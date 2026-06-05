# Wave A — UI for Shipped Backends (Design)

**Date:** 2026-06-05
**Status:** Approved, pending implementation plan
**Scope:** `docs/TODO.md` items §3a, §4a, §4b, §5d, §11a, §13b

## Context

`docs/TODO.md` lists ~25 independent features. They were decomposed into three waves;
**Wave A** is the first sub-project: six features whose Rust/Tauri backends are already
merged to `main` (migrations V006–V011, repos, commands, bindings) and whose **frontend
UIs are the only remaining work** — plus one small backend edit (see Net-Worth Consistency).

The other waves are out of scope here and will get their own spec → plan → build cycles:
- **Wave B** — pure-frontend computed features (§3b, §3c, §3d, §6c, §9b, §9c, §13a, §14b)
- **Wave C** — new-backend / large features (§2, §10a–e, §14a, §11b/c, §12a/b, §7c, §8b, CSV exports)

## Goals

- Ship the six Wave A UIs, each independently mergeable.
- Make "net worth" consistent across the three surfaces that display it.
- Keep the green bar: all existing Rust tests + 53 frontend tests + 0 TS errors stay green.

## Non-Goals

- No new migrations (all tables already exist; next migration remains V012 for later work).
- No CSV export, no manual rule builder, no agent activity log (those are Wave C).
- No drag-and-drop, no new charts beyond the §3a net-worth area chart.

## Key Decision — Net-Worth Consistency

Three surfaces display net worth and currently disagree:
- **§3a chart** is fed by the `net_worth_snapshots` table, recorded by `record_today()` which
  **sums bank-account balances only**.
- **§4a/§4b** specify the Accounts header and Today hero show **accounts + manual assets − liabilities**.

**Decision (user-approved): make them consistent now.** Update the backend snapshot logic so
the recorded value matches the headline. This is a contained, migration-free repo change.

**Backend change** — `crates/finsight-core/src/repos/net_worth.rs`, `record_today()`:

```rust
pub fn record_today(conn: &mut Connection) -> CoreResult<()> {
    let accounts: i64 = accounts::list_summaries(conn)?.iter().map(|a| a.balance_cents).sum();
    let assets: i64 = manual_assets::list(conn)?.iter().map(|a| a.value_cents).sum();
    let liabilities: i64 = liabilities::list(conn)?.iter().map(|l| l.balance_cents).sum();
    record_snapshot(conn, accounts + assets - liabilities)
}
```

`accounts::list_summaries`, `manual_assets::list`, and `liabilities::list` all already exist.
Note: historical snapshots recorded before this change remain bank-only; the line corrects
itself forward as new daily snapshots accumulate. This is acceptable and noted in the UI plan.

## Architecture

Wave A = six frontend additions + one backend edit. The unifying thread is a single
`netWorth = accounts + manualAssets − liabilities` value consumed by three surfaces
(Today hero, Accounts header, and — via the snapshot — the §3a chart). It is centralized in
one hook, `useNetWorth()`, so no surface re-derives it ad hoc.

### Build order (each independently shippable)

1. Backend snapshot fix + `useNetWorth` hook (foundation).
2. §4a Manual assets + §4b Liabilities (shared Accounts section + asset/liability drawers).
3. §3a Net-worth chart (now consistent with the headline).
4. §5d Reimbursable/split flags.
5. §11a Agent proposals.
6. §13b Agent memory.

## Data Layer (new hook files)

Follows the existing `ui/src/api/hooks/transactions.ts` pattern: `useQuery`/`useMutation`,
unwrap via `if (result.status === "error") throw new Error(result.error.message)`, and
`qc.invalidateQueries(...)` on mutation success. Import commands/types from `../api/client`.

| File | Exports |
|------|---------|
| `hooks/networth.ts` | `useNetWorthHistory(days)` (query key `["networth-history", days]`); `useNetWorth()` — derives live `accounts + assets − liabilities` from the three list queries for the headline |
| `hooks/assets.ts` | `useManualAssets`, `useCreateManualAsset`, `useUpdateManualAsset`, `useDeleteManualAsset`, `useLiabilities`, `useCreateLiability`, `useUpdateLiability`, `useDeleteLiability` |
| `hooks/proposals.ts` | `useRuleProposals`, `useAcceptRuleProposal`, `useDeclineRuleProposal` (accept invalidates both `rule-proposals` and `rules`) |
| `hooks/agentMemory.ts` | `useAgentMemory`, `useForgetAgentMemory` |
| `hooks/transactions.ts` (edit) | add `useSetTransactionFlags` (invalidates `transactions`, today summary, needs-review count) |

### Verified backend types (from `ui/src/api/bindings.ts`)

```
ManualAsset    = { id, name, assetType, valueCents, currency, notes|null, createdAt, updatedAt }
NewManualAsset = { name, assetType, valueCents, currency, notes|null }
ManualAssetPatch = { name|null, assetType|null, valueCents|null, currency|null, notes|null }

Liability      = { id, name, liabilityType, balanceCents, limitCents|null, aprPct|null, payoffDate|null, currency, createdAt, updatedAt }
NewLiability   = { name, liabilityType, balanceCents, limitCents|null, aprPct|null, payoffDate|null, currency }
LiabilityPatch = { name|null, liabilityType|null, balanceCents|null, limitCents|null, aprPct|null, payoffDate|null, currency|null }

RuleProposal   = { id, whenLabel, description, pattern, categoryId, status, createdAt }
AgentMemory    = { id, kind, description, merchantKey|null, createdAt }
NetWorthPoint  = { date, totalCents }
```

Commands: `recordNetWorthSnapshot()`, `listNetWorthHistory(days)`,
`listManualAssets/createManualAsset/updateManualAsset/deleteManualAsset`,
`listLiabilities/createLiability/updateLiability/deleteLiability`,
`listRuleProposals/acceptRuleProposal/declineRuleProposal`,
`listAgentMemory/forgetAgentMemory`, `setTransactionFlags(id, isReimbursable, isSplit)`.

## The Six UI Pieces

### §3a — Net-worth area chart (`ui/src/screens/Today.tsx`)
- SVG area chart above the stat row, mirroring `Reports.tsx`'s `NetLine` path/circle style with
  a gradient fill (accent → transparent) per `design/plutus/project/components/today.jsx`
  `NetWorthChart`.
- Range toolbar `1M / 3M / 6M / 1Y / All`, default **6M**. Range → days: 30 / 90 / 180 / 365 /
  `36500` (All). Fed by `useNetWorthHistory(days)`.
- Last point glows (radius-14 accent circle); X-axis month labels in `var(--mono)`.
- Empty/short-history state: if `< 2` points, show a muted "Net worth history is still building" stub.

### §4a — Manual assets (`ui/src/screens/Accounts.tsx`)
- New "Manual assets" section below the bank-accounts table.
- Each row: icon by `assetType`, name, type chip, value (right, `money` class for privacy blur).
  Sub-line shows "updated {updatedAt date}" — **not** the design's `delta90d` arrow (no trend field
  on the type).
- "Add manual asset" button opens a reusable `AssetDrawer` (slide-in `Drawer.tsx`,
  react-hook-form + zod) handling create **and** edit. Row click → edit; delete from within drawer.
- `assetType` options: `cash`, `property`, `vehicle`, `investment`, `crypto`, `other`
  (icons: currency / house / car / chart / currency / chart respectively from `Icons.tsx`).

### §4b — Liabilities (`ui/src/screens/Accounts.tsx`)
- New "Liabilities" section below manual assets.
- Each row: name, type chip, balance (`var(--negative)`), APR (`aprPct`), payoff date, progress bar
  `width = balanceCents / limitCents` (omit bar when `limitCents` is null).
- `LiabilityDrawer` (create + edit), `liabilityType` options: `mortgage`, `loan`, `credit-card`, `other`.
- **Net-worth header** atop the Accounts screen: `accounts + assets − liabilities` via `useNetWorth()`.
  The Today hero number also uses `useNetWorth()` so both agree with each other and with the chart.

### §5d — Reimbursable / split flags (`ui/src/components/TransactionDrawer.tsx` + table)
- Two toggle buttons in the drawer (`.tog` style) calling `setTransactionFlags(id, isReimbursable, isSplit)`.
- Flagged rows in the transactions table show small chips: "Reimbursable" and/or "Split".

### §11a — Agent proposals (`ui/src/screens/Rules.tsx`)
- Dashed accent-border card below the rules list (per `design/plutus/project/components/rules.jsx`).
- Each proposal row: `whenLabel` eyebrow, `description` text, "Accept" (`btn.primary`) + "Decline"
  (`btn.ghost.sm`). Accept → `acceptRuleProposal` (materializes a rule); both invalidate.
- Empty state: "No proposals right now. Agent reviews as you categorize." Hide the card when the list is empty.

### §13b — Agent memory (`ui/src/screens/Insights.tsx`)
- "What the agent has learned" list below the insight cards, using the **`description`** field.
- "Forget" uses a **client-side deferred-delete undo**: the row is optimistically removed and a
  sonner toast with an "Undo" action is shown; the actual `forgetAgentMemory(id)` call fires after
  ~5s unless undo is clicked (which cancels the timer and restores the row). This delivers a real
  undo with **zero backend change** (there is no re-create command).

## Error Handling

- Hooks throw on `result.status === "error"`; tanstack-query surfaces it. Mutations show a
  `toast.error()` on failure (matching existing screens).
- Drawer forms validate with zod before submit (positive amounts, required name/type).
- Deferred-delete undo: if the deferred `forgetAgentMemory` call fails, restore the row and toast the error.

## Testing

Per CLAUDE.md's green bar (66 Rust, 53 frontend, 0 TS errors):
- **Rust:** extend `net_worth` tests to assert `record_today` folds in manual assets and liabilities
  (insert one asset + one liability, assert snapshot = accounts + assets − liabilities).
- **Frontend (vitest):** new-hook error-unwrap tests; Accounts net-worth header renders
  `accounts + assets − liabilities`; agent-proposals Accept calls the mutation and removes the row;
  agent-memory Forget shows undo and cancels the delete when undo is clicked.
- `cd ui && npx tsc --noEmit` clean.

## Open Questions

None. All field names verified against `bindings.ts`; net-worth consistency resolved above.
