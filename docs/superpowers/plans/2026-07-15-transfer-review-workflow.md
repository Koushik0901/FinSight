# Transfer Review Workflow (Settle-Up) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user resolve ambiguous person-to-person money flows once per counterparty with a 3-way verdict (Transfer / Settle-up / Real), where settle-up nets inflows against expense, and surface it as a grouped review card in the Inbox.

**Architecture:** Extend the existing V046 verdict machinery (sticky `transfer_override`, `transfer_verdict_siblings`, bulk-apply, `%name%` generalization) from binary to 3-way. Add a `settle_up` column whose netting lives entirely in `metrics.rs` (single source of truth) and every other expense-aggregating query. Persistence to *future* imports (rule `treatment`) is a later phase.

**Tech Stack:** Rust (finsight-core repos + metrics, finsight-app commands, finsight-agent categorizer), rusqlite/SQLCipher, refinery migrations, Tauri+specta bindings, React+TS+vitest, tanstack-query.

**Design doc:** `docs/superpowers/specs/2026-07-15-transfer-review-workflow-design.md`

**Phasing (stop-safe order):** Phase 1 (migration+netting) and Phase 2 (verdict funnel + grouped surface) deliver the full user value on existing rows. Phase 3 (rule-treatment persistence to future imports) is additive. Phase 4 is real-data measurement.

---

## Phase 1 — settle-up column + netting

### Task 1: Migration V049 — `settle_up` + rule `treatment`

**Files:**
- Create: `crates/finsight-core/migrations/V049__settle_up_and_rule_treatment.sql`

- [ ] **Step 1: Write the migration**

```sql
-- Settle-up treatment: a person-to-person flow whose inflows NET against
-- expense (never income). Reimbursement/shared-cost model. Metrics interpret it.
ALTER TABLE transactions ADD COLUMN settle_up INTEGER NOT NULL DEFAULT 0;

-- A rule can now carry a treatment beyond categorization, so a per-counterparty
-- verdict persists to future imports: 'categorize' (default, existing) |
-- 'transfer' | 'settle_up'. category_id is only read when treatment='categorize'.
ALTER TABLE rules ADD COLUMN treatment TEXT NOT NULL DEFAULT 'categorize';
```

- [ ] **Step 2: Run migrations to verify they apply**

Run: `cargo test -p finsight-core --lib db::tests`
Expected: PASS (migration discovery + apply is exercised by the existing db tests; no error on the new V049).

- [ ] **Step 3: Commit**

```bash
git add crates/finsight-core/migrations/V049__settle_up_and_rule_treatment.sql
git commit -m "feat(db): V049 settle_up column + rule treatment"
```

### Task 2: Read `settle_up` into the Transaction model

**Files:**
- Modify: `crates/finsight-core/src/models/transaction.rs` (add field)
- Modify: `crates/finsight-core/src/repos/transactions.rs` (both SELECT column lists + row mappers at ~138/244 and ~619/668, and the `new`/default at ~70)

- [ ] **Step 1: Add the field to the model**

In `models/transaction.rs`, after `pub is_reimbursable: bool,` add:
```rust
    pub settle_up: bool,
```

- [ ] **Step 2: Add to every SELECT + row mapper**

In `repos/transactions.rs`, both SELECT strings that list `t.is_reimbursable, t.is_split, …` — add `t.settle_up,` and bump every subsequent positional index by 1 in the corresponding `r.get::<_, i64>(N)?` mapper. Add `settle_up: r.get::<_, i64>(<its index>)? != 0,` next to `is_reimbursable`. Set `settle_up: false` in the `new`/default constructor at ~line 70.

- [ ] **Step 3: Verify it compiles + existing tests pass**

Run: `cargo test -p finsight-core --lib repos::transactions`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-core/src/models/transaction.rs crates/finsight-core/src/repos/transactions.rs
git commit -m "feat(core): read settle_up on Transaction"
```

### Task 3: Settle-up netting in `metrics.rs`

**Files:**
- Modify: `crates/finsight-core/src/metrics.rs` — `income_expense_since` (~174), `income_expense_between` (~192), and the month-weighted `_for` query (~427)
- Test: `crates/finsight-core/src/metrics.rs` (tests module)

- [ ] **Step 1: Write the failing test**

Add to the metrics tests module (adapt fixture helpers already present in that module):
```rust
#[test]
fn settle_up_inflow_nets_against_expense_never_income() {
    let (_d, db) = fresh_db();
    let conn = db.get().unwrap();
    insert_account(&conn, "chk");
    // Joe: $11,475 out (expense) + $3,000 in — ruled settle-up.
    conn.execute(
        "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,settle_up,status,created_at) VALUES\
         ('o','chk','2026-05-02T12:00:00Z',-1147500,'e-transfer joe',0,1,'cleared',datetime('now')),\
         ('i','chk','2026-05-10T12:00:00Z', 300000,'e-transfer joe',0,1,'cleared',datetime('now'))",
        [],
    ).unwrap();
    let (income, expense) = income_expense_since(&conn, "2026-05-01T00:00:00Z").unwrap();
    assert_eq!(income, 0, "settle-up inflow is never income");
    assert_eq!(expense, 847500, "settle-up nets: 11,475 - 3,000 = 8,475");
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p finsight-core --lib metrics::tests::settle_up_inflow_nets_against_expense_never_income`
Expected: FAIL (income=300000, expense=1147500 — no netting yet).

- [ ] **Step 3: Apply the netting to all three query bodies**

Replace the income/expense CASE pair in `income_expense_since` and `income_expense_between` with:
```sql
COALESCE(SUM(CASE WHEN amount_cents > 0 AND settle_up = 0 THEN amount_cents ELSE 0 END), 0),
COALESCE(SUM(CASE WHEN settle_up = 1 THEN -amount_cents
                  WHEN amount_cents < 0 THEN -amount_cents
                  ELSE 0 END), 0)
```
And the weighted `_for` variant (~427) analogously, multiplying by `t.mw`:
```sql
COALESCE(SUM(CASE WHEN t.amount_cents > 0 AND t.settle_up = 0 THEN t.amount_cents * t.mw ELSE 0 END), 0),
COALESCE(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents * t.mw
                  WHEN t.amount_cents < 0 THEN -t.amount_cents * t.mw
                  ELSE 0 END), 0)
```

- [ ] **Step 4: Run test to verify it passes + no metrics regression**

Run: `cargo test -p finsight-core --lib metrics`
Expected: PASS (new test + all existing cashflow tests green — non-settle_up rows unaffected since `settle_up=0` is the default).

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/metrics.rs
git commit -m "feat(metrics): settle-up inflows net against expense, never income"
```

### Task 4: Net settle-up in every OTHER expense-aggregating query

**Files:**
- Modify + Test: whichever of `crates/finsight-core/src/repos/categories.rs`, `crates/finsight-core/src/spending/*.rs` aggregate expense with `amount_cents < 0`.

- [ ] **Step 1: Find every expense-aggregating query**

Run: `grep -rnE "amount_cents < 0|amount_cents > 0" crates/finsight-core/src/repos/categories.rs crates/finsight-core/src/spending/`
For each query that sums expense, a positive `settle_up=1` inflow is currently dropped (filtered by `< 0`) instead of netted.

- [ ] **Step 2: Write a failing test per category/spending surface**

For the category-breakdown function, add a test: a `settle_up=1` inflow categorized to the same category as a matching outflow reduces that category's spend total. (Mirror the Task 3 fixture; assert the category total is netted.)

- [ ] **Step 3: Run to see it fail**, then **Step 4: apply the same CASE netting** (`WHEN settle_up = 1 THEN -amount_cents`) to each expense sum, **Step 5: run to pass**, **Step 6: commit** per surface.

```bash
git commit -m "feat(core): net settle-up in category/spending expense sums"
```

### Task 4b: Netting sweep across agent + command expense surfaces (plan gap found during Task 4)

**Why:** Task 4 revealed the same `WHERE amount_cents < 0` drop-the-inflow risk in ~11 files under `crates/finsight-agent/src` (`context.rs`, `finance.rs`, `reasoning/tools/read.rs`) and `crates/finsight-app/src/commands` (`budget.rs`, `reports.rs`, `insights.rs`, `inbox.rs`, `agent.rs`, `copilot_chat.rs`, …). If Today/spending net settle-up but Budget/Reports/Copilot show gross, the feature is inconsistent and the Copilot answers with wrong numbers.

**Files:** the above (audit each; change only AGGREGATE expense sums, never row-level filters/lists).

- [ ] **Step 1:** `grep -rnE "amount_cents < 0|SUM\(.*amount_cents" crates/finsight-agent/src crates/finsight-app/src/commands`; classify each hit (aggregate expense SUM → net; row-level list/filter/balance/investment → leave).
- [ ] **Step 2:** Prefer routing a surface through `metrics.rs` cashflow (the canonical, already-netted source) over hand-rolling the CASE, where the surface just needs income/expense totals. Where a bespoke per-category/per-window sum is genuinely needed, apply the same `WHEN settle_up = 1 THEN -amount_cents` netting + broaden `WHERE` to include settle-up inflows.
- [ ] **Step 3:** TDD each changed aggregate; run the owning crate's tests one cargo invocation at a time.
- [ ] **Step 4: Commit** `feat(core): net settle-up across agent + command expense surfaces`.

*(Sequenced after Phase 2 — the verdict funnel/UI don't depend on it, and it all lands in one PR — but REQUIRED before finishing the branch.)*

---

## Phase 2 — 3-way verdict + grouped review surface

### Task 5: 3-way counterparty verdict in the repo

**Files:**
- Modify + Test: `crates/finsight-core/src/repos/transactions.rs` (add `set_counterparty_verdict`, generalize `apply_transfer_override_to_matching` → `apply_verdict_to_matching`; keep `set_transfer_override` as the transfer arm)

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn settle_up_verdict_marks_and_leaves_the_undecided_queue() {
    let (_d, db) = fresh_db();
    let mut conn = db.get().unwrap();
    seed_categories(&conn);
    insert_account(&conn, "chk");
    conn.execute(
        "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
         ('j','chk','2026-05-02T12:00:00Z',-50000,'Internet Banking E-TRANSFER 111 Joe','cleared',datetime('now'))",
        [],
    ).unwrap();
    let t = set_counterparty_verdict(&mut conn, "j", Verdict::SettleUp).unwrap();
    assert!(t.settle_up, "settle_up set");
    assert!(!t.is_transfer, "settle-up is not a transfer");
    // transfer_override=0 marks it decided → out of the undecided predicate.
    let undecided: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE transfer_override IS NULL AND settle_up=0 AND category_id IS NULL AND transfer_peer_id IS NULL AND id='j'",
        [], |r| r.get(0)).unwrap();
    assert_eq!(undecided, 0);
}
```

- [ ] **Step 2: Run to fail** (`Verdict`/`set_counterparty_verdict` undefined).

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict { Transfer, SettleUp, Real }

/// Record a per-transaction verdict. Transfer reuses set_transfer_override's
/// semantics; SettleUp sets settle_up=1 + transfer_override=0 (decided,
/// netted) and clears the anomaly flag; Real sets transfer_override=0,
/// settle_up=0 (decided real income/expense).
pub fn set_counterparty_verdict(conn: &mut Connection, id: &str, verdict: Verdict) -> CoreResult<Transaction> {
    match verdict {
        Verdict::Transfer => set_transfer_override(conn, id, true),
        Verdict::SettleUp => {
            conn.execute(
                "UPDATE transactions SET settle_up=1, transfer_override=0, is_transfer=0, \
                 transfer_peer_id=NULL, is_anomaly=0 WHERE id=?1", params![id])?;
            get_by_id(conn, id)
        }
        Verdict::Real => {
            conn.execute(
                "UPDATE transactions SET settle_up=0, transfer_override=0, is_transfer=0, \
                 transfer_peer_id=NULL WHERE id=?1", params![id])?;
            get_by_id(conn, id)
        }
    }
}
```
Then generalize the bulk apply to take a `Verdict` (route each id through `set_counterparty_verdict`) and keep the undecided-predicate WHERE clause from `apply_transfer_override_to_matching`.

- [ ] **Step 4: Run to pass** (`cargo test -p finsight-core --lib repos::transactions`).
- [ ] **Step 5: Commit** `feat(core): 3-way counterparty verdict (transfer/settle-up/real)`

### Task 6: `list_unresolved_counterparties` read

**Files:**
- Modify + Test: `crates/finsight-core/src/repos/transactions.rs`

- [ ] **Step 1: Write the failing test** — insert 3 undecided `e-transfer joe` rows (2 out, 1 in) + a decided row; assert one group `{label:"joe", txn_count:3, outflow, inflow}` and the decided row excluded.

- [ ] **Step 2: Run to fail.**

- [ ] **Step 3: Implement.** Group the undecided predicate rows by their `suggested_rule_pattern` (only `%name%` patterns; bare `INTERNET TRANSFER` rows fold into a single unnamed bucket). Return `Vec<UnresolvedCounterparty { pattern, label, txn_count, inflow_cents, outflow_cents }>` ordered by `abs(inflow-outflow)` desc. Compute grouping in Rust over the fetched undecided rows (reuse `suggested_rule_pattern`) rather than SQL, so the `%name%` logic stays single-sourced.

- [ ] **Step 4: Run to pass. Step 5: Commit** `feat(core): list unresolved counterparties for review`

### Task 7: Tauri commands + bindings

**Files:**
- Modify: `crates/finsight-app/src/commands/transactions.rs` (add `set_counterparty_verdict`, `apply_verdict_to_similar`, `list_unresolved_counterparties`; keep the old two as thin wrappers or migrate callers)
- Modify: `crates/finsight-app/src/lib.rs` (`collect_commands![…]`)
- Generated: `ui/src/api/bindings.ts`

- [ ] **Step 1:** Add the three `#[tauri::command] #[specta::specta] pub async fn` wrappers (mirror `set_transaction_transfer`'s `run(&db, move |conn| …)` shape). Serialize `Verdict` as a camelCase string enum for the bindings.
- [ ] **Step 2:** Register them in `build_specta_builder()`.
- [ ] **Step 3:** Regenerate bindings: `cargo run -p finsight-tauri --bin export_bindings`
- [ ] **Step 4:** `cargo build -p finsight-app` + `cd ui && npx tsc --noEmit` → 0 errors.
- [ ] **Step 5: Commit** `feat(app): counterparty verdict + unresolved-list commands`

### Task 8: Inbox grouped review card

**Files:**
- Create: `ui/src/components/inbox/UnresolvedPeopleCard.tsx`
- Create: `ui/src/components/inbox/UnresolvedPeopleCard.test.tsx`
- Modify: `ui/src/api/hooks/inbox.ts` (or `transactions.ts`) — `useUnresolvedCounterparties`, `useSetCounterpartyVerdict`
- Modify: `ui/src/screens/Inbox.tsx` (render the card in the needs-review feed)

- [ ] **Step 1: Write the failing test** — render `UnresolvedPeopleCard` with two mock counterparties (Joe: out+in, Swathi: out only); assert both rows render with net in/out, three verb buttons each, and that clicking "Settle-up" for Joe calls the verdict mutation with Joe's pattern and optimistically removes the row. Amounts carry `className="money"`.

- [ ] **Step 2: Run to fail** (`cd ui && npx vitest run src/components/inbox/UnresolvedPeopleCard.test.tsx`).

- [ ] **Step 3: Implement** the card (reuse the existing `Card` component + `.chip`/`.btn` classes; one row per counterparty with `[Transfer][Settle-up][Real]`; on click call `useSetCounterpartyVerdict` with the pattern + verdict, toast the applied count, invalidate the list + cashflow queries). Wire it into `Inbox.tsx`'s feed above/below the existing review sections, gated on `count > 0`.

- [ ] **Step 4: Run to pass** (vitest for the file) + `npx tsc --noEmit`.
- [ ] **Step 5: Verify live** (preview workflow): open the dev server, confirm the card renders and a verdict removes the group + updates the Inbox badge.
- [ ] **Step 6: Commit** `feat(inbox): grouped "people with unresolved money" review card`

---

## Phase 3 — persist verdicts to future imports (rule treatment)

### Task 9: Rule model carries a treatment

**Files:**
- Modify: `crates/finsight-core/src/models/rule.rs` (`Rule` + `NewRule` gain `treatment: String`)
- Modify: `crates/finsight-core/src/repos/rules.rs` (`list_active` SELECT + mapper, `insert` INSERT)
- Test: `repos/rules.rs`

- [ ] **Step 1: Write failing test** — insert a `NewRule { treatment: "settle_up", … }`; `list_active` returns it with the treatment.
- [ ] **Step 2: Run to fail. Step 3:** add `treatment` to the struct, SELECT `treatment`, INSERT it, map it. **Step 4: pass. Step 5: commit** `feat(core): rules carry a treatment`.

### Task 10: Verdict creates a treatment rule; cascade applies treatment rules

**Files:**
- Modify + Test: `crates/finsight-core/src/repos/transactions.rs` (on a counterparty verdict with a `%name%` pattern, upsert a `rules` row `{pattern, treatment}`)
- Create + Test: `crates/finsight-core/src/repos/rules.rs::apply_treatment_rules(conn)` — for each active rule with `treatment IN ('transfer','settle_up')`, apply to matching rows that are still undecided (both signs), **but never override an explicit per-row `transfer_override`** (precedence). Mirror `apply_to_uncategorized`'s LIKE semantics.
- Modify: the post-import cascade (where `pair_transfers` runs — `crates/finsight-agent/src/*` or the import command) to call `apply_treatment_rules` after pairing.

- [ ] **Step 1: Write failing test** — a `%joe% settle_up` rule + a fresh imported `e-transfer joe` inflow → after `apply_treatment_rules`, the row has `settle_up=1`; a row with explicit `transfer_override=1` is untouched.
- [ ] **Step 2: fail → Step 3: implement → Step 4: pass → Step 5: commit** `feat(core): counterparty verdicts persist to future imports`.

---

## Phase 4 — real-data measurement

### Task 11: audit_probe measures settle-up netting (by pattern, not by name)

**Files:**
- Modify: `crates/finsight-app/tests/audit_probe.rs`

- [ ] **Step 1:** In the probe, after the cascade, apply a **settle-up verdict by generic rule** to the largest unresolved counterparties *found in the data* (iterate `list_unresolved_counterparties`, rule the top-N as settle-up) — never hard-code "joe/swathi". Print before/after income & expense.
- [ ] **Step 2: Run** `cargo test -p finsight-app --release --test audit_probe -- --ignored --nocapture` and record that expense sheds the netted repayments and income sheds the inflows. This is a *measurement*, not an assertion, per the general-harness rule.
- [ ] **Step 3: Commit** `test(audit): measure settle-up netting on real data by pattern`.

---

## Final: full suite + finish

- [ ] `cargo test --workspace` (serialize — never run two cargo invocations concurrently on Windows, link 1104) → green
- [ ] `cd ui && npx vitest run && npx tsc --noEmit` → green
- [ ] Update `CLAUDE.md` green-bar counts.
- [ ] REQUIRED SUB-SKILL: superpowers:finishing-a-development-branch (present options; likely push + PR into main).
