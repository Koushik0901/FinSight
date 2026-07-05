# Phase 7B — App-Wide Anticipatory Execution: Final Report

Goal: generalize the CSV-import prepare/commit principle across FinSight — start safe backend work at the earliest signal, reuse prepared results, make final actions feel instant — with **generic** infrastructure, dependency-aware invalidation, and real measurement. Correctness, deterministic finance, approval safety, and cache integrity preserved.

Branch: `phase7b-anticipatory`. Green bar: **317 frontend tests, 0 TS errors** (`tsc -b` clean) (+ backend suites unchanged from Phase 7's 341/0/9).

---

## 1. Dependency audit

Full audit in [`2026-07-05-phase7b-interaction-dependency-audit.md`](2026-07-05-phase7b-interaction-dependency-audit.md): each of the ~17 flows documented as `SIGNAL → INPUTS → PREPARE → BLOCKED → COMMIT → DERIVED → INVALIDATION`, with a per-flow verdict.

**Verdict (consistent with Phase 7's finding that most flows are already cheap):** the audit justified a *small, generic* set rather than per-screen prefetch hacks —
1. a centralized dependency-aware invalidation map (highest leverage: correctness + retires duplication);
2. prefetch-on-intent (highest visible win: warms the 8–13-query mount burst);
3. measurement instrumentation to gate the rest.

Everything else is prefetch-covered or already instant (indexed `listTransactions`, summary-packed Copilot context, backend-aggregated reports, debounced search) — no bespoke infra, per the "no infra for sub-100ms flows" bar.

---

## 2. Generic infrastructure built (reused across flows, not per-screen)

The audit mapped most of the task-2 checklist onto TanStack Query primitives already in use (version IDs + dedup = query keys + cache; stale-result rejection = superseded observers drop results; progress = Tauri `emit`). The two genuine gaps got generic modules:

### 2a. Dependency-aware invalidation map — `ui/src/api/invalidation.ts`
A single mutation-**domain** → affected-derived-**keys** graph. A mutation declares *what changed* (`invalidateDomains(qc, "transactions")`), not *which caches to drop*. Retires ~150 hand-listed `invalidateQueries` calls across 10 hook files (the same 6-key transaction cluster was copy-pasted into a dozen mutations). Domains are granular and composable (`goals` vs `budgetEnvelopes`; `simplefin` = transactions + accounts + import; `agentApply` = agentActions + transactions). 6 unit tests.

**Fixed real correctness bugs surfaced by centralizing:**
- **Under-invalidation:** SimpleFin sync/import/reconcile only dropped the `["transactions"]`/`["accounts"]` roots, leaving `month-totals`, `net-worth`, `budget-envelopes`, `spending-breakdown` **stale after new rows landed**. Agent recategorization only refreshed the review count. Both now invalidate the full ledger fan-out.
- **Dead keys:** `["net-worth"]` / `["net-worth-history"]` were invalidation targets with **no query registered** — the net-worth chart's real key is `["networth-history", days]` (one word), so the chart went stale after every import/sync. The map now targets the correct key.
- **Over-invalidation:** account create/edit/rebalance needlessly dropped `month-totals` (balances ≠ transaction totals) — removed (dependency-aware, not refetch-all).

### 2b. Prefetch-on-intent — `ui/src/api/prefetch.ts`
`prefetchRoute(qc, path)` warms a route's summary queries on the earliest safe signal (Sidebar hover/focus); `prefetchAccountTransactions(qc, id)` warms an account's first transactions page on row hover. Each descriptor is keyed **byte-identically** to the destination screen's hook, verified end-to-end (`prefetch.test.tsx` prefetches, reads through the *real* hook, asserts the command is called once — a drifted key fails the test instead of silently warming nothing). Reads-only, idempotent (dedupes against fresh/in-flight entries), no-ops off the desktop runtime, and dies on Delete-All via the existing `qc.clear()`.

### 2c. Perf instrumentation — `ui/src/utils/perf.ts`
Opt-in (`localStorage.finsightPerf="1"` / `?perf=1`; zero overhead off). A QueryCache subscriber records every fetch's wall-clock duration tagged by key-root + hit/miss; a `RouteTimer` records nav-intent→content-painted per route (settles at `isFetching===0`, so a warm prefetched route reads ~0ms). Buffered to `window.__finsightPerf` with `export()`/`summary()` + console breadcrumbs — the packaged app self-instruments a measurement run.

---

## 3. Flows changed

| Flow | Change | Kind |
|---|---|---|
| Navigation (all routes) | Sidebar hover/focus → `prefetchRoute` | anticipatory |
| Accounts → transactions | account-row hover → `prefetchAccountTransactions` (first page + categories) | anticipatory |
| Every mutation (txn/account/category/rule/goal/budget/sync/agent/copilot) | `invalidateDomains` (dependency-aware) | orchestration + correctness |
| Transactions search | (Phase 7) 250ms debounce + superseded-drop | already anticipatory |
| CSV import | (Phase 7) prepare/commit fold + preview | the reference pattern |

---

## 4. Invalidation graph (derived-data orchestration, task 4)

Encoded in `invalidation.ts`; see the audit doc's table. Summary of the dependency edges:
- **transactions** → transactions(+infinite), month-totals, categories-with-spending, spending-breakdown, budget-envelopes, journey-status, needs-review-count, recurring, networth-history, account-balance-history/sparklines, agent-status, financial-health-score.
- **accounts** → accounts, account-owners, networth-history, account-balance-*, budget-envelopes, journey-status, financial-health-score.
- **categories** → categories(+with-spending), spending-breakdown, transactions (labels), budget-envelopes, recurring, rules. (Explicitly NOT net-worth/accounts.)
- **goals** / **budgetEnvelopes** / **rules** / **agentActions** — narrow, independent.
- **composites**: agentApply = agentActions + transactions; simplefin/importCommit = transactions + accounts + import.
- **reset** = `qc.clear()` (frontend) + backend `ResetBarrier` drain (Phase 7B).

---

## 5. Measurement

**Method (per user):** `tauri build` → install the real NSIS package → drive it with computer-use (mouse/keyboard on the actual installed app, real SQLCipher DB with real sample data: 3 accounts, ~2000 transactions, 249 needing review, 64 anomalies) → the app self-instruments (§2c) → `Ctrl+Alt+S` exports `perf.summary()` to the clipboard → `read_clipboard` captures it. Real desktop, not proxies.

**A measurement bug was found and fixed by this process, not assumed away.** The first driven pass returned every `route:*` entry at ~0ms — including genuinely cold first visits to `/insights`, `/budget`, `/categories` that the same capture's `query:*` entries proved did real 10–100ms+ backend work. Root cause: `RouteTimer`'s `isFetching`-effect fired in the same commit as the route-change effect, so its first read was the *previous* route's already-settled value, not a signal the new route had started fetching. Fixed (commit `f926651`) to only close a route on a `0` that was preceded by an observed `>0` for that route, with a bounded 32ms grace fallback for routes served entirely from a warm cache. Re-ran the identical driven sequence after the fix — this is that corrected data.

**Captured `perf.summary()` (installed release build, real ledger, this session):**

| Label | count | p50 | p95 | max | Note |
|---|---|---|---|---|---|
| `route:/` (Today, revisit) | 1 | 14ms | 14ms | 14ms | already warm from launch |
| `route:/recurring` (cold) | 1 | 26ms | 26ms | 26ms | ≈ `query:recurring` p50 27ms — dominant query matches route time |
| `route:/budget` (hover-then-click) | 1 | 31ms | 31ms | 31ms | ≈ `query:budget-envelopes` 11ms + `query:budget-history` 14ms |
| `route:/accounts` (hover-then-click) | 1 | 35ms | 35ms | 35ms | accounts/owners/household queries, partly pre-warmed by hover |
| `route:/categories` (hover-then-click) | 1 | 41ms | 41ms | 41ms | ≈ `query:categories-with-spending` (31–102ms range) |
| `route:/insights` (cold) | 1 | 81ms | 81ms | 81ms | ≈ `query:financial-health-score` 90ms |
| `route:/accounts/…/transactions` (hover-then-click) | 1 | 57ms | 57ms | 57ms | account-row hover prefetch + `query:transactions-infinite` 10ms |
| `route:/reports` (cold + revisit) | 2 | 132ms | 132ms | 132ms | see caveat below |
| `query:report-data` | 2 | 122ms | 122ms | 122ms | Reports' own dominant query |
| `query:categories-with-spending` | 7 | 31ms | 102ms | 102ms | widest spread — read across many screens |
| `query:financial-health-score` | 2 | 90ms | 90ms | 90ms | |
| `query:savings-rate-history` | 3 | 31ms | 90ms | 90ms | |
| `query:accounts` | 8 | 18ms | 27ms | 27ms | |
| `query:transactions-infinite` | 4 | 10ms | 16ms | 16ms | search + account-open combined |

Full raw capture (all ~25 labels) is in the session transcript; the table above is the representative subset per flow.

**Reading it:** every `route:*` value is now non-zero and tracks its screen's real dominant query cost (`recurring` 26↔27ms, `insights` 81↔90ms, `categories` 41ms↔31–102ms range) — the fix produced believable, internally-consistent numbers instead of the flat 0 the bug produced. The **cold** group (Reports/Insights/Recurring, direct click, no hover) and the **hover-then-click** group (Accounts, account-row, Budget, Categories) can't be cleanly diffed pairwise (different screens have different backend cost), but the mechanism they exercise — prefetch firing on hover before the click lands — is separately proven exact-cache-hit-or-miss by `prefetch.test.tsx`, so real-desktop numbers here corroborate rather than re-prove that.

**Caveat on `route:/reports` (132ms, n=2):** the `summary()` percentile formula (`floor(p/100 * n)` index into the sorted array) collapses p50/p95/max to the same value whenever n=2 — it cannot distinguish which of the two visits (cold vs. the later revisit) was faster from this view alone. 132ms is a valid upper bound consistent with `query:report-data`'s own 122ms p95, but whether the revisit was actually faster than the cold visit is not resolvable from `summary()` with only 2 samples; a future refinement would keep min/last alongside p50/p95/max. Flagging this rather than asserting a specific revisit speedup I can't prove from the data.

**Automated evidence (frontend suite, not desktop timings, unchanged from before the driven pass):**
- **Prefetch cache-hit is real, not aspirational:** `prefetch.test.tsx` proves the warmed key is the one the screen reads (command called exactly once across prefetch + hook).
- **Invalidation correctness, measurable as fewer stale surfaces:** the sync/recategorize under-invalidation and the dead net-worth-chart key were real bugs (stale UI), not just perf — now fixed and covered by `invalidation.test.ts`.
- **Rapid-hover dedup:** proven — 10 hovers → 1 fetch (`anticipatory.concurrency.test.tsx`).

---

## 6. Concurrency / edge tests (task 6)

`anticipatory.concurrency.test.tsx` (5) + `prefetch.test.tsx` (3) + `invalidation.test.ts` (6) + Phase 7's `useDebouncedValue` (3):
- rapid repeated hovers → single fetch (idempotent prefetch);
- per-account keys isolate (a stale prefetch can't masquerade as another account);
- Delete-All (`qc.clear`) drops every prefetched/derived entry;
- `invalidateDomains` marks stale without evicting (no flash of empty);
- superseded search queries are dropped (debounce + TanStack).

---

## 7. Deferred flows (audit-tagged, measure-first)

Not built — each needs the real-desktop numbers to justify, and Phase 7 showed the backends are already cheap:
- **Category-edit affected-count preview** — a `COUNT` is likely cheap enough to run debounced without a backend prepare; gate on measurement.
- **Bulk-recategorization proposal prepare** — strong prepare/approve fit *if* the proposal compute is expensive; must re-validate at execute time (executor EXECUTE-TIME GUARD already does).
- **Scenario deterministic preview** — only the non-LLM chip presets are deterministic; debounce-preview those if measured slow.
- **Copilot tool concurrency** — parallelize only provably-independent read-only tools; SQLite read contention + the sequential reasoning loop make this measure-gated.

---

## 8. Remaining risks / invariants held

- **Stale results can't overwrite current state:** query-key versioning + superseded-drop; prefetch keys are per-account/per-filter so a stale warm can't cross-contaminate (tested).
- **No speculative mutation:** all prepare/prefetch is reads-only; mutations still fire only on explicit action; approvals re-validate at execute time.
- **Delete-All destroys prepared/derived state:** `qc.clear()` (frontend) + `ResetBarrier` drain (backend). Any *future* deferred-commit anticipatory backend work must take a `ResetBarrier` lease (Phase 7B closed this writer class).
- **Prefetch drift risk:** mitigated by the key-match test, but a screen changing its query key without updating the descriptor would silently stop warming (the test catches an *existing* descriptor's drift; a newly-added screen query isn't auto-covered). Documented for maintainers.
- **`RouteTimer` route-to-content** treats `isFetching===0` as "content ready"; a route with a late secondary fetch could mark early. Acceptable for relative before/after; noted.
