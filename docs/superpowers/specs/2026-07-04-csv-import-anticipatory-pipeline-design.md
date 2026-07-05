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

## Baselines (before)

Measured with `cargo bench -p finsight-providers --bench import_phases` (criterion 0.5.1, `sample_size(20)`, plotters backend — Gnuplot not installed) over `samples/amex-all-time-statement.csv` (~1988 rows, 1976 imported after the sanity check `rows_imported > 1000` passed on every setup run). Bench source: `crates/finsight-providers/benches/import_phases.rs`.

Final mapping used (had to be worked out from the raw file, not assumed): `skip_header_rows: 1`, columns `[Date, Skip, Merchant, Amount]` (column 2, "Date Processed", is unused), `date_format: "%d %b %Y"`, `amount_convention: PositiveIsOutflow` (charges are positive/outflow, payments/credits negative — the inverse of the common US-bank convention), `decimal_separator: '.'`, `delimiter: None`. This mapping was validated end-to-end; no parse errors on the file, comfortably over 1000 rows imported.

| Phase | Median | What it measures |
|---|---|---|
| `read_decode` | 39.924 µs | File read + layered decode (BOM sniff → UTF-8 → Windows-1252 fallback), no parsing |
| `parse_only` | 1.2854 ms | Parsing every data row into `ParsedRow` via the real `parse_row`, no I/O, no DB |
| `import_amex_full` | 1.6734 s | End-to-end `CsvProvider::import`: read + decode + parse + reconcile + insert + batched commits, against a fresh seeded DB per iteration |
| `categorize_builtin` | 245.54 ms | `categorize::apply_builtin_categorization`, run once against a DB that already has the amex import committed |
| `pair_transfers` | 30.211 ms | `categorize::pair_transfers`, same setup |
| `recompute_anomalies` | 21.414 ms | `anomaly::recompute_anomalies`, same setup |
| `net_worth_backfill` | 37.736 ms | `repos::net_worth::backfill_history_from_transactions`, same setup |
| `net_worth_record_today` | 16.736 ms | `repos::net_worth::record_today`, same setup |

Not isolated: **`reconcile_only`**. `CsvProvider::import` interleaves reconcile with insert/commit inside a single loop over one open `rusqlite::Transaction`, and there is no public entry point that stops right after reconciliation without also inserting. Isolating this cleanly requires the read-only `prepare()` from D2, which this benchmarking task must not add (no product `pub` visibility changes allowed here). Per-phase math below substitutes for a direct measurement.

### Verdict

**Important correctness note on the arithmetic:** `CsvProvider::import` (the `import_amex_full` bench) does **not** call the post-commit cascade steps — `apply_builtin_categorization`, `pair_transfers`, `recompute_anomalies`, and the two `net_worth` functions all run *after* `import()` returns, from `import_csv` in `finsight-app`, each on its own connection. So `import_amex_full` already *is* read+decode+parse+reconcile+insert in full; the cascade is a separate, additive cost on top of it, not a component to subtract out of it.

Since `read_decode` (0.04 ms) and `parse_only` (1.29 ms) are both negligible next to the 1673 ms end-to-end number, essentially the entire `import_amex_full` time — **≈1671 ms** — is attributable to reconcile+insert (per-row `reconcile_excluding_batch` against the DB, plus batched writes). That is the dominant cost inside the import step itself.

Comparing the two additive stages of the full user-visible operation (import, then cascade):
- Import (read+decode+parse+reconcile+insert): **1673.4 ms**
- Post-commit cascade (sum of the 5 steps above): **351.6 ms** (245.54 + 30.211 + 21.414 + 37.736 + 16.736)
- Combined: 2025.0 ms → import is **~83%** of total, cascade is **~17%**

**Verdict: the post-commit cascade does NOT dominate.** Reconcile+insert inside `CsvProvider::import` is the larger share by a wide margin (~1671 ms vs ~352 ms, roughly 4.8x). This points D2 (anticipatory prepare, moving parse+reconcile off the commit-time critical path) as the higher-leverage target over D4 (cascade scoping). D4 still has some value (352 ms is not nothing, and `pair_transfers`/`recompute_anomalies` numbers here are likely underestimates of production cost since this DB has only a single account with only the amex import in it — e.g. `pair_transfers` has no second account to pair against, so its 30 ms is mostly scan overhead rather than real transfer-pairing work), but it is not the D1 gate for prioritizing D2 first.
