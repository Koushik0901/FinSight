# Task 1 Report: Migration and core model/repo for planned transactions

**Status:** DONE

## What I Implemented

1. **Migration file** (`crates/finsight-core/migrations/V018__planned_transactions.sql`):
   - Created `planned_transactions` table with columns: `id`, `description`, `amount_cents`, `account_id`, `category_id`, `due_date`, `status`, `source`, `created_at`
   - Added foreign key references to `accounts` and `categories` with `ON DELETE SET NULL`
   - Created indexes on `due_date` and `status` for efficient querying

2. **Model file** (`crates/finsight-core/src/models/planned_transaction.rs`):
   - `PlannedTransaction` struct with all fields from the table
   - `NewPlannedTransaction` struct for insert operations (no id, status, or created_at)
   - `PlannedTxnFilter` struct with optional `status` and `due_before` fields

3. **Repo file** (`crates/finsight-core/src/repos/planned_transactions.rs`):
   - `list(conn, filter)` - queries with optional status/due_before filters, ordered by due_date ASC
   - `get(conn, id)` - fetches a single planned transaction by ID
   - `insert(conn, new)` - creates a new planned transaction with UUID and timestamp
   - `update_status(conn, id, status)` - updates the status field
   - `delete(conn, id)` - removes a planned transaction
   - Unit tests for insert/list and update/delete operations

4. **Module registrations**:
   - Added `pub mod planned_transaction;` and re-exports to `crates/finsight-core/src/models/mod.rs`
   - Added `pub mod planned_transactions;` to `crates/finsight-core/src/repos/mod.rs`

## Test Results

```
running 2 tests
test repos::planned_transactions::tests::insert_and_list_planned_transactions ... ok
test repos::planned_transactions::tests::update_status_and_delete ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 51 filtered out
```

## Concerns

None. The implementation follows existing patterns from `goals.rs` and compiles cleanly.
