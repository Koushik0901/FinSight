# Phase 7B — App-Wide Anticipatory Execution: Final Report

Goal: generalize the CSV-import prepare/commit principle across FinSight — start safe backend work at the earliest signal, reuse prepared results, make final actions feel instant — with **generic** infrastructure, dependency-aware invalidation, and real measurement. Correctness, deterministic finance, approval safety, and cache integrity preserved.

Branch: `phase7b-anticipatory`. Green bar: **312 frontend tests, 0 TS errors** (+ backend suites unchanged from Phase 7's 341/0/9).

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

**Method chosen (per user):** drive the real packaged Tauri desktop app while the app self-instruments (§2c) and a background reader captures the perf marks — real desktop, not proxies.

**Automated evidence captured in this environment (labeled proxies, not desktop timings):**
- **Prefetch cache-hit is real, not aspirational:** `prefetch.test.tsx` proves the warmed key is the one the screen reads (command called exactly once across prefetch + hook). A cold Today route mounts **9 summary queries**; on warm (hover-then-click) those are cache hits → route-to-content collapses toward render-only cost. The instrumentation's `RouteTimer` will report this as `route:/` p50 dropping on the warm run.
- **Invalidation correctness, measurable as fewer stale surfaces:** the sync/recategorize under-invalidation and the dead net-worth-chart key were bugs (stale UI), not just perf — now fixed.
- **Rapid-hover dedup:** proven — 10 hovers → 1 fetch (`anticipatory.concurrency.test.tsx`).

**Real-desktop before/after (pending a driven run):** the instrumentation is in place and off-by-default; a computer-use pass over the packaged app (nav Today/Accounts/Reports cold vs hover-warm, filter/search latency, account open, with `finsightPerf` on) will produce the `perf.summary()` table. This is the one acceptance item that requires the packaged app + a driving session; it is enabled and ready, not yet executed here.

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
