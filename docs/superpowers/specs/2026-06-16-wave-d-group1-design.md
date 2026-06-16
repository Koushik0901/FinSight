# Wave D Group 1 — Design Spec

**Date:** 2026-06-16
**Status:** Approved
**Scope:** Two polish features — split transaction UI and OS-level budget/bill notifications. Windows/Linux installers are out of scope (built locally on demand; macOS workflow already exists).

---

## Feature 1: Split Transaction UI

### Overview

A single charge can span multiple spending categories (e.g., a Costco trip covering Groceries + Household). Today the `is_split` flag exists on `transactions` and the `Split` chip in `TransactionDrawer` toggles it, but there is no way to actually define the split amounts. This feature adds a `transaction_splits` table and a modal UI for editing splits.

### Database — `V012__transaction_splits.sql`

```sql
CREATE TABLE transaction_splits (
  id           TEXT    PRIMARY KEY,
  txn_id       TEXT    NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  category_id  TEXT    REFERENCES categories(id),
  amount_cents INTEGER NOT NULL
);
CREATE INDEX idx_splits_txn ON transaction_splits(txn_id);
```

When a transaction is split:
- The parent transaction's `category_id` is set to `NULL`. All category attribution comes from the splits rows.
- Sum of splits `amount_cents` must equal the parent's `amount_cents`. Enforced in Rust (returns a user-friendly error); not a DB constraint.
- If the user removes all splits, `is_split` flips back to `false` and the parent `category_id` stays `NULL` (unassigned — user can re-categorise normally).

### Rust — `crates/finsight-core`

New repo module `crates/finsight-core/src/repos/splits.rs`:

```rust
pub struct TransactionSplit {
    pub id: String,
    pub txn_id: String,
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

pub struct SplitInput {
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

pub fn list(conn: &mut Connection, txn_id: &str) -> CoreResult<Vec<TransactionSplit>>;
pub fn set(conn: &mut Connection, txn_id: &str, splits: &[SplitInput]) -> CoreResult<()>;
// set() validates sum == parent.amount_cents, deletes existing splits, inserts new ones,
// sets transactions.category_id = NULL, sets transactions.is_split = true.
// If splits is empty: deletes all splits, sets is_split = false (category stays NULL).
```

### Rust — `crates/finsight-app`

New commands in `crates/finsight-app/src/commands/transactions.rs`:

```rust
#[tauri::command]
#[specta::specta]
pub async fn get_transaction_splits(
    db: State<'_, Db>,
    transaction_id: String,
) -> AppResult<Vec<TransactionSplit>>

#[tauri::command]
#[specta::specta]
pub async fn set_transaction_splits(
    db: State<'_, Db>,
    transaction_id: String,
    splits: Vec<SplitInput>,
) -> AppResult<()>
```

Both use the `run()` pattern. Register in `build_specta_builder()` in `crates/finsight-app/src/lib.rs`.

### Category spending impact

`crates/finsight-core/src/repos/categories.rs` — `list_with_spending` query updated to account for splits:

- For **non-split** transactions (`is_split = 0`): attribute `amount_cents` to `category_id` as before.
- For **split** transactions (`is_split = 1`): attribute each split's `amount_cents` to its `category_id` via a `LEFT JOIN transaction_splits`.

Concretely, replace the direct aggregation with a UNION. The non-split branch uses `NOT EXISTS` (not `is_split = 0`) so that legacy transactions where `is_split = true` but no splits rows exist yet still contribute to their parent category rather than disappearing from both branches:

```sql
-- non-split spending (includes legacy is_split=true rows that have no splits yet)
SELECT t.category_id, SUM(ABS(t.amount_cents))
FROM transactions t
WHERE t.amount_cents < 0
  AND t.category_id IS NOT NULL
  AND <date filter>
  AND NOT EXISTS (SELECT 1 FROM transaction_splits WHERE txn_id = t.id)
GROUP BY t.category_id

UNION ALL

-- split spending
SELECT ts.category_id, SUM(ts.amount_cents)
FROM transaction_splits ts
JOIN transactions t ON t.id = ts.txn_id
WHERE t.amount_cents < 0
  AND ts.category_id IS NOT NULL
  AND <date filter>
GROUP BY ts.category_id
```

This change flows through to Budget envelopes, Categories screen, and Reports automatically.

### Frontend

**`ui/src/api/hooks/transactions.ts`** — two new hooks:
- `useTransactionSplits(txnId: string)` — `useQuery` wrapping `getTransactionSplits`
- `useSetTransactionSplits()` — `useMutation` wrapping `setTransactionSplits`, invalidates `["transactions"]` and `["categories"]`

**`ui/src/components/TransactionDrawer.tsx`** — changes when `is_split = true`:
- Replace the `CategoryPicker` with a read-only summary row:
  ```
  Split · {n} categories · ${total}    [Edit splits →]
  ```
- The "Split" chip toggle now opens `SplitModal` directly when turning on (rather than just toggling the boolean), so the user is immediately prompted to define splits.
- When turning the chip off: calls `setTransactionSplits(id, [])` which clears splits and resets `is_split = false`.

**`ui/src/components/SplitModal.tsx`** (new file) — reuses `Drawer.tsx`:

```
┌─────────────────────────────────────┐
│  Split transaction                  │
│  Total: $100.00                     │
│                                     │
│  [$62 assigned ━━━━━━━━▌   $38 left]│  ← balance bar (var(--accent) fill)
│                                     │
│  [CategoryPicker]      [$62.00    ] │  ← row 1
│  [CategoryPicker]      [$38.00    ] │  ← row 2
│  [+ Add split                     ] │
│                                     │
│  [Cancel]            [Save splits] │
└─────────────────────────────────────┘
```

- Minimum 2 splits. "Save splits" disabled if sum ≠ total or fewer than 2 splits.
- Amount inputs are plain number fields (step 0.01). Live balance bar updates as amounts change.
- "×" button removes a split row (minimum 2 enforced).
- On save: calls `setTransactionSplits`, closes modal, invalidates queries.
- Error toast if sum mismatch slips through.

---

## Feature 2: Budget Notifications + Bill Due Date Reminders

### Overview

OS-level push notifications (system tray / Notification Centre) for two events:
1. A budget envelope is exceeded for the first time this month.
2. A recurring bill is due within 3 days.

Notifications deduplicate via the existing settings KV store. A toggle in Settings lets users opt out.

### Tauri setup

Add to `crates/finsight-app/Cargo.toml`:
```toml
tauri-plugin-notification = "2"
```

Register in `crates/finsight-app/src/lib.rs`:
```rust
.plugin(tauri_plugin_notification::init())
```

Add `"notification:default"` to the permissions array in `src-tauri/capabilities/default.json` (the existing capabilities file — do not create a new one):
```json
{
  "permissions": [
    "core:default",
    "dialog:default",
    "opener:default",
    "notification:default"
  ]
}
```

macOS `Info.plist` (create `src-tauri/Info.plist` — Tauri 2 merges it into the app bundle automatically):
```xml
<key>NSUserNotificationUsageDescription</key>
<string>FinSight sends budget alerts and bill reminders.</string>
```

### Notification module — `crates/finsight-app/src/notifications.rs`

```rust
pub async fn check_and_fire(app: &AppHandle, db: &Db) -> AppResult<()>
```

Steps:
1. Read `notifications.enabled` from settings KV; return early if `false` or absent (default `true` — no explicit setup needed first run).
2. **Overflow check**: query budget envelopes joined with this month's spending. For each envelope where `spent > budget_cents`:
   - Key = `notifications.overflow.{category_id}.{YYYY-MM}`
   - Skip if key exists in settings KV.
   - Fire: title `"{label} · over budget"`, body `"${spent/100:.2} spent of ${budget/100:.2} budget"`.
   - Write key to settings KV.
3. **Bill due check**: call `recurring::list_upcoming(conn, 3)` (new core helper that returns items with `next_expected` within the next 3 calendar days).
   - Key = `notifications.bill.{merchant_key}.{YYYY-MM-DD}` (using `next_expected` date).
   - Skip if key exists.
   - Fire: title `"{merchant} · due {in N days}"`, body `"${amount/100:.2}"`.
   - Write key.

`recurring::list_upcoming(conn, days: u32)` is a new function in `crates/finsight-core/src/repos/recurring.rs` that reuses the existing recurring detection SQL, filtering to items where `next_expected <= date('now', '+{days} days')`.

### Trigger points

`check_and_fire` is called (via `tauri::async_runtime::spawn`) from:
1. **App startup** — in the `setup` closure in `crates/finsight-app/src/lib.rs`, after DB init.
2. **After import** — at the end of `import_transactions` in `commands/transactions.rs`.
3. **After apply plan** — at the end of `apply_next_month_plan` in `commands/budget.rs`.

### Settings UI

In `ui/src/screens/Settings.tsx`, add a "Notifications" row to the Appearance section (or a new sub-section):

```
Notifications
Budget alerts and bill reminders     [toggle on/off]
```

Toggle calls a new command:
```rust
#[tauri::command]
#[specta::specta]
pub async fn set_notifications_enabled(db: State<'_, Db>, enabled: bool) -> AppResult<()>
```

Which writes `notifications.enabled` to the settings KV. A companion `get_notifications_enabled() -> AppResult<bool>` command reads it (default `true` when absent).

---

## Out of Scope

- Windows/Linux installer workflows — built locally; macOS workflow already exists.
- Per-split notes field — the split UI only tracks category + amount. Notes on the parent transaction still apply.
- Smart split suggestions (e.g., auto-split based on merchant rules) — future work.
- Notification sound / badge count — OS defaults apply; no customisation.
- Notification history / in-app inbox — OS notification centre handles this.

---

## File Map

| File | Action |
|---|---|
| `crates/finsight-core/migrations/V012__transaction_splits.sql` | Create |
| `crates/finsight-core/src/repos/splits.rs` | Create |
| `crates/finsight-core/src/repos/mod.rs` | Modify (add `pub mod splits`) |
| `crates/finsight-core/src/repos/categories.rs` | Modify (UNION split spending) |
| `crates/finsight-core/src/repos/recurring.rs` | Modify (add `list_upcoming`) |
| `crates/finsight-app/Cargo.toml` | Modify (add notification plugin) |
| `crates/finsight-app/src/lib.rs` | Modify (register plugin, call check_and_fire on setup) |
| `crates/finsight-app/src/notifications.rs` | Create |
| `crates/finsight-app/src/commands/transactions.rs` | Modify (add split commands, call check_and_fire after import) |
| `crates/finsight-app/src/commands/budget.rs` | Modify (call check_and_fire after apply_next_month_plan) |
| `crates/finsight-app/src/commands/settings.rs` | Modify (add get/set_notifications_enabled) |
| `src-tauri/tauri.conf.json` | Modify (notification capability) |
| `src-tauri/Info.plist` | Create (macOS notification description) |
| `ui/src/api/hooks/transactions.ts` | Modify (add split hooks) |
| `ui/src/components/TransactionDrawer.tsx` | Modify (split summary row + modal trigger) |
| `ui/src/components/SplitModal.tsx` | Create |
| `ui/src/screens/Settings.tsx` | Modify (notifications toggle) |
