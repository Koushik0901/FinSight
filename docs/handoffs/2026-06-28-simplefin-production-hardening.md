# SimpleFIN Production Hardening Handoff

Date: 2026-06-28

This handoff documents the production hardening pass that followed the initial SimpleFIN repair and CSV/SimpleFIN coordination work. It focuses on durable import review, safer reconciliation, hardened background sync, drift correctness, and future-agent operating rules.

Design spec: `docs/superpowers/specs/2026-06-28-simplefin-production-hardening-design.md`
Prior handoff: `docs/handoffs/2026-06-28-simplefin-import-coordination.md`

## Completed work

### Durable reconciliation workbench

- Added migration `crates/finsight-core/migrations/V026__import_reconciliation_workbench.sql`.
- Added `import_candidates` for incoming CSV/SimpleFIN transactions that should not be automatically inserted or merged.
- Added `import_candidate_matches` for possible existing transaction matches with scores, match kind, explanations, and recommended-match flag.
- Added core models in:
  - `crates/finsight-core/src/models/import_candidate.rs`
  - `crates/finsight-core/src/models/sync_run.rs`
- Added repositories in:
  - `crates/finsight-core/src/repos/import_candidates.rs`
  - `crates/finsight-core/src/repos/sync_runs.rs`
- Added Tauri commands in `crates/finsight-app/src/commands/simplefin.rs`:
  - `list_import_review_candidates`
  - `accept_import_candidate_match`
  - `create_import_candidate_transaction`
  - `dismiss_import_candidate`
- Regenerated `ui/src/api/bindings.ts`.
- Added frontend hooks in `ui/src/api/hooks/simplefin.ts`.
- Added Inbox Import Review cards/actions in `ui/src/screens/Inbox.tsx`.

### Reconciliation behavior

- `crates/finsight-providers/src/simplefin/matcher.rs` now supports a higher-level reconciliation decision:
  - automatic exact provider/import ID match;
  - pending-to-posted provider match;
  - amount-tolerant fuzzy scoring;
  - confidence thresholding;
  - collision detection within a batch;
  - `NeedsReview` outcomes for ambiguous/colliding/medium-confidence candidates.
- CSV import in `crates/finsight-providers/src/csv/mod.rs` now queues review candidates instead of forcing every fuzzy match into skip/insert.
- SimpleFIN sync in `crates/finsight-providers/src/simplefin/sync.rs` now queues review candidates and reports `queued_for_review`.
- Workbench resolution preserves user-owned fields when matching against an existing transaction.

## Sync hardening

### Scheduler lifecycle

`crates/finsight-app/src/sync_scheduler.rs` now separates:

- background sync enabled/disabled state;
- scheduler shutdown state;
- sync-in-progress guard.

Turning background sync Off pauses the loop instead of killing the task. Turning it On resumes the loop. Manual Sync all and background sync share the same guard, preventing overlapping sync jobs.

### Retry/backoff

SimpleFIN fetch now retries transient errors with exponential backoff:

- 1s
- 2s
- 4s
- 8s

It does not retry unrecoverable errors such as revoked/auth access, payment required, invalid access URL, or missing credentials. After exhausted retry or unretryable failure, the connection is marked error and a durable sync alert is created.

### Sync run audit

The new `sync_runs` table records:

- trigger (`manual`, `background`, `initial`);
- status (`running`, `success`, `partial`, `failed`);
- account success/failure totals;
- added/updated/skipped/queued counts;
- error summary.

This makes background sync observable after the immediate command response is gone.

## Balance drift correctness

The old `account_balances` primary key allowed one balance row per `(account_id, as_of_date)`, so a ledger recompute could overwrite the SimpleFIN bank snapshot for the same day. V026 rebuilds the table with primary key `(account_id, as_of_date, source)`.

Current balance-source conventions:

- `simplefin`: bank-reported provider snapshot.
- `ledger_recomputed`: local ledger sum after transaction changes/imports.
- `manual`: manually seeded or manually entered balance.

`crates/finsight-providers/src/simplefin/drift.rs` now compares ledger totals only against the latest `source = 'simplefin'` snapshot.

## Holdings freshness

SimpleFIN sync now refreshes account provider metadata (`extra_json`, `raw_json`, available balance, balance date) from the fresh payload before holdings import. Investment holdings import uses that fresh payload instead of stale account metadata.

Key files:

- `crates/finsight-providers/src/simplefin/sync.rs`
- `crates/finsight-providers/src/simplefin/holdings.rs`
- `crates/finsight-app/src/sync_scheduler.rs`

## UI behavior

Inbox now has an Import Review section when pending candidates exist. Each candidate shows:

- source (`CSV` or `SimpleFIN`);
- confidence;
- amount/date/merchant;
- reason it needs review;
- recommended match and alternative matches.

Available actions:

- Accept match.
- Select an alternative match.
- Create new transaction.
- Dismiss candidate.

Monetary amounts use `className="money"` where applicable so privacy mode still works.

## Behavioral contract for future agents

- Do not bypass the shared reconciliation engine for CSV or SimpleFIN transaction imports.
- Automatic matching should remain limited to exact IDs or high-confidence, non-colliding fuzzy matches.
- Ambiguous, colliding, or medium/low-confidence candidates must be persisted to Import Review rather than silently inserted or merged.
- Matching an import candidate to an existing transaction may enrich provider-owned metadata, but must not overwrite user-owned fields such as notes/categories/flags.
- Keep SimpleFIN credentials in the OS keychain only. Do not store setup tokens or access URLs in docs, source, logs, or tests.
- Drift checks must use the latest `source = 'simplefin'` bank snapshot, not the latest balance row of any source.
- Manual/background sync must continue sharing the same overlap guard.
- If new providers are added, reuse or generalize the workbench/reconciliation flow instead of adding provider-specific duplicate logic.

## Validation run

- `cargo check -p finsight-core -p finsight-providers -p finsight-app`
  - Passed.
- `cargo run -p finsight-tauri --bin export_bindings`
  - Passed and regenerated `ui/src/api/bindings.ts`.
- `cargo test -p finsight-core migrations`
  - Passed: 2 migration tests.
- `cargo test -p finsight-providers`
  - Passed: 40 provider tests, 4 CSV integration tests, 1 ignored HTTPS/wiremock SimpleFIN client test.
- `cargo test -p finsight-app`
  - Passed.
- `cd ui && npx tsc --noEmit`
  - Passed.
- `cd ui && npx vitest run src/screens/Inbox.test.tsx src/screens/Settings.test.tsx src/screens/Accounts.test.tsx`
  - Passed: 22 tests.

## Important caveats

- A large number of unrelated modified/untracked files existed before this documentation update. Do not revert unrelated user/agent work.
- `V024__simplefin_connections.sql` and `V025__simplefin_phase4.sql` are still untracked in the current working tree alongside the new V026 migration.
- Latest migration is V026. Next migration should be `V027__description.sql`.
- The selected IDE `.env` value looked like a real API key. It should be rotated if real and `.env` should remain untracked.
