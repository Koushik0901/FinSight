# FinSight Phase 7 — App-Wide Optimization Report

**Goal:** Make FinSight faster and more immediate without changing correct behavior — profile first, optimize bottlenecks by evidence, compute as early as it is safe, and never trade correctness for speed.

This report covers the work done in the app-wide optimization slice, building on the already-landed CSV import anticipatory pipeline (see `2026-07-04-csv-import-anticipatory-pipeline-design.md`). It records baselines, the bottlenecks found, the optimizations applied with before/after numbers, concurrency/cancellation guarantees, tests, cache-invalidation correctness, and remaining risks.

---

## 1. Method

- **Backend hot paths** measured with the existing criterion harness `crates/finsight-providers/benches/import_phases.rs` over the real `samples/amex-all-time-statement.csv` (~1988 rows), `sample_size(20)`.
- **Frontend hot paths** assessed by inspection of the React Query wiring (query keys, invalidation, refetch triggers), IPC payload shapes, and render/refetch triggers — not micro-benchmarks, since the cost there is IPC round-trips and re-renders, not CPU.
- Each change followed the per-slice rhythm already established in the repo: **baseline → change → after → tests → commit**.

---

## 2. Starting picture (from the import slice)

The CSV import pipeline was already made anticipatory (parse+reconcile moved off the Import click via a read-only `prepare()`), taking end-to-end import from **1.67 s → 198 ms** (~8.45×). That work left the **post-commit cascade** as the largest remaining on-click cost:

| Cascade step | Before |
|---|---|
| `categorize_builtin` | **223.6 ms** |
| `net_worth_backfill` | 36.5 ms |
| `pair_transfers` | 30.1 ms |
| `recompute_anomalies` | 20.1 ms |
| `net_worth_record_today` | 16.6 ms |
| **cascade total** | **≈327 ms** |

`categorize_builtin` alone was ~68% of the cascade — the clear next target.

---

## 3. Optimizations applied

### 3.1 Builtin categorization hot loop — `perf(categorize)`

**Bottleneck (evidence):** `apply_builtin_categorization` looped over every uncategorized/transfer row (~2000 on a fresh import) calling `tx.execute(<constant SQL>)`, which **re-parses/re-compiles the statement on every call** — the same N+1 statement-recompilation cost whose removal gave CSV import its 8.45× win. It also wrote `is_transfer` to *every* pending row even when the flag was unchanged.

**Fix:**
- `prepare_cached` the three hot statements (transfer-flag UPDATE, category UPDATE, categorizations INSERT) once and reuse the cached handles across all rows.
- Select the current `is_transfer` and only issue the flag write when it actually flips (the vast majority of rows are non-transfers already reading 0 — pure waste before).

**Result:** `categorize_builtin` **223.6 ms → 145.5 ms (−34.9%)**, ~78 ms off every import's cascade.
**Correctness:** all 17 `categorize` unit tests pass unchanged (behavior parity — the writes skipped were provably no-ops).

### 3.2 Transactions search debounce — `perf(transactions)`

**Bottleneck (evidence):** the transactions search box put `search` directly into the TanStack Query key with **no debounce**, so every keystroke fired a fresh `listTransactions` query + Tauri IPC round-trip + SQL `LIKE '%…%'` scan. Typing "starbucks" = 9 backend queries, 8 of them immediately superseded.

**Fix:** added a reusable `useDebouncedValue` hook and debounced the search term by 250 ms before it enters the query key. The `<input>` stays bound to the raw value, so typing is still instant; a burst of keystrokes collapses to a single trailing query. Date/preset filters are discrete and left un-debounced.

**Result:** a keystroke burst now issues **1 query instead of N**. Stale in-flight queries are superseded automatically by the query-key change (React Query drops the orphaned result — it can't overwrite newer state).
**Correctness:** 3 new `useDebouncedValue` unit tests (immediate initial value, deferred update, burst-collapse); existing `AccountTransactions` tests unchanged.

### 3.3 Delete-All has an airtight success boundary — `fix(reset)`

**Bottleneck / correctness gap (evidence):** the background agent had **no cancellation** at all, and a first best-effort fix (a monotonic epoch each writer checked between batches) still had a TOCTOU gap: a writer that had already passed its epoch check could commit *one more* batch/step **after** Delete-All returned success. The requirement is stronger: *once Delete-All reports success, no operation started against the previous ledger epoch may commit any observable user or derived state.*

**Fix — a drain barrier (`ResetBarrier`, in `finsight_core`, held on `Db`):**
- A monotonic **epoch** plus a shared/exclusive **gate**.
- A background writer snapshots the epoch when it starts and holds a shared `WriterLease` across the critical section that spans *both* its epoch re-check and its commit. `superseded()` (a cheap epoch compare) lets it bail promptly.
- `delete_all_data` calls `begin_reset()`, which advances the epoch and takes the **exclusive** guard. That guard **cannot be granted until every outstanding lease drains**, and it is held across the wipe.

Because shared and exclusive access are mutually exclusive, a writer's `(re-check → commit)` can never interleave with a reset's `(bump → wipe)`:
- If the writer holds the lease first, the reset blocks until the writer commits and drops it; the wipe then removes whatever was committed.
- If the reset holds the exclusive guard first, the writer's lease blocks until the wipe completes; the writer then re-checks the epoch, sees it advanced, and skips its commit.

Either way **nothing an operation started against the previous epoch can survive past the moment Delete-All reports success.** Applied to both real straddling writers:
- **Import pipeline** (`import_csv`): one lease held across the whole import + post-commit cascade (categorization/`ensure_default_categories`, transfer pairing, anomaly recompute, net-worth). A concurrent Delete-All drains it before wiping; if a reset already happened the import aborts up front.
- **Agent categorizer** (`run_job`): a lease around each commit unit (per rule-pass row, per LLM chunk), with the slow LLM call kept *outside* the lease so the drain stays fast (≤ one write).

`AgentHandle`'s ad-hoc `reset_epoch` was removed — the barrier on `Db` is the single source of truth, reachable by every writer via the `db` it already holds. Frontend `queryClient.clear()` on Delete-All still drops all cached/prepared/derived query state.

**Correctness / tests:**
- Barrier unit tests: a reset blocks until an in-flight lease drains; a lease taken after a reset sees the new epoch; a current-epoch lease is not superseded.
- End-to-end DB test (`reset.rs`): a writer that leased a category (a non-self-healing write, like `ensure_default_categories`) before a concurrent Delete-All — the wipe blocks on the lease, the writer sees `superseded()` and skips, and after completion **no pre-reset state survives**.
- Categorizer test: a custom provider triggers `begin_reset()` *during* the LLM call; the categorizer takes its write lease, sees the advanced epoch, and writes nothing — while the normal path still categorizes.

**Background-writer audit (so "no operation" is enumerated, not assumed).** Every writer that can be in flight when Delete-All is invoked — the scheduled/network/LLM-straddling ones especially — was reviewed and classified:

| Writer | Straddle | Inserts survive a wipe? | Handling |
|---|---|---|---|
| CSV import + post-commit cascade | file parse (100s ms) | categories/net-worth: **yes** | **leased** across import + cascade |
| Agent categorizer (rule + LLM) | LLM (s–min) | FK-guarded, mostly self-healing | **leased** per commit unit |
| SimpleFin sync — manual (`sync_local_account`) | network fetch (s) | transactions/accounts: **yes** | **leased** across commit |
| SimpleFin sync — **scheduled/batch** (`sync_scheduler`) | network fetch (s) | transactions: **yes** | **leased** across commit |
| SimpleFin account import | network fetch (s) | accounts: **yes** | **leased** across account-create loop |
| Recipe runner — background (`CheckDueRecipes`) | LLM (s) | action bundle: **yes** | **leased** across plan commit |
| Recipe command — manual (`trigger_recipe`) | LLM (s) | action bundle: **yes** | **leased** across plan commit |
| Copilot chat turn (`stream_copilot_message`) | reasoning LLM loop (s–min) | action bundle: **yes** | **leased** across bundle commit |
| Action executor (approved bundles) | user-gated | `UPDATE … WHERE id` (0 rows post-wipe) + FK-guarded `categorizations` | self-healing; not leased |
| `detect_anomalies` writes | LLM (s) | `UPDATE … WHERE id` only | self-healing; prompt-skip on `superseded()` |
| Single-statement user mutations (add/edit/delete, category ops) | none (atomic) | n/a | atomic + user-serialized; barrier API available if ever needed |

The scheduled SimpleFin sync was the one genuinely-concurrent writer with a long straddle and *unguarded top-level inserts* — the sharpest case the boundary must cover — and is now leased on both its manual and batch paths.

### 3.4 Disable `refetchOnWindowFocus` — `perf(query)`

**Bottleneck (evidence):** the `QueryClient` set `staleTime`/`retry` but left `refetchOnWindowFocus` at its default (**true**). FinSight is a local-first desktop app; every time the user tabbed away and back, *every* mounted query older than `staleTime` refetched — a burst of Tauri IPC calls + SQL across the whole active query set — for data that only changes via in-app actions (which already invalidate precisely).

**Fix:** `refetchOnWindowFocus: false` in the default query options.
**Result:** window focus no longer replays the active query set. No staleness introduced — the SQLite ledger has no external writer; imports/mutations/sync all invalidate on success. 298 FE tests still pass.

### 3.5 Account-scoped anomaly recompute — `perf(anomaly)`

**Bottleneck (evidence):** `recompute_anomalies` cleared *every* flag and re-ran the full median/MAD grouping over the *entire* ledger on each import, even though an import into one account can only shift the merchant groups present in that account.

**Fix (correctness-gated scoping):** `recompute_anomalies_for_account(account_id)` recomputes only the groups with a member in the imported account and clears flags only on those groups' rows, leaving every other merchant's flags untouched. The in-scope key set is built inline during the single load pass (no extra query, merchants normalized once), and the scoped clear is proportional to the small set of *currently-flagged* rows in those groups. The import cascade now calls this variant. The authoritative full `recompute_anomalies` is unchanged and still used by the manual "recompute anomalies" command.

**Correctness (the gate):** a two-DB equivalence test builds byte-identical ledgers (two accounts, a shared cross-account merchant, an untouched B-only merchant with a pre-existing flag) and diverges only at the final call — full vs scoped — asserting the resulting `is_anomaly`/`ai_explanation` match **row-for-row**, and that the untouched merchant's flag survives the scoped pass.

**Result (measured, single-account amex = worst case for scoping):** full **43.4 ms** vs scoped **43.8 ms** — statistically identical (an initial scoped prototype regressed +19% from a redundant `DISTINCT` query + double normalization; eliminating those made it neutral). On multi-account ledgers the scoped pass skips every untouched group, so it is strictly cheaper as account count grows. Net: no regression on the common case, real upside on the multi-account case the plan's "scope to the affected window" targets.

---

## 4. Areas profiled and found already-healthy (no change needed)

Evidence-based decisions **not** to change things (avoiding speculative churn):

- **`listTransactions` query** — single query with `LEFT JOIN`s (no N+1), `ORDER BY posted_at DESC LIMIT/OFFSET`, backed by `idx_transactions_account_posted (account_id, posted_at DESC)` from `V027`. The list is already paginated (50/page via `useInfiniteTransactions`); it never loads thousands of rows at once. The real cost was the undebounced search (3.2), now fixed.
- **Copilot context (`context.rs::build_context`)** — already context-packs **summaries/aggregates** (SUM/GROUP BY over merchants, categories, budgets) rather than shipping raw transaction payloads over IPC, satisfying the plan's "avoid excessive raw transaction payloads / context-pack backend summaries." Queries share one connection and are inherently sequential; parallelizing would add SQLite read contention for little gain.
- **Reports/dashboard hooks** — aggregation is done backend-side (`getMonthTotals`, `getSavingsRateHistory`) with 60 s `staleTime`; no client-side full-table recompute or refetch storm.
- **Post-commit cascade concurrency** — deliberately left sequential. SQLite is single-writer; running the cascade steps concurrently would contend on the write lock (risking `SQLITE_BUSY`), not speed up. `pair_transfers` is ordering-dependent on the keyword pass and cannot move regardless. The lever here is doing *less* work (scoping), which is deferred (see §7).

- **`net_worth::backfill_history_from_transactions` — measured, left as-is.** It looked like an O(months) N-query recompute (one `SUM(... WHERE date > month_end)` per month), so I prototyped a single grouped `strftime('%Y-%m', …) GROUP BY` scan + in-memory prefix sums, guarded by a reference-equivalence test (gap month + future-dated rows) that passed byte-for-byte. **But the bench regressed 36.5 ms → 88.7 ms (+142%).** `strftime`/`GROUP BY` on `posted_at` can't use the `(account_id, posted_at)` index and pays a per-row function cost, whereas the per-month queries are cheap indexed range scans. Reverted — a clean reminder that the plan's "profile first" is not optional: the assumed bottleneck was already index-optimal.

---

## 5. Concurrency, cancellation & staleness guarantees

- **Stale results can't overwrite newer state:** transaction search relies on TanStack Query's query-key versioning — a superseded query's result is dropped, not written back. The import prepare preview is keyed on `(path, accountId, mapping)` and re-keys on any edit.
- **Delete-All has an airtight boundary:** the `ResetBarrier` drain (§3.3) guarantees no operation started against the previous epoch commits after Delete-All returns success — the wipe drains outstanding writer leases and holds the exclusive gate across itself; late writers observe the advanced epoch and skip. Frontend `queryClient.clear()` drops all cached/prepared/derived query state.
- **No early mutation:** the categorization guard aborts *before* the next write, never mid-write; SQLite serializes any single in-flight statement, so no torn writes.
- **Deterministic output preserved:** the categorize change only removed provably-redundant writes (skipped flag writes were no-ops; cached statements are byte-identical SQL).

---

## 6. Tests / green bar

- New tests: `useDebouncedValue` (3), categorizer reset-cancellation (1), anomaly scoped-vs-full equivalence (1).
- `cargo test --workspace`: **341 passed, 0 failed, 9 ignored** (was 335; +3 `ResetBarrier` ordering, +1 end-to-end reset-drain, +1 categorizer reset-during-LLM, +1 anomaly equivalence — the earlier interim reset test was replaced, not added). `finsight-app` suite (incl. import command) green after the drain-barrier wiring.
- `ui` vitest: **298 passed** (was 295; +3 debounce).
- `tsc --noEmit`: **0 errors**.
- No Tauri command signatures changed → **no bindings regeneration required** (only internal fn signatures changed).

---

## 7. Remaining risks / deferred work

- **Post-commit cascade scoping (Task 7 / D4) — done for anomalies, retired for net-worth.** `recompute_anomalies` is now account-scoped on import (§3.5), equivalence-tested and benchmark-neutral on single-account with multi-account upside. `net_worth::backfill_history_from_transactions` was investigated and found **already index-optimal** — the single-query rewrite regressed it (§4), so it is *not* a fruitful scoping target and is intentionally left as-is. This closes the D4 scoping item.
- **Reset boundary — now airtight (was a residual).** The earlier best-effort epoch had a one-batch TOCTOU window; the `ResetBarrier` drain (§3.3) closes it for both straddling writers (import pipeline, categorizer): the wipe drains outstanding leases and holds the exclusive gate across itself, so no operation started against the previous epoch can commit after Delete-All returns success. Remaining boundary note: single-statement synchronous user mutations (manual add/edit/delete) are not leased — atomic and not concurrently initiable with Delete-All by one user — but the barrier API is available to wrap them if that ever changes.
- **No-op-write claim verified:** skipping unchanged `is_transfer` writes (§3.1) is exactly a no-op because a `grep` of all migrations confirms **no triggers exist on `transactions`** (or any table) — so a value-unchanged UPDATE had no observable side effect to lose.
- **Search `LIKE '%…%'`** remains a scan (un-indexable prefix wildcard); fine at current dataset sizes with pagination + debounce. FTS would be the move only if datasets grow large.
