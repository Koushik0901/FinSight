# SimpleFIN Production Hardening and Import Reconciliation Workbench Design

Date: 2026-06-28
Status: Approved
Scope: Production hardening for SimpleFIN sync plus a full CSV/SimpleFIN reconciliation workbench inspired by Actual Budget-style safety.

## Context

FinSight already has a substantial SimpleFIN integration: per-connection credentials, account import, transaction sync, CSV/SimpleFIN duplicate matching, batch/background sync, drift alerts, transfer suggestions, and investment holdings modules. A review found several gaps that keep it from being production-grade:

- Retry/backoff is described in docs but not implemented in the scheduler.
- The background scheduler uses one flag for both "enabled" and "task should keep running", so turning sync off can stop the task permanently.
- Fuzzy matching requires exact amount equality and therefore does not handle minor amount noise.
- Balance drift can compare the ledger to a recomputed ledger snapshot instead of the latest bank-reported SimpleFIN snapshot.
- Holdings import can read stale `extra_json` instead of the fresh account payload fetched during sync.
- Sync failures mark connection status but do not create durable Inbox alerts.
- Ambiguous import matches have no durable review queue; the system must either skip, update, or insert immediately.

This design fixes those gaps and adds a reconciliation workbench so high-confidence matches remain automatic while ambiguous or colliding imports become durable, reviewable tasks.

## Goals

1. Make SimpleFIN manual and background sync reliable, observable, retryable, and restartable.
2. Keep CSV import and SimpleFIN sync coordinated through one reconciliation engine.
3. Avoid duplicate transactions while preserving user-owned fields.
4. Add a durable import review workflow for ambiguous matches, collisions, and low-confidence candidates.
5. Keep bank-reported balances, ledger-recomputed balances, and drift alerts logically separate.
6. Import investment holdings from the freshest SimpleFIN account payload available in the sync run.
7. Surface sync failures as actionable Inbox alerts.

## Non-goals

- No OS-level background wakeups. Background sync runs only while the app process is open.
- No automatic destructive merge of ambiguous candidates.
- No storage of SimpleFIN setup tokens or access URLs outside the OS keychain.
- No broad provider abstraction rewrite. The design improves the current SimpleFIN and CSV surfaces first.

## Architecture

### Sync scheduler

`crates/finsight-app/src/sync_scheduler.rs` remains the orchestrator for background and manual sync, but its state model changes:

- `enabled`: whether background sync should run.
- `shutdown`: whether the scheduler task should exit.
- `sync_guard`: prevents overlapping background/manual sync jobs.
- `interval_minutes`: configurable interval stored in settings.

Turning sync Off pauses the loop without killing the task. Turning sync On resumes it. Manual Sync all uses the same sync guard so it cannot overlap with a background run.

The scheduler records every batch run in `sync_runs` and records per-account outcomes in the returned `AccountSyncResult`.

### Provider fetch and commit pipeline

SimpleFIN fetch returns both transactions and the fresh account payload. The commit phase runs in a DB transaction and:

- writes the latest bank balance snapshot with `source = 'simplefin'`;
- refreshes account `extra_json` and `raw_json` from the fetched payload;
- imports holdings from that fresh payload for investment accounts;
- routes all transaction candidates through the reconciliation engine;
- runs transfer detection and drift detection after successful transaction commit.

### Reconciliation engine

The reconciliation engine is shared by CSV import and SimpleFIN sync. It accepts a normalized incoming transaction candidate and returns one of:

- `AutoInsert`: no plausible existing match.
- `AutoMatch`: exact or high-confidence, non-colliding match.
- `NeedsReview`: medium/low confidence, collision, or ambiguous alternatives.
- `RejectDuplicate`: exact duplicate with no update needed.

Matching order:

1. Exact imported/provider ID on the same account.
2. Pending-to-posted match for the same provider account.
3. Same-account fuzzy scoring using amount, date proximity, merchant/payee similarity, and source metadata.
4. Collision and ambiguity checks across the current batch.

Amount matching supports a small configurable tolerance for fuzzy candidates. Exact ID remains the only zero-risk match; fuzzy auto-match requires a high confidence score and no collision.

### Reconciliation workbench

The workbench persists unresolved candidates and possible matches. It is surfaced in Inbox as an Import Review section. Users can:

- accept the recommended match;
- choose an alternative match;
- create a new transaction;
- dismiss a candidate;
- bulk approve only high-confidence, non-colliding candidates.

Workbench actions are explicit and auditable. They do not silently overwrite user-owned fields.

### Post-sync processors

Post-sync processors remain separate modules:

| Processor | Responsibility |
|---|---|
| Transfer detection | Suggest likely transfers across linked accounts. |
| Holdings import | Upsert securities and daily holdings snapshots from fresh SimpleFIN payloads. |
| Drift detection | Compare ledger sum to latest `source = 'simplefin'` bank balance snapshot. |
| Sync alerts | Create Inbox alerts for auth, payment, missing keychain credential, transient failure exhaustion, and drift. |

## Data model

Create migration `V026__import_reconciliation_workbench.sql`.

### `import_candidates`

Durable unresolved incoming transactions.

Columns:

- `id TEXT PRIMARY KEY`
- `source TEXT NOT NULL CHECK(source IN ('csv', 'simplefin'))`
- `import_id TEXT`
- `sync_run_id TEXT`
- `account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE`
- `candidate_json TEXT NOT NULL`
- `raw_payload_json TEXT`
- `imported_id TEXT`
- `external_tx_id TEXT`
- `external_account_id TEXT`
- `posted_at TEXT NOT NULL`
- `amount_cents INTEGER NOT NULL`
- `merchant_raw TEXT NOT NULL`
- `confidence INTEGER NOT NULL DEFAULT 0`
- `reason TEXT NOT NULL`
- `status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'resolved', 'dismissed'))`
- `resolution TEXT`
- `resolved_transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL`
- `created_at TEXT NOT NULL`
- `resolved_at TEXT`

Indexes:

- `(status, created_at DESC)`
- `(account_id, status)`
- `(source, status)`
- `(sync_run_id)`

### `import_candidate_matches`

Possible matches for each unresolved candidate.

Columns:

- `id TEXT PRIMARY KEY`
- `candidate_id TEXT NOT NULL REFERENCES import_candidates(id) ON DELETE CASCADE`
- `transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE`
- `match_kind TEXT NOT NULL`
- `score INTEGER NOT NULL`
- `is_recommended INTEGER NOT NULL DEFAULT 0`
- `explanation_json TEXT`
- `created_at TEXT NOT NULL`

Indexes:

- `(candidate_id, score DESC)`
- `(transaction_id)`

### `sync_runs`

Observable audit of manual and background sync runs.

Columns:

- `id TEXT PRIMARY KEY`
- `trigger TEXT NOT NULL CHECK(trigger IN ('manual', 'background', 'initial'))`
- `status TEXT NOT NULL CHECK(status IN ('running', 'success', 'partial', 'failed'))`
- `started_at TEXT NOT NULL`
- `finished_at TEXT`
- `accounts_total INTEGER NOT NULL DEFAULT 0`
- `accounts_succeeded INTEGER NOT NULL DEFAULT 0`
- `accounts_failed INTEGER NOT NULL DEFAULT 0`
- `added INTEGER NOT NULL DEFAULT 0`
- `updated INTEGER NOT NULL DEFAULT 0`
- `skipped INTEGER NOT NULL DEFAULT 0`
- `queued_for_review INTEGER NOT NULL DEFAULT 0`
- `error_summary TEXT`

Index:

- `(started_at DESC)`

### Balance source convention

Use these `account_balances.source` values consistently:

- `simplefin`: latest bank-reported balance snapshot.
- `ledger_recomputed`: balance recomputed from local transactions after user edits/imports.
- `manual`: manually seeded or manually entered balance.

Drift detection must read the newest `simplefin` snapshot only and compare it with `SUM(transactions.amount_cents)`.

## Data flow

### CSV import

1. Parse CSV rows into normalized candidates.
2. For each candidate, call the shared reconciliation engine.
3. Auto-insert safe new rows.
4. Auto-skip exact duplicates.
5. Queue ambiguous/colliding rows in `import_candidates`.
6. Finish the import audit with counts for added, skipped, and queued.
7. Recompute linked-account ledger balances using `ledger_recomputed`.

### SimpleFIN sync

1. Scheduler starts a `sync_runs` row.
2. Fetch accounts/transactions from SimpleFIN with retry policy.
3. For each fetched account:
   - refresh local account provider metadata, including `extra_json` and `raw_json`;
   - write provider balance snapshot as `simplefin`;
   - normalize transactions into candidates;
   - run reconciliation;
   - auto-match/insert safe candidates;
   - queue ambiguous candidates;
   - import holdings from the fresh account payload;
   - run drift detection against latest `simplefin` snapshot.
4. Create sync-error alerts for failures that survive retry or should not retry.
5. Mark connection status active/error.
6. Finish `sync_runs` with success, partial, or failed status.

### Workbench resolution

Accept match:

- Apply provider metadata to the selected existing transaction.
- Preserve user-owned fields: `notes`, `category_id`, reimbursable/split flags, and user-edited merchant/category state.
- Mark candidate resolved with `resolution = 'matched'`.

Create new:

- Insert the candidate as a new transaction.
- Mark candidate resolved with `resolution = 'created'`.

Dismiss:

- Mark candidate dismissed and retain the row for audit/history.

Bulk approve:

- Only allowed for pending candidates whose recommended match is high confidence and collision-free.

## Retry and error policy

Retry transient failures with exponential backoff: 1s, 2s, 4s, 8s.

Do not retry:

- 403/auth/revoked access.
- 402/payment required.
- invalid/missing keychain credential.
- invalid access URL.

Retry:

- network timeout;
- 5xx provider failures;
- SimpleFIN `act.failed`, `act.missingdata`, and non-auth transient connection errors.

When retries exhaust, mark the connection `status = 'error'`, set `last_error`, record the sync run as partial/failed, and create a `simplefin_alerts` row with `alert_type = 'sync_error'`.

## UI design

### Inbox: Import Review

Add a new section for unresolved import candidates. Each card shows:

- source badge: CSV or SimpleFIN;
- date, merchant, amount, account;
- reason/confidence;
- recommended existing match with date, merchant, amount, and score;
- alternative matches if present.

Actions:

- Accept match.
- Choose alternative.
- Create new.
- Dismiss.
- Bulk approve selected high-confidence candidates.

### Existing surfaces

- Settings keeps SimpleFIN connection list, connection health, background interval, and remove action.
- Accounts keeps Sync all and per-account sync.
- Inbox keeps Bank sync alerts and transfer suggestions.
- Monetary values use `className="money"` so privacy mode keeps working.

## Testing strategy

### Rust

- Scheduler tests:
  - Off pauses without killing the task.
  - On resumes the task.
  - Manual/background jobs cannot overlap.
  - Retry classifier retries transient errors and does not retry auth/payment errors.
- Matcher tests:
  - exact provider ID match;
  - pending-to-posted match;
  - amount tolerance;
  - merchant/date scoring;
  - batch collision creates review candidates;
  - ambiguous alternatives create review candidates;
  - no plausible match inserts safely.
- Workbench repository tests:
  - create candidate with matches;
  - accept recommended match;
  - choose alternative;
  - create new;
  - dismiss;
  - bulk approve eligibility.
- Provider tests:
  - SimpleFIN sync refreshes `extra_json` before holdings import;
  - holdings snapshots use fresh payload;
  - drift uses latest `source = 'simplefin'` snapshot, not `ledger_recomputed`;
  - sync-error alert created after unretryable error or exhausted retries.
- CSV integration tests:
  - CSV before SimpleFIN queues ambiguity when match is not high confidence;
  - SimpleFIN before CSV avoids duplicates;
  - collision rows do not attach to the same existing transaction.

### Frontend

- Inbox Import Review renders candidate cards and actions.
- Accept/Create/Dismiss invalidates candidates, transactions, accounts, and alerts queries.
- Settings interval Off/On behavior maps correctly to sync settings.
- Accounts Sync all renders queued-for-review counts and errors.

### Validation commands

Run targeted tests first:

- `cargo test -p finsight-core`
- `cargo test -p finsight-providers simplefin`
- `cargo test -p finsight-providers --test csv_integration`
- `cargo check -p finsight-app`
- `cd ui && npx vitest run src/screens/Inbox.test.tsx src/screens/Settings.test.tsx src/screens/Accounts.test.tsx`
- `cd ui && npx tsc --noEmit`

Escalate to full suites after targeted tests pass or when shared API changes require it:

- `cargo test --workspace`
- `cd ui && npx vitest run`

Regenerate bindings after Rust command/type changes:

- `cargo run -p finsight-tauri --bin export_bindings`

## Acceptance criteria

- Background sync can be disabled and re-enabled without restarting the app.
- Manual Sync all cannot overlap with background sync.
- Transient SimpleFIN failures retry with backoff; auth/payment/missing credential errors do not retry.
- Sync failures create durable Inbox alerts.
- Drift compares ledger sum to latest SimpleFIN bank snapshot only.
- Holdings import reads fresh synced provider payloads.
- CSV and SimpleFIN use the same reconciliation engine.
- High-confidence non-colliding matches are automatic.
- Ambiguous/colliding imports are queued in Inbox and never silently duplicated.
- Workbench actions correctly resolve candidates and preserve user-owned transaction fields.
- Bindings, targeted tests, typecheck, and relevant frontend tests pass.

## Spec self-review

- Placeholder scan: no TBD/TODO placeholders remain.
- Internal consistency: scheduler, reconciliation, workbench, drift, holdings, and UI sections align.
- Scope check: this is intentionally a large but coherent production hardening slice centered on SimpleFIN/import correctness.
- Ambiguity check: automatic vs review behavior is explicit; secrets stay in keychain; drift source rules are explicit.
