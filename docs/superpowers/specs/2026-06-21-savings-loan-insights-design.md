# Savings & Loan Insights Design

## Goal

Make FinSight goals smarter by connecting them to real accounts and liabilities:

1. A debt-payoff goal stays in sync with its linked liability balance.
2. A savings goal projects compound growth using the actual APY of its linked account.
3. Drawers gently prompt users to fill in missing APY / loan metadata so the app has better data to reason with.

## Background

Recent work added `accounts.apy_pct`, `liabilities.original_balance_cents`, and `liabilities.started_at`. This feature builds on those fields by wiring them into goals and projections.

Current state:
- Goals have no relationship to accounts or liabilities.
- Debt-payoff goals rely on manually updated `current_cents`.
- The Goals screen compound-growth projector uses a hard-coded 7% annual return.
- Data-quality reminders for missing APR/min-payment exist only inside the agent, not in the UI.

## Data model

### Migration: `V022__goal_links.sql`

```sql
ALTER TABLE goals ADD COLUMN liability_id TEXT REFERENCES liabilities(id) ON DELETE SET NULL;
ALTER TABLE goals ADD COLUMN account_id  TEXT REFERENCES accounts(id)  ON DELETE SET NULL;
```

### Rust models

Update `Goal`, `NewGoal`, and `GoalPatch` in `crates/finsight-core/src/repos/goals.rs`:

```rust
pub struct Goal {
    // ... existing fields ...
    pub liability_id: Option<String>,
    pub account_id: Option<String>,
}
```

### TypeScript bindings

Regenerate `ui/src/api/bindings.ts` so `Goal`, `NewGoal`, and `GoalPatch` expose `liabilityId` and `accountId`.

## Liability-goal auto-sync

### Behavior

- When a goal has `liability_id`, the UI disables manual "current balance" edits and shows the linked liability's name and balance.
- `liabilities::update` calls a new helper `goals::sync_linked_liabilities(conn, liability_id)` that sets `current_cents = balance_cents` for every linked goal.
- `liabilities::delete` cascades via `ON DELETE SET NULL`; linked goals become normal manual goals again.
- For linked debt-payoff goals, the default `target_cents` is the liability's `original_balance_cents` when available; otherwise the user enters it manually.
- Progress percentage continues to be `current_cents / target_cents`, clamped 0–100.

### Why write-through instead of read-time derivation

Approach B stores the synced value in `goals.current_cents`. This keeps existing queries, agent context, and progress math unchanged; no consumer needs to know the goal is linked.

## APY-aware projections

### Behavior

- Replace `projectCompoundValue(monthlyCents, years)` with `projectCompoundValue(monthlyCents, years, annualRate)`.
- The annual rate comes from the linked account's `apy_pct / 100`.
- If no account is linked or APY is missing, fall back to 0.07 (7%).
- The GoalCard displays the actual rate being used, e.g. "at 4.5% APY" or "at 7% (default)".

### Optional backend command

Add `project_goal_growth(goal_id, years)` as a Tauri command so the agent can reuse the projection later. For the initial implementation the frontend can compute it directly; the command is a thin wrapper.

## Inline drawer hints

### AccountDrawer

When the account type is Savings and `apy_pct` is empty, show a non-blocking hint under the APY field:

> Add an APY so savings projections use your real rate.

### LiabilityDrawer

When `original_balance_cents` is empty, show:

> Add an original balance to track payoff progress.

When `started_at` is empty, show:

> Add a start date to see how long you've been paying this down.

Hints use the existing `hint` style and do not block form submission.

## Agent integration

- `goal_context` and `wellness_context` already consume `goals.current_cents`. Because the DB value is kept in sync, no agent changes are required for liability-linked goals.
- Optionally extend the planner system prompt to note that linked debt-payoff goals reflect live liability balances.

## Error handling and edge cases

- A goal may be linked to a liability **or** an account, not both. The UI enforces one link at a time.
- Deleting a linked liability or account sets the corresponding goal column to `NULL` via `ON DELETE SET NULL`.
- If a liability balance exceeds the goal target, progress is clamped to 100%.
- Invalid `liability_id` or `account_id` values are rejected by foreign-key constraints.

## UI/UX changes

### Goals screen

- `NewGoalForm`: add optional "Linked liability" dropdown (filtered to liabilities with `balance_cents > 0`) and optional "Linked account" dropdown (filtered to Savings/Investment accounts).
- `GoalCard`:
  - For linked liabilities: show liability name, APR, and current balance; disable manual current-balance edits.
  - For linked accounts: show account name and APY in the compound-growth panel.

### Accounts screen drawers

- Add inline hints as described above.

## Testing

### Rust

- Migration test verifying `liability_id` and `account_id` columns exist.
- Repo test: updating a liability balance updates the linked goal's `current_cents`.
- Repo test: deleting a liability clears `liability_id` on linked goals.
- Agent context test: a linked debt-payoff goal reflects the liability balance in wellness context.

### Frontend

- `GoalCard` renders linked liability details and disables manual balance edit.
- `GoalCard` compound-growth panel uses linked account APY and falls back to 7%.
- `AccountDrawer` shows APY hint when Savings type is selected and APY is empty.
- `LiabilityDrawer` shows original-balance and start-date hints when empty.

## Open questions

None. Clarifying questions were resolved:
- Debt-payoff goals auto-sync from the linked liability.
- Goals link to an account via `account_id` for APY.
- Data-quality nudges appear inline in drawers.

## Implementation approach

Approach B: backend sync + backend projections. This keeps the existing goal progress and agent context code unchanged while giving us a clean path to expose projection logic to the agent later.
