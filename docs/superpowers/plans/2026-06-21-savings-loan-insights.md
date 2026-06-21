# Savings & Loan Insights Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire goals to liabilities and savings accounts so debt-payoff goals auto-sync, savings projections use real APY, and drawers prompt for missing metadata.

**Architecture:** Add `liability_id` and `account_id` to `goals`, keep `current_cents` as the synced source of truth, expose a backend projection command, and update the Goals screen + drawers.

**Tech Stack:** Rust (rusqlite, serde, specta, chrono), TypeScript/React (TanStack Query, react-hook-form, Vitest).

## Global Constraints

- All SQL changes live in `crates/finsight-core/migrations/`.
- All repository functions live in `crates/finsight-core/src/repos/`.
- All Tauri commands live in `crates/finsight-app/src/commands/` and must be `pub async fn` with `#[tauri::command]` and `#[specta::specta]`.
- Regenerate `ui/src/api/bindings.ts` via `cargo run -p finsight-tauri --bin export_bindings` after every Rust command/type change.
- Use design tokens from `ui/src/styles/tokens.css`; never hardcode colors.
- Tests must pass: `cargo test --workspace`, `cd ui && npx vitest run`, `cd ui && npx tsc --noEmit`.

---

## File structure

- **Create:**
  - `crates/finsight-core/migrations/V022__goal_links.sql` — add `liability_id`/`account_id` columns.
  - `crates/finsight-core/src/repos/goal_links.rs` — sync helpers and linked lookups (optional; can live in `goals.rs`).
- **Modify:**
  - `crates/finsight-core/src/repos/goals.rs` — add fields, update queries, add sync helper.
  - `crates/finsight-core/src/repos/liabilities.rs` — call sync helper on update/delete.
  - `crates/finsight-core/src/repos/accounts.rs` — add `get_by_id`.
  - `crates/finsight-app/src/commands/budget.rs` — add `project_goal_growth`, update `GoalDto`/`NewGoalInput`.
  - `crates/finsight-app/src/lib.rs` — register new command.
  - `ui/src/api/hooks/budget.ts` — add `useProjectGoalGrowth`, update `useCreateGoal` input type.
  - `ui/src/screens/Goals.tsx` — linked dropdowns, linked card UI, APY projection.
  - `ui/src/components/AccountDrawer.tsx` — APY hint.
  - `ui/src/components/LiabilityDrawer.tsx` — original-balance and start-date hints.
  - `ui/src/api/bindings.ts` — regenerated.
- **Test:**
  - `crates/finsight-core/src/repos/goals.rs` (inline tests).
  - `crates/finsight-core/src/repos/liabilities.rs` (inline tests).
  - `ui/src/screens/Goals.test.tsx` — goal card + form tests.
  - `ui/src/components/AccountDrawer.test.tsx` — APY hint.
  - `ui/src/components/LiabilityDrawer.test.tsx` — metadata hints.

---

### Task 1: Database migration

**Files:**
- Create: `crates/finsight-core/migrations/V022__goal_links.sql`

**Interfaces:**
- Produces: `goals.liability_id`, `goals.account_id` columns with FK constraints.

- [ ] **Step 1: Write migration**

```sql
ALTER TABLE goals ADD COLUMN liability_id TEXT REFERENCES liabilities(id) ON DELETE SET NULL;
ALTER TABLE goals ADD COLUMN account_id  TEXT REFERENCES accounts(id)  ON DELETE SET NULL;
```

- [ ] **Step 2: Verify migration runs**

Run: `cargo test -p finsight-core --lib db::tests::migrations_run`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/migrations/V022__goal_links.sql
git commit -m "migrate: add liability_id and account_id to goals"
```

---

### Task 2: Extend goals repo model and queries

**Files:**
- Modify: `crates/finsight-core/src/repos/goals.rs`

**Interfaces:**
- Consumes: new columns from migration.
- Produces: `Goal { liability_id, account_id }`, `NewGoal { liability_id, account_id }`, `GoalPatch { liability_id, account_id }`, `sync_linked_liabilities(conn, liability_id)`.

- [ ] **Step 1: Add fields to structs**

```rust
#[derive(Debug, Clone)]
pub struct Goal {
    pub id: String,
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub current_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub liability_id: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewGoal {
    pub name: String,
    pub goal_type: String,
    pub target_cents: i64,
    pub monthly_cents: i64,
    pub target_date: Option<String>,
    pub color: String,
    pub notes: Option<String>,
    pub purpose: Option<String>,
    pub liability_id: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GoalPatch {
    pub name: Option<String>,
    pub target_cents: Option<i64>,
    pub current_cents: Option<i64>,
    pub monthly_cents: Option<i64>,
    pub target_date: Option<Option<String>>,
    pub color: Option<String>,
    pub notes: Option<String>,
    pub purpose: Option<Option<String>>,
    pub liability_id: Option<Option<String>>,
    pub account_id: Option<Option<String>>,
}
```

- [ ] **Step 2: Update `list` query and mapping**

```rust
pub fn list(conn: &mut Connection) -> CoreResult<Vec<Goal>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, target_cents, current_cents, monthly_cents, \
                target_date, color, notes, purpose, sort_order, created_at, \
                liability_id, account_id \
         FROM goals WHERE archived_at IS NULL ORDER BY sort_order, created_at",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Goal {
            id: r.get(0)?,
            name: r.get(1)?,
            goal_type: r.get(2)?,
            target_cents: r.get(3)?,
            current_cents: r.get(4)?,
            monthly_cents: r.get(5)?,
            target_date: r.get(6)?,
            color: r.get(7)?,
            notes: r.get(8)?,
            purpose: r.get(9)?,
            sort_order: r.get(10)?,
            created_at: r.get(11)?,
            liability_id: r.get(12)?,
            account_id: r.get(13)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows { out.push(row?); }
    Ok(out)
}
```

- [ ] **Step 3: Update `insert`**

```rust
pub fn insert(conn: &mut Connection, g: NewGoal) -> CoreResult<Goal> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO goals(id, name, type, target_cents, current_cents, monthly_cents, \
                           target_date, color, notes, purpose, sort_order, created_at, \
                           liability_id, account_id)
         VALUES(?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8, ?9, 0, ?10, ?11, ?12)",
        params![
            id, g.name, g.goal_type, g.target_cents, g.monthly_cents,
            g.target_date, g.color, g.notes, g.purpose, now,
            g.liability_id, g.account_id
        ],
    )?;
    Ok(Goal {
        id, name: g.name, goal_type: g.goal_type, target_cents: g.target_cents,
        current_cents: 0, monthly_cents: g.monthly_cents, target_date: g.target_date,
        color: g.color, notes: g.notes, purpose: g.purpose, sort_order: 0,
        created_at: now, liability_id: g.liability_id, account_id: g.account_id,
    })
}
```

- [ ] **Step 4: Add sync helper**

```rust
pub fn sync_linked_liabilities(conn: &mut Connection, liability_id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE goals SET current_cents = COALESCE((SELECT balance_cents FROM liabilities WHERE id = ?1), 0)
         WHERE liability_id = ?1",
        params![liability_id],
    )?;
    Ok(())
}
```

- [ ] **Step 5: Add a getter for single goal**

```rust
pub fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Goal> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, target_cents, current_cents, monthly_cents, \
                target_date, color, notes, purpose, sort_order, created_at, \
                liability_id, account_id \
         FROM goals WHERE id = ?1 AND archived_at IS NULL",
    )?;
    let mut rows = stmt.query_map(params![id], |r| {
        Ok(Goal {
            id: r.get(0)?, name: r.get(1)?, goal_type: r.get(2)?,
            target_cents: r.get(3)?, current_cents: r.get(4)?, monthly_cents: r.get(5)?,
            target_date: r.get(6)?, color: r.get(7)?, notes: r.get(8)?,
            purpose: r.get(9)?, sort_order: r.get(10)?, created_at: r.get(11)?,
            liability_id: r.get(12)?, account_id: r.get(13)?,
        })
    })?;
    rows.next().transpose()?.ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into())
}
```

- [ ] **Step 6: Update tests to compile**

Update `set_monthly_cents_updates_correctly` test's `NewGoal` to include `liability_id: None, account_id: None`.

- [ ] **Step 7: Run cargo check**

Run: `cargo check -p finsight-core`
Expected: no errors

- [ ] **Step 8: Commit**

```bash
git add crates/finsight-core/src/repos/goals.rs
git commit -m "feat: add liability_id/account_id to goals repo"
```

---

### Task 3: Wire liability sync into liabilities repo

**Files:**
- Modify: `crates/finsight-core/src/repos/liabilities.rs`

**Interfaces:**
- Consumes: `goals::sync_linked_liabilities`.
- Produces: automatic `current_cents` sync on liability balance update.

- [ ] **Step 1: Import sync helper**

```rust
use crate::repos::goals;
```

- [ ] **Step 2: Call sync after balance update**

In `update`, after the `balance_cents` update block:

```rust
if patch.balance_cents.is_some() {
    goals::sync_linked_liabilities(conn, id)?;
}
```

- [ ] **Step 3: Add test for sync**

```rust
#[test]
fn updating_liability_balance_syncs_linked_goal() {
    use crate::repos::goals;
    let (_d, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let l = create(&mut conn, NewLiability {
        name: "Car loan".into(), liability_type: "loan".into(), balance_cents: 15_000_000,
        limit_cents: None, apr_pct: None, min_payment_cents: None,
        payoff_date: None, original_balance_cents: Some(20_000_000),
        started_at: None, currency: "USD".into(),
    }).unwrap();
    let goal = goals::insert(&mut conn, goals::NewGoal {
        name: "Pay off car".into(), goal_type: "debt-payoff".into(),
        target_cents: 20_000_000, monthly_cents: 500_00,
        target_date: None, color: "#C9F950".into(), notes: None, purpose: None,
        liability_id: Some(l.id.clone()), account_id: None,
    }).unwrap();
    assert_eq!(goal.current_cents, 0);

    update(&mut conn, &l.id, LiabilityPatch { balance_cents: Some(12_000_000), ..Default::default() }).unwrap();
    let synced = goals::get_by_id(&mut conn, &goal.id).unwrap();
    assert_eq!(synced.current_cents, 12_000_000);
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p finsight-core --lib repos::liabilities::tests`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/repos/liabilities.rs
git commit -m "feat: sync linked goal balances when liability updates"
```

---

### Task 4: Add account getter

**Files:**
- Modify: `crates/finsight-core/src/repos/accounts.rs`

**Interfaces:**
- Produces: `accounts::get_by_id(conn, id) -> CoreResult<Account>`.

- [ ] **Step 1: Add function after `update`**

```rust
pub fn get_by_id(conn: &mut Connection, id: &str) -> CoreResult<Account> {
    conn.query_row(
        "SELECT id, owner, bank, type, name, last4, currency, color, archived_at, \
                liquidity_type, emergency_fund_eligible, goal_earmark, apy_pct, created_at \
         FROM accounts WHERE id = ?1",
        params![id],
        |r| {
            let archived_s: Option<String> = r.get(8)?;
            let created_s: String = r.get(13)?;
            Ok(Account {
                id: r.get(0)?, owner: r.get(1)?, bank: r.get(2)?,
                r#type: AccountType::from_db(&r.get::<_, String>(3)?),
                name: r.get(4)?, last4: r.get(5)?, currency: r.get(6)?,
                color: r.get(7)?,
                archived_at: archived_s.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                liquidity_type: r.get(9)?,
                emergency_fund_eligible: r.get::<_, i64>(10)? != 0,
                goal_earmark: r.get(11)?,
                apy_pct: r.get(12)?,
                created_at: DateTime::parse_from_rfc3339(&created_s).unwrap().with_timezone(&Utc),
            })
        },
    )
    .map_err(Into::into)
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p finsight-core`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/src/repos/accounts.rs
git commit -m "feat: add get_by_id for accounts"
```

---

### Task 5: Add APY-aware projection command

**Files:**
- Modify: `crates/finsight-app/src/commands/budget.rs`

**Interfaces:**
- Consumes: `goals::get_by_id`, `accounts::get_by_id`.
- Produces: `project_goal_growth(goal_id, years) -> ProjectedValue`.

- [ ] **Step 1: Add return DTO**

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectedValue {
    pub years: i32,
    pub value_cents: i64,
    pub annual_rate: f64,
}
```

- [ ] **Step 2: Add command**

```rust
#[tauri::command]
#[specta::specta]
pub async fn project_goal_growth(
    state: tauri::State<'_, AppState>,
    goal_id: String,
    years: i32,
) -> AppResult<ProjectedValue> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let goal = goals::get_by_id(conn, &goal_id)?;
        let annual_rate = if let Some(account_id) = &goal.account_id {
            accounts::get_by_id(conn, account_id)
                .ok()
                .and_then(|a| a.apy_pct)
                .unwrap_or(0.07)
                / 100.0
        } else {
            0.07
        };
        let value_cents = if goal.monthly_cents <= 0 || years <= 0 {
            0
        } else {
            let r = annual_rate / 12.0;
            let n = (years * 12) as i64;
            let fv = goal.monthly_cents as f64 * ((f64::powi(1.0 + r, n as i32) - 1.0) / r);
            fv.round() as i64
        };
        Ok(ProjectedValue { years, value_cents, annual_rate })
    })
    .await
    .map_err(AppError::from)
}
```

- [ ] **Step 3: Update GoalDto and NewGoalInput**

Add to `GoalDto`:
```rust
pub liability_id: Option<String>,
pub account_id: Option<String>,
```

Add to `NewGoalInput`:
```rust
pub liability_id: Option<String>,
pub account_id: Option<String>,
```

Update `goal_to_dto` to include new fields. Update `create_goal` to pass them into `goals::NewGoal`.

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p finsight-app`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/budget.rs
git commit -m "feat: add project_goal_growth command and GoalDto links"
```

---

### Task 6: Register command and regenerate bindings

**Files:**
- Modify: `crates/finsight-app/src/lib.rs`
- Modify: `ui/src/api/bindings.ts` (regenerated)

**Interfaces:**
- Produces: TypeScript `commands.projectGoalGrowth` and updated `GoalDto`/`NewGoalInput` types.

- [ ] **Step 1: Register command**

Add to `build_specta_builder()` in the budget section:
```rust
commands::budget::project_goal_growth,
```

- [ ] **Step 2: Regenerate bindings**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: success; `ui/src/api/bindings.ts` updated.

- [ ] **Step 3: Run TypeScript check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/lib.rs ui/src/api/bindings.ts
git commit -m "chore: register project_goal_growth and regenerate bindings"
```

---

### Task 7: Frontend hooks

**Files:**
- Modify: `ui/src/api/hooks/budget.ts`

**Interfaces:**
- Produces: `useProjectGoalGrowth(goalId, years)`, updated `useCreateGoal` input.

- [ ] **Step 1: Add projection hook**

```typescript
import { commands, type ProjectedValue } from "../client";

export function useProjectGoalGrowth(goalId: string | undefined, years: number) {
  return useQuery<ProjectedValue>({
    queryKey: ["goal-projection", goalId, years],
    queryFn: async () => {
      if (!goalId) throw new Error("goalId required");
      const result = await commands.projectGoalGrowth(goalId, years);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime() && !!goalId,
  });
}
```

- [ ] **Step 2: Update createGoal mutation input**

The mutation already takes `NewGoalInput` which now includes `liabilityId`/`accountId` from bindings.

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/hooks/budget.ts
git commit -m "feat: add useProjectGoalGrowth hook"
```

---

### Task 8: NewGoalForm linked fields

**Files:**
- Modify: `ui/src/screens/Goals.tsx`

**Interfaces:**
- Consumes: `useLiabilities`, `useAccounts`, `useCreateGoal`.
- Produces: form state for `liabilityId`/`accountId` and filtered dropdowns.

- [ ] **Step 1: Add imports and state**

```typescript
import { useLiabilities } from "../api/hooks/assets";
import { useAccounts } from "../api/hooks/accounts";
import { useProjectGoalGrowth } from "../api/hooks/budget";
import { useLiabilities } from "../api/hooks/assets";
import { useAccounts } from "../api/hooks/accounts";

// inside NewGoalForm:
const [liabilityId, setLiabilityId] = useState<string>("");
const [accountId, setAccountId] = useState<string>("");
```

- [ ] **Step 2: Filter options**

```typescript
const { data: liabilities = [] } = useLiabilities();
const { data: accounts = [] } = useAccounts();
const linkableLiabilities = liabilities.filter((l) => l.balanceCents > 0);
const savingsAccounts = accounts.filter((a) => a.type === "Savings");
```

- [ ] **Step 3: Auto-fill target from liability original balance**

```typescript
const selectedLiability = liabilities.find((l) => l.id === liabilityId);
useEffect(() => {
  if (selectedLiability?.originalBalanceCents && !target) {
    setTarget(String(selectedLiability.originalBalanceCents / 100));
  }
}, [liabilityId, selectedLiability, target]);
```

- [ ] **Step 4: Enforce one link at a time**

```typescript
const handleLiabilityChange = (id: string) => {
  setLiabilityId(id);
  if (id) setAccountId("");
};
const handleAccountChange = (id: string) => {
  setAccountId(id);
  if (id) setLiabilityId("");
};
```

- [ ] **Step 5: Add dropdowns and update submit**

Add inside form grid:
```tsx
<Select label="Linked liability (optional)" value={liabilityId} onChange={(e) => handleLiabilityChange(e.target.value)}>
  <option value="">None</option>
  {linkableLiabilities.map((l) => <option key={l.id} value={l.id}>{l.name} · {money(l.balanceCents)}</option>)}
</Select>
<Select label="Linked savings account (optional)" value={accountId} onChange={(e) => handleAccountChange(e.target.value)}>
  <option value="">None</option>
  {savingsAccounts.map((a) => <option key={a.id} value={a.id}>{a.bank} {a.name} · {a.apyPct ?? "—"}% APY</option>)}
</Select>
```

Update `input`:
```typescript
const input: NewGoalInput = {
  ...existingFields,
  liabilityId: liabilityId || null,
  accountId: accountId || null,
};
```

- [ ] **Step 6: Commit**

```bash
git add ui/src/screens/Goals.tsx
git commit -m "feat: add linked liability/account fields to new goal form"
```

---

### Task 9: GoalCard linked state and APY projection

**Files:**
- Modify: `ui/src/screens/Goals.tsx`

**Interfaces:**
- Consumes: `useProjectGoalGrowth`, linked liability/account data from `GoalDto`.
- Produces: read-only balance for linked liabilities, APY-aware projection panel.

- [ ] **Step 1: Use projection hook and linked lookups**

```typescript
const { data: proj10 } = useProjectGoalGrowth(goal.id, 10);
const { data: proj20 } = useProjectGoalGrowth(goal.id, 20);
const { data: proj30 } = useProjectGoalGrowth(goal.id, 30);
const { data: liabilities = [] } = useLiabilities();
const { data: accounts = [] } = useAccounts();
const linkedLiability = liabilities.find((l) => l.id === goal.liabilityId);
const linkedAccount = accounts.find((a) => a.id === goal.accountId);
```

- [ ] **Step 2: Disable manual balance edit when linked**

```typescript
const isLinkedToLiability = !!goal.liabilityId;
```

Replace balance edit button with static display when linked:
```tsx
{isLinkedToLiability ? (
  <span className="num money">{money(goal.currentCents)}</span>
) : (
  <Button onClick={() => setEditingBalance(true)} ...>...</Button>
)}
```

- [ ] **Step 3: Show linked liability/account info**

Add after progress bar:
```tsx
{linkedLiability && (
  <div className="row row-sm" style={{ fontSize: 12, color: "var(--ink-mute)", marginTop: 8 }}>
    Linked to {linkedLiability.name} · {linkedLiability.aprPct ?? "—"}% APR · updates automatically
  </div>
)}
{linkedAccount && (
  <div className="row row-sm" style={{ fontSize: 12, color: "var(--ink-mute)", marginTop: 8 }}>
    Linked to {linkedAccount.name} · {(proj10?.annualRate ?? 0.07) * 100}% APY
  </div>
)}
```

- [ ] **Step 4: Update projection panel**

Replace hard-coded 7% copy and `projectCompoundValue` usage:
```tsx
<div style={{ fontSize: 13.5, marginBottom: 10 }}>
  If you invest <span className="money">{money(goal.monthlyCents)}</span>/month for the long run at {(proj10?.annualRate ?? 0.07) * 100}% APY:
</div>
{[proj10, proj20, proj30].map((p) => p && (
  <div key={p.years} ...>
    <span className="muted">{p.years} years</span>
    <span className="money" style={{ fontWeight: 600 }}>{money(p.valueCents)}</span>
  </div>
))}
```

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Goals.tsx
git commit -m "feat: render linked state and APY projection in GoalCard"
```

---

### Task 10: AccountDrawer APY hint

**Files:**
- Modify: `ui/src/components/AccountDrawer.tsx`

**Interfaces:**
- Consumes: form `type` and `apy_pct` values.
- Produces: inline hint when Savings is selected and APY empty.

- [ ] **Step 1: Add hint markup**

After the APY input:
```tsx
{watch("type") === "Savings" && !watch("apy_pct") && (
  <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
    Add an APY so savings projections use your real rate.
  </div>
)}
```

- [ ] **Step 2: Commit**

```bash
git add ui/src/components/AccountDrawer.tsx
git commit -m "feat: add APY hint to AccountDrawer"
```

---

### Task 11: LiabilityDrawer metadata hints

**Files:**
- Modify: `ui/src/components/LiabilityDrawer.tsx`

**Interfaces:**
- Consumes: form `original_balance_dollars` and `started_at` values.
- Produces: inline hints when fields are empty.

- [ ] **Step 1: Add hints**

After original balance input:
```tsx
{!watch("original_balance_dollars") && (
  <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
    Add an original balance to track payoff progress.
  </div>
)}
```

After started_at input:
```tsx
{!watch("started_at") && (
  <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
    Add a start date to see how long you've been paying this down.
  </div>
)}
```

- [ ] **Step 2: Commit**

```bash
git add ui/src/components/LiabilityDrawer.tsx
git commit -m "feat: add original balance and start date hints to LiabilityDrawer"
```

---

### Task 12: Rust tests

**Files:**
- Modify: `crates/finsight-core/src/repos/goals.rs`
- Modify: `crates/finsight-core/src/repos/liabilities.rs` (already added in Task 3)

**Interfaces:**
- Produces: passing tests for migration, sync, and FK behavior.

- [ ] **Step 1: Add goal link test**

```rust
#[test]
fn insert_goal_with_links_round_trip() {
    use crate::repos::liabilities;
    let (_d, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let l = liabilities::create(&mut conn, liabilities::NewLiability {
        name: "Loan".into(), liability_type: "loan".into(), balance_cents: 5_000_00,
        limit_cents: None, apr_pct: None, min_payment_cents: None,
        payoff_date: None, original_balance_cents: None, started_at: None, currency: "USD".into(),
    }).unwrap();
    let goal = insert(&mut conn, NewGoal {
        name: "Payoff".into(), goal_type: "debt-payoff".into(), target_cents: 5_000_00, monthly_cents: 100_00,
        target_date: None, color: "#C9F950".into(), notes: None, purpose: None,
        liability_id: Some(l.id.clone()), account_id: None,
    }).unwrap();
    assert_eq!(goal.liability_id, Some(l.id));
    let fetched = get_by_id(&mut conn, &goal.id).unwrap();
    assert_eq!(fetched.liability_id, Some(l.id));
}
```

- [ ] **Step 2: Add delete-clears-link test**

```rust
#[test]
fn deleting_liability_clears_goal_link() {
    use crate::repos::liabilities;
    let (_d, db) = fresh_db();
    let mut conn = db.get().unwrap();
    let l = liabilities::create(&mut conn, liabilities::NewLiability {
        name: "Loan".into(), liability_type: "loan".into(), balance_cents: 5_000_00,
        limit_cents: None, apr_pct: None, min_payment_cents: None,
        payoff_date: None, original_balance_cents: None, started_at: None, currency: "USD".into(),
    }).unwrap();
    let goal = insert(&mut conn, NewGoal {
        name: "Payoff".into(), goal_type: "debt-payoff".into(), target_cents: 5_000_00, monthly_cents: 100_00,
        target_date: None, color: "#C9F950".into(), notes: None, purpose: None,
        liability_id: Some(l.id.clone()), account_id: None,
    }).unwrap();
    liabilities::delete(&mut conn, &l.id).unwrap();
    let fetched = get_by_id(&mut conn, &goal.id).unwrap();
    assert!(fetched.liability_id.is_none());
}
```

- [ ] **Step 3: Run Rust tests**

Run: `cargo test --workspace`
Expected: PASS (except ignored Linux keychain tests)

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/repos/goals.rs crates/finsight-core/src/repos/liabilities.rs
git commit -m "test: goal link round-trip and liability sync"
```

---

### Task 13: Frontend tests

**Files:**
- Modify: `ui/src/screens/Goals.test.tsx` (or create)
- Modify: `ui/src/components/AccountDrawer.test.tsx`
- Modify: `ui/src/components/LiabilityDrawer.test.tsx`

**Interfaces:**
- Produces: passing jsdom tests for linked UI and hints.

- [ ] **Step 1: Add GoalCard linked-liability test**

```typescript
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const mockGoal = {
  id: "g1", name: "Car payoff", goalType: "debt-payoff",
  targetCents: 2000000, currentCents: 1500000, monthlyCents: 50000,
  targetDate: null, color: "#C9F950", notes: null, purpose: null,
  sortOrder: 0, createdAt: "2024-01-01T00:00:00Z",
  liabilityId: "l1", accountId: null,
};

it("renders linked liability goal without manual edit button", () => {
  render(<QueryClientProvider client={new QueryClient()}><Goals /></QueryClientProvider>);
  expect(screen.getByText("Car payoff")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /edit balance/i })).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Add AccountDrawer hint test**

```typescript
it("shows APY hint for Savings accounts without APY", async () => {
  render(<AccountDrawer open account={{ ...mockAccount, type: "Savings", apyPct: null }} onClose={() => {}} />);
  expect(await screen.findByText(/Add an APY so savings projections/i)).toBeInTheDocument();
});
```

- [ ] **Step 3: Add LiabilityDrawer hint tests**

```typescript
it("shows original balance hint when empty", async () => {
  render(<LiabilityDrawer open liability={{ ...mockLiability, originalBalanceCents: null }} onClose={() => {}} />);
  expect(await screen.findByText(/Add an original balance/i)).toBeInTheDocument();
});

it("shows start date hint when empty", async () => {
  render(<LiabilityDrawer open liability={{ ...mockLiability, startedAt: null }} onClose={() => {}} />);
  expect(await screen.findByText(/Add a start date/i)).toBeInTheDocument();
});
```

- [ ] **Step 4: Run frontend tests**

Run: `cd ui && npx vitest run`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Goals.test.tsx ui/src/components/AccountDrawer.test.tsx ui/src/components/LiabilityDrawer.test.tsx
git commit -m "test: linked goals, APY projection, drawer hints"
```

---

### Task 14: Full green bar and final checks

**Files:**
- Workspace

- [ ] **Step 1: Rust tests**

Run: `cargo test --workspace`
Expected: all pass (Linux keychain ignored)

- [ ] **Step 2: Frontend tests**

Run: `cd ui && npx vitest run`
Expected: PASS

- [ ] **Step 3: TypeScript check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors

- [ ] **Step 4: Review diff**

Run: `git diff --stat`
Expected: targeted changes only; no stray edits.

- [ ] **Step 5: Final commit if clean**

```bash
git status
# if only expected changes remain:
git commit --amend -m "feat: savings and loan insights"
```

---

## Self-review checklist

- [ ] **Spec coverage:** Every spec requirement maps to a task above.
- [ ] **Placeholder scan:** No "TBD", "TODO", or vague "add validation" steps.
- [ ] **Type consistency:** `liabilityId`/`accountId` naming matches bindings; Rust `liability_id`/`account_id` matches DB columns.
- [ ] **FK behavior:** `ON DELETE SET NULL` is covered by migration and tested.
- [ ] **APY fallback:** 7% default is implemented in command and UI.
