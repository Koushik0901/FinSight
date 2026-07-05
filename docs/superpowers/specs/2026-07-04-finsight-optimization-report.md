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

### 3.3 Delete-All cancels in-flight categorization — `fix(reset)`

**Bottleneck / correctness gap (evidence):** the background agent had **no cancellation**. A `CategorizeAll` job already running (or queued) when the user hit *Delete All Data* would keep writing categorizations — and orphan `categorizations` rows — against the freshly-wiped ledger. The plan explicitly requires "Delete must cancel work … never reuse deleted/stale data."

**Fix (version/epoch pattern the plan calls for):**
- `AgentHandle` now holds a monotonic `reset_epoch: Arc<AtomicU64>`.
- `delete_all_data` calls `agent.cancel_running_work()` (bumps the epoch) **before** wiping the DB.
- **Writer 1 — the agent categorizer:** `categorizer::run_job` snapshots the epoch at start and checks it at every batch boundary — per-row in the rule pass, per-chunk in the LLM pass — aborting early (before the next write) if it changed.
- **Writer 2 — the import post-commit cascade:** `import_csv` snapshots the epoch before touching the ledger and re-checks before each cascade step (`apply_builtin_categorization`/`ensure_default_categories`, `pair_transfers`, `recompute_anomalies`, `net_worth`). A Delete-All landing during an import therefore can't re-seed default categories or repopulate derived state into the wiped ledger.

**Result:** both background writers that could mutate state after a reset are now cancelled. Combined with the frontend's existing `queryClient.clear()` on Delete-All (which drops *all* cached queries incl. `csv-prepare`, transactions, and every derived/report/chart cache), the client cache and both backend writers are invalidated/cancelled by a wipe.
**Correctness:** new test simulates a reset landing between the rule and LLM passes and asserts no categorization is written; the normal (non-cancelled) path still categorizes.

### 3.4 Disable `refetchOnWindowFocus` — `perf(query)`

**Bottleneck (evidence):** the `QueryClient` set `staleTime`/`retry` but left `refetchOnWindowFocus` at its default (**true**). FinSight is a local-first desktop app; every time the user tabbed away and back, *every* mounted query older than `staleTime` refetched — a burst of Tauri IPC calls + SQL across the whole active query set — for data that only changes via in-app actions (which already invalidate precisely).

**Fix:** `refetchOnWindowFocus: false` in the default query options.
**Result:** window focus no longer replays the active query set. No staleness introduced — the SQLite ledger has no external writer; imports/mutations/sync all invalidate on success. 298 FE tests still pass.

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
- **Delete-All cancels + invalidates:** frontend `queryClient.clear()` (all caches, prepared plans, derived state) + backend reset-epoch cancels **both** post-wipe writers — the agent categorizer and the import post-commit cascade.
- **No early mutation:** the categorization guard aborts *before* the next write, never mid-write; SQLite serializes any single in-flight statement, so no torn writes.
- **Deterministic output preserved:** the categorize change only removed provably-redundant writes (skipped flag writes were no-ops; cached statements are byte-identical SQL).

---

## 6. Tests / green bar

- New tests: `useDebouncedValue` (3), categorizer reset-cancellation (1).
- `cargo test --workspace`: **336 passed, 0 failed, 9 ignored** (was 335; +1 categorizer reset-cancellation test). `finsight-app` suite (incl. import command) green after the cascade guard.
- `ui` vitest: **298 passed** (was 295; +3 debounce).
- `tsc --noEmit`: **0 errors**.
- No Tauri command signatures changed → **no bindings regeneration required** (only internal fn signatures changed).

---

## 7. Remaining risks / deferred work

- **Post-commit cascade scoping (Task 7 / D4, partially retired):** `net_worth::backfill_history_from_transactions` was investigated and found to be **already index-optimal** — the single-query rewrite regressed it (see §4), so it is *not* a fruitful scoping target. That leaves `recompute_anomalies` (~20 ms, full-history merchant grouping): a scoped variant would need the imported merchant set and must recompute each touched merchant's *whole* group (one new row shifts that merchant's median/MAD) while leaving untouched merchants' flags intact — with an equivalence test as the gate. At ~20 ms it is low priority; deferred.
- **Reset race residual:** a categorization chunk (or a single cascade step / the import write itself) already *past* its epoch check when the wipe lands can complete that one unit of work. The window is one batch/step; FK + `UPDATE`-0-rows behavior makes it self-healing. A fully preemptive cancel would require aborting the in-flight LLM future / `spawn_blocking` task, which is out of scope.
- **No-op-write claim verified:** skipping unchanged `is_transfer` writes (§3.1) is exactly a no-op because a `grep` of all migrations confirms **no triggers exist on `transactions`** (or any table) — so a value-unchanged UPDATE had no observable side effect to lose.
- **Search `LIKE '%…%'`** remains a scan (un-indexable prefix wildcard); fine at current dataset sizes with pagination + debounce. FTS would be the move only if datasets grow large.
