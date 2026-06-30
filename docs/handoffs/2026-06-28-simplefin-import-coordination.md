# SimpleFIN and Import Coordination Handoff

Date: 2026-06-28

Follow-up production hardening handoff: `docs/handoffs/2026-06-28-simplefin-production-hardening.md`.

This handoff documents the SimpleFIN repair and CSV/SimpleFIN import reconciliation work completed in this session so future agents can understand the current state and avoid reintroducing duplicate-import bugs.

## Completed work

### SimpleFIN credential refresh and sync repair

- Claimed the replacement SimpleFIN setup token provided by the user and stored the resulting access URL in the OS keychain under `com.finsight.simplefin.access`.
- Relinked existing SimpleFIN accounts to refreshed connection rows instead of creating stale duplicate connections/accounts.
- Synced the local encrypted app database after the refresh.
- Final local sync result: 7 linked accounts synced, 253 transactions added, 7 transactions updated, 3 skipped, 0 errors.

### SimpleFIN code fixes

- `crates/finsight-core/src/repos/connections.rs`
  - Added `upsert_by_conn_id` so refreshed bridge credentials update existing SimpleFIN connection rows by provider `conn_id`.
  - Clears stale connection errors when a refreshed connection is successfully upserted.
- `crates/finsight-core/src/repos/accounts.rs`
  - Added `upsert_simplefin_account`.
  - Uses `(connection_id, external_account_id)` first, then falls back to `simplefin_account_id` so refreshed credentials relink existing accounts instead of inserting duplicates.
  - Preserves user-facing nickname when refreshing provider metadata.
- `crates/finsight-app/src/commands/simplefin.rs`
  - `save_simplefin_setup_token` now upserts provider connections and relinks existing accounts surfaced by the new access URL.
  - SimpleFIN sync marks connection status/error metadata on failures and clears it on success.
  - Imported SimpleFIN account `name` remains the provider name; user nickname is stored separately.
- `crates/finsight-app/src/sync_scheduler.rs`
  - Batch/background sync marks connection errors on fetch/commit failures and marks successful connections active with `last_synced_at`.
- `crates/finsight-providers/src/simplefin/sync.rs`
  - Initial sync uses a 44-day lookback. The bridge rejected uncapped, 90-day, and 89-day initial requests with range errors/warnings, while 44 days stays under the common 45-day recommended range.
  - Subsequent sync keeps the 14-day lookback.
- `ui/src/screens/Accounts.tsx`
  - Account display name now prefers `nickname`, then `official_name`, then `name`, avoiding raw provider IDs where a nickname is absent.

### CSV and SimpleFIN import coordination

- `crates/finsight-providers/src/simplefin/matcher.rs`
  - Shared matcher used by both CSV import and SimpleFIN sync.
  - Matching order:
    1. Exact `imported_id` match.
    2. Fuzzy same-account match on amount, date window, and merchant/payee similarity.
    3. No match inserts a new transaction.
  - `find_match_excluding` supports per-batch collision avoidance so two incoming rows do not attach to the same existing fuzzy match.
- `crates/finsight-providers/src/csv/mod.rs`
  - CSV import now uses the shared matcher instead of exact-only `account_id + posted_at + amount + merchant_raw` dedup.
  - CSV rows matching existing SimpleFIN transactions are skipped as duplicates.
  - CSV inserts now persist full transaction metadata including `source = 'csv'`, `pending`, external ID fields, and raw sync field placeholders.
  - CSV import recomputes linked-account balances after import.
- `crates/finsight-providers/src/simplefin/sync.rs`
  - SimpleFIN sync now enriches matching CSV-created transactions with `imported_id`, `source`, raw sync payload, pending state, `external_tx_id`, and `external_account_id`.
  - User-edited fields such as `notes` and `category_id` are preserved when SimpleFIN updates a matched CSV transaction.
  - Per-sync collision avoidance prevents multiple incoming SimpleFIN transactions from fuzzy-matching the same existing ledger row.

## Behavioral contract for future agents

- CSV and SimpleFIN are not separate silos. Both must route through the shared reconciliation matcher before inserting transactions.
- Provider IDs are highest fidelity and must be tried before fuzzy matching.
- Fuzzy matching is intentionally conservative: same account, same amount, within a date window, with merchant/payee similarity scoring.
- Ambiguous or duplicate-looking rows should not create duplicate ledger entries.
- SimpleFIN is allowed to enrich CSV-created rows with provider metadata, but must not overwrite user-owned fields such as notes and category.
- CSV import should remain useful for backfill/history while SimpleFIN continues syncing current data.
- Linked-account balances must be recomputed or refreshed after imports so CSV and SimpleFIN do not leave account totals stale.
- Do not store SimpleFIN access URLs or setup tokens in documentation or source code. Credentials belong in the OS keychain.

## Validation run

- `cargo test -p finsight-providers`
  - Passed: 40 provider tests.
  - Ignored: 1 HTTPS/wiremock SimpleFIN client test.
  - Passed: 4 CSV integration tests.
- `cargo check -p finsight-app`
  - Passed.
- `cd ui && npx tsc --noEmit`
  - Passed after the Accounts display-name change.
- `cd ui && npx vitest run src/screens/Accounts.test.tsx`
  - Passed: 3 tests.

## Regression coverage added

- `simplefin_enriches_existing_csv_match_without_duplicate`
  - Verifies SimpleFIN sync enriches a CSV-created transaction without duplicating it and preserves user notes.
- `csv_import_skips_matching_simplefin_transaction`
  - Verifies CSV import skips a row that matches an already-synced SimpleFIN transaction.

## Notes for future work

- A durable Import Review workbench now exists for ambiguous/colliding import matches. See `docs/simplefin-production-hardening-handoff.md`.
- If SimpleFIN bridge range behavior changes, revisit `INITIAL_LOOKBACK_DAYS` in `crates/finsight-providers/src/simplefin/sync.rs`.
- If additional providers are added, they should reuse or generalize the shared reconciliation matcher rather than adding provider-specific duplicate logic.
