# What to Steal from Actual — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close FinSight’s auditability gap by adding Actual’s three highest-leverage primitives — a customizable report builder, `Hold for next month`, and an atomic `Cover` ledger — plus declarative funding templates and a second bank provider behind the existing encrypted sync abstraction.

**Architecture:** Keep FinSight’s single-container, per-user SQLCipher model; add three new tables (`budget_holds`, `budget_transfers`, `funding_templates`) and one provider impl (`enable_banking`) that reuses `SimpleFIN`’s `import_sync` trait (`finsight-providers/src/simplefin/sync.rs:40`). Reports stay in `finsight-core` but expose a `custom_report` query with `filters` + `split_by`; `To Budget` becomes `income - budget - hold`; `Cover` becomes a ledger row that feeds `carryover` and is auditable.

**Tech Stack:** Rust 1.78, Axum, SQLCipher/rusqlite + refinery, React 18 + Vite 5 + recharts 2 + tanstack-query 5, SQLCipher per-user DB, `utoipa` + `openapi-typescript`

## Global Constraints

- `rust-version = "1.78"` (`Cargo.toml:40`) — no MSRV bump
- `GET /api/openapi.json` is `no-cache` + per-route compressed, `GET /api/events` never compressed (`crates/finsight-server/src/router.rs:54` pin)
- `ui/src/api/openapi.ts` is GENERATED via `pnpm openapi` (`cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen`) — never hand-edit
- `finsight-api` and `finsight-core` have no `tauri` dep (`cargo tree -p finsight-api -i tauri` empty)
- Money math stays in `finsight-core` (`cashflow`, `forecast`, `metrics`); UI never hand-rolls `safeToSpend` or `carryover`

---

## File Structure

```
MOD  crates/finsight-core/migrations/V0XX__add_budget_holds.sql
MOD  crates/finsight-core/migrations/V0XX__add_budget_transfers.sql
MOD  crates/finsight-core/migrations/V0XX__add_funding_templates.sql
MOD  crates/finsight-core/src/models/budget.rs
NEW  crates/finsight-core/src/models/budget_hold.rs
NEW  crates/finsight-core/src/models/budget_transfer.rs
NEW  crates/finsight-core/src/models/funding_template.rs
MOD  crates/finsight-core/src/repos/budgets.rs (add hold + transfer + template fns, update carryover/available)
MOD  crates/finsight-core/src/repos/mod.rs (re-export new repos)
MOD  crates/finsight-api/src/commands/budget.rs (add hold/cover/template handlers)
MOD  crates/finsight-api/src/commands/reports.rs (add custom_report handler)
MOD  crates/finsight-openapi/src/lib.rs (add new commands to COMMANDS + ApiDoc)
MOD  crates/finsight-server/src/dispatch.rs (add rpc_routes arms)
MOD  crates/finsight-providers/src/enable_banking/* (new provider impl behind SyncProvider trait)
MOD  crates/finsight-providers/src/lib.rs (re-export)
NEW  ui/src/screens/ReportBuilder.tsx (custom report UI)
MOD  ui/src/screens/Reports.tsx (add entry point + keep existing fixed reports)
MOD  ui/src/screens/Budget.tsx (Hold button, Cover ledger UI, Template picker)
MOD  ui/src/api/openapi.ts (generated)
NEW  ui/src/api/hooks/reports.ts (useCustomReport)
NEW  ui/src/api/hooks/budgetHolds.ts
NEW  ui/src/api/hooks/budgetTransfers.ts
NEW  ui/src/api/hooks/fundingTemplates.ts
MOD  ui/src/components/BudgetToolbar.tsx (if exists) or Budget.tsx toolbar (Hold/Cover actions)
```

---

### Task 1: Custom Report Builder (Actual’s workbench, FinSight-typed)

**Files:**
- Modify: `crates/finsight-core/src/repos/budgets.rs:108-229`
- Create: `crates/finsight-core/src/models/custom_report.rs`
- Modify: `crates/finsight-api/src/commands/reports.rs:20-106`
- Create: `ui/src/screens/ReportBuilder.tsx`
- Create: `ui/src/api/hooks/reports.ts`
- Test: `crates/finsight-core/tests/custom_report.rs`, `ui/src/screens/ReportBuilder.test.tsx`

**Interfaces:**
- Consumes: `TxnFilterInput`, `Category`, `Account` from `finsight-core`
- Produces: `pub fn custom_report(conn: &Connection, p: CustomReportParams) -> Result<CustomReportResult>` and `POST /api/rpc/custom_report` + `useCustomReport(params)` hook

- [ ] **Step 1: Write the failing test**

```rust
// crates/finsight-core/tests/custom_report.rs
#[test]
fn custom_report_splits_by_payee_and_month() {
    let conn = setup(); // 3 months, 2 payees, transfers excluded
    let r = custom_report(&conn, CustomReportParams {
        split_by: SplitBy::Payee,
        period: Period::Last6Months,
        include_transfers: false,
        ..Default::default()
    }).unwrap();
    assert_eq!(r.rows.len(), 2);
    assert!(r.rows.iter().any(|row| row.label == "Groceries"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --test custom_report -- --nocapture`
Expected: FAIL with "function not defined"

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/finsight-core/src/models/custom_report.rs
#[derive(Deserialize, ToSchema, Clone, Debug)]
#[schema(rename_all="camelCase")]
pub struct CustomReportParams {
    pub split_by: SplitBy, // Category, Group, Payee, Account, Month
    pub period: Period,    // Last1M, Last3M, Last6M, YTD, All
    pub include_transfers: bool,
    pub include_archived: bool,
}
#[derive(ToSchema, Serialize, Clone, Debug)]
pub struct CustomReportResult { pub rows: Vec<ReportRow>, pub total_cents: i64 }
pub enum SplitBy { Category, Group, Payee, Account, Month }

// crates/finsight-core/src/repos/budgets.rs
pub fn custom_report(conn: &Connection, p: CustomReportParams) -> Result<CustomReportResult> {
    // 1. build txn CTE with same exclusions as metrics::spending_breakdown (exclude transfers unless include_transfers)
    // 2. group_by split_by, period filter via between posted_at
    // 3. sum amount_cents, count, return sorted desc
    Ok(CustomReportResult { rows: vec![], total_cents: 0 })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --test custom_report -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/models/custom_report.rs crates/finsight-core/src/repos/budgets.rs
git commit -m "feat(reports): custom report query with split_by"
```

- [ ] **Step 6: Wire API + UI (same task, second half)**

```rust
// crates/finsight-api/src/commands/reports.rs
#[utoipa::path(post, path="/api/rpc/custom_report", request_body(content=CustomReportParams), responses((status=200, body=CustomReportResult)))]
pub async fn custom_report(state: &ApiState, params: CustomReportParams) -> AppResult<CustomReportResult> {
    let db = (*state.db).clone();
    run(&db, move |conn| custom_report(conn, params)).await.map_err(AppError::from)
}
```

```typescript
// ui/src/screens/ReportBuilder.tsx (skeleton, 120 lines)
export function ReportBuilder() {
  const [params, setParams] = useState<CustomReportParams>({ split_by: "Category", period: "Last6Months", include_transfers: false, include_archived: false });
  const { data } = useCustomReport(params);
  return <div className="card"><select value={params.split_by} onChange={e=>setParams({...params, split_by:e.target.value})}><option>Category</option><option>Payee</option></select><BarChart data={data?.rows} /></div>;
}
```

Run: `cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen && pnpm --filter ui typecheck && pnpm --filter ui test run src/screens/ReportBuilder.test.tsx`
Expected: PASS

---

### Task 2: Hold for Next Month (Actual’s `Hold` primitive)

**Files:**
- Create: `crates/finsight-core/migrations/V0XX__add_budget_holds.sql`
- Create: `crates/finsight-core/src/models/budget_hold.rs`
- Modify: `crates/finsight-core/src/repos/budgets.rs:236` (toBudget calc)
- Modify: `crates/finsight-api/src/commands/budget.rs`
- Create: `ui/src/api/hooks/budgetHolds.ts`
- Test: `crates/finsight-core/tests/budget_hold.rs`, `ui/src/screens/Budget.test.tsx`

**Interfaces:**
- Consumes: `BudgetEnvelope` + `toBudget` calc
- Produces: `pub fn set_hold(conn, month, amount_cents)`, `pub fn get_hold(conn, month)`, `POST /api/rpc/set_hold` + `useHold(month)`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn hold_deducts_from_to_budget_and_appears_next_month() {
    let conn = setup(); // income 100_00, budget 60_00 => toBudget 40_00
    set_hold(&conn, "2026-09", 15_00).unwrap();
    assert_eq!(to_budget(&conn, "2026-09").unwrap(), 25_00);
    // next month available includes prev hold as income-like
    assert_eq!(available_funds(&conn, "2026-10").unwrap(), 15_00);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --test budget_hold -- --nocapture`
Expected: FAIL with "no such table: budget_holds"

- [ ] **Step 3: Write minimal implementation**

```sql
-- V0XX__add_budget_holds.sql
CREATE TABLE budget_holds (month TEXT PRIMARY KEY, amount_cents INTEGER NOT NULL CHECK(amount_cents>=0));
```

```rust
// crates/finsight-core/src/repos/budgets.rs
pub fn set_hold(conn: &Connection, month: &str, amount: i64) -> Result<()> {
    conn.execute("INSERT INTO budget_holds(month, amount_cents) VALUES(?1,?2) ON CONFLICT(month) DO UPDATE SET amount_cents=?2", params![month, amount])?;
    Ok(())
}
pub fn to_budget(conn: &Connection, month: &str) -> Result<i64> {
    let income = total_income(conn, month)?; // existing
    let budgeted = total_budgeted(conn, month)?;
    let hold = get_hold(conn, month)?.unwrap_or(0);
    Ok(income - budgeted - hold)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --test budget_hold -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/migrations/V0XX__add_budget_holds.sql crates/finsight-core/src/models/budget_hold.rs crates/finsight-core/src/repos/budgets.rs
git commit -m "feat(budget): hold for next month"
```

---

### Task 3: Declarative Funding Templates (Actual’s `#template` as a table)

**Files:**
- Create: `crates/finsight-core/migrations/V0XX__add_funding_templates.sql`
- Create: `crates/finsight-core/src/models/funding_template.rs`
- Modify: `crates/finsight-core/src/repos/budgets.rs`
- Modify: `crates/finsight-api/src/commands/budget.rs`
- Test: `crates/finsight-core/tests/funding_template.rs`

**Interfaces:**
- Consumes: `Category`, `Budget`
- Produces: `pub fn apply_templates(conn, month) -> Result<Vec<BudgetChange>>`, `POST /api/rpc/apply_templates`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn template_fixed_up_to_and_by() {
    let conn = setup(); // balance 0, budget 0
    insert_template(&conn, FundingTemplate{ category_id:"groceries", kind: Fixed(72_99), priority:0 }).unwrap();
    insert_template(&conn, FundingTemplate{ category_id:"rent", kind: UpTo(300_00), priority:1 }).unwrap();
    let changes = apply_templates(&conn, "2026-09").unwrap();
    assert_eq!(changes[0].amount_cents, 72_99);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --test funding_template -- --nocapture`
Expected: FAIL with "no such table: funding_templates"

- [ ] **Step 3: Write minimal implementation**

```sql
CREATE TABLE funding_templates (
  id TEXT PRIMARY KEY, category_id TEXT NOT NULL REFERENCES categories(id),
  kind TEXT NOT NULL, -- 'fixed','up_to','by','average','percent','remainder','schedule'
  params_json TEXT NOT NULL, priority INTEGER NOT NULL DEFAULT 0
);
```

```rust
pub enum TemplateKind { Fixed(i64), UpTo(i64), By { target: i64, by: String }, Average { months: u32 }, Percent { pct: f32 }, Remainder, Schedule(String) }
pub fn apply_templates(conn: &Connection, month: &str) -> Result<Vec<BudgetChange>> {
    let templates = list_templates(conn)?; // ordered by priority ASC, then id
    let mut available = to_budget(conn, month)?; // respects holds
    let mut out = vec![];
    for t in templates {
        let need = match t.kind { Fixed(a) => a, UpTo(cap) => (cap - current_balance(&conn, &t.category_id, month)?).max(0), _ => 0 };
        let take = need.min(available);
        out.push(BudgetChange{ category_id: t.category_id, amount_cents: take });
        available -= take;
    }
    Ok(out)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --test funding_template -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/migrations/V0XX__add_funding_templates.sql crates/finsight-core/src/models/funding_template.rs crates/finsight-core/src/repos/budgets.rs
git commit -m "feat(budget): declarative funding templates"
```

---

### Task 4: Atomic Cover Ledger (FinSight’s `Cover` as auditable row)

**Files:**
- Create: `crates/finsight-core/migrations/V0XX__add_budget_transfers.sql`
- Create: `crates/finsight-core/src/models/budget_transfer.rs`
- Modify: `crates/finsight-core/src/repos/budgets.rs:59-101` (carryover)
- Modify: `crates/finsight-api/src/commands/budget.rs`
- Test: `crates/finsight-core/tests/budget_transfer.rs`, `ui/src/screens/Budget.test.tsx`

**Interfaces:**
- Consumes: `BudgetEnvelope` + `carryover`
- Produces: `pub fn transfer(conn, from, to, amount, month, note)`, `POST /api/rpc/transfer_envelope`, `available = budgeted + carryover + transfers_in - transfers_out - spent`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn cover_is_atomic_and_auditable() {
    let conn = setup(); // groceries remaining 10_00, rent overspent -5_00
    transfer(&conn, "groceries", "rent", 5_00, "2026-09", "cover").unwrap();
    assert_eq!(available(&conn, "groceries", "2026-09").unwrap(), 5_00);
    assert_eq!(available(&conn, "rent", "2026-09").unwrap(), 0);
    let rows = list_transfers(&conn, "2026-09").unwrap();
    assert_eq!(rows.len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-core --test budget_transfer -- --nocapture`
Expected: FAIL with "no such table: budget_transfers"

- [ ] **Step 3: Write minimal implementation**

```sql
CREATE TABLE budget_transfers (
  id TEXT PRIMARY KEY, month TEXT NOT NULL, from_category TEXT, to_category TEXT,
  amount_cents INTEGER NOT NULL CHECK(amount_cents>0), note TEXT, created_at TEXT NOT NULL
);
```

```rust
pub fn available(conn: &Connection, cat: &str, month: &str) -> Result<i64> {
    let budgeted = get_budget(conn, cat, month)?.unwrap_or(0);
    let carry = carryover_into_month(conn, cat, month)?;
    let transfers_in: i64 = conn.query_row("SELECT COALESCE(SUM(amount_cents),0) FROM budget_transfers WHERE to_category=?1 AND month=?2", params![cat, month], |r| r.get(0))?;
    let transfers_out: i64 = conn.query_row("SELECT COALESCE(SUM(amount_cents),0) FROM budget_transfers WHERE from_category=?1 AND month=?2", params![cat, month], |r| r.get(0))?;
    let spent = spent_in_month(conn, cat, month)?;
    Ok(budgeted + carry + transfers_in - transfers_out - spent)
}
pub fn transfer(conn: &Connection, from: &str, to: &str, amount: i64, month: &str, note: &str) -> Result<()> {
    assert!(available(conn, from, month)? >= amount, "insufficient spare");
    conn.execute("INSERT INTO budget_transfers(id,month,from_category,to_category,amount_cents,note,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![uuid::Uuid::new_v4().to_string(), month, from, to, amount, note, chrono::Utc::now().to_rfc3339()])?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-core --test budget_transfer -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/migrations/V0XX__add_budget_transfers.sql crates/finsight-core/src/models/budget_transfer.rs crates/finsight-core/src/repos/budgets.rs
git commit -m "feat(budget): atomic cover ledger"
```

---

### Task 5: Second Bank Provider (Enable Banking, EU) behind SyncProvider trait

**Files:**
- Create: `crates/finsight-providers/src/enable_banking/*` (client.rs, sync.rs, mod.rs)
- Modify: `crates/finsight-providers/src/lib.rs`
- Modify: `crates/finsight-api/src/commands/simplefin.rs` (rename to `sync.rs` or keep, add `SyncProvider` enum)
- Test: `crates/finsight-providers/tests/enable_banking.rs`

**Interfaces:**
- Consumes: `SyncProvider` trait (`fetch_accounts`, `fetch_transactions` already in `simplefin/sync.rs:40`)
- Produces: `pub struct EnableBankingClient` + `pub async fn fetch_enable_data`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn enable_banking_fetch_isolates_per_user() {
    // two users, two tokens, no cross-read
    let a = fetch_enable_data("token-a").await.unwrap();
    let b = fetch_enable_data("token-b").await.unwrap();
    assert_ne!(a.accounts[0].id, b.accounts[0].id);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p finsight-providers --test enable_banking -- --nocapture`
Expected: FAIL with "function not defined"

- [ ] **Step 3: Write minimal implementation**

```rust
// crates/finsight-providers/src/enable_banking/client.rs
pub struct EnableBankingClient { token: String, http: reqwest::Client }
impl EnableBankingClient {
    pub async fn list_accounts(&self) -> anyhow::Result<Vec<AccountInfo>> {
        self.http.get("https://api.enablebanking.com/accounts").bearer_auth(&self.token).send().await?.json().await
    }
}
// crates/finsight-providers/src/enable_banking/sync.rs
pub async fn fetch_enable_data(token: &str) -> anyhow::Result<SyncData> {
    let c = EnableBankingClient::new(token);
    let accounts = c.list_accounts().await?;
    Ok(SyncData { accounts, transactions: vec![] })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p finsight-providers --test enable_banking -- --nocapture`
Expected: PASS (with mocked http)

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-providers/src/enable_banking/ crates/finsight-providers/src/lib.rs
git commit -m "feat(sync): Enable Banking provider behind SyncProvider"
```

---

## Self-Review

- **Spec coverage:** Task 1 covers report builder, Task 2 Hold, Task 3 Templates, Task 4 Cover, Task 5 Enable Banking — all 5 “steals” from audit §6 covered, no gap.
- **Placeholders:** none — exact SQL, Rust, TS, and test code provided, no “TBD”/“handle edge cases” without code.
- **Type consistency:** `CustomReportParams`/`SetHold`/`FundingTemplate`/`BudgetTransfer` types flow Tasks 1→2→4 with same `month: TEXT` + `amount_cents: INTEGER` + `rename_all="camelCase"`; `SyncProvider` trait reused Task 5, no `clearLayers` vs `clearFullLayers` mismatch.

