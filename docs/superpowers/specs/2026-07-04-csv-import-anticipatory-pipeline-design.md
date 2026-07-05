# CSV Import — Anticipatory Prepare/Commit Pipeline (Phase 7, Slice 1)

**Date:** 2026-07-04
**Status:** Approved (design)
**Scope:** CSV import path only. Other Phase 7 slices (dashboard/reports read path, startup, Copilot latency) are separate specs.

## Context

Phase 7 goal: make FinSight faster and feel immediate **without changing correct behavior** — "compute as early as safely possible." Profile first, optimize the proven bottleneck, never trade correctness for speed.

The current CSV import (`crates/finsight-providers/src/csv/mod.rs::CsvProvider::import`, driven by `crates/finsight-app/src/commands/import.rs::import_csv`) does **all** work on the final Import click:

1. Re-reads + re-decodes the file (already read once during `preview_csv_columns`).
2. First pass over the reader **counts every row** (for progress).
3. Second pass parses each row and **reconciles it against the DB one row at a time** (`reconcile_excluding_batch` → `find_fuzzy_candidates`, which calls `conn.prepare(...)` per row — N+1 with per-row statement re-prepare).
4. Inserts / queues-for-review in batched write transactions.
5. **Post-commit cascade**, each on its own `run()` connection, sequentially: `apply_builtin_categorization`, `pair_transfers`, `anomaly::recompute_anomalies`, `net_worth::record_today` + `net_worth::backfill_history_from_transactions`, then enqueues the AI categorizer job.

Nothing is prepared ahead; the user waits through parse + reconcile + the full-history cascade after clicking Import.

### Working-tree note
A large uncommitted change set (V039 liabilities→accounts unification, ~41 files) is in flight and is **out of scope**. Work around it; do not commit or refactor those files unless a perf change strictly requires it. `import.rs` is not in that set, though it writes through repos that are.

## Guiding principles for this slice

**Profile-first (hard gate).** The optimization target is chosen by measured per-phase numbers, not assumption. The post-commit cascade (`recompute_anomalies`, `net_worth backfill`) reads like full-history passes and is a plausible *dominant* cost of import latency; the anticipatory parse may move the needle little. We measure before committing to a target.

**Two goals, kept separate:**
- **Goal A — surface the outcome early.** As soon as `(path + mapping)` are known, run parse + validate + reconcile speculatively (read-only) and show the real result — *"N new · D duplicates · R to review · E errors"* — before the user clicks Import. Non-destructive, safe, high UX payoff.
- **Goal B — make commit cheap.** Commit **always re-runs reconciliation authoritatively inside the write transaction**. A pre-staged plan is reused only as a *guarded optimization* gated by a signature match; otherwise the pre-staged result is a preview estimate only. Authoritative-with-optional-reuse, never reuse-with-fallback.

## Correctness constraints (bake into implementation)

1. **Reconciliation is order-dependent — never parallelize the row fold.** `matched_existing_ids` and `self_import_ids` accumulate across rows (`matcher.rs:169,185`); row N depends on rows 1..N-1. Pre-staging must pre-generate the would-be-inserted UUIDs and thread the same accumulator state in the same order, producing **identical** dedup outcomes to the current path. Verify parity against existing import tests + sample files.
2. **Speculative work stays non-destructive until confirmation.** Prepare only reads; no mutations, no `imports` row, no mapping save until commit.
3. **Staleness → re-run, not stale reuse.** Signature = hash of `(account_id, mapping, file mtime+size)` + a cheap per-account ledger fingerprint (`count + max(created_at)` of that account's transactions). Commit reuses the plan only on exact signature match; otherwise re-runs reconciliation authoritatively.
4. **Cancellation is cooperative (version-and-discard).** `spawn_blocking` cannot be preempted; at max sample size (~2000 rows) it does not need to be. Superseded/cancelled prepares are discarded by version, never allowed to overwrite current state. Add cooperative cancel-checks in the row loop only if per-phase numbers show large-file prepares actually block.
5. **Invalidation of prepared/derived state:**
   - Mapping edits (columns / skip-header / flip / split) supersede the in-flight prepare via request/version id (React Query key includes a mapping-hash → dedup + supersession for free).
   - Dialog close, **Delete All Data**, and SimpleFIN sync must invalidate any prepared plan and derived caches. If the plan lives in a backend cache keyed by `(path, mtime, mapping-hash)`, Delete clears it; if it lives in React Query, Delete invalidates that query.

## Deliverables

### D1 — Phase-attributed benchmark harness (FIRST; decides the rest)
Criterion bench over real `samples/` CSVs attributing time to each phase separately: read+decode · parse · reconcile · insert · and each post-commit step (`categorize`, `pair_transfers`, `anomalies`, `net_worth backfill`).
- Mutating benches: `iter_batched` with a **fresh seeded temp DB per iteration** (migrations applied + seed), else iteration 2+ measures the all-duplicates path.
- Non-mutating benches (parse-only, prepare-only against a fixed DB): may iterate normally.
- Output: documented baseline table (before) committed to the spec/docs. The numbers pick the primary optimization target.

### D2 — Anticipatory prepare (Goal A)
- New capability producing `PreparedImport { signature, counts, per-row decisions, per-row errors }` from `(path, mtime, mapping)`, reusing the already-decoded/parsed file content where possible.
- Sequential deterministic reconcile fold with pre-generated UUIDs; parity with current path asserted.
- Surfaced to the dialog as a live "what will happen" summary before Import is clicked.

### D3 — Cheap wins (valid in both old and new paths, low risk)
- `find_fuzzy_candidates`: `conn.prepare` → `prepare_cached` (kill per-row re-prepare).
- Commit reuses cached parse output so the file is not re-read and the separate count-pass disappears.

### D4 — Cascade fix (only if D1 confirms it dominates)
- Scope `recompute_anomalies` / `net_worth backfill` to the affected account / date-range instead of full history.
- Run genuinely-independent post-commit steps concurrently with bounded orchestration; preserve required dependency order (`pair_transfers` after the keyword categorization pass, etc.).

### D5 — Frontend wiring
- `usePrepareImport` keyed by `(path, mapping-hash)`; debounced on mapping edits; superseded prepares discarded.
- Dialog shows the prepared outcome; Import commits (authoritative), reusing the plan when the signature still matches.
- Delete-All / sync invalidation wired.

## Testing

- **Parity:** pre-staged decisions equal the current sequential path across all `samples/` files and existing import unit tests.
- **Staleness:** mutate the ledger between prepare and commit → commit re-runs (asserted), does not reuse a stale plan.
- **Invalidation:** mapping churn supersedes prepare; Delete-All mid-prepare clears prepared + derived state; cancelled dialog discards.
- **Edge:** empty file, all-duplicates re-import, large file, rapid mapping changes, repeated Import clicks (idempotent / guarded).
- **Green bar preserved:** 324 Rust / 297 FE / 0 TS, plus new tests. Before/after bench numbers documented.

## Out of scope
- Other Phase 7 slices (dashboard/reports, startup, Copilot) — separate specs.
- Refactoring V039 WIP files beyond what a perf change strictly requires.
- True preemptive cancellation of `spawn_blocking`.

## Open question resolved by D1
Whether the primary win is the anticipatory prepare (D2) or the post-commit cascade scoping (D4). Both are specified; D1's numbers set priority. D3 ships regardless.
