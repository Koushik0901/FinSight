# Changelog

## 0.2.0 — 2026-08-30

### Mobile UX Redesign (phone-first)
- New `MobileShell`, `MobileHeader`, `MobileBottomNav`, `MobileMoreScreen` with single 768px breakpoint, 520px centered shell, `viewport-fit=cover` + `env(safe-area-inset-*)` (top/bottom) and `100dvh`.
- 14 mobile screens: Today, Transactions, Copilot, Budget, Accounts, Goals, Recurring, Categories, Rules, Scenarios, Cashflow, Reports, Insights, More — each with thumb-friendly lists, bottom sheets, segmented controls, sticky actions, 44px+ targets.
- Primitives: `BottomSheet` (FocusLock + inert + scroll lock + drag 96px), `MobileList`/`MobileListItem`, `MobileSection`, `MobilePageHeader`, `StickyActionBar`, `SegmentedControl`, `MobileFilterSheet`, `MobileStat`, `MobileEmptyState`.
- Desktop preserved via `useIsMobile` gate in `App.tsx`; no 521–900 hybrid.

### YNAB/Actual Parity (P0/P1/P2 — 15 items)
- `toBudget`/`available_funds` now `income - budgeted - hold` (Actual), `hold` parks for next month, `available_funds` adds next-month hold as income-like.
- Cross-currency `cashflow`/`forecast`/`metrics` via `primary_currency_clause` + `non_investment_txn_predicate`.
- Payee grouping, spending breakdown, custom_report all scoped to primary currency and non-investment.
- `carryover_into_month_fast` batches rollover check (1 SELECT) vs N+1.
- `GoalPeriod` integer `monthlyEquivalent` `(cents*ppy+6)/12` (no float drift).
- Budget `allowOverAssign` soft-lock, `MONTH_LOCKED` error code, `is_over_assigned` helper.

### Review Fixes (13 findings — 2 P0, 5 P1, 6 P2)
- Cashflow horizon clamp 1–1000 days restored at `get_cashflow_forecast` + `build_forecast`.
- `toBudget` guard retroactive warning for `is_over_assigned` when income==0.
- MobileBudget now `viewMonth` + `allowOverAssign` confirm + Hold UI with Save/Clear/Undo.
- Rollover N+1 batched, Payee currency/investment scoping, schedule cron/interval slash handling, forecast one-time drain cap.
- Soft-lock `MONTH_LOCKED:` prefix, weekly integer, BottomSheet FocusLock/inert, agent_memory `COALESCE` unique index (`V070`).

### Tech
- `openapi.json`/`openapi.ts` regenerated (`pnpm openapi`), `tsc -b` clean, `vite build` 3.9M, `cargo fmt` clean.
- Migrations: `V067__payee_grouping_fix`, `V068__goal_period`, `V069__category_rollover`, `V070__fix_agent_memory_unique`.
