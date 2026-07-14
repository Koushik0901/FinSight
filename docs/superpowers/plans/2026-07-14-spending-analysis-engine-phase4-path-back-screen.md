# Spending Analysis Engine — Phase 4 (Path Back screen + proactive Inbox) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Surface the shipped engine in the UI: a **Path Back** screen (gap vs your normal, the classification banner, ranked drivers split into recurring *levers* vs *self-correcting* one-offs, an honest target row, and one-click sticky annotations), plus **proactive** surfacing — an Inbox action item when a `regime_shift` is detected.

**Architecture:** Thin Tauri commands wrap the engine (`run()` helper, `AppState`); the engine's output types gain a `specta::Type` derive so bindings generate. One read command (`get_spending_path_back`) feeds the whole screen (classification + plan); one write command (`set_spending_annotation`) powers the Keep/One-time chips. Proactive = a new `ActionItem` pushed by `get_action_items` (Inbox renders it automatically — no `Inbox.tsx` change). The screen uses the app's own design system (`.screen`, `Card`/`Button`/`Badge`, `money()`, `var(--ink)`/`--accent`/`--negative`/`--warning`/`--positive`) so it sits natively beside Reports.

**Tech Stack:** Rust (tauri, specta, rusqlite), tauri-specta bindings, React + TypeScript + tanstack-query + vitest.

**Prereq:** Phases 1–3 merged (`finsight-core::spending` with `classify`, `plan`, `annotate`, `baseline::{trailing,latest_activity_month}`). Design reference: the approved mockup (this session) — regime banner, 3 stat tiles (recent / normal / gap), a target row that shows reachable-vs-structural, and two columns (levers with Keep/One-time actions · self-correcting).

**Scope note:** Follow-on (NOT here): a dedicated eval spike fixture; per-currency `month_total`.

---

## File structure

- Modify `crates/finsight-core/src/spending/mod.rs` — `specta::Type` on `Mechanism`, `Persistence`, `Driver`.
- Modify `crates/finsight-core/src/spending/classify.rs` — `specta::Type` on `PeriodClass`, `PeriodAssessment`.
- Modify `crates/finsight-core/src/spending/plan.rs` — `specta::Type` on `SpendingPlan`; add `self_correcting: Vec<Driver>`.
- Create `crates/finsight-app/src/commands/spending.rs` — `get_spending_path_back`, `set_spending_annotation`, `PathBackView`.
- Modify `crates/finsight-app/src/commands/mod.rs` — `pub mod spending;`.
- Modify `crates/finsight-app/src/lib.rs` — register the two commands.
- Modify `crates/finsight-app/src/commands/inbox.rs` — push a regime-shift `ActionItem`.
- Regenerate `ui/src/api/bindings.ts` (via `export_bindings`).
- Create `ui/src/api/hooks/spending.ts` — `usePathBack`, `useSetSpendingAnnotation`.
- Create `ui/src/screens/PathBack.tsx` — the screen.
- Create `ui/src/screens/PathBack.test.tsx` — vitest.
- Modify `ui/src/App.tsx` — lazy import + route `/path-back`.
- Modify `ui/src/components/Sidebar.tsx` — `NAV_MAIN` entry.

---

### Task 1: Core — `specta::Type` derives + `SpendingPlan.self_correcting`

**Files:** `mod.rs`, `classify.rs`, `plan.rs` (all under `crates/finsight-core/src/spending/`)

- [ ] **Step 1: `mod.rs`.** Add `use specta::Type;` near `use serde::{Deserialize, Serialize};`. Add `Type` to the derive lists of `enum Mechanism`, `enum Persistence`, and `struct Driver`. (Leave `PersistenceSubtotals`/`DecomposeResult`/`Window` unchanged — not returned to the frontend.)

- [ ] **Step 2: `classify.rs`.** Add `use specta::Type;`. Add `Type` to `enum PeriodClass` and `struct PeriodAssessment`.

- [ ] **Step 3: `plan.rs`.** Add `use specta::Type;`. Add `Type` to `struct SpendingPlan`. Add a field to `SpendingPlan` after `pub levers: Vec<Driver>,`:
```rust
    /// The one-off drivers that lapse on their own — shown as "leave them".
    pub self_correcting: Vec<Driver>,
```
Then in `plan_spending_reduction`, replace the `let levers: Vec<Driver> = d.drivers.into_iter()...collect();` block with (uses `iter().cloned()` so both lists can be derived from the same `drivers`):
```rust
    let levers: Vec<Driver> = d
        .drivers
        .iter()
        .filter(|dr| {
            dr.delta_cents > 0
                && dr.user_verdict.is_none()
                && matches!(dr.persistence, Persistence::Recurring | Persistence::Emerging)
        })
        .cloned()
        .collect();
    let self_correcting: Vec<Driver> = d
        .drivers
        .iter()
        .filter(|dr| {
            dr.delta_cents > 0
                && (dr.user_verdict.as_deref() == Some("one_off")
                    || (dr.user_verdict.is_none() && dr.persistence == Persistence::OneOff))
        })
        .cloned()
        .collect();
```
And add `self_correcting,` to the `Ok(SpendingPlan { … })` construction.

- [ ] **Step 4: extend a plan test.** In `plan.rs`'s `splits_self_correcting_from_recoverable_and_flags_structural_target`, after the existing `levers` assertions, add:
```rust
        assert!(p.self_correcting.iter().any(|d| d.display == "FLAIR AIRLINES"), "the one-off flight shows in self_correcting");
        assert!(!p.self_correcting.iter().any(|d| d.display == "SAVE ON FOODS"), "the recurring grocery is not self-correcting");
```

- [ ] **Step 5: Verify + commit.**
Run: `cargo test -p finsight-core --lib spending::plan`. Expect all PASS. Also `cargo build -p finsight-core` to confirm the `Type` derives compile.
```
git add crates/finsight-core/src/spending/mod.rs crates/finsight-core/src/spending/classify.rs crates/finsight-core/src/spending/plan.rs
git commit -m "feat(spending): specta::Type on engine outputs + SpendingPlan.self_correcting

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: App commands + bindings

**Files:** Create `crates/finsight-app/src/commands/spending.rs`; Modify `commands/mod.rs`, `lib.rs`

- [ ] **Step 1: Create `crates/finsight-app/src/commands/spending.rs`:**
```rust
use crate::error::{AppError, AppResult};
use crate::AppState;
use finsight_core::repos::run;
use finsight_core::spending::classify::{self, PeriodAssessment};
use finsight_core::spending::plan::{self, SpendingPlan};
use finsight_core::spending::{annotate, baseline};
use serde::Serialize;
use specta::Type;

/// Everything the Path Back screen needs, in one read: the period's
/// classification (normal / spike / regime) and the honest reduction plan
/// (levers vs self-correcting vs structural).
#[derive(Debug, Clone, Serialize, Type)]
pub struct PathBackView {
    pub period: String,
    pub assessment: PeriodAssessment,
    pub plan: SpendingPlan,
}

/// `period` defaults to the most recent month with activity; `None` result means
/// there is no spending to plan from.
#[tauri::command]
#[specta::specta]
pub async fn get_spending_path_back(
    state: tauri::State<'_, AppState>,
    period: Option<String>,
    target_monthly_cents: Option<i64>,
) -> AppResult<Option<PathBackView>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let period = match period {
            Some(p) if p.len() >= 7 => p,
            _ => match baseline::latest_activity_month(conn)? {
                Some(ym) => ym,
                None => return Ok(None),
            },
        };
        let assessment = classify::classify_spending_period(conn, &period)?;
        let plan = plan::plan_spending_reduction(conn, &period, target_monthly_cents)?;
        Ok(Some(PathBackView { period, assessment, plan }))
    })
    .await
    .map_err(AppError::from)
}

/// Write a sticky verdict on a driver (`one_off` | `expected` | `investment` |
/// `reset`), keyed by the canonical merchant key the view returns.
#[tauri::command]
#[specta::specta]
pub async fn set_spending_annotation(
    state: tauri::State<'_, AppState>,
    merchant_key: String,
    verdict: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        if verdict == "reset" {
            annotate::clear_annotation(conn, &merchant_key)?;
        } else if annotate::VERDICTS.contains(&verdict.as_str()) {
            annotate::set_annotation(conn, &merchant_key, &verdict, None)?;
        }
        Ok(())
    })
    .await
    .map_err(AppError::from)
}
```

- [ ] **Step 2: Declare the module.** In `crates/finsight-app/src/commands/mod.rs`, add `pub mod spending;` with the other `pub mod` lines.

- [ ] **Step 3: Register the commands.** In `crates/finsight-app/src/lib.rs`, inside `collect_commands![ … ]`, add (next to the `commands::reports::*` block):
```rust
        commands::spending::get_spending_path_back,
        commands::spending::set_spending_annotation,
```

- [ ] **Step 4: Compile + regenerate bindings.**
Run: `cargo build -p finsight-app` (must compile — if `PeriodAssessment`/`SpendingPlan`/`Driver` aren't `Type`, revisit Task 1).
Then from the repo root: `cargo run -p finsight-tauri --bin export_bindings`
Confirm `ui/src/api/bindings.ts` now contains `getSpendingPathBack`, `setSpendingAnnotation`, and a `PathBackView` type. Then `cd ui && npx tsc --noEmit` (0 errors).

- [ ] **Step 5: Commit.**
```
git add crates/finsight-app/src/commands/spending.rs crates/finsight-app/src/commands/mod.rs crates/finsight-app/src/lib.rs ui/src/api/bindings.ts
git commit -m "feat(app): get_spending_path_back + set_spending_annotation commands

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Proactive — regime-shift Inbox item

**Files:** Modify `crates/finsight-app/src/commands/inbox.rs`

- [ ] **Step 1: Read the tail of `get_action_items`** to find where the last `items.push(...)` block ends and `Ok(items)` (or the sort/return) begins. Insert the new block just before the items are returned/sorted.

- [ ] **Step 2: Push a regime-shift item** (uses the file's existing `fmt_money` + `ActionItem`):
```rust
        // ── Spending regime shift ─────────────────────────────────────────────
        // Proactively surface when the latest month is a SUSTAINED step up vs
        // the user's normal (not a one-off spike). Deep-links to the Path Back
        // screen. Uses the same engine the screen + Copilot use, so the numbers
        // reconcile everywhere.
        if let Some(ym) = finsight_core::spending::baseline::latest_activity_month(conn)? {
            if let Ok(a) = finsight_core::spending::classify::classify_spending_period(conn, &ym) {
                if a.class == finsight_core::spending::classify::PeriodClass::RegimeShift {
                    let over = (a.period_total_cents - a.baseline_monthly_cents).max(0);
                    items.push(ActionItem {
                        id: "spending-regime-shift".to_string(),
                        category: "budget".to_string(),
                        priority: "high".to_string(),
                        title: "Your spending has stepped up from your normal".to_string(),
                        detail: format!(
                            "This isn't a one-off — the last month is running about {} above your usual, and recent months are elevated too. See what's driving it and your path back.",
                            fmt_money(over)
                        ),
                        action_label: "See your path back".to_string(),
                        action_route: "/path-back".to_string(),
                        badge_count: None,
                        amount_cents: Some(over),
                    });
                }
            }
        }
```

- [ ] **Step 3: Verify + commit.**
Run: `cargo test -p finsight-app --lib` (existing inbox tests still pass; the block compiles). If there's an inbox test asserting an exact item count on a seeded regime, adjust per the seed — but the default seed is spike-free so most fixtures won't trip it.
```
git add crates/finsight-app/src/commands/inbox.rs
git commit -m "feat(app): proactive Inbox item when spending shows a regime shift

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Frontend hook

**Files:** Create `ui/src/api/hooks/spending.ts`

- [ ] **Step 1: Create the hook** (mirror `hooks/inbox.ts` + a mutation like the simplefin hooks):
```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type PathBackView } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function usePathBack(period: string | null, targetMonthlyCents: number | null) {
  return useQuery<PathBackView | null>({
    queryKey: ["path-back", period, targetMonthlyCents],
    queryFn: async () => {
      const result = await commands.getSpendingPathBack(period, targetMonthlyCents);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    enabled: isTauriRuntime(),
  });
}

export function useSetSpendingAnnotation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (v: { merchantKey: string; verdict: string }) => {
      const result = await commands.setSpendingAnnotation(v.merchantKey, v.verdict);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["path-back"] });
    },
  });
}
```
(Confirm the exact command names + `PathBackView` field names against the regenerated `bindings.ts` — specta camelCases command names but the engine structs are snake_case, e.g. `plan.structural_gap_cents`, `plan.recent_monthly_cents`, `driver.delta_cents`, `driver.merchant_key`, `driver.user_verdict`, `assessment.class`.)

- [ ] **Step 2: Commit.**
```
git add ui/src/api/hooks/spending.ts
git commit -m "feat(ui): usePathBack + useSetSpendingAnnotation hooks

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: The Path Back screen

**Files:** Create `ui/src/screens/PathBack.tsx`

**Read first:** `ui/src/screens/Reports.tsx` (stat-row + `.screen` header pattern), `ui/src/screens/Inbox.tsx` (Card/Button/Badge usage), `ui/src/utils/format.ts` (`money`), `ui/src/components/Icons.tsx` (available icons). Match those conventions exactly — this screen must look native.

**Data contract** (from `usePathBack`): `view.assessment.{class, period_total_cents, baseline_monthly_cents, upper_band_cents, note}` and `view.plan.{recent_monthly_cents, baseline_monthly_cents, self_correcting_cents, recoverable_recurring_cents, projected_after_levers_cents, target_monthly_cents, structural_gap_cents, note, levers, self_correcting}`, `view.period`. Each driver: `{merchant_key, display, delta_cents, mechanism, persistence, user_verdict}`.

- [ ] **Step 1: Build the screen** with these sections (match the approved mockup, in app tokens):

1. **Header** (`.screen` > `.screen-header`): eyebrow `Path back · <period>`, `<h1>Getting back to your normal.</h1>`, and a right-aligned classification pill: for `regime_shift` a `<Badge tone="warning">Regime shift — not a blip</Badge>`, for `episodic_spike` `<Badge tone="accent">One-month spike</Badge>`, for `normal` `<Badge tone="positive">Within your normal</Badge>`. A `CopilotNudge` ("Help me build a plan to get back to my normal") is a nice touch.
2. **Stat row** (reuse Reports' `.stat-row` / `.stat`): Recent (`plan.recent_monthly_cents` + "/mo"), Your normal (`plan.baseline_monthly_cents`, sub "median · 12 mo"), The gap (`recent − baseline`, `.stat` with warning emphasis when positive). Blur amounts with `className="money"` (privacy mode).
3. **Target row** (a `Card`): a controlled number input bound to local `target` state (dollars), debounced into the `usePathBack(period, target*100)` query. Show the honest result from the fresh `plan`: a two-segment bar (reachable width ∝ `projected_after_levers_cents`, structural width ∝ `structural_gap_cents` when set) and the `plan.note`. When `structural_gap_cents` is set, render the "structural" segment in `var(--warning)` and state the structural amount with `money(structural_gap_cents)`.
4. **Two columns** (`grid`, `repeat(auto-fit,minmax(260px,1fr))`):
   - **Your levers · trim these** (`~money(recoverable_recurring_cents)`): map `plan.levers` to rows — `display`, `+money(delta_cents)` in `var(--accent)`, a small mechanism tag (map `mechanism` → label: `new`→"new", `frequency_up`→"more often", `price_up`→"pricier", `mixed`→"more + pricier"), and two chips: **Keep** (`useSetSpendingAnnotation` → verdict `expected`) and **One-time** (→ `one_off`). On click, call the mutation with `{ merchantKey: driver.merchant_key, verdict }` and `toast.success`. A driver with `user_verdict` set shows a muted "· kept" / "· one-time" tag and a **Undo** chip (verdict `reset`).
   - **Self-correcting · leave them** (`~money(self_correcting_cents)`, muted): map `plan.self_correcting` to read-only rows (`display`, `+money(delta_cents)`, tag). Footer line: "Already behind you — no action. If any recurs, it moves to your levers automatically."
5. **Loading / empty / not-Tauri:** `if (isLoading) return <div className="stub">Charting your path back…</div>;` `if (!view) return <EmptyState … "No spending to analyze yet" />;`.

Keep it clean and native — lean on existing classes (`.screen`, `.stat-row`, `.stat`, `Card`, `Button`, `Badge`, `.eyebrow`, `.muted`, `.money`). No hardcoded colors — tokens only. All amounts through `money()`.

- [ ] **Step 2: Type-check.** `cd ui && npx tsc --noEmit` — 0 errors.

- [ ] **Step 3: Commit.**
```
git add ui/src/screens/PathBack.tsx
git commit -m "feat(ui): Path Back screen — gap, drivers, honest target, sticky verdicts

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Wire the route + sidebar

**Files:** Modify `ui/src/App.tsx`, `ui/src/components/Sidebar.tsx`

- [ ] **Step 1: App.tsx.** Add a lazy import next to the others: `const PathBack = lazy(() => import("./screens/PathBack"));`. Add a route inside `<Routes>` (near `/reports`): `<Route path="/path-back" element={<PathBack />} />`.

- [ ] **Step 2: Sidebar.tsx.** Add an entry to `NAV_MAIN` after the `reports` entry (pick an apt existing icon — `I.Goal` or `I.TrendingDown` if present, else `I.Spark`): `{ id: "path-back", path: "/path-back", label: "Path back", Icon: I.Goal },`. (Confirm the icon exists in `Icons.tsx`; use one that does.)

- [ ] **Step 3: Verify + commit.**
Run: `cd ui && npx tsc --noEmit` (0 errors).
```
git add ui/src/App.tsx ui/src/components/Sidebar.tsx
git commit -m "feat(ui): route + sidebar entry for the Path Back screen

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 7: Screen test + full verification

**Files:** Create `ui/src/screens/PathBack.test.tsx`

- [ ] **Step 1: Write a vitest** that mocks `usePathBack`/`useSetSpendingAnnotation` (or `commands`) and asserts: the regime banner renders; a lever row shows its display + delta; clicking **Keep** calls the annotation mutation with the driver's `merchant_key` and verdict `expected`; the structural-gap copy appears when `structural_gap_cents` is set. Follow the existing screen-test pattern (`ui/src/screens/Reports.test.tsx` / `Inbox` tests) — vitest + `@testing-library/react`, mock the hook module with `vi.mock`.

- [ ] **Step 2: Run the frontend suite + types.**
```
cd ui && npx vitest run src/screens/PathBack.test.tsx
cd ui && npx tsc --noEmit
```
Both green.

- [ ] **Step 3: Rust green-bar.**
```
cargo test -p finsight-core --lib spending
cargo test -p finsight-app --lib
```
All PASS.

- [ ] **Step 4: Commit.**
```
git add ui/src/screens/PathBack.test.tsx
git commit -m "test(ui): Path Back screen renders drivers + writes verdicts

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-review

**Spec coverage:** Path Back screen (spec §8 dedicated screen) — Tasks 1–6. Proactive surfacing (spec §8 "proactive Insights → Inbox") — Task 3. Reconciliation (spec §5) — the screen + Inbox + Copilot all read the same `finsight-core::spending`; the command is a thin wrapper. Sticky-annotation UX (spec §7) — the Keep/One-time/Undo chips. Honest target (spec §14) — the structural-gap segment + `plan.note`.

**Placeholder scan:** none. The two "confirm against bindings/Icons" notes are grounded verification steps with concrete fallbacks.

**Type consistency:** `PathBackView` (app) wraps core `PeriodAssessment` + `SpendingPlan` (both now `Type`). `SpendingPlan.self_correcting` added Task 1, consumed Task 5. Command names `getSpendingPathBack`/`setSpendingAnnotation` (specta-camelCased) used identically in the hook. Engine struct fields are snake_case in TS (`structural_gap_cents`, `merchant_key`, …) — the screen must use snake_case, matching the `Transaction` convention this repo documents.

**Known assumptions to verify during execution:** exact `bindings.ts` names/shapes after regeneration (Task 2 Step 4); an apt existing icon in `Icons.tsx` (Task 6); no inbox test asserts an exact count that the new item would break (Task 3).

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-14-spending-analysis-engine-phase4-path-back-screen.md`. Continuing with **Subagent-Driven** execution; I'll verify the screen renders (types + vitest, and a real preview if feasible) before finishing.
