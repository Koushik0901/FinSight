# Phase 7B — Interaction / Dependency Audit

Goal: for every major flow, find the earliest safe point to begin non-destructive backend work, reuse prepared results, and make final actions feel instant — with **generic** infrastructure, not one-off hacks.

**Method:** each flow is documented as
`USER SIGNAL → KNOWN INPUTS → SAFE PREPARE → BLOCKED DEPS → COMMIT → AFFECTED DERIVED → INVALIDATION`.
A verdict tags each flow **[ANTICIPATE]** (justifies new anticipatory work), **[HAVE]** (already anticipatory/instant), or **[DEFER]** (cheap or not worth infra — measured/reasoned).

**Grounding from Phase 7 (already established, not re-litigated):** `listTransactions` is single-query + indexed (`idx_transactions_account_posted`) + paginated (50/page); Copilot `build_context` is summary-packed (no raw payloads); Reports are backend-aggregated with 60s staleTime; the import prepare/commit pipeline exists; Delete-All has an airtight `ResetBarrier` drain + `qc.clear()`. `refetchOnWindowFocus` is off; transactions search is debounced.

**Two systemic findings (drive the infrastructure work):**
1. **Navigation is cold.** `Sidebar` uses plain `NavLink` with no prefetch; each screen fires 8–13 read queries *on mount*. First paint of a route waits on that burst. → **prefetch-on-intent** (7B.3).
2. **Invalidation is hand-maintained and duplicated.** ~150 `invalidateQueries` calls across 10 hook files; the same 6-key transaction cluster (`transactions, month-totals, categories-with-spending, budget-envelopes, spending-breakdown, journey-status`) is copy-pasted into every txn mutation. Forgetting a key = silent stale UI; over-listing = needless refetch. → **centralized dependency-aware invalidation map** (7B.2).

---

## Derived-data dependency graph (the invalidation map, task 4)

Mutation **domains** and the derived read-keys each must invalidate. This is the single source of truth that 7B.2 encodes and 7B.5 tests.

| Domain (what changed) | Invalidates (affected derived data) |
|---|---|
| **transactions** (create/update/delete/flags/splits, import, sync commit, recategorize) | `transactions`, `transactions-infinite`, `month-totals`, `categories-with-spending`, `spending-breakdown`, `budget-envelopes`, `journey-status`, `needs-review-count`, `recurring`, `net-worth`, `net-worth-history`, `account-balance-history`, `account-balance-sparklines`, `agent-status` (anomaly count), `financial-health-score` |
| **accounts** (create/edit/balance/delete/owners) | `accounts`, `account-owners`, `net-worth`, `net-worth-history`, `account-balance-history`, `account-balance-sparklines`, `budget-envelopes`, `journey-status`, `financial-health-score` |
| **categories** (create/rename/archive/color/spending-type/guidance) | `categories`, `categories-with-spending`, `spending-breakdown`, `transactions` (labels), `budget-envelopes`, `recurring`, `rules` |
| **rules** (create/toggle/delete) | `rules`, `rule-proposals` |
| **budget** (envelopes/goals) | `budget-envelopes`, `budget-history`, `goals`, `goal-projection`, `journey-status`, `net-worth` (goal earmarks), `plan-next-month` |
| **agentActions** (bundle approve/reject/apply) | `action-bundles`, `action-bundle`, `action-items`, `execution-log`, plus **transactions** domain (an applied bundle mutates the ledger) |
| **copilotConversation** (send/edit/delete msg) | `conversations`, `conversation-messages`, `agent-sessions` |
| **simplefin** (connect/sync/disconnect/purge) | full **transactions** + **accounts** domains, `unfinished-imports`, `csv-saved-mapping`, `csv-prepare` |
| **reset** (Delete-All) | **everything** — `qc.clear()` + backend `ResetBarrier` drain |

Principle: a mutation invalidates its domain, not "everything." `net-worth` etc. are true dependents of `transactions`/`accounts` and belong in those domains; they are **not** invalidated by `categories` or `rules` changes (today some mutations over-invalidate; the map fixes that).

---

## Per-flow audit

### Startup / onboarding — [DEFER]
- SIGNAL: app launch. INPUTS: none (DB key from keychain). PREPARE: migrations + provider bootstrap already run in `.setup()`. BLOCKED: first render needs onboarding-state. COMMIT: onboarding steps write settings. DERIVED: none heavy. INVALIDATION: `onboarding`, `onboarding-state`.
- Verdict: one-time, gated on user typing; not a latency hotspot. Defer.

### Navigation (sidebar → route) — [ANTICIPATE]
- SIGNAL: **hover/focus on a NavLink** (intent) precedes the click by 100s of ms. INPUTS: target route (static). PREPARE: `prefetchQuery` the destination screen's summary queries with the **exact keys** the screen uses. BLOCKED: none (all reads). COMMIT: none. DERIVED: none. INVALIDATION: none.
- Verdict: **the highest-visibility win.** Cold routes fire 8–13 queries on mount; prefetching on hover warms the cache so the click paints from cache. 7B.3.

### Accounts list — [ANTICIPATE (row-hover prefetch)]
- SIGNAL: hover on an account row (intent to open its transactions). INPUTS: `accountId`. PREPARE: `prefetchQuery(["transactions-infinite", {accountId,…}])` first page + `categories-with-spending`. BLOCKED: none. COMMIT: none on hover.
- Verdict: opening an account is a top flow; row-hover prefetch makes it instant. 7B.3.

### Transactions / search / filter / pagination — [HAVE + tighten]
- SIGNAL: filter/search edit. INPUTS: filter object. PREPARE: query keyed on filter; **search already debounced (250ms)**; superseded queries are dropped by TanStack (stale-result rejection is built-in). Pagination via `useInfiniteTransactions`. BLOCKED: none. COMMIT: row edits → **transactions** domain.
- Verdict: mostly done in Phase 7. Confirm date/preset changes reuse cache; ensure the invalidation map replaces the hand lists. Minor.

### CSV import — [HAVE]
- The reference implementation: `prepare_csv_import` fold + preview, debounced on mapping edits, `csv-prepare` invalidated on close/import/Delete-All. Commit reuses the prepared fold. Nothing to add; it *is* the pattern being generalized.

### Categories / category rules — [ANTICIPATE (edit preview) — audit-gated]
- SIGNAL: editing a category (rename/color/archive) or a rule pattern. INPUTS: category/rule id + new value. PREPARE: a **read-only affected-count** ("this rule matches N transactions", "archiving affects N categorized txns") *before* save — a genuine separable read that may justify a small backend prepare IF measured non-trivial. BLOCKED: the actual rename/archive waits for Save (mutation). DERIVED: **categories** domain. 
- Verdict: preview-before-save is real anticipatory value; gate the backend prepare on measured cost (a `COUNT` is likely cheap enough to run live/debounced without new infra).

### Bulk recategorization — [ANTICIPATE — audit-gated]
- SIGNAL: user opens a recategorize flow (e.g. "recategorize all COSTCO as Groceries"). INPUTS: merchant/pattern + target category. PREPARE: compute the **proposal set** (which txns would change) read-only, debounced, *before* the user approves. BLOCKED: the write waits for explicit approval; must re-validate at execute time (executor already has an EXECUTE-TIME GUARD). DERIVED: **transactions** domain. 
- Verdict: strong fit for prepare/approve. Confirm whether a proposal-preview path exists; if not and it's expensive, a read-only prepare is justified.

### Needs Review / anomalies — [DEFER]
- SIGNAL: open Inbox/anomalies filter. INPUTS: preset. PREPARE: it's a filtered `listTransactions` (indexed) + `needs-review-count`. Prefetchable via nav prefetch (7B.3). COMMIT: review actions → transactions domain.
- Verdict: covered by nav/row prefetch + the invalidation map. No bespoke work.

### Dashboard / Today — [ANTICIPATE via prefetch]
- SIGNAL: nav intent to `/`. INPUTS: none. PREPARE: prefetch the ~13 Today queries on hover. BLOCKED: none. Verdict: the landing route benefits most from nav prefetch; also the biggest query burst. 7B.3.

### Reports / charts — [HAVE + prefetch]
- Backend-aggregated, 60s staleTime. PREPARE: prefetch `month-totals`, `savings-rate-history`, `spending-breakdown` on nav intent. Verdict: prefetch only; no backend change (Phase 7 confirmed already index-optimal).

### Insights — [DEFER + prefetch]
- `financial-health-score`, memory, actions. Prefetchable on nav. Verdict: nav prefetch; no bespoke.

### Recurring / subscriptions — [DEFER + prefetch]
- `recurring` detection is backend; cache it, prefetch on nav. Verdict: nav prefetch.

### Scenarios / recipes / journey — [ANTICIPATE (scenario preview) / DEFER (rest)]
- Scenarios: LLM-driven run → save (run-then-save; **not** speculative-persist — confirmed in Phase 7B reset audit). Deterministic scenario math (chip presets, no LLM) can recompute a **debounced preview** as sliders/params change. Recipes/journey: prefetch on nav.
- Verdict: scenario deterministic-preview debounce is the only anticipatory candidate here; gate on whether params are deterministic.

### Copilot tools / artifacts — [HAVE / audit-gated concurrency]
- Context is summary-packed. Independent **deterministic** tool/context reads could run concurrently (task 3 example) — but SQLite read contention + the reasoning engine's sequential tool loop make this audit-gated; only parallelize provably-independent read-only tools. Streaming/cancellation preserved. 
- Verdict: inspect the reasoning engine's tool dispatch; parallelize only if independent reads are on the critical path. Likely DEFER unless measured.

### Approvals — [HAVE]
- Rendering an approval may prepare validation/context (read-only), but the mutation waits for explicit approval and **re-validates at execute time** (executor EXECUTE-TIME GUARD, already present). A cached proposal is a hint, never a decision. Verdict: correct already; ensure any new prepared proposal (recategorization) follows this.

### Delete All Data — [HAVE]
- `ResetBarrier` drain + `qc.clear()`. Every new prepared/cached result must die here — `qc.clear()` covers frontend caches; any deferred-commit backend path must take a `ResetBarrier` lease. Verdict: done; a standing invariant for all 7B work.

---

## Prioritized verdict (what actually gets built)

Consistent with Phase 7's finding that most flows are already cheap, the audit justifies a **small, generic** set:

1. **7B.2 — Centralized dependency-aware invalidation map.** Highest leverage: retires ~150 duplicated calls, fixes over/under-invalidation, unit-testable. Touches correctness app-wide.
2. **7B.3 — Prefetch-on-intent** (Sidebar hover/focus + account-row hover). Highest *visible* win: warms the 8–13-query burst before the click.
3. **Measurement harness (7B.4)** — perf marks + real-desktop capture, to prove 1–2 and to gate the audit-tagged maybes.
4. **Audit-gated, measure-first:** category-edit affected-count preview, bulk-recategorization proposal prepare, scenario deterministic preview, Copilot tool concurrency. Build **only** those the desktop measurement shows are slow enough to matter; otherwise DEFER with the number.

Everything else is **prefetch-covered or already instant** — no bespoke infrastructure, per the "no infra for sub-100ms flows" bar.
