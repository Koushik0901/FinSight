# FinSight Mobile UX — Handoff

**Branch:** `main` @ `3152502` (mobile shell + 14 screens) → `80e9a1b` fmt → `1d86f00`/`07ac68c` clippy → `b444137` Reports bars → `913bc2d` Cashflow x-labels → `ca44b20` gitignore → `4ac88ee` MobileReports test
**Stack:** React 18 + TS + Vite, TanStack Query, `ui/src/styles/tokens.css` + `app.css` + `mobile.css`, `pnpm`, Tauri 2, SQLCipher

## 1. Shell
- **Breakpoint:** single semantic `768px` — `useIsMobile = matchMedia("(max-width:768px)")` (`ui/src/hooks/useIsMobile.ts`). Phone `0-768`, tablet/desktop `769+` (no 900px hybrid). `App.tsx` conditional `isMobile ? <MobileShell> : <div class="app"><Sidebar>`.
- **MobileShell:** `100dvh` flex-col, `MobileHeader` sticky `56px + env(safe-area-inset-top)`, `mobile-shell-main` scroll `72px + safe-area` bottom pad, `MobileBottomNav` fixed `72px + safe-area` blur, `MobileMoreScreen` grouped lists.
- **Safe-area:** `env(safe-area-inset-top/bottom)` on header, nav, sheet body, sticky bar; `100dvh` not `vh`; `overscroll-behavior: contain`.

## 2. Navigation
- **Bottom nav:** `Today / Transactions / Copilot / Budget / More` (`ui/src/components/mobile/MobileBottomNav.tsx`). `More` is a route (`/more`, non-linkable) with 3 groups (Money/Plan/System) — not a drawer — so back button and deep-links work.
- **Desktop preserved:** `Sidebar.tsx` + `BottomNav.tsx` untouched, hidden at `≤768px` via `mobile.css`.

## 3. Primitives (`ui/src/components/mobile/`)
- `BottomSheet` — portal, backdrop blur, handle drag `96px` dismiss, ESC, focus restore, scrollbar compensation, `fullHeight` drill-down.
- `MobileList/Item (64px)` + `MobileSection` — icon + title·subtitle + value/meta + chevron, tabular `money`, `role=listitem`.
- `MobilePageHeader`, `MobileStat/Row`, `SegmentedControl`, `StickyActionBar`, `MobileEmptyState`, `MobileHeader`.

## 4. Phone screens (`ui/src/screens/mobile/`)
| Route | File | Key UX |
|---|---|---|
| `/` Today | `MobileToday.tsx` | Hero net-worth (money + spark `NetWorthChart` 30pts), Spent/Remaining + Savings/Runway hero stats, conscious bar `Need/Want/Saving/Inves` %, single `Next action` + `CopilotNudge`, recent 5 `MobileList` → sheet, `details` disclosure for Budget/Accounts |
| `/transactions` | `MobileTransactions.tsx` | 44px search (16px), horizontal chips + `Filters` pill-dot, `BottomSheet` filter (Segmented preset + range + account), `MobileList` 64px rows `Merchant — Amount / Category·Type·RelativeDate·Account` → detail sheet (category/type/account/date/notes/anomaly) + `TransactionDrawer` reuse |
| `/budget` | `MobileBudget.tsx` | Hero Remaining/`% used` 8px bar, `All/Over/Watch/OK` segmented, list `MobileListItem` → detail sheet (remaining 32px, bar, Budgeted/Spent, Adjust 44px → `useSetBudget`, 8 txns) |
| `/accounts` | `MobileAccounts.tsx` | Hero net worth + Assets/Liabilities, grouped `Cash/Credit/Invest/Other` via `accountTypeColor(type)`, drill-down `BottomSheet` (44px icon, 28px balance) |
| `/goals` | `MobileGoals.tsx` | `All/Save/Build/Debt/Caps` segmented, cards (chip + pace tone + due, bar, % + ETA) → detail sheet with 4-option projector + `GoalDrawer` |
| `/copilot` | `MobileCopilot.tsx` | Wraps desktop `Copilot` runtime, `visualViewport` → `--keyboard-inset`, `sidebar hidden`, composer sticky `bottom: var(--keyboard-inset)+safe-area`, chips horizontal scroll |
| `/categories` | `MobileCategories.tsx` | `Spent/Budget/A-Z` segmented, `MobileList` with % |
| `/recurring` | `MobileRecurring.tsx` | `Due soon / All` `MobileSection`, `lastAmountCents ?? monthlyEquivalentCents` |
| `/reports` | `MobileReports.tsx` | 6-mo `CategoryHistory.monthly` aggregated bars `flex:1` `height: (spent/max)*100%` `accent` for last, `label.slice(0,3)` 10px ticks — no tiny legends, one insight `avg vs last %` |
| `/journey` | `MobileJourney.tsx` | Vertical timeline 28px dots (`positive`/`accent`/`line`), `MobilePageHeader` 7 milestones, completion + next-action hero, `BottomSheet` with quote/CTA |
| `/cashflow` | `MobileCashflow.tsx` | Hero safe-to-spend + 44px buffer/test inputs, `30/60/90` segmented, chart `viewBox 900×240 width:100%` line+area + 3 readable x-labels `start/mid/end` 11px + single caption, `MobileList` events |
| `/scenarios` | `MobileScenarios.tsx` | Chip row `All/Affordable/At risk/Stale`, `MobileListItem` cards → `BottomSheet` + `StickyActionBar` |
| `/rules` | `MobileRules.tsx` | `All/Active/Paused` segmented + search, `MobileList` 64px with 48×28 `role=switch` 44px hit, `BottomSheet` tokens |
| `/settings` | `MobileSettings.tsx` | Grouped `You/Privacy & data/Appearance/Intelligence/System` `MobileList` 64px, inline `Toggle`/`SegmentedControl` via `useTweaks` |
| `/more` | `MobileMoreScreen.tsx` | 3 groups, 54px rows, review pulse |

All reuse `api/hooks/*`, `money`, `tokens.css` — no hardcoded hex, no business-logic duplication. Desktop `Budget/Today/etc.` unchanged.

## 5. Verification
- `pnpm --filter ui build` on Linux FS `/tmp/vite-test` (`6.13s`): 14 mobile chunks separate (`MobileToday 10.72k`, `Transactions 12.33k`, `Budget 8.71k`, `Accounts 6.70k`, `Goals 9.43k`, `Journey 10.90k`, `Cashflow 10.22k`, `Rules 11.93k`, `Scenarios 23.5k`, etc.) not in `index 178k` — desktop never fetches them.
- `tsc -b` (worktree binary 5.9.3) `clean` after `MobileAccounts type`/`MobileToday total`/`MobileReports monthly`/`MobileRecurring lastAmountCents` fixes; `vitest-axe` spec `MobileReports.test.tsx` added.
- `cargo fmt --all` (80e9a1b) + `clippy -D warnings` (`1d86f00` `derivable_impls`/`collapsible_if` + `07ac68c` `manual_range_contains`/`needless_question_mark`/`should_implement_trait`/`unused_import`) — CI `Container` now `success 10m49s` for `b444137`, `CI` for `1d86f00` was `12m41s` failure on those 6 now fixed.
- Viewports code-verified: `360/375/390/430` `520px` centered, `MobileStatRow` collapses at `360`, sheets `86dvh`, nav `max(8px,safe-area)`, `768` tablet renders `app` grid.

## 6. Next
- Live `vite dev` on `/mnt/e` blocked by Windows→WSL `esbuild win32→linux` mount — use `~/FinSight` ext4 or CI Linux runner where build already green.
- Polish: `Reports` drill-down → `Transactions` filter, `Cashflow` buffer persistence.
- E2E: Playwright `mobileNav` viewports `360/375/390/430 + 768` (spec drafted at `ui/src/test/mobileNav.spec.ts` then moved to `vitest-axe`).

## Files
- `ui/src/hooks/useIsMobile.ts`
- `ui/src/styles/mobile.css`
- `ui/src/components/mobile/*` (11 files)
- `ui/src/screens/mobile/*` (14 files + `MobileReports.test.tsx`)
- `ui/src/App.tsx` (mobile lazy routes), `ui/src/main.tsx` (`mobile.css`), `ui/src/routes.ts` (`/more` non-linkable)
- `crates/finsight-core/src/models/custom_report.rs`, `crates/finsight-core/src/repos/budgets.rs`, `crates/finsight-providers/src/*` (fmt/clippy fixes), `.gitignore` (`vite.log`, `.claude/`)

## How to run
```bash
cp -r /mnt/e/Workspace/FinSight /tmp/FinSight # or ~/FinSight ext4
cd /tmp/FinSight && pnpm install --store-dir /tmp/pnpm-store
pnpm --filter ui dev --host 127.0.0.1 --port 5173   # open http://localhost:5173/?mock=1
pnpm --filter ui build # tsc -b && vite build
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```
