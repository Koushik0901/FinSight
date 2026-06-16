# Wave D Group 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add split transaction UI (one charge across multiple categories) and OS-level budget-overflow + bill-due push notifications.

**Architecture:** Feature 1 adds a `transaction_splits` DB table, a new `repos/splits.rs` core module, two new Tauri commands, and a SplitModal React component that slots into the existing TransactionDrawer. The category-spending query in `commands/transactions.rs` is updated with a CTE UNION to attribute split spending correctly. Feature 2 adds `tauri-plugin-notification`, a `notifications.rs` app module that checks for overflows and upcoming bills via settings-KV deduplication, and a Settings toggle backed by two new commands.

**Tech Stack:** Rust/Tauri 2 backend · rusqlite · React 18 + TypeScript + Vite · tanstack-query · tauri-plugin-notification 2

---

## Feature 1: Split Transaction UI

### Task 1: V012 migration + splits repo

**Files:**
- Create: `crates/finsight-core/migrations/V012__transaction_splits.sql`
- Create: `crates/finsight-core/src/repos/splits.rs`
- Modify: `crates/finsight-core/src/repos/mod.rs`

- [ ] **Step 1: Write the failing test**

Open `crates/finsight-core/src/repos/splits.rs` (new file) and add:

```rust
use crate::error::{CoreError, CoreResult};
use rusqlite::{params, Connection};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub struct TransactionSplit {
    pub id: String,
    pub txn_id: String,
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

#[derive(Debug, Clone)]
pub struct SplitInput {
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

pub fn list(conn: &mut Connection, txn_id: &str) -> CoreResult<Vec<TransactionSplit>> {
    let mut stmt = conn.prepare(
        "SELECT id, txn_id, category_id, amount_cents FROM transaction_splits WHERE txn_id = ?1 ORDER BY rowid"
    )?;
    let rows = stmt.query_map(params![txn_id], |r| {
        Ok(TransactionSplit {
            id: r.get(0)?,
            txn_id: r.get(1)?,
            category_id: r.get(2)?,
            amount_cents: r.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(CoreError::Sqlite)
}

/// Replace all splits for a transaction atomically.
///
/// Rules:
/// - `splits` must have >= 2 entries OR be empty (to clear).
/// - Sum of `splits[*].amount_cents` must equal `abs(parent.amount_cents)`.
/// - On success with non-empty splits: sets `transactions.is_split = 1`, `category_id = NULL`.
/// - On success with empty splits: sets `transactions.is_split = 0` (category stays NULL).
pub fn set(conn: &mut Connection, txn_id: &str, splits: &[SplitInput]) -> CoreResult<()> {
    if !splits.is_empty() {
        if splits.len() < 2 {
            return Err(CoreError::InvalidState("at least 2 splits required".into()));
        }
        let parent_abs: i64 = conn.query_row(
            "SELECT ABS(amount_cents) FROM transactions WHERE id = ?1",
            params![txn_id],
            |r| r.get(0),
        ).map_err(CoreError::Sqlite)?;
        let total: i64 = splits.iter().map(|s| s.amount_cents).sum();
        if total != parent_abs {
            return Err(CoreError::InvalidState(format!(
                "splits sum {total} != transaction abs amount {parent_abs}"
            )));
        }
    }

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM transaction_splits WHERE txn_id = ?1", params![txn_id])?;
    for s in splits {
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO transaction_splits (id, txn_id, category_id, amount_cents) VALUES (?1, ?2, ?3, ?4)",
            params![id, txn_id, s.category_id, s.amount_cents],
        )?;
    }
    let (is_split, cat_id): (i64, Option<String>) = if splits.is_empty() {
        (0, None)
    } else {
        (1, None)
    };
    tx.execute(
        "UPDATE transactions SET is_split = ?1, category_id = ?2 WHERE id = ?3",
        params![is_split, cat_id, txn_id],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::test_db;

    fn insert_test_txn(conn: &mut Connection, amount_cents: i64) -> String {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO transactions (id, account_id, merchant_raw, amount_cents, posted_at, import_id, created_at)
             VALUES (?1, 'acc1', 'Costco', ?2, '2026-06-01', 'imp1', '2026-06-01T00:00:00Z')",
            params![id, amount_cents],
        ).unwrap();
        id
    }

    #[test]
    fn set_and_list_splits() {
        let db = test_db();
        let mut conn = db.get().unwrap();
        // Need a minimal account row for FK
        conn.execute(
            "INSERT OR IGNORE INTO accounts (id, bank, name, type, balance_cents, currency, created_at) VALUES ('acc1','Bank','Chk','checking',0,'USD','2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        let txn_id = insert_test_txn(&mut conn, -10000); // $100 expense

        set(&mut conn, &txn_id, &[
            SplitInput { category_id: None, amount_cents: 6000 },
            SplitInput { category_id: None, amount_cents: 4000 },
        ]).unwrap();

        let splits = list(&mut conn, &txn_id).unwrap();
        assert_eq!(splits.len(), 2);
        assert_eq!(splits[0].amount_cents, 6000);
        assert_eq!(splits[1].amount_cents, 4000);

        // is_split should be 1, category_id NULL
        let (is_split, cat_id): (i64, Option<String>) = conn.query_row(
            "SELECT is_split, category_id FROM transactions WHERE id = ?1",
            params![txn_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(is_split, 1);
        assert!(cat_id.is_none());
    }

    #[test]
    fn clear_splits_resets_flag() {
        let db = test_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO accounts (id, bank, name, type, balance_cents, currency, created_at) VALUES ('acc1','Bank','Chk','checking',0,'USD','2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        let txn_id = insert_test_txn(&mut conn, -5000);
        set(&mut conn, &txn_id, &[
            SplitInput { category_id: None, amount_cents: 3000 },
            SplitInput { category_id: None, amount_cents: 2000 },
        ]).unwrap();
        set(&mut conn, &txn_id, &[]).unwrap();

        let splits = list(&mut conn, &txn_id).unwrap();
        assert!(splits.is_empty());
        let is_split: i64 = conn.query_row(
            "SELECT is_split FROM transactions WHERE id = ?1", params![txn_id], |r| r.get(0)
        ).unwrap();
        assert_eq!(is_split, 0);
    }

    #[test]
    fn rejects_sum_mismatch() {
        let db = test_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO accounts (id, bank, name, type, balance_cents, currency, created_at) VALUES ('acc1','Bank','Chk','checking',0,'USD','2026-01-01T00:00:00Z')",
            [],
        ).unwrap();
        let txn_id = insert_test_txn(&mut conn, -10000);
        let err = set(&mut conn, &txn_id, &[
            SplitInput { category_id: None, amount_cents: 3000 },
            SplitInput { category_id: None, amount_cents: 3000 },
        ]).unwrap_err();
        assert!(matches!(err, CoreError::InvalidState(_)));
    }
}
```

- [ ] **Step 2: Create the migration file**

Create `crates/finsight-core/migrations/V012__transaction_splits.sql`:

```sql
CREATE TABLE transaction_splits (
  id           TEXT    PRIMARY KEY,
  txn_id       TEXT    NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  category_id  TEXT    REFERENCES categories(id),
  amount_cents INTEGER NOT NULL
);
CREATE INDEX idx_splits_txn ON transaction_splits(txn_id);
```

- [ ] **Step 3: Register the module**

In `crates/finsight-core/src/repos/mod.rs`, add after the last `pub mod` line:

```rust
pub mod splits;
```

- [ ] **Step 4: Run tests to verify**

```bash
cargo test -p finsight-core --lib repos::splits::tests
```

Expected: 3 tests pass (`set_and_list_splits`, `clear_splits_resets_flag`, `rejects_sum_mismatch`).

If you see "no such table: transaction_splits" it means the test DB helper doesn't run migrations — check `crates/finsight-core/src/db.rs` for `test_db()`. If it creates a raw SQLite DB without migrations, run the migration SQL manually in the test helper or use `run_migrations` there.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/migrations/V012__transaction_splits.sql crates/finsight-core/src/repos/splits.rs crates/finsight-core/src/repos/mod.rs
git commit -m "feat: add transaction_splits table (V012) and splits repo"
```

---

### Task 2: Update category spending query to include split attribution

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs` (the `list_categories_with_spending` function, around line 185)

- [ ] **Step 1: Write a test asserting split spending appears under split categories**

At the bottom of `crates/finsight-app/src/commands/transactions.rs`, or in an integration test, add:

```rust
#[cfg(test)]
mod spending_tests {
    // Integration test: split transaction spending must attribute to split categories, not parent category.
    // This is a SQL-level test. Run via: cargo test -p finsight-app --lib commands::transactions::spending_tests
    // (Lightweight: creates an in-memory DB, inserts data, calls the query directly.)
}
```

Because `list_categories_with_spending` is a Tauri command (hard to unit-test in isolation), verify it manually after implementing:
- Load demo data in dev mode (Settings → Load demo data)
- Mark a transaction as split with two categories
- Check that the Categories screen shows spending for each split category, not the parent

- [ ] **Step 2: Replace the spending query in `list_categories_with_spending`**

Find the `run(&db, move |conn| { ... })` block in `list_categories_with_spending` (around line 205). Replace the `conn.prepare(...)` call with this CTE-based query:

```rust
let mut stmt = conn.prepare(
    "WITH spending AS (
       -- Non-split transactions (or legacy is_split=true rows with no splits yet)
       SELECT t.category_id, t.posted_at, ABS(t.amount_cents) AS cents
       FROM transactions t
       WHERE t.amount_cents < 0
         AND t.category_id IS NOT NULL
         AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
       UNION ALL
       -- Split transaction spending
       SELECT ts.category_id, t.posted_at, ts.amount_cents AS cents
       FROM transaction_splits ts
       JOIN transactions t ON t.id = ts.txn_id
       WHERE t.amount_cents < 0
         AND ts.category_id IS NOT NULL
     )
     SELECT
       c.id, c.label, COALESCE(c.color,''), c.group_id, COALESCE(g.label,''),
       COALESCE(SUM(CASE WHEN s.posted_at >= ?1 THEN s.cents ELSE 0 END), 0),
       COALESCE(SUM(CASE WHEN s.posted_at >= ?2 AND s.posted_at < ?1 THEN s.cents ELSE 0 END), 0),
       COUNT(CASE WHEN s.posted_at >= ?1 THEN 1 END),
       COALESCE(SUM(CASE WHEN s.posted_at >= ?3 THEN s.cents ELSE 0 END), 0),
       COALESCE(MAX(b.amount_cents), 0)
     FROM categories c
     LEFT JOIN category_groups g ON g.id = c.group_id
     LEFT JOIN spending s ON s.category_id = c.id
     LEFT JOIN budgets b ON b.category_id = c.id AND b.month = ?4
     WHERE c.archived_at IS NULL
     GROUP BY c.id, c.label, c.color, c.group_id, g.label
     ORDER BY 6 DESC, g.sort_order, c.sort_order",
)?;
```

The `query_map` call and the `Ok(CategoryWithSpending { ... })` mapping remain identical — only the SQL string changes.

- [ ] **Step 3: Run all Rust tests**

```bash
cargo test --workspace
```

Expected: all existing tests still pass (no regressions in category spending).

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/transactions.rs
git commit -m "feat: attribute split transaction spending via UNION CTE in categories query"
```

---

### Task 3: Add get_transaction_splits + set_transaction_splits Tauri commands

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs`
- Modify: `crates/finsight-app/src/lib.rs`

- [ ] **Step 1: Add types and commands to transactions.rs**

At the bottom of `crates/finsight-app/src/commands/transactions.rs`, add:

```rust
// ── Split transaction commands ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransactionSplitDto {
    pub id: String,
    pub txn_id: String,
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SplitInputDto {
    pub category_id: Option<String>,
    pub amount_cents: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn get_transaction_splits(
    state: tauri::State<'_, AppState>,
    transaction_id: String,
) -> AppResult<Vec<TransactionSplitDto>> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        finsight_core::repos::splits::list(conn, &transaction_id).map(|v| {
            v.into_iter().map(|s| TransactionSplitDto {
                id: s.id,
                txn_id: s.txn_id,
                category_id: s.category_id,
                amount_cents: s.amount_cents,
            }).collect()
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_transaction_splits(
    state: tauri::State<'_, AppState>,
    transaction_id: String,
    splits: Vec<SplitInputDto>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let inputs: Vec<finsight_core::repos::splits::SplitInput> = splits.into_iter()
            .map(|s| finsight_core::repos::splits::SplitInput {
                category_id: s.category_id,
                amount_cents: s.amount_cents,
            })
            .collect();
        finsight_core::repos::splits::set(conn, &transaction_id, &inputs)
    })
    .await
    .map_err(AppError::from)
}
```

Also add `serde::Deserialize` to the existing `use serde::{Deserialize, Serialize};` import if it isn't already present. Check the top of the file — `Deserialize` is already imported via `TxnFilterInput` using `use serde::{Deserialize, Serialize};`. If not, add it.

- [ ] **Step 2: Register in build_specta_builder()**

In `crates/finsight-app/src/lib.rs`, find `build_specta_builder()`. Add the two new commands inside `collect_commands![...]`, after the existing transaction commands (after `commands::transactions::set_transaction_flags`):

```rust
commands::transactions::get_transaction_splits,
commands::transactions::set_transaction_splits,
```

- [ ] **Step 3: Run cargo test**

```bash
cargo test --workspace
```

Expected: all pass, no compile errors.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/transactions.rs crates/finsight-app/src/lib.rs
git commit -m "feat: add get/set_transaction_splits Tauri commands"
```

---

### Task 4: Regenerate TypeScript bindings

**Files:**
- Modify: `ui/src/api/bindings.ts` (auto-generated — do not edit manually)

- [ ] **Step 1: Run the bindings exporter**

```bash
cargo run -p finsight-tauri --bin export_bindings
```

Expected: `ui/src/api/bindings.ts` is updated. You should see `getTransactionSplits` and `setTransactionSplits` appear in the generated `commands` object, plus `TransactionSplitDto` and `SplitInputDto` types.

- [ ] **Step 2: TypeScript check**

```bash
cd ui && npx tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/bindings.ts
git commit -m "chore: regenerate bindings — add split transaction commands"
```

---

### Task 5: Frontend — split hooks + SplitModal + TransactionDrawer wiring

**Files:**
- Modify: `ui/src/api/hooks/transactions.ts`
- Create: `ui/src/components/SplitModal.tsx`
- Modify: `ui/src/components/TransactionDrawer.tsx`

- [ ] **Step 1: Add split hooks to transactions.ts**

At the bottom of `ui/src/api/hooks/transactions.ts`, add:

```typescript
export function useTransactionSplits(txnId: string | undefined) {
  return useQuery({
    queryKey: ["splits", txnId],
    queryFn: async () => {
      if (!txnId) return [];
      const result = await commands.getTransactionSplits(txnId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: !!txnId,
  });
}

export function useSetTransactionSplits() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ txnId, splits }: {
      txnId: string;
      splits: Array<{ categoryId: string | null; amountCents: number }>;
    }) => {
      const result = await commands.setTransactionSplits(
        txnId,
        splits.map(s => ({ categoryId: s.categoryId, amountCents: s.amountCents }))
      );
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: (_data, vars) => {
      qc.invalidateQueries({ queryKey: ["transactions"] });
      qc.invalidateQueries({ queryKey: ["splits", vars.txnId] });
      qc.invalidateQueries({ queryKey: ["categories"] });
      qc.invalidateQueries({ queryKey: ["today-summary"] });
    },
  });
}
```

- [ ] **Step 2: Create SplitModal.tsx**

Create `ui/src/components/SplitModal.tsx`:

```tsx
import { useState, useEffect } from "react";
import { toast } from "sonner";
import Drawer from "./Drawer";
import CategoryPicker from "./CategoryPicker";
import { useSetTransactionSplits } from "../api/hooks/transactions";
import type { TransactionSplitDto } from "../api/bindings";

interface SplitRow {
  id: string; // local UI key only
  categoryId: string | null;
  amountCents: number;
}

interface Props {
  open: boolean;
  onClose: () => void;
  transactionId: string;
  totalCents: number; // abs value — always positive
  existingSplits: TransactionSplitDto[];
}

function newRow(): SplitRow {
  return { id: Math.random().toString(36).slice(2), categoryId: null, amountCents: 0 };
}

export default function SplitModal({ open, onClose, transactionId, totalCents, existingSplits }: Props) {
  const setSplits = useSetTransactionSplits();

  const [rows, setRows] = useState<SplitRow[]>([newRow(), newRow()]);

  useEffect(() => {
    if (!open) return;
    if (existingSplits.length >= 2) {
      setRows(existingSplits.map(s => ({
        id: s.id,
        // TransactionSplitDto has camelCase fields (serde rename_all = "camelCase")
        categoryId: s.categoryId ?? null,
        amountCents: s.amountCents,
      })));
    } else {
      setRows([newRow(), newRow()]);
    }
  }, [open, existingSplits]);

  const assigned = rows.reduce((sum, r) => sum + r.amountCents, 0);
  const remaining = totalCents - assigned;
  const pct = Math.min(100, (assigned / totalCents) * 100);
  const balanced = assigned === totalCents;

  function updateRow(id: string, patch: Partial<SplitRow>) {
    setRows(prev => prev.map(r => r.id === id ? { ...r, ...patch } : r));
  }

  function removeRow(id: string) {
    if (rows.length <= 2) return;
    setRows(prev => prev.filter(r => r.id !== id));
  }

  async function handleSave() {
    if (!balanced) {
      toast.error(`Splits must sum to $${(totalCents / 100).toFixed(2)}`);
      return;
    }
    try {
      await setSplits.mutateAsync({
        txnId: transactionId,
        splits: rows.map(r => ({ categoryId: r.categoryId, amountCents: r.amountCents })),
      });
      toast.success("Splits saved");
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not save splits");
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title="Split transaction">
      <div style={{ marginBottom: 8, fontSize: 13, color: "var(--ink-mute)" }}>
        Total: ${(totalCents / 100).toFixed(2)}
      </div>

      {/* Balance bar */}
      <div style={{ height: 6, background: "var(--line)", borderRadius: 3, marginBottom: 4, overflow: "hidden" }}>
        <div style={{
          height: "100%",
          width: `${pct}%`,
          background: balanced ? "var(--accent)" : "var(--negative)",
          borderRadius: 3,
          transition: "width 0.15s",
        }} />
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: "var(--ink-mute)", marginBottom: 16 }}>
        <span>${(assigned / 100).toFixed(2)} assigned</span>
        <span style={{ color: remaining === 0 ? "var(--accent)" : "var(--negative)" }}>
          {remaining === 0 ? "✓ balanced" : `$${(Math.abs(remaining) / 100).toFixed(2)} ${remaining > 0 ? "left" : "over"}`}
        </span>
      </div>

      {/* Split rows */}
      {rows.map(row => (
        <div key={row.id} style={{ display: "flex", gap: 8, alignItems: "flex-start", marginBottom: 10 }}>
          <div style={{ flex: 1 }}>
            <CategoryPicker
              value={row.categoryId}
              onChange={val => updateRow(row.id, { categoryId: val })}
            />
          </div>
          <input
            type="number"
            step="0.01"
            min="0.01"
            placeholder="0.00"
            value={row.amountCents > 0 ? (row.amountCents / 100).toFixed(2) : ""}
            onChange={e => updateRow(row.id, { amountCents: Math.round(parseFloat(e.target.value || "0") * 100) })}
            style={{ width: 90, background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 6, padding: "6px 8px", fontSize: 14, color: "var(--ink)" }}
          />
          {rows.length > 2 && (
            <button type="button" onClick={() => removeRow(row.id)} style={{ color: "var(--ink-faint)", fontSize: 18, lineHeight: 1, padding: "4px 6px", background: "none", border: "none", cursor: "pointer" }}>×</button>
          )}
        </div>
      ))}

      <button
        type="button"
        onClick={() => setRows(prev => [...prev, newRow()])}
        style={{ marginBottom: 24, background: "none", border: "1px dashed var(--line)", borderRadius: 6, color: "var(--ink-mute)", padding: "6px 12px", fontSize: 13, cursor: "pointer", width: "100%" }}
      >
        + Add split
      </button>

      <div className="form-actions">
        <button type="button" onClick={onClose}>Cancel</button>
        <button type="button" className="primary" disabled={!balanced || setSplits.isPending} onClick={handleSave}>
          {setSplits.isPending ? "Saving…" : "Save splits"}
        </button>
      </div>
    </Drawer>
  );
}
```

- [ ] **Step 3: Update TransactionDrawer.tsx**

In `ui/src/components/TransactionDrawer.tsx`:

1. Add imports at the top (after existing imports):

```tsx
import SplitModal from "./SplitModal";
import { useTransactionSplits, useSetTransactionSplits } from "../api/hooks/transactions";
```

2. Add state inside the component function (after `const [deleteConfirm, setDeleteConfirm] = useState(false)`):

```tsx
const [splitModalOpen, setSplitModalOpen] = useState(false);
const { data: existingSplits = [] } = useTransactionSplits(transaction?.id);
const clearSplits = useSetTransactionSplits();
```

3. Find the Category picker block (around line 157):
```tsx
<div>
  <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>Category</div>
  <CategoryPicker value={selectedCategory} onChange={setSelectedCategory} />
</div>
```

Replace with:

```tsx
<div>
  <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>Category</div>
  {transaction?.is_split ? (
    <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 10px", background: "var(--surface-2)", borderRadius: 7, border: "1px solid var(--line)" }}>
      <span style={{ flex: 1, fontSize: 13, color: "var(--ink-mute)" }}>
        Split · {existingSplits.length} {existingSplits.length === 1 ? "category" : "categories"} · ${(Math.abs(transaction.amount_cents) / 100).toFixed(2)}
      </span>
      <button type="button" onClick={() => setSplitModalOpen(true)} style={{ fontSize: 12, color: "var(--accent)", background: "none", border: "none", cursor: "pointer", padding: 0 }}>
        Edit splits →
      </button>
    </div>
  ) : (
    <CategoryPicker value={selectedCategory} onChange={setSelectedCategory} />
  )}
</div>
```

4. Find the Split chip button (around line 183):
```tsx
<button
  type="button"
  className={`chip${transaction.is_split ? " accent" : ""}`}
  aria-pressed={transaction.is_split}
  onClick={async () => {
  try {
    await setFlags.mutateAsync({ id: transaction.id, isReimbursable: transaction.is_reimbursable, isSplit: !transaction.is_split });
  } catch (err) {
    toast.error(err instanceof Error ? err.message : "Could not update flag");
  }
}}
>
  Split
</button>
```

Replace with:

```tsx
<button
  type="button"
  className={`chip${transaction.is_split ? " accent" : ""}`}
  aria-pressed={transaction.is_split}
  onClick={async () => {
    if (!transaction.is_split) {
      // Turning ON: open split modal immediately to define splits
      setSplitModalOpen(true);
    } else {
      // Turning OFF: clear splits (resets is_split = false)
      try {
        await clearSplits.mutateAsync({ txnId: transaction.id, splits: [] });
      } catch (err) {
        toast.error(err instanceof Error ? err.message : "Could not clear splits");
      }
    }
  }}
>
  Split
</button>
```

5. Add `SplitModal` just before the closing `</Drawer>` tag (or just before the delete button section):

```tsx
{transaction && (
  <SplitModal
    open={splitModalOpen}
    onClose={() => setSplitModalOpen(false)}
    transactionId={transaction.id}
    totalCents={Math.abs(transaction.amount_cents)}
    existingSplits={existingSplits}
  />
)}
```

- [ ] **Step 4: TypeScript check**

```bash
cd ui && npx tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 5: Frontend tests**

```bash
cd ui && npx vitest run
```

Expected: all 105 tests pass (no regressions).

- [ ] **Step 6: Commit**

```bash
git add ui/src/api/hooks/transactions.ts ui/src/components/SplitModal.tsx ui/src/components/TransactionDrawer.tsx
git commit -m "feat: split transaction UI — SplitModal + TransactionDrawer wiring"
```

---

## Feature 2: OS Budget + Bill Notifications

### Task 6: Add tauri-plugin-notification + capabilities

**Files:**
- Modify: `crates/finsight-app/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`
- Create: `src-tauri/Info.plist`

- [ ] **Step 1: Add the plugin dependency**

In `crates/finsight-app/Cargo.toml`, in the `[dependencies]` section after `tauri-plugin-opener = "2"`:

```toml
tauri-plugin-notification = "2"
```

- [ ] **Step 2: Add notification permission to capabilities**

In `src-tauri/capabilities/default.json`, add `"notification:default"` to the permissions array:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:default",
    "opener:default",
    "notification:default"
  ]
}
```

- [ ] **Step 3: Create macOS Info.plist**

Create `src-tauri/Info.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>NSUserNotificationUsageDescription</key>
    <string>FinSight sends budget alerts and bill reminders.</string>
</dict>
</plist>
```

- [ ] **Step 4: Register the plugin in configure_app()**

In `crates/finsight-app/src/lib.rs`, find the `.plugin(tauri_plugin_opener::init())` line in `configure_app()` and add the notification plugin after it:

```rust
.plugin(tauri_plugin_opener::init())
.plugin(tauri_plugin_notification::init())
```

Also add the use statement at the top of the file (or in the relevant scope) if needed — Tauri plugins are typically self-contained and don't need explicit use imports for `init()`.

- [ ] **Step 5: Verify it compiles**

```bash
cargo check -p finsight-app
```

Expected: no errors. If `tauri_plugin_notification` is not found, run `cargo update` to fetch the new dependency.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-app/Cargo.toml src-tauri/capabilities/default.json src-tauri/Info.plist crates/finsight-app/src/lib.rs
git commit -m "feat: add tauri-plugin-notification and macOS Info.plist"
```

---

### Task 7: Create notifications.rs module

**Files:**
- Create: `crates/finsight-app/src/notifications.rs`
- Modify: `crates/finsight-app/src/lib.rs` (add `pub mod notifications` and call `check_and_fire` on startup)

- [ ] **Step 1: Create crates/finsight-app/src/notifications.rs**

```rust
//! Budget-overflow and bill-due OS notifications.
//! Called from app setup and after import/plan operations.

use crate::error::AppResult;
use chrono::{Duration, NaiveDate, Utc};
use finsight_core::{settings, Db};
use finsight_core::repos::run;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

const ENABLED_KEY: &str = "notifications.enabled";

pub async fn check_and_fire(app: &AppHandle, db: &Db) -> AppResult<()> {
    let db = db.clone();
    let app = app.clone();
    let to_fire = run(&db, move |conn| {
        // Check enabled (default true when absent)
        let enabled: Option<bool> = settings::get(conn, ENABLED_KEY)?;
        if enabled == Some(false) {
            return Ok(vec![]);
        }

        let mut notifications: Vec<(String, String)> = Vec::new(); // (title, body)
        let now = Utc::now();
        let this_month = now.format("%Y-%m").to_string();

        // ── 1. Budget overflow check ──────────────────────────────────────────
        let this_month_start = now.format("%Y-%m-01").to_string();

        struct EnvelopeRow { category_id: String, label: String, budget: i64, spent: i64 }

        let mut stmt = conn.prepare(
            "WITH spending AS (
               SELECT t.category_id, ABS(t.amount_cents) AS cents, t.posted_at
               FROM transactions t
               WHERE t.amount_cents < 0
                 AND t.category_id IS NOT NULL
                 AND NOT EXISTS (SELECT 1 FROM transaction_splits ts WHERE ts.txn_id = t.id)
               UNION ALL
               SELECT ts.category_id, ts.amount_cents AS cents, t.posted_at
               FROM transaction_splits ts
               JOIN transactions t ON t.id = ts.txn_id
               WHERE t.amount_cents < 0 AND ts.category_id IS NOT NULL
             )
             SELECT c.id, c.label, b.amount_cents,
                    COALESCE(SUM(CASE WHEN s.posted_at >= ?1 THEN s.cents ELSE 0 END), 0) AS spent
             FROM categories c
             JOIN budgets b ON b.category_id = c.id AND b.month = ?2
             LEFT JOIN spending s ON s.category_id = c.id
             WHERE c.archived_at IS NULL AND b.amount_cents > 0
             GROUP BY c.id, c.label, b.amount_cents
             HAVING spent > b.amount_cents",
        )?;
        let over_envelopes: Vec<EnvelopeRow> = stmt
            .query_map(rusqlite::params![this_month_start, this_month], |r| {
                Ok(EnvelopeRow {
                    category_id: r.get(0)?,
                    label: r.get(1)?,
                    budget: r.get(2)?,
                    spent: r.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        for env in &over_envelopes {
            let dedup_key = format!("notifications.overflow.{}.{}", env.category_id, this_month);
            let already_sent: Option<bool> = settings::get(conn, &dedup_key)?;
            if already_sent.is_some() {
                continue;
            }
            notifications.push((
                format!("{} · over budget", env.label),
                format!(
                    "${:.2} spent of ${:.2} budget",
                    env.spent as f64 / 100.0,
                    env.budget as f64 / 100.0
                ),
            ));
            settings::set(conn, &dedup_key, &true)?;
        }

        // ── 2. Bill due in 3 days check ───────────────────────────────────────
        let cutoff = (now - Duration::days(395)).format("%Y-%m-%d").to_string();
        let today = now.format("%Y-%m-%d").to_string();
        let in_3 = (now + Duration::days(3)).format("%Y-%m-%d").to_string();

        let mut stmt2 = conn.prepare(
            "WITH dated AS (
               SELECT merchant_raw, date(posted_at) AS d, amount_cents,
                      LAG(date(posted_at)) OVER (PARTITION BY merchant_raw ORDER BY posted_at) AS prev_d
               FROM transactions
               WHERE posted_at >= ?1
             ),
             gaps AS (
               SELECT merchant_raw, d, amount_cents,
                      julianday(d) - julianday(prev_d) AS gap
               FROM dated WHERE prev_d IS NOT NULL
             ),
             agg AS (
               SELECT merchant_raw, AVG(gap) AS avg_gap, MAX(d) AS last_seen,
                      MAX(amount_cents) AS last_amount, COUNT(*) AS occ
               FROM gaps WHERE gap BETWEEN 5 AND 400
               GROUP BY merchant_raw
               HAVING occ >= 2 AND AVG(gap) < 400 AND MAX(amount_cents) < 0
             )
             SELECT merchant_raw, avg_gap, last_seen, last_amount FROM agg",
        )?;

        // Collect candidate bills first (releases stmt2 borrow), then dedup with settings KV.
        struct BillCandidate { merchant: String, next_str: String, amount: i64 }
        let candidates: Vec<BillCandidate> = stmt2
            .query_map(rusqlite::params![cutoff], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(merchant, avg_gap, last_seen, amount)| {
                let last = NaiveDate::parse_from_str(&last_seen, "%Y-%m-%d").ok()?;
                let next = last + Duration::days(avg_gap.round() as i64);
                let next_str = next.format("%Y-%m-%d").to_string();
                if next_str >= today && next_str <= in_3 {
                    Some(BillCandidate { merchant, next_str, amount })
                } else {
                    None
                }
            })
            .collect();
        // stmt2 borrow released here — conn is free again for settings reads/writes.

        let today_naive = NaiveDate::parse_from_str(&today, "%Y-%m-%d").ok();
        for bill in &candidates {
            let dedup_key = format!(
                "notifications.bill.{}.{}",
                bill.merchant.to_lowercase().replace(' ', "_"),
                bill.next_str
            );
            let already: Option<bool> = settings::get(conn, &dedup_key)?;
            if already.is_some() {
                continue;
            }
            let days_away = today_naive
                .and_then(|t| NaiveDate::parse_from_str(&bill.next_str, "%Y-%m-%d").ok().map(|n| (n - t).num_days()))
                .unwrap_or(0);
            let when = if days_away == 0 {
                "today".to_string()
            } else {
                format!("in {days_away} day{}", if days_away == 1 { "" } else { "s" })
            };
            notifications.push((
                format!("{} · due {when}", bill.merchant),
                format!("${:.2}", (bill.amount.unsigned_abs() as f64) / 100.0),
            ));
            settings::set(conn, &dedup_key, &true)?;
        }

        Ok(notifications)
    })
    .await
    .map_err(crate::error::AppError::from)?;

    for (title, body) in to_fire {
        let _ = app.notification().builder()
            .title(&title)
            .body(&body)
            .show();
    }
    Ok(())
}
```

- [ ] **Step 2: Register the module in lib.rs**

In `crates/finsight-app/src/lib.rs`, add after the existing `pub mod commands;` and `pub mod error;` lines:

```rust
pub mod notifications;
```

- [ ] **Step 3: Call check_and_fire on app startup**

In the `setup` closure in `configure_app()` in `lib.rs`, after `app.manage(state);`:

```rust
// Fire notifications on startup (budget overflow, upcoming bills)
let notify_app = app.handle().clone();
let notify_db = db.clone();
tauri::async_runtime::spawn(async move {
    let _ = crate::notifications::check_and_fire(&notify_app, &notify_db).await;
});
```

- [ ] **Step 4: Compile check**

```bash
cargo check -p finsight-app
```

Expected: no errors. If you see issues with `tauri_plugin_notification::NotificationExt`, ensure it's imported — the trait is in scope when the plugin is in `Cargo.toml`.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/notifications.rs crates/finsight-app/src/lib.rs
git commit -m "feat: add notifications module (check_and_fire) with budget overflow and bill reminders"
```

---

### Task 8: Wire notification triggers + add get/set_notifications_enabled commands

**Files:**
- Modify: `crates/finsight-app/src/commands/settings.rs`
- Modify: `crates/finsight-app/src/commands/budget.rs`
- Modify: `crates/finsight-app/src/commands/import.rs`
- Modify: `crates/finsight-app/src/lib.rs`

- [ ] **Step 1: Add get/set_notifications_enabled to settings.rs**

In `crates/finsight-app/src/commands/settings.rs`, add after the existing `set_currency` command:

```rust
const NOTIFICATIONS_ENABLED_KEY: &str = "notifications.enabled";

#[tauri::command]
#[specta::specta]
pub async fn get_notifications_enabled(state: tauri::State<'_, AppState>) -> AppResult<bool> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let val: Option<bool> = settings::get(conn, NOTIFICATIONS_ENABLED_KEY)?;
        Ok(val.unwrap_or(true)) // default enabled
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_notifications_enabled(
    state: tauri::State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        settings::set(conn, NOTIFICATIONS_ENABLED_KEY, &enabled)
    })
    .await
    .map_err(AppError::from)
}
```

- [ ] **Step 2: Call check_and_fire after import_csv**

In `crates/finsight-app/src/commands/import.rs`, find the end of `import_csv` (the line that says `Ok(summary)`). Before it, add:

```rust
// Fire notifications after import (new transactions may push envelopes over budget)
let notify_app = app.clone();
let notify_db = (*state.db).clone();
tauri::async_runtime::spawn(async move {
    let _ = crate::notifications::check_and_fire(&notify_app, &notify_db).await;
});
```

You'll need `use crate::notifications;` or use the full path `crate::notifications::check_and_fire`. Since the function is `pub` in `crate::notifications`, the full path works without an explicit use.

- [ ] **Step 3: Call check_and_fire after apply_next_month_plan**

In `crates/finsight-app/src/commands/budget.rs`, find `apply_next_month_plan`. The function signature currently is:

```rust
pub async fn apply_next_month_plan(
    state: tauri::State<'_, AppState>,
    assignments: Vec<PlanAssignment>,
) -> AppResult<()> {
```

Change it to accept `AppHandle` (needed to fire notifications):

```rust
pub async fn apply_next_month_plan(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    assignments: Vec<PlanAssignment>,
) -> AppResult<()> {
```

Then at the end of the function, just before the final `Ok(())` (which is inside the `.map_err(AppError::from)` chain — actually it's from the `run()` return), restructure as follows. Find:

```rust
    .await
    .map_err(AppError::from)
}
```

Replace with:

```rust
    .await
    .map_err(AppError::from)?;

    let notify_db = (*state.db).clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::notifications::check_and_fire(&app, &notify_db).await;
    });
    Ok(())
}
```

- [ ] **Step 4: Register new commands in lib.rs**

In `build_specta_builder()`, add after the existing settings commands (search for `set_currency`):

```rust
commands::settings::get_notifications_enabled,
commands::settings::set_notifications_enabled,
```

- [ ] **Step 5: Compile and test**

```bash
cargo test --workspace
```

Expected: all tests pass. If there are issues with `app: tauri::AppHandle` not being provided to `apply_next_month_plan` in tests, check if there are integration tests that call it directly — if so, use `tauri::test::mock_app()` or restructure the test.

- [ ] **Step 6: Commit**

```bash
git add crates/finsight-app/src/commands/settings.rs crates/finsight-app/src/commands/budget.rs crates/finsight-app/src/commands/import.rs crates/finsight-app/src/lib.rs
git commit -m "feat: wire notification triggers and add get/set_notifications_enabled commands"
```

---

### Task 9: Regenerate bindings + Settings UI toggle

**Files:**
- Modify: `ui/src/api/bindings.ts` (auto-generated)
- Modify: `ui/src/screens/Settings.tsx`

- [ ] **Step 1: Regenerate bindings**

```bash
cargo run -p finsight-tauri --bin export_bindings
```

Expected: `bindings.ts` updated with `getNotificationsEnabled` and `setNotificationsEnabled`.

- [ ] **Step 2: Add notifications toggle to Settings.tsx**

In `ui/src/screens/Settings.tsx`, find the imports block and add:

```tsx
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
```

(These may already be imported — check and add only what's missing.)

Add a hook near the top of the `Settings` component function (after the existing `const { data: currentCurrency = "USD" } = ...` line):

```tsx
const qcSettings = useQueryClient();
const { data: notificationsEnabled = true } = useQuery({
  queryKey: ["notifications-enabled"],
  queryFn: async () => {
    const r = await commands.getNotificationsEnabled();
    if (r.status === "error") throw new Error(r.error.message);
    return r.data;
  },
});
const setNotifications = useMutation({
  mutationFn: async (enabled: boolean) => {
    const r = await commands.setNotificationsEnabled(enabled);
    if (r.status === "error") throw new Error(r.error.message);
  },
  onSuccess: () => qcSettings.invalidateQueries({ queryKey: ["notifications-enabled"] }),
});
```

Find the Appearance section (search for `§12c: Appearance section`). Inside the `<div style={{ display: "flex", flexDirection: "column", gap: 16 }}>`, after the Currency row and before the closing `</div>`, add:

```tsx
{/* Notifications */}
<div style={{ display: "flex", alignItems: "center", gap: 12 }}>
  <span style={{ width: 80, fontSize: 13, color: "var(--ink-mute)" }}>Notifications</span>
  <div className="toolbar" style={{ display: "inline-flex" }}>
    <button
      className={notificationsEnabled ? "on" : ""}
      aria-pressed={notificationsEnabled}
      onClick={() => setNotifications.mutate(true)}
    >
      On
    </button>
    <button
      className={!notificationsEnabled ? "on" : ""}
      aria-pressed={!notificationsEnabled}
      onClick={() => setNotifications.mutate(false)}
    >
      Off
    </button>
  </div>
  <span style={{ fontSize: 12, color: "var(--ink-faint)" }}>Budget alerts and bill reminders</span>
</div>
```

- [ ] **Step 3: TypeScript check**

```bash
cd ui && npx tsc --noEmit
```

Expected: 0 errors.

- [ ] **Step 4: Frontend tests**

```bash
cd ui && npx vitest run
```

Expected: all 105 tests pass.

- [ ] **Step 5: Full Rust tests**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add ui/src/api/bindings.ts ui/src/screens/Settings.tsx
git commit -m "feat: notifications Settings toggle and final bindings regeneration"
```

---

## Final verification

- [ ] Run `pnpm tauri:dev` and load demo data (Settings → Load demo data)
- [ ] Open a transaction, toggle Split on — confirm SplitModal opens
- [ ] Add two splits summing to the transaction total, save — confirm Categories screen shows spending in the split categories (not the parent)
- [ ] Toggle Split off — confirm chips shows un-split state
- [ ] Confirm Settings → Appearance → Notifications On/Off toggle works
- [ ] On a machine with a budget configured, verify a budget overflow notification fires on import (or check logs if in dev mode)

**Green bar after this wave:** 106+ Rust tests, 105 frontend tests, 0 TypeScript errors.
