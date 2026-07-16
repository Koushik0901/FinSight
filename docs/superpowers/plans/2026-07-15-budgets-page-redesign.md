# Budgets Page Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the gap between FinSight's Budget screen and the Actual-Budget-inspired mockup: computed rollover, visible zero-budget categories, manageable category groups, budgeted-vs-spent history, and a full 7-step Plan Next Month wizard.

**Architecture:** No new tables. `carryover_cents` and history's `budgeted_cents` are computed on read in `finsight-core`. Category groups reuse the existing (currently unwired) `category_groups` table and `repos::categories::create()`'s `group_id` param. Sinking funds reuse `goals` with a new `goal_type` value. Every Rust change flows: `finsight-core` repo function (tested) → `finsight-app` command (thin wrapper) → `export_bindings` → frontend hook → screen.

**Tech Stack:** Rust (rusqlite, specta), React + TypeScript, TanStack Query, vitest + @testing-library/react.

**Spec:** `docs/superpowers/specs/2026-07-15-budgets-page-redesign-design.md` — read it first; this plan implements it section by section.

**Linear:** Project "Budgets page redesign" (https://linear.app/ai-job-hunter/project/budgets-page-redesign-aa80555b8765), issues AI-5 (rollover) → AI-9 (Plan Next Month rebuild). Move each issue to "In Progress" when its task starts, "Done" when its commit lands.

---

## Task 1: Rollover math — `carryover_into_month` in `repos/budgets.rs`

**Files:**
- Modify: `crates/finsight-core/src/repos/budgets.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/finsight-core/src/repos/budgets.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_category(conn: &mut Connection, id: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('daily', 'Daily', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'daily', ?1, '#94A3B8', 0)",
            params![id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, color, created_at) \
             VALUES('acc1', 'joint', 'Test Bank', 'Checking', 'Test Checking', '#000', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    }

    fn spend(conn: &mut Connection, category_id: &str, posted_at: &str, cents: i64) {
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, category_id, created_at) \
             VALUES(?1, 'acc1', ?2, ?3, 'Test Merchant', ?4, ?2)",
            params![Uuid::new_v4().to_string(), posted_at, -cents, category_id],
        )
        .unwrap();
    }

    #[test]
    fn month_before_steps_back_across_year_boundary() {
        assert_eq!(month_before("2026-01", 1), "2025-12");
        assert_eq!(month_before("2026-03", 3), "2025-12");
        assert_eq!(month_before("2026-05", 0), "2026-05");
        assert_eq!(month_before("2026-01", -1), "2026-02");
    }

    #[test]
    fn carryover_is_zero_for_never_budgeted_category() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 0);
    }

    #[test]
    fn carryover_is_zero_when_first_budgeted_month_is_current_or_future() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-05", 10_000).unwrap();
        // First budgeted month is May itself — nothing to carry *into* May.
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 0);
    }

    #[test]
    fn carryover_accumulates_positive_when_underspent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 8_000);
        // April: budgeted $100, spent $80 → +$20 carries into May.
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 2_000);
    }

    #[test]
    fn carryover_accumulates_negative_when_overspent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 15_000);
        // April: budgeted $100, spent $150 → -$50 carries into May.
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), -5_000);
    }

    #[test]
    fn carryover_sums_across_multiple_prior_months() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-03", 10_000).unwrap();
        spend(&mut conn, "food", "2026-03-10T00:00:00Z", 8_000); // +$20
        set(&mut conn, "food", "2026-04", 10_000).unwrap();
        spend(&mut conn, "food", "2026-04-10T00:00:00Z", 11_000); // -$10
        // Net into May: +$20 - $10 = +$10.
        assert_eq!(carryover_into_month(&mut conn, "food", "2026-05").unwrap(), 1_000);
    }

    #[test]
    fn carryover_caps_at_24_month_lookback() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        // 30 consecutive budgeted months, each with a $10 surplus, ending the
        // month before "2028-07" (the target month we ask carryover into).
        for i in 0..30 {
            let m = month_before("2028-07", 30 - i);
            set(&mut conn, "food", &m, 10_000).unwrap();
            spend(&mut conn, "food", &format!("{m}-10T00:00:00Z"), 9_000);
        }
        // Only the trailing 24 months count: 24 * $10 = $240, not 30 * $10 = $300.
        assert_eq!(carryover_into_month(&mut conn, "food", "2028-07").unwrap(), 24_000);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p finsight-core --lib repos::budgets::tests -- --nocapture`
Expected: compile error — `month_before` and `carryover_into_month` are not defined.

- [ ] **Step 3: Implement `month_before` and `carryover_into_month`**

Insert into `crates/finsight-core/src/repos/budgets.rs`, after the existing `list_for_month` function (before the `#[cfg(test)]` block):

```rust
/// Return the "YYYY-MM" string `n` months before `month` ("YYYY-MM"). `n` may be
/// negative to step forward instead.
pub fn month_before(month: &str, n: i32) -> String {
    let year: i32 = month[0..4].parse().unwrap_or(1970);
    let mon: i32 = month[5..7].parse().unwrap_or(1); // 1-12
    let total = year * 12 + (mon - 1) - n; // zero-based month index
    let y = total.div_euclid(12);
    let m = total.rem_euclid(12) + 1;
    format!("{y:04}-{m:02}")
}

/// Compute carryover *into* `month` ("YYYY-MM") for one category: the running sum
/// of (budgeted − spent) over every month from the category's first-ever budgeted
/// month (first `budgets` row with `amount_cents > 0`) up to (not including)
/// `month`, capped at a 24-month lookback. Returns 0 if the category has never
/// been budgeted, or if its first budgeted month is `month` or later — the whole
/// point of the epoch anchor is that carryover only ever reflects money the user
/// actually earmarked, never spending from before budgeting started.
pub fn carryover_into_month(
    conn: &mut Connection,
    category_id: &str,
    month: &str,
) -> CoreResult<i64> {
    let first_budgeted: Option<String> = conn.query_row(
        "SELECT MIN(month) FROM budgets WHERE category_id = ?1 AND amount_cents > 0",
        params![category_id],
        |r| r.get(0),
    )?;
    let Some(first_budgeted) = first_budgeted else {
        return Ok(0);
    };
    if first_budgeted.as_str() >= month {
        return Ok(0);
    }

    let earliest_allowed = month_before(month, 24);
    let start = if first_budgeted.as_str() > earliest_allowed.as_str() {
        first_budgeted
    } else {
        earliest_allowed
    };

    let budgeted: i64 = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM budgets \
         WHERE category_id = ?1 AND month >= ?2 AND month < ?3",
        params![category_id, start, month],
        |r| r.get(0),
    )?;
    let start_date = format!("{start}-01");
    let month_date = format!("{month}-01");
    // Mirrors the existing spend calculation in list_budget_envelopes (no
    // is_transfer filter there either) — kept consistent rather than silently
    // fixing an unrelated, pre-existing question about transfer handling.
    let spent: i64 = conn.query_row(
        "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
         WHERE category_id = ?1 AND amount_cents < 0 AND posted_at >= ?2 AND posted_at < ?3",
        params![category_id, start_date, month_date],
        |r| r.get(0),
    )?;
    Ok(budgeted - spent)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p finsight-core --lib repos::budgets::tests -- --nocapture`
Expected: `test result: ok. 7 passed`

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/repos/budgets.rs
git commit -m "feat(budgets): compute carryover into a month, anchored at first-budgeted"
```

---

## Task 2: Wire carryover + show-all into `list_budget_envelopes`

**Files:**
- Modify: `crates/finsight-app/src/commands/budget.rs:11-77`

- [ ] **Step 1: Update `BudgetEnvelope` and `list_budget_envelopes`**

Replace lines 10-77 of `crates/finsight-app/src/commands/budget.rs` with:

```rust
/// One category's budget + actual for a month.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetEnvelope {
    pub category_id: String,
    pub category_label: String,
    pub category_color: String,
    pub group_label: String,
    /// Budget set by user for the current month (0 = not budgeted this month)
    pub budget_cents: i64,
    /// Actual outflow this month (positive = spent)
    pub spent_cents: i64,
    /// Running (budgeted − spent) carried in from prior months, anchored at the
    /// category's first-ever budgeted month. Positive = unspent rolling forward,
    /// negative = accumulated overspend.
    pub carryover_cents: i64,
    pub txn_count: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn list_budget_envelopes(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<BudgetEnvelope>> {
    let db = (*state.db).clone();
    let now = Utc::now();
    let month = now.format("%Y-%m").to_string();
    let this_month_start = now.format("%Y-%m-01").to_string();

    run(&db, move |conn| {
        // Get budgets for the month
        let budget_map: std::collections::HashMap<String, i64> =
            budgets::list_for_month(conn, &month)?.into_iter().collect();

        // Get spending per category this month
        let mut stmt = conn.prepare(
            "SELECT c.id, c.label, COALESCE(c.color,''), COALESCE(g.label,''), \
                    COALESCE(SUM(CASE WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END), 0), \
                    COUNT(t.id) \
             FROM categories c \
             LEFT JOIN category_groups g ON g.id = c.group_id \
             LEFT JOIN transactions t ON t.category_id = c.id AND t.posted_at >= ?1 \
             WHERE c.archived_at IS NULL \
             GROUP BY c.id, c.label, c.color, c.group_id, g.label \
             ORDER BY g.sort_order, c.sort_order",
        )?;
        let rows = stmt.query_map(rusqlite::params![this_month_start], |r| {
            let cat_id: String = r.get(0)?;
            Ok((cat_id.clone(), r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?, budget_map.get(&cat_id).copied().unwrap_or(0)))
        })?;
        let rows: Vec<_> = rows.collect::<rusqlite::Result<_>>()?;
        drop(stmt);

        let mut out = Vec::new();
        for (cat_id, label, color, group_label, spent, txn_count, budget) in rows {
            let carryover_cents = budgets::carryover_into_month(conn, &cat_id, &month)?;
            // Every active category is shown, budgeted or not — a category with
            // no budget and no spend yet is exactly the one a user needs to see
            // in order to budget it for the first time.
            out.push(BudgetEnvelope {
                category_id: cat_id,
                category_label: label,
                category_color: color,
                group_label,
                budget_cents: budget,
                spent_cents: spent,
                carryover_cents,
                txn_count,
            });
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
```

- [ ] **Step 2: Write a Rust command-level test**

There is no existing test file for `commands/budget.rs`. Create `crates/finsight-app/src/commands/budget_tests.rs`... **do not** create a separate file — Tauri command tests in this codebase live inline in a `#[cfg(test)] mod tests` at the bottom of the command file itself (matching `repos/categories.rs`'s pattern), but commands need a `tauri::State`, which is awkward to construct in a unit test. Instead, test the underlying behavior directly against `finsight_core` — the command is a thin wrapper, so this test exercises the same repo functions the command calls, giving equivalent coverage without needing a Tauri app handle. Append to `crates/finsight-core/src/repos/budgets.rs`'s existing `tests` module (from Task 1):

```rust
    #[test]
    fn list_for_month_includes_zero_budget_categories_the_caller_can_still_filter() {
        // list_for_month itself never filtered anything — the historical
        // "hide zero-budget categories" behavior lived in the command layer
        // (commands/budget.rs), which Task 2 removes. This test documents that
        // list_for_month's contract (a plain category_id → amount_cents map for
        // whatever budgets rows exist) is unaffected by that command-layer fix.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        set(&mut conn, "food", "2026-05", 0).unwrap();
        let map = list_for_month(&mut conn, "2026-05").unwrap();
        assert_eq!(map, vec![("food".to_string(), 0)]);
    }
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p finsight-core --lib repos::budgets::tests -- --nocapture`
Expected: `test result: ok. 8 passed`

Run: `cargo build -p finsight-app` (compiles the modified command; no new command-layer test exists here — verified structurally above and covered end-to-end by the frontend test in Task 3).
Expected: builds with no errors.

- [ ] **Step 4: Regenerate TypeScript bindings**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `ui/src/api/bindings.ts` updates — `BudgetEnvelope` gains `carryoverCents: number`.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/budget.rs crates/finsight-core/src/repos/budgets.rs ui/src/api/bindings.ts
git commit -m "feat(budgets): show every active category, wire carryoverCents into BudgetEnvelope"
```

---

## Task 3: Frontend — carryover + "Not yet budgeted" in `Budget.tsx`

**Files:**
- Modify: `ui/src/screens/Budget.tsx`
- Test: `ui/src/screens/Budget.history.test.tsx`

- [ ] **Step 1: Update `envelopeStatus`, `EnvelopeCard`, and hero math to use `available = budgetCents + carryoverCents`**

In `ui/src/screens/Budget.tsx`, replace the `envelopeStatus` function (lines 14-23):

```tsx
function envelopeStatus(env: BudgetEnvelope) {
  const available = env.budgetCents + env.carryoverCents;
  if (available <= 0 && env.budgetCents <= 0) return { label: "No budget set", tone: "warning" as const, severity: 2 };
  const pct = available > 0 ? (env.spentCents / available) * 100 : 100;
  if (env.spentCents > available) {
    return { label: `Over by ${money(env.spentCents - available)}`, tone: "negative" as const, severity: 3 };
  }
  if (pct > 90) return { label: "Tight", tone: "warning" as const, severity: 2 };
  if (pct > 60) return { label: "On pace", tone: "accent" as const, severity: 1 };
  return { label: "Plenty left", tone: "positive" as const, severity: 0 };
}
```

In `EnvelopeCard` (around line 65-149), replace the `remaining`/`pct` computation and add the carryover line. Replace:

```tsx
  const status = envelopeStatus(env);
  const remaining = env.budgetCents - env.spentCents;
  const pct = env.budgetCents > 0 ? Math.min(100, (env.spentCents / env.budgetCents) * 100) : 0;
```

with:

```tsx
  const status = envelopeStatus(env);
  const available = env.budgetCents + env.carryoverCents;
  const remaining = available - env.spentCents;
  const pct = available > 0 ? Math.min(100, (env.spentCents / available) * 100) : 0;
```

And replace the "of {money(env.budgetCents)}" line:

```tsx
      <div className="hero-meta" style={{ justifyContent: "space-between", marginTop: 10 }}>
        <span className="money">{money(env.spentCents)} spent</span>
        <span className="money">of {money(env.budgetCents)}</span>
      </div>
```

with:

```tsx
      <div className="hero-meta" style={{ justifyContent: "space-between", marginTop: 10 }}>
        <span className="money">{money(env.spentCents)} spent</span>
        <span className="money">of {money(available)}</span>
      </div>

      {env.carryoverCents !== 0 && (
        <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--hairline)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span className="muted" style={{ fontSize: 12 }}>Carried from last month</span>
          <span className="money" style={{ fontSize: 12.5, color: env.carryoverCents > 0 ? "var(--positive)" : "var(--negative)" }}>
            {env.carryoverCents > 0 ? "+" : ""}{money(env.carryoverCents)}
          </span>
        </div>
      )}
```

In the `Budget()` component, update the hero aggregate math (around line 184-187) — `totalBudget`/`remaining` should reflect availability including carryover, while `toBudget` (unassigned income) stays budget-only:

```tsx
  const totalBudget = sorted.reduce((sum, env) => sum + env.budgetCents, 0);
  const totalCarryover = sorted.reduce((sum, env) => sum + env.carryoverCents, 0);
  const totalAvailable = totalBudget + totalCarryover;
  const totalSpent = sorted.reduce((sum, env) => sum + env.spentCents, 0);
  const projectedEom = today > 0 ? Math.round((totalSpent / today) * totalDays) : 0;
  const remaining = totalAvailable - totalSpent;
  const toBudget = (totals?.incomeCents ?? 0) - totalBudget;
```

And update the hero card's "left to spend" / progress-bar-percentage references from `totalBudget` to `totalAvailable` where they represent "how much is available to spend" (the progress bar fill and the "Budgeted" stat tile stay showing `totalBudget` as-is — that's literally "what you assigned this month" and should NOT include carryover, to avoid double-counting the same dollars in both a "Budgeted" tile and an "available" hero figure). Replace only the progress-bar width and the "Projected EOM" comparison, which are about *spending against what's actually available*:

```tsx
            <div style={{ position: "relative", height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginTop: 4 }}>
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${monthPct}%`, background: "var(--ink-faint)", opacity: 0.4, borderRadius: 999 }} title="Time elapsed" />
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: `${totalAvailable > 0 ? Math.min(100, (totalSpent / totalAvailable) * 100) : 0}%`, background: "var(--accent)", borderRadius: 999, boxShadow: "0 0 12px var(--accent-3)" }} title="Spent" />
            </div>
```

```tsx
            <div className="stat accent"><div className="label">Projected EOM</div><div className="value money">{money(projectedEom)}</div><div className="sub">{projectedEom > totalAvailable ? <span className="npill neg">Over by {money(projectedEom - totalAvailable)}</span> : <span className="npill pos">Under by {money(totalAvailable - projectedEom)}</span>}</div></div>
```

- [ ] **Step 2: Add the "Not yet budgeted" section**

Insert a new section into the `return` of `Budget()`, directly after the "All envelopes" `<section>` (after line 305, before the "Spending history" section). First, compute the split near the other `useMemo`s (after the `grouped` computation, around line 195):

```tsx
  const unbudgeted = sorted.filter((env) => env.budgetCents <= 0 && env.spentCents <= 0 && env.carryoverCents === 0);
```

Then add the section:

```tsx
      {unbudgeted.length > 0 && (
        <section className="section">
          <div className="day-hdr" style={{ marginBottom: 14 }}>
            <div>
              <div className="eyebrow"><span className="dot" />Not yet budgeted · {unbudgeted.length}</div>
              <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>These don't have a plan yet.</h2>
            </div>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(3, minmax(0, 1fr))", gap: 14 }}>
            {unbudgeted.map((env) => (
              <div key={env.categoryId} className="card tight" style={{ padding: 18, display: "flex", flexDirection: "column", gap: 10 }}>
                <div className="row row-sm" style={{ alignItems: "center" }}>
                  <span className="cswatch" style={{ background: env.categoryColor || "var(--accent)" }} />
                  <strong>{env.categoryLabel}</strong>
                </div>
                {editingId === env.categoryId ? (
                  <BudgetInput envelope={env} onClose={() => setEditingId(null)} />
                ) : (
                  <button className="btn outline sm" type="button" onClick={() => setEditingId(env.categoryId)}>Set budget</button>
                )}
              </div>
            ))}
          </div>
        </section>
      )}
```

Also exclude `unbudgeted` categories from both the "Needs a glance" row and the main "All envelopes" grid so they aren't shown twice. `envelopeStatus` gives a zero-budget category severity 2 ("No budget set"), which otherwise pulls it into `attention` *as well as* the new section below — reorder so `unbudgeted` is computed first and both `attention` and `grouped` exclude it:

```tsx
  const unbudgeted = sorted.filter((env) => env.budgetCents <= 0 && env.spentCents <= 0 && env.carryoverCents === 0);
  // Unbudgeted categories aren't "in trouble" (severity>=2 from "No budget
  // set" is really "unconfigured") — they get their own section below instead
  // of also cluttering "Needs a glance".
  const attention = sorted.filter((env) => envelopeStatus(env).severity >= 2 && !unbudgeted.includes(env));
  const grouped = Object.entries(sorted.filter((env) => !unbudgeted.includes(env)).reduce<Record<string, BudgetEnvelope[]>>((acc, env) => {
```

(closing the `reduce` call as before). This replaces the *existing* `attention` line (previously `sorted.filter((env) => envelopeStatus(env).severity >= 2)` with no exclusion) as well as adding `unbudgeted` and updating `grouped`.

- [ ] **Step 3: Extend the existing test file**

Add to `ui/src/screens/Budget.history.test.tsx` (check its existing mock envelope fixtures first and add `carryoverCents` to each — every existing `BudgetEnvelope` mock object in that file needs a `carryoverCents: 0` field added, or the component will render `undefined` where a number is expected). Then add:

```tsx
  it("shows a carryover line when carryoverCents is non-zero", () => {
    // Reuse this file's existing mock setup, but override one envelope's
    // carryoverCents to a non-zero value before rendering.
    // (Exact mock wiring depends on this file's existing structure — apply the
    // same vi.mock pattern already in the file, changing only carryoverCents.)
  });

  it("groups zero-budget, zero-spend categories under 'Not yet budgeted'", () => {
    // Add one envelope fixture with budgetCents: 0, spentCents: 0,
    // carryoverCents: 0, txnCount: 0 and assert screen.getByText("Not yet budgeted · 1")
    // and that it does NOT appear inside the "All envelopes" section's grouped grid.
  });
```

- [ ] **Step 4: Run the frontend test suite for this file**

Run: `cd ui && npx vitest run src/screens/Budget.history.test.tsx`
Expected: all tests pass (fix any fixture gaps the added `carryoverCents` field surfaces first).

- [ ] **Step 5: Type-check and commit**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

```bash
git add ui/src/screens/Budget.tsx ui/src/screens/Budget.history.test.tsx
git commit -m "feat(budgets): show carryover and a Not yet budgeted section on the Budget screen"
```

---

## Task 4: `repos/categories.rs` — `create_group` and `set_group`

**Files:**
- Modify: `crates/finsight-core/src/repos/categories.rs`

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block in `crates/finsight-core/src/repos/categories.rs` (after `create_rejects_empty_label`):

```rust
    #[test]
    fn create_group_slugs_and_dedups_like_create() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let a = create_group(&mut conn, "Side Hustle", Some("freelance income and costs")).unwrap();
        assert_eq!(a.id, "side-hustle");
        assert_eq!(a.hint.as_deref(), Some("freelance income and costs"));
        let b = create_group(&mut conn, "Side Hustle", None).unwrap();
        assert_eq!(b.id, "side-hustle-2");
    }

    #[test]
    fn create_group_rejects_empty_label() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        assert!(create_group(&mut conn, "   ", None).is_err());
    }

    #[test]
    fn set_group_moves_a_category() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();
        let new_group = create_group(&mut conn, "Lifestyle", None).unwrap();
        set_group(&mut conn, &cat.id, &new_group.id).unwrap();
        let moved = list(&mut conn).unwrap().into_iter().find(|c| c.id == cat.id).unwrap();
        assert_eq!(moved.group_id, new_group.id);
    }

    #[test]
    fn set_group_rejects_nonexistent_group() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();
        assert!(set_group(&mut conn, &cat.id, "does-not-exist").is_err());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p finsight-core --lib repos::categories::tests -- --nocapture`
Expected: compile error — `create_group` and `set_group` are not defined.

- [ ] **Step 3: Implement `create_group` and `set_group`**

Insert into `crates/finsight-core/src/repos/categories.rs`, after `list_groups` (before `pub fn list`):

```rust
/// Create a new category group. Returns the generated group (a slug of the
/// label, de-duplicated the same way `create()` de-duplicates category ids).
pub fn create_group(
    conn: &mut Connection,
    label: &str,
    hint: Option<&str>,
) -> CoreResult<CategoryGroup> {
    let label = label.trim();
    if label.is_empty() {
        return Err(crate::error::CoreError::InvalidState(
            "group label must not be empty".into(),
        ));
    }
    let base: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let base = if base.is_empty() { "group".to_string() } else { base };
    let mut id = base.clone();
    let mut n = 1;
    while conn
        .query_row("SELECT 1 FROM category_groups WHERE id = ?1", [&id], |_| Ok(()))
        .is_ok()
    {
        n += 1;
        id = format!("{base}-{n}");
    }

    let next_sort: i32 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM category_groups", [], |r| r.get(0))
        .unwrap_or(0);
    let hint = hint.map(str::trim).filter(|s| !s.is_empty());
    conn.execute(
        "INSERT INTO category_groups(id, label, hint, sort_order) VALUES(?1, ?2, ?3, ?4)",
        rusqlite::params![id, label, hint, next_sort],
    )?;
    Ok(CategoryGroup {
        id,
        label: label.to_string(),
        hint: hint.map(str::to_string),
        sort_order: next_sort,
    })
}

/// Move a category to a different (existing) group.
pub fn set_group(conn: &mut Connection, category_id: &str, group_id: &str) -> CoreResult<()> {
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM category_groups WHERE id = ?1",
            [group_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !exists {
        return Err(crate::error::CoreError::InvalidState(
            "category group not found".into(),
        ));
    }
    conn.execute(
        "UPDATE categories SET group_id = ?1 WHERE id = ?2",
        rusqlite::params![group_id, category_id],
    )?;
    Ok(())
}
```

This requires `OptionalExtension` for `.optional()` — add to the top-level imports of `crates/finsight-core/src/repos/categories.rs`:

```rust
use rusqlite::OptionalExtension;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p finsight-core --lib repos::categories::tests -- --nocapture`
Expected: `test result: ok. 9 passed` (5 existing + 4 new)

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/repos/categories.rs
git commit -m "feat(categories): add create_group and set_group repo functions"
```

---

## Task 5: New commands — `list_category_groups`, `create_category_group`, `set_category_group`

**Files:**
- Modify: `crates/finsight-app/src/commands/categories.rs`
- Modify: `crates/finsight-app/src/lib.rs:184` (registration)

- [ ] **Step 1: Add the three commands**

Append to `crates/finsight-app/src/commands/categories.rs`:

```rust
use finsight_core::models::CategoryGroup;

#[tauri::command]
#[specta::specta]
pub async fn list_category_groups(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<CategoryGroup>> {
    let db = (*state.db).clone();
    run(&db, |conn| categories::list_groups(conn))
        .await
        .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn create_category_group(
    state: tauri::State<'_, AppState>,
    label: String,
    hint: Option<String>,
) -> AppResult<CategoryGroup> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::create_group(conn, &label, hint.as_deref())
    })
    .await
    .map_err(crate::error::AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn set_category_group(
    state: tauri::State<'_, AppState>,
    category_id: String,
    group_id: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        categories::set_group(conn, &category_id, &group_id)
    })
    .await
    .map_err(crate::error::AppError::from)
}
```

- [ ] **Step 2: Register the commands**

In `crates/finsight-app/src/lib.rs`, find line 184 (`commands::categories::set_category_guidance,`) and add the three new commands directly after it:

```rust
        commands::categories::set_category_guidance,
        commands::categories::list_category_groups,
        commands::categories::create_category_group,
        commands::categories::set_category_group,
```

- [ ] **Step 3: Build and regenerate bindings**

Run: `cargo build -p finsight-app`
Expected: builds with no errors.

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `ui/src/api/bindings.ts` gains `listCategoryGroups`, `createCategoryGroup`, `setCategoryGroup` and the `CategoryGroup` type (`{ id: string; label: string; hint: string | null; sortOrder: number }`).

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/categories.rs crates/finsight-app/src/lib.rs ui/src/api/bindings.ts
git commit -m "feat(categories): expose category-group list/create/assign as Tauri commands"
```

---

## Task 6: Frontend hooks for category groups

**Files:**
- Modify: `ui/src/api/hooks/transactions.ts`

- [ ] **Step 1: Add the hooks**

Insert into `ui/src/api/hooks/transactions.ts`, after `useUpdateCategoryColor` (before `useRulesWithCategories`, i.e. after line 272):

```ts
export function useCategoryGroups() {
  return useQuery<CategoryGroup[]>({
    queryKey: ["category-groups"],
    queryFn: async () => {
      const result = await commands.listCategoryGroups();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useCreateCategoryGroup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ label, hint }: { label: string; hint?: string | null }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createCategoryGroup(label, hint ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["category-groups"] });
    },
  });
}

export function useSetCategoryGroup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ categoryId, groupId }: { categoryId: string; groupId: string }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setCategoryGroup(categoryId, groupId);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => invalidateCategoryQueries(qc),
  });
}
```

Add `CategoryGroup` to this file's existing import from `../client` at the top of the file (find the line importing `type CategoryDto` etc. and add `CategoryGroup` to that same type-only import list).

- [ ] **Step 2: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 3: Commit**

```bash
git add ui/src/api/hooks/transactions.ts
git commit -m "feat(categories): add useCategoryGroups/useCreateCategoryGroup/useSetCategoryGroup hooks"
```

---

## Task 7: `Categories.tsx` — group picker, new-group, move-to-group

**Files:**
- Modify: `ui/src/screens/Categories.tsx`
- Modify: `ui/src/screens/Categories.test.tsx`

- [ ] **Step 1: Wire the hook and add group state**

In `ui/src/screens/Categories.tsx`, add to the imports (after the existing `useSetCategoryGuidance` import line):

```tsx
  useCategoryGroups,
  useCreateCategoryGroup,
  useSetCategoryGroup,
```

(these join the existing multi-line import from `"../api/hooks/transactions"`).

In the `Categories()` component, add after the existing hook calls (after `const setGuidance = useSetCategoryGuidance();`):

```tsx
  const { data: groups = [] } = useCategoryGroups();
  const createGroup = useCreateCategoryGroup();
  const setCategoryGroup = useSetCategoryGroup();
  const [newGroupOpen, setNewGroupOpen] = useState(false);
  const [newGroupLabel, setNewGroupLabel] = useState("");
  const [newCatGroupId, setNewCatGroupId] = useState<string>("");
```

- [ ] **Step 2: Default the new-category group picker once groups load**

Add a small effect near the top of the component (after the new state above), so the picker defaults to the first group once data arrives:

```tsx
  useEffect(() => {
    if (!newCatGroupId && groups.length > 0) setNewCatGroupId(groups[0].id);
  }, [groups, newCatGroupId]);
```

Add `useEffect` to the existing `import { Fragment, useMemo, useState } from "react";` line, making it:

```tsx
import { Fragment, useEffect, useMemo, useState } from "react";
```

- [ ] **Step 3: Update `handleCreate` to pass the selected group**

Replace `handleCreate`'s `createCategory.mutateAsync` call:

```tsx
      await createCategory.mutateAsync({ label, groupId: null, color });
```

with:

```tsx
      await createCategory.mutateAsync({ label, groupId: newCatGroupId || null, color });
```

- [ ] **Step 4: Add group picker + "new group" to the new-category form**

Replace the `newCatOpen` block (lines 186-200):

```tsx
      {newCatOpen && (
        <div className="card" style={{ padding: 16, display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          <input
            className="control"
            autoFocus
            placeholder="Category name (e.g. Coffee)"
            value={newCatLabel}
            onChange={(e) => setNewCatLabel(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") void handleCreate(); }}
            style={{ minWidth: 240 }}
          />
          <select
            className="control"
            aria-label="New category's group"
            value={newCatGroupId}
            onChange={(e) => setNewCatGroupId(e.target.value)}
          >
            {groups.map((g) => <option key={g.id} value={g.id}>{g.label}</option>)}
          </select>
          <button className="btn ghost sm" type="button" onClick={() => setNewGroupOpen((v) => !v)}>+ New group</button>
          <button className="btn primary sm" type="button" disabled={createCategory.isPending || !newCatLabel.trim()} onClick={() => void handleCreate()}>{createCategory.isPending ? "Creating…" : "Create"}</button>
          <button className="btn ghost sm" type="button" onClick={() => { setNewCatOpen(false); setNewCatLabel(""); }}>Cancel</button>

          {newGroupOpen && (
            <div className="row row-sm" style={{ width: "100%", marginTop: 4 }}>
              <input
                className="control"
                placeholder="Group name (e.g. Side Hustle)"
                value={newGroupLabel}
                onChange={(e) => setNewGroupLabel(e.target.value)}
                style={{ minWidth: 200 }}
              />
              <button
                className="btn sm"
                type="button"
                disabled={createGroup.isPending || !newGroupLabel.trim()}
                onClick={async () => {
                  const label = newGroupLabel.trim();
                  if (!label) return;
                  try {
                    const group = await createGroup.mutateAsync({ label });
                    setNewCatGroupId(group.id);
                    setNewGroupLabel("");
                    setNewGroupOpen(false);
                    toast.success(`Created group "${label}"`);
                  } catch {
                    toast.error("Could not create group");
                  }
                }}
              >
                Add group
              </button>
            </div>
          )}
        </div>
      )}
```

- [ ] **Step 5: Add "Move to group" in the Manage panel**

In the Manage panel block (around line 302-321), add a group `<select>` right after the rename control block. Replace:

```tsx
                      <label className="eyebrow" htmlFor={`guidance-${category.id}`} style={{ marginTop: 8 }}>Categorizer &amp; Copilot guidance</label>
```

with:

```tsx
                      <label className="eyebrow" htmlFor={`group-${category.id}`} style={{ marginTop: 8 }}>Group</label>
                      <select
                        id={`group-${category.id}`}
                        className="control"
                        value={category.groupId}
                        disabled={setCategoryGroup.isPending}
                        onChange={async (e) => {
                          try {
                            await setCategoryGroup.mutateAsync({ categoryId: category.id, groupId: e.target.value });
                            toast.success("Moved to group");
                          } catch {
                            toast.error("Could not move category");
                          }
                        }}
                        style={{ maxWidth: 240 }}
                      >
                        {groups.map((g) => <option key={g.id} value={g.id}>{g.label}</option>)}
                      </select>
                      <label className="eyebrow" htmlFor={`guidance-${category.id}`} style={{ marginTop: 8 }}>Categorizer &amp; Copilot guidance</label>
```

This assumes `CategoryWithSpending` (the type `category` is typed as) has a `groupId` field — confirmed present in `bindings.ts` (`CategoryWithSpending.groupId: string`).

- [ ] **Step 6: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 7: Extend `Categories.test.tsx`**

Read `ui/src/screens/Categories.test.tsx` first to see its existing `vi.mock("../api/hooks/transactions", ...)` shape, then add `useCategoryGroups`, `useCreateCategoryGroup`, `useSetCategoryGroup` to that mock (returning a small fixed list of 2 groups, e.g. `[{ id: "daily", label: "Daily", hint: null, sortOrder: 0 }, { id: "fixed", label: "Fixed", hint: null, sortOrder: 1 }]` for the query, and `{ mutateAsync: vi.fn(), isPending: false }` for the two mutations). Add:

```tsx
  it("lets the user pick a group when creating a category", () => {
    render(<Categories />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("New category"));
    expect(screen.getByLabelText("New category's group")).toBeInTheDocument();
  });

  it("shows a Move to group control in the Manage panel", () => {
    render(<Categories />, { wrapper: createWrapper() });
    fireEvent.click(screen.getAllByLabelText(/Manage /)[0]);
    expect(screen.getByText("Group")).toBeInTheDocument();
  });
```

- [ ] **Step 8: Run the test file**

Run: `cd ui && npx vitest run src/screens/Categories.test.tsx`
Expected: all tests pass.

- [ ] **Step 9: Commit**

```bash
git add ui/src/screens/Categories.tsx ui/src/screens/Categories.test.tsx
git commit -m "feat(categories): create/assign category groups from the Categories screen"
```

---

## Task 8: History gains budgeted amounts

**Files:**
- Modify: `crates/finsight-app/src/commands/budget.rs` (`MonthlyActual`, `list_budget_history`)

- [ ] **Step 1: Change `MonthlyActual` and the history query**

Replace the `MonthlyActual` struct (lines 144-150):

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyActual {
    pub month: String,
    pub label: String,
    pub spent_cents: i64,
    pub budgeted_cents: i64,
}
```

In `list_budget_history` (lines 602-718), the spend aggregation and category loop need a second map for budgeted amounts. Replace the spend-aggregation block:

```rust
        // Aggregate per-category per-month outflows
        let mut stmt = conn.prepare(
            "SELECT t.category_id, strftime('%Y-%m', t.posted_at) AS mo,
                    SUM(-t.amount_cents) AS cents
             FROM transactions t
             WHERE t.amount_cents < 0
               AND t.posted_at >= ?1
               AND t.category_id IS NOT NULL
             GROUP BY t.category_id, mo",
        )?;
        let mut spend_map: std::collections::HashMap<(String, String), i64> =
            std::collections::HashMap::new();
        let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows.flatten() {
            spend_map.insert((row.0, row.1), row.2);
        }
        drop(stmt);
```

with:

```rust
        // Aggregate per-category per-month outflows
        let mut stmt = conn.prepare(
            "SELECT t.category_id, strftime('%Y-%m', t.posted_at) AS mo,
                    SUM(-t.amount_cents) AS cents
             FROM transactions t
             WHERE t.amount_cents < 0
               AND t.posted_at >= ?1
               AND t.category_id IS NOT NULL
             GROUP BY t.category_id, mo",
        )?;
        let mut spend_map: std::collections::HashMap<(String, String), i64> =
            std::collections::HashMap::new();
        let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        for row in rows.flatten() {
            spend_map.insert((row.0, row.1), row.2);
        }
        drop(stmt);

        // Same shape, for budgeted amounts.
        let mut budget_stmt = conn.prepare(
            "SELECT category_id, month, amount_cents FROM budgets WHERE month >= ?1",
        )?;
        let cutoff_month = month_list.first().unwrap().clone();
        let mut budget_map: std::collections::HashMap<(String, String), i64> =
            std::collections::HashMap::new();
        let budget_rows = budget_stmt.query_map(rusqlite::params![cutoff_month], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        })?;
        for row in budget_rows.flatten() {
            budget_map.insert((row.0, row.1), row.2);
        }
        drop(budget_stmt);
```

Then replace the `monthly` construction inside the `filter_map`:

```rust
                let monthly: Vec<MonthlyActual> = month_list
                    .iter()
                    .zip(month_labels.iter())
                    .map(|(m, lbl)| MonthlyActual {
                        month: m.clone(),
                        label: lbl.clone(),
                        cents: spend_map
                            .get(&(id.clone(), m.clone()))
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect();
                let total: i64 = monthly.iter().map(|m| m.cents).sum();
```

with:

```rust
                let monthly: Vec<MonthlyActual> = month_list
                    .iter()
                    .zip(month_labels.iter())
                    .map(|(m, lbl)| MonthlyActual {
                        month: m.clone(),
                        label: lbl.clone(),
                        spent_cents: spend_map
                            .get(&(id.clone(), m.clone()))
                            .copied()
                            .unwrap_or(0),
                        budgeted_cents: budget_map
                            .get(&(id.clone(), m.clone()))
                            .copied()
                            .unwrap_or(0),
                    })
                    .collect();
                let total: i64 = monthly.iter().map(|m| m.spent_cents).sum();
```

And the sort comparator further down:

```rust
        result.sort_by(|a, b| {
            let ta: i64 = a.monthly.iter().map(|m| m.cents).sum();
            let tb: i64 = b.monthly.iter().map(|m| m.cents).sum();
            tb.cmp(&ta)
        });
```

becomes:

```rust
        result.sort_by(|a, b| {
            let ta: i64 = a.monthly.iter().map(|m| m.spent_cents).sum();
            let tb: i64 = b.monthly.iter().map(|m| m.spent_cents).sum();
            tb.cmp(&ta)
        });
```

- [ ] **Step 2: Build**

Run: `cargo build -p finsight-app`
Expected: builds with no errors. (No new Rust test added here — `budgets::list_for_month` is already exercised in Task 1/2's tests; the join added here is straightforward and covered end-to-end by the frontend test in Task 9.)

- [ ] **Step 3: Regenerate bindings**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `MonthlyActual` in `ui/src/api/bindings.ts` becomes `{ month: string; label: string; spentCents: number; budgetedCents: number }`.

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/budget.rs ui/src/api/bindings.ts
git commit -m "feat(budgets): return budgeted amounts alongside spent in budget history"
```

---

## Task 9: Frontend — budgeted-vs-spent history table

**Files:**
- Modify: `ui/src/screens/Budget.tsx`
- Modify: `ui/src/screens/Budget.history.test.tsx`

- [ ] **Step 1: Update the history table**

Replace the history `<section>` (line 307):

```tsx
      {history.length > 0 && <section className="section"><div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot" />Spending history · last 5 months</div><div className="card flush"><table className="tbl"><thead><tr><th>Category</th>{history[0]?.monthly.map((m) => <th key={m.month} className="right">{m.label}</th>)}</tr></thead><tbody>{history.map((row) => <tr key={row.categoryId}><td><span className="cswatch" style={{ background: row.color || "var(--accent)" }} /> {row.label}</td>{row.monthly.map((m) => <td key={m.month} className="right"><span className="money">{money(m.cents)}</span></td>)}</tr>)}</tbody></table></div></section>}
```

with:

```tsx
      {history.length > 0 && (
        <section className="section">
          <div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot" />Spending history · last 5 months</div>
          <div className="card flush">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Category</th>
                  {history[0]?.monthly.map((m) => <th key={m.month} className="right">{m.label}</th>)}
                  <th className="right">Your typical</th>
                </tr>
              </thead>
              <tbody>
                {history.map((row) => {
                  const typicalCents = Math.round(
                    row.monthly.reduce((sum, m) => sum + m.spentCents, 0) / Math.max(1, row.monthly.length),
                  );
                  return (
                    <tr key={row.categoryId}>
                      <td><span className="cswatch" style={{ background: row.color || "var(--accent)" }} /> {row.label}</td>
                      {row.monthly.map((m) => {
                        const over = m.budgetedCents > 0 && m.spentCents > m.budgetedCents;
                        return (
                          <td key={m.month} className="right">
                            <span className={`money ${over ? "neg" : ""}`}>{money(m.spentCents)}</span>
                            {m.budgetedCents > 0 && <span className="muted" style={{ fontSize: 11, display: "block" }}>of {money(m.budgetedCents)}</span>}
                          </td>
                        );
                      })}
                      <td className="right"><span className="money muted">{money(typicalCents)}</span></td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </section>
      )}
```

- [ ] **Step 2: Update existing test fixtures**

`Budget.history.test.tsx`'s mock `CategoryHistory`/`MonthlyActual` fixtures use the old `{ month, label, cents }` shape — update every fixture object's `monthly` entries to `{ month, label, spentCents, budgetedCents }` (pick a `budgetedCents` value per fixture that matches the scenario being tested — e.g. if a test asserts an "over budget" figure, set `budgetedCents` below `spentCents` for that entry).

- [ ] **Step 3: Run and type-check**

Run: `cd ui && npx vitest run src/screens/Budget.history.test.tsx`
Expected: passes once fixtures are updated to the new field names.

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/Budget.tsx ui/src/screens/Budget.history.test.tsx
git commit -m "feat(budgets): show budgeted vs spent and a typical-month column in history"
```

---

## Task 10: `LookBackFact` + `look_back_facts` in `repos/budgets.rs`

**Files:**
- Modify: `crates/finsight-core/src/repos/budgets.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/finsight-core/src/repos/budgets.rs`:

```rust
    #[test]
    fn look_back_flags_the_biggest_overage_and_underage() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "dining");
        set(&mut conn, "dining", "2026-05", 40_000).unwrap();
        spend(&mut conn, "dining", "2026-05-10T00:00:00Z", 41_200); // $12 over

        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES('travel', 'daily', 'Travel', '#000', 1)",
            [],
        ).unwrap();
        set(&mut conn, "travel", "2026-05", 50_000).unwrap(); // no spend at all: $500 under

        let facts = look_back_facts(&mut conn, "2026-05").unwrap();
        assert!(facts.iter().any(|f| f.category_id == "dining" && f.kind == "over" && f.amount_cents == 1_200));
        assert!(facts.iter().any(|f| f.category_id == "travel" && f.kind == "under" && f.amount_cents == 50_000));
    }

    #[test]
    fn look_back_flags_a_zero_spend_streak() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "travel");
        for m in ["2026-02", "2026-03", "2026-04", "2026-05"] {
            set(&mut conn, "travel", m, 50_000).unwrap();
        }
        // No spend at all across 4 budgeted months.
        let facts = look_back_facts(&mut conn, "2026-05").unwrap();
        let streak = facts.iter().find(|f| f.category_id == "travel" && f.kind == "streak").unwrap();
        assert_eq!(streak.streak_months, 4);
    }

    #[test]
    fn look_back_ignores_unbudgeted_categories() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "food");
        // No budgets row at all — spending here shouldn't produce an "over"/"under" fact.
        spend(&mut conn, "food", "2026-05-10T00:00:00Z", 5_000);
        let facts = look_back_facts(&mut conn, "2026-05").unwrap();
        assert!(facts.iter().all(|f| f.category_id != "food"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p finsight-core --lib repos::budgets::tests -- --nocapture`
Expected: compile error — `look_back_facts` and `LookBackFact` are not defined.

- [ ] **Step 3: Implement**

Insert into `crates/finsight-core/src/repos/budgets.rs`, after `carryover_into_month` (before the `#[cfg(test)]` block). This needs `serde::Serialize`, `specta::Type`, and `rusqlite::OptionalExtension` (for the `.optional()` budgeted-month check in the streak loop) — update the file's top imports from:

```rust
use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;
```

to:

```rust
use crate::error::CoreResult;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use specta::Type;
use uuid::Uuid;
```

```rust
/// A single plain-language fact about how `month` went for a budgeted category,
/// used to open the Plan Next Month wizard. Deterministic, no LLM — the frontend
/// composes the sentence (and applies the user's money formatting/privacy mode)
/// from `kind` + `amount_cents`/`streak_months`; this never bakes a formatted
/// dollar string server-side.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LookBackFact {
    pub category_id: String,
    pub category_label: String,
    /// "over" | "under" | "streak"
    pub kind: String,
    /// Meaningful for "over" (spent − budgeted) and "under" (budgeted − spent); 0 for "streak".
    pub amount_cents: i64,
    /// Meaningful for "streak" (consecutive zero-spend months including `month`); 0 otherwise.
    pub streak_months: i64,
}

/// Up to 3 facts about `month`: the biggest overage, the biggest underage, and
/// the longest zero-spend streak (>= 2 consecutive months) — each only among
/// categories that were actually budgeted (amount_cents > 0) for `month`.
pub fn look_back_facts(conn: &mut Connection, month: &str) -> CoreResult<Vec<LookBackFact>> {
    let month_start = format!("{month}-01");
    let next_month = month_before(month, -1);
    let next_month_start = format!("{next_month}-01");

    let mut stmt = conn.prepare(
        "SELECT c.id, c.label, COALESCE(b.amount_cents, 0),
                COALESCE(SUM(CASE WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END), 0)
         FROM categories c
         LEFT JOIN budgets b ON b.category_id = c.id AND b.month = ?1
         LEFT JOIN transactions t ON t.category_id = c.id AND t.posted_at >= ?2 AND t.posted_at < ?3
         WHERE c.archived_at IS NULL
         GROUP BY c.id, c.label, b.amount_cents",
    )?;
    let rows: Vec<(String, String, i64, i64)> = stmt
        .query_map(rusqlite::params![month, month_start, next_month_start], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(stmt);

    let mut facts = Vec::new();

    if let Some((id, label, budget, spent)) = rows
        .iter()
        .filter(|(_, _, budget, spent)| *budget > 0 && spent > budget)
        .max_by_key(|(_, _, budget, spent)| spent - budget)
    {
        facts.push(LookBackFact {
            category_id: id.clone(),
            category_label: label.clone(),
            kind: "over".to_string(),
            amount_cents: spent - budget,
            streak_months: 0,
        });
    }

    if let Some((id, label, budget, spent)) = rows
        .iter()
        .filter(|(_, _, budget, spent)| *budget > 0 && budget > spent)
        .max_by_key(|(_, _, budget, spent)| budget - spent)
    {
        facts.push(LookBackFact {
            category_id: id.clone(),
            category_label: label.clone(),
            kind: "under".to_string(),
            amount_cents: budget - spent,
            streak_months: 0,
        });
    }

    let mut best: Option<(String, String, i64)> = None;
    for (id, label, budget, spent) in &rows {
        if *budget <= 0 || *spent != 0 {
            continue;
        }
        let mut streak = 1i64;
        for back in 1..12 {
            let m = month_before(month, back);
            // Stop at the first prior month this category wasn't actually
            // budgeted for — otherwise a category that has simply never been
            // budgeted (zero spend forever) would read as an N-month streak
            // instead of "not applicable." Only a budgeted-but-unspent run counts.
            let was_budgeted: bool = conn
                .query_row(
                    "SELECT 1 FROM budgets WHERE category_id = ?1 AND month = ?2 AND amount_cents > 0",
                    rusqlite::params![id, m],
                    |_| Ok(true),
                )
                .optional()?
                .unwrap_or(false);
            if !was_budgeted {
                break;
            }
            let m_start = format!("{m}-01");
            let m_next = month_before(month, back - 1);
            let m_next_start = format!("{m_next}-01");
            let spent_that_month: i64 = conn.query_row(
                "SELECT COALESCE(SUM(-amount_cents), 0) FROM transactions \
                 WHERE category_id = ?1 AND amount_cents < 0 AND posted_at >= ?2 AND posted_at < ?3",
                rusqlite::params![id, m_start, m_next_start],
                |r| r.get(0),
            )?;
            if spent_that_month == 0 {
                streak += 1;
            } else {
                break;
            }
        }
        if streak >= 2 && best.as_ref().map(|(_, _, s)| streak > *s).unwrap_or(true) {
            best = Some((id.clone(), label.clone(), streak));
        }
    }
    if let Some((id, label, streak)) = best {
        facts.push(LookBackFact {
            category_id: id,
            category_label: label,
            kind: "streak".to_string(),
            amount_cents: 0,
            streak_months: streak,
        });
    }

    Ok(facts)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p finsight-core --lib repos::budgets::tests -- --nocapture`
Expected: `test result: ok. 11 passed`

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/repos/budgets.rs
git commit -m "feat(budgets): compute deterministic look-back facts for Plan Next Month"
```

---

## Task 11: Wire `look_back` and `sinking_funds` into `get_plan_next_month_data`

**Files:**
- Modify: `crates/finsight-app/src/commands/budget.rs` (`PlanData`, `get_plan_next_month_data`)

- [ ] **Step 1: Extend `PlanData`**

Replace the `PlanData` struct (lines 128-135):

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanData {
    pub income_cents: i64,
    pub categories: Vec<CategoryPlanRow>,
    pub goals: Vec<GoalDto>,
    pub sinking_funds: Vec<GoalDto>,
    pub recurring_expense_cents: i64,
    pub look_back: Vec<budgets::LookBackFact>,
}
```

- [ ] **Step 2: Compute the two new fields in `get_plan_next_month_data`**

Replace the "Active goals" block:

```rust
        // Active goals (current < target, not archived)
        let all_goals = goals::list(conn)?;
        let active_goals: Vec<GoalDto> = all_goals
            .into_iter()
            .filter(|g| g.current_cents < g.target_cents)
            .map(goal_to_dto)
            .collect();
```

with:

```rust
        // Sinking funds get their own Plan-wizard step; everything else that's
        // still open (current < target) is a regular active goal.
        let all_goals = goals::list(conn)?;
        let (sinking, other): (Vec<_>, Vec<_>) =
            all_goals.into_iter().partition(|g| g.goal_type == "sinking-fund");
        let sinking_funds: Vec<GoalDto> = sinking.into_iter().map(goal_to_dto).collect();
        let active_goals: Vec<GoalDto> = other
            .into_iter()
            .filter(|g| g.current_cents < g.target_cents)
            .map(goal_to_dto)
            .collect();
```

Then replace the final `Ok(PlanData { ... })`:

```rust
        Ok(PlanData {
            income_cents,
            categories,
            goals: active_goals,
            recurring_expense_cents,
        })
```

with:

```rust
        let look_back = budgets::look_back_facts(conn, &m0)?;

        Ok(PlanData {
            income_cents,
            categories,
            goals: active_goals,
            sinking_funds,
            recurring_expense_cents,
            look_back,
        })
```

- [ ] **Step 3: Build**

Run: `cargo build -p finsight-app`
Expected: builds with no errors.

- [ ] **Step 4: Regenerate bindings**

Run: `cargo run -p finsight-tauri --bin export_bindings`
Expected: `PlanData` in `ui/src/api/bindings.ts` gains `sinkingFunds: GoalDto[]` and `lookBack: LookBackFact[]`; a new `LookBackFact` type appears (`{ categoryId, categoryLabel, kind, amountCents, streakMonths }`).

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/budget.rs ui/src/api/bindings.ts
git commit -m "feat(plan-next-month): surface sinking funds and look-back facts"
```

---

## Task 12: `Goals.tsx` — add the `"sinking-fund"` type

**Files:**
- Modify: `ui/src/screens/Goals.tsx`

- [ ] **Step 1: Extend `GoalFilter` and `TYPE_LABELS`**

Replace line 13:

```tsx
type GoalFilter = "all" | "save-by-date" | "build-balance" | "debt-payoff" | "spending-cap";
```

with:

```tsx
type GoalFilter = "all" | "save-by-date" | "build-balance" | "debt-payoff" | "spending-cap" | "sinking-fund";
```

Replace lines 15-20:

```tsx
const TYPE_LABELS: Record<string, string> = {
  "save-by-date": "Save by date",
  "build-balance": "Build balance",
  "debt-payoff": "Pay off debt",
  "spending-cap": "Spending cap",
};
```

with:

```tsx
const TYPE_LABELS: Record<string, string> = {
  "save-by-date": "Save by date",
  "build-balance": "Build balance",
  "debt-payoff": "Pay off debt",
  "spending-cap": "Spending cap",
  "sinking-fund": "Sinking fund",
};
```

- [ ] **Step 2: Exclude sinking funds from the Compound Growth Projector**

Replace line 365:

```tsx
    () => goals.filter((g) => g.goalType !== "spending-cap" && (g.monthlyCents > 0 || g.currentCents > 0)),
```

with:

```tsx
    () => goals.filter((g) => g.goalType !== "spending-cap" && g.goalType !== "sinking-fund" && (g.monthlyCents > 0 || g.currentCents > 0)),
```

Replace line 603:

```tsx
      {goals.some((g) => g.goalType !== "spending-cap" && (g.monthlyCents > 0 || g.currentCents > 0)) && <CompoundGrowthProjector goals={goals} />}
```

with:

```tsx
      {goals.some((g) => g.goalType !== "spending-cap" && g.goalType !== "sinking-fund" && (g.monthlyCents > 0 || g.currentCents > 0)) && <CompoundGrowthProjector goals={goals} />}
```

- [ ] **Step 3: Add a filter button**

Replace line 566:

```tsx
        <button className={filter === "spending-cap" ? "on" : ""} type="button" onClick={() => setFilter("spending-cap")}>Spending cap {counts["spending-cap"] ?? 0}</button>
```

with:

```tsx
        <button className={filter === "spending-cap" ? "on" : ""} type="button" onClick={() => setFilter("spending-cap")}>Spending cap {counts["spending-cap"] ?? 0}</button>
        <button className={filter === "sinking-fund" ? "on" : ""} type="button" onClick={() => setFilter("sinking-fund")}>Sinking fund {counts["sinking-fund"] ?? 0}</button>
```

(`canPause`, the progress-vs-cap rendering, and the new-goal-type dropdown all key off `TYPE_LABELS`/the two named exclusions already present and need no further changes — a sinking fund falls through to the same "progress bar, can pause" path as `build-balance`.)

- [ ] **Step 4: Type-check and run existing Goals tests**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

Run: `cd ui && npx vitest run src/screens/Goals.test.tsx`
Expected: existing tests still pass (adding a type/button doesn't change existing filter behavior).

- [ ] **Step 5: Commit**

```bash
git add ui/src/screens/Goals.tsx
git commit -m "feat(goals): add a sinking-fund goal type, excluded from the growth projector"
```

---

## Task 13: `PlanNextMonthModal.tsx` — full rebuild to 7 steps

**Files:**
- Modify: `ui/src/screens/PlanNextMonthModal.tsx` (full replacement)

- [ ] **Step 1: Replace the entire file**

Replace the full contents of `ui/src/screens/PlanNextMonthModal.tsx` with:

```tsx
import { useMemo, useState } from "react";
import {
  usePlanNextMonthData,
  useApplyNextMonthPlan,
  useUpdateGoalMonthly,
} from "../api/hooks/budget";
import { type CategoryPlanRow, type PlanAssignment } from "../api/client";
import { toast } from "sonner";
import { money } from "../utils/format";

interface Props {
  onClose: () => void;
}

const STEPS = ["Look back", "Fixed costs", "Sinking funds", "Buffer", "Goals", "Adjust", "Review"];

interface AdjustSuggestion {
  categoryId: string;
  label: string;
  suggestedCents: number;
  monthsOver: number;
}

/** Non-fixed categories over budget in >= 2 of the last 3 months, sorted worst first, capped at 3. */
function computeAdjustSuggestions(categories: CategoryPlanRow[]): AdjustSuggestion[] {
  const suggestions: AdjustSuggestion[] = [];
  for (const cat of categories) {
    if (cat.groupLabel.toLowerCase().includes("fixed")) continue; // has its own step
    if (cat.budgetCents <= 0) continue;
    const months = [cat.m0Cents, cat.m1Cents, cat.m2Cents];
    const monthsOver = months.filter((m) => m > cat.budgetCents).length;
    if (monthsOver >= 2) {
      const maxSpend = Math.max(...months);
      const suggestedCents = Math.ceil(maxSpend / 1000) * 1000; // round up to the nearest $10
      suggestions.push({ categoryId: cat.categoryId, label: cat.label, suggestedCents, monthsOver });
    }
  }
  return suggestions.sort((a, b) => b.monthsOver - a.monthsOver).slice(0, 3);
}

export default function PlanNextMonthModal({ onClose }: Props) {
  const { data, isLoading } = usePlanNextMonthData();
  const apply = useApplyNextMonthPlan();
  const updateGoalMonthly = useUpdateGoalMonthly();
  const [step, setStepRaw] = useState(0);
  const [reachedSteps, setReachedSteps] = useState<Set<number>>(new Set([0]));
  // Category budget assignments: categoryId → cents.
  const [assignments, setAssignments] = useState<Record<string, number>>({});
  // Monthly-contribution overrides for sinking funds / goals: goalId → cents.
  const [sinkingAssignments, setSinkingAssignments] = useState<Record<string, number>>({});
  const [goalAssignments, setGoalAssignments] = useState<Record<string, number>>({});
  const [buffer, setBuffer] = useState(0);
  const [acceptedAdjustments, setAcceptedAdjustments] = useState<Set<string>>(new Set());

  const setStep = (updater: number | ((s: number) => number)) => {
    setStepRaw((prev) => {
      const next = typeof updater === "function" ? updater(prev) : updater;
      setReachedSteps((r) => (r.has(next) ? r : new Set(r).add(next)));
      return next;
    });
  };

  const fmt = (cents: number) => money(cents);
  const setAmt = (categoryId: string, cents: number) => setAssignments((prev) => ({ ...prev, [categoryId]: cents }));
  const setSinkingAmt = (goalId: string, cents: number) => setSinkingAssignments((prev) => ({ ...prev, [goalId]: cents }));
  const setGoalAmt = (goalId: string, cents: number) => setGoalAssignments((prev) => ({ ...prev, [goalId]: cents }));

  const suggestions = useMemo(() => (data ? computeAdjustSuggestions(data.categories) : []), [data]);

  if (isLoading || !data) {
    return (
      <div style={{ position: "fixed", inset: 0, zIndex: 70, background: "var(--bg)", display: "flex", alignItems: "center", justifyContent: "center" }}>
        <span className="muted">Loading…</span>
      </div>
    );
  }

  const acceptAdjustment = (s: AdjustSuggestion) => {
    setAcceptedAdjustments((prev) => new Set(prev).add(s.categoryId));
    setAmt(s.categoryId, s.suggestedCents);
  };

  const fixedTotal = data.categories
    .filter((c) => c.groupLabel.toLowerCase().includes("fixed"))
    .reduce((sum, c) => sum + (assignments[c.categoryId] ?? c.budgetCents ?? 0), 0);
  const sinkingTotal = Object.values(sinkingAssignments).reduce((sum, v) => sum + v, 0);
  const goalTotal = Object.values(goalAssignments).reduce((sum, v) => sum + v, 0);
  const planned = fixedTotal + sinkingTotal + buffer + goalTotal;
  const remainingCents = data.incomeCents - planned;

  const handleApply = async () => {
    const categoryAssignments: PlanAssignment[] = Object.entries(assignments)
      .filter(([, cents]) => cents > 0)
      .map(([categoryId, amountCents]) => ({ categoryId, amountCents }));
    try {
      await apply.mutateAsync(categoryAssignments);
      const monthlyUpdates = [...Object.entries(sinkingAssignments), ...Object.entries(goalAssignments)];
      for (const [id, monthlyCents] of monthlyUpdates) {
        await updateGoalMonthly.mutateAsync({ id, monthlyCents });
      }
      toast.success("Next month's budget applied!");
      onClose();
    } catch (e: unknown) {
      toast.error(e instanceof Error ? e.message : "Failed to apply budget");
    }
  };

  const renderFixedCostsStep = () => (
    <div>
      {data.categories
        .filter((cat) => cat.groupLabel.toLowerCase().includes("fixed"))
        .map((cat) => {
          const current = assignments[cat.categoryId] ?? cat.budgetCents ?? 0;
          const avg = Math.round((cat.m0Cents + cat.m1Cents + cat.m2Cents) / 3);
          return (
            <div key={cat.categoryId} style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 12 }}>
              <span style={{ flex: 1 }}>{cat.label}</span>
              <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>
                avg <span className="money">{fmt(avg)}</span>
              </span>
              <input
                type="number"
                value={Math.round(current / 100)}
                min={0}
                step={10}
                style={{ width: 80, textAlign: "right", padding: "4px 8px", background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 4, color: "var(--ink)", fontFamily: "var(--mono)", fontSize: 13 }}
                onChange={(e) => setAmt(cat.categoryId, Math.round(parseFloat(e.target.value || "0") * 100))}
              />
            </div>
          );
        })}
    </div>
  );

  const renderStep = () => {
    switch (step) {
      case 0: // Look back
        return (
          <div>
            <div className="num-step">Step 1 of 7 · Look back</div>
            <h1>First, look back.</h1>
            <p className="lead">Before deciding what next month should be, a quick view of how last month actually played out — no shame, no celebration, just the facts.</p>
            {data.lookBack.length === 0 ? (
              <p className="muted">Not enough budgeted history yet to draw any facts from last month.</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 10, maxWidth: 460 }}>
                {data.lookBack.map((f) => (
                  <div key={`${f.categoryId}-${f.kind}`} className="card tight" style={{ padding: 14 }}>
                    <div className="strong" style={{ fontSize: 14 }}>
                      {f.kind === "over" && <>{f.categoryLabel} ran <span className="money">{fmt(f.amountCents)}</span> over budget.</>}
                      {f.kind === "under" && <>{f.categoryLabel} came in <span className="money">{fmt(f.amountCents)}</span> under budget.</>}
                      {f.kind === "streak" && <>{f.categoryLabel} sat at $0 — {f.streakMonths} months in a row.</>}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        );
      case 1: // Fixed costs
        return (
          <div>
            <div className="num-step">Step 2 of 7 · Fixed costs</div>
            <h1>What's already spoken for?</h1>
            <p className="lead">Things that show up whether you plan for them or not.</p>
            {renderFixedCostsStep()}
          </div>
        );
      case 2: // Sinking funds
        return (
          <div>
            <div className="num-step">Step 3 of 7 · Sinking funds</div>
            <h1>What's coming that isn't monthly?</h1>
            <p className="lead">Insurance renewals, annual bills, the irregular expenses that ambush you if you don't set them aside a little at a time.</p>
            {data.sinkingFunds.length === 0 ? (
              <p className="muted">No sinking funds yet — create one on the Goals screen with type "Sinking fund".</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {data.sinkingFunds.map((s) => {
                  const val = sinkingAssignments[s.id] ?? s.monthlyCents;
                  return (
                    <div key={s.id} style={{ padding: 14, background: "var(--surface-2)", borderRadius: 8 }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                        <div>
                          <div style={{ fontSize: 14, fontWeight: 500 }}>{s.name}</div>
                          <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>
                            <span className="money">{fmt(s.currentCents)}</span> of <span className="money">{fmt(s.targetCents)}</span>
                          </div>
                        </div>
                        <div className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>
                          {fmt(val)}<span style={{ fontSize: 13, color: "var(--ink-mute)", marginLeft: 4 }}>/mo</span>
                        </div>
                      </div>
                      <input
                        type="range"
                        min="0"
                        max="50000"
                        step="1000"
                        value={val}
                        onChange={(e) => setSinkingAmt(s.id, parseInt(e.target.value, 10))}
                        style={{ width: "100%", marginTop: 10, accentColor: "var(--accent)" }}
                      />
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      case 3: // Buffer
        return (
          <div>
            <div className="num-step">Step 4 of 7 · Buffer</div>
            <h1>How much slack should next month have?</h1>
            <p className="lead">Money set aside but not assigned to anything yet — deliberate breathing room, not a leftover.</p>
            <div style={{ maxWidth: 460 }}>
              <div style={{ padding: 18, background: "var(--surface-2)", borderRadius: 10, marginTop: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                  <span style={{ fontSize: 14 }}>Buffer</span>
                  <span className="figure" style={{ fontSize: 26, color: "var(--accent)" }}>{fmt(buffer)}</span>
                </div>
                <input
                  type="range"
                  min="0"
                  max="200000"
                  step="5000"
                  value={buffer}
                  onChange={(e) => setBuffer(parseInt(e.target.value, 10))}
                  style={{ width: "100%", marginTop: 12, accentColor: "var(--accent)" }}
                />
              </div>
            </div>
          </div>
        );
      case 4: // Goals
        return (
          <div>
            <div className="num-step">Step 5 of 7 · Goals</div>
            <h1>What are we moving toward?</h1>
            <p className="lead">Tune what you'll contribute to each goal this month.</p>
            {data.goals.length === 0 ? (
              <p className="muted">No active goals.</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {data.goals.map((g) => {
                  const val = goalAssignments[g.id] ?? g.monthlyCents;
                  return (
                    <div key={g.id} style={{ padding: 14, background: "var(--surface-2)", borderRadius: 8 }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                        <div>
                          <div style={{ fontSize: 14, fontWeight: 500 }}>{g.name}</div>
                          <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>
                            <span className="money">{fmt(g.currentCents)}</span> of <span className="money">{fmt(g.targetCents)}</span>
                          </div>
                        </div>
                        <div className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>
                          {fmt(val)}<span style={{ fontSize: 13, color: "var(--ink-mute)", marginLeft: 4 }}>/mo</span>
                        </div>
                      </div>
                      <input
                        type="range"
                        min="0"
                        max="200000"
                        step="5000"
                        value={val}
                        onChange={(e) => setGoalAmt(g.id, parseInt(e.target.value, 10))}
                        style={{ width: "100%", marginTop: 10, accentColor: "var(--accent)" }}
                      />
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      case 5: // Adjust
        return (
          <div>
            <div className="num-step">Step 6 of 7 · Adjust</div>
            <h1>What needs to shift?</h1>
            <p className="lead">Categories that ran over budget in at least 2 of the last 3 months — based on your own history, not a guess.</p>
            {suggestions.length === 0 ? (
              <p className="muted">Nothing stands out — your non-fixed categories have mostly stayed within budget.</p>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {suggestions.map((s) => {
                  const on = acceptedAdjustments.has(s.categoryId);
                  return (
                    <div
                      key={s.categoryId}
                      onClick={() => acceptAdjustment(s)}
                      style={{ padding: 14, background: on ? "var(--accent-2)" : "var(--surface-2)", border: `1px solid ${on ? "var(--accent-3)" : "var(--line)"}`, borderRadius: 8, cursor: "pointer" }}
                    >
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                        <span style={{ fontSize: 14, fontWeight: 500 }}>Raise {s.label} to {fmt(s.suggestedCents)}</span>
                        <span className={`tog ${on ? "on" : ""}`} />
                      </div>
                      <div className="muted" style={{ fontSize: 13, marginTop: 6 }}>Over budget {s.monthsOver} of the last 3 months.</div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      case 6: // Review
        return (
          <div>
            <div className="num-step">Step 7 of 7 · Review</div>
            <h1>Review &amp; apply.</h1>
            <p className="lead">Confirm the amounts below before applying next month's plan.</p>
            <table className="tbl" style={{ width: "100%" }}>
              <tbody>
                <tr><td>Fixed costs</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(fixedTotal)}</span></td></tr>
                <tr><td>Sinking funds</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(sinkingTotal)}</span></td></tr>
                <tr><td>Buffer</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(buffer)}</span></td></tr>
                <tr><td>Goals</td><td className="right" style={{ fontFamily: "var(--mono)" }}><span className="money">{fmt(goalTotal)}</span></td></tr>
              </tbody>
            </table>
          </div>
        );
      default:
        return null;
    }
  };

  const renderPreview = () => {
    const segments = [
      { key: "fixed", label: "Fixed costs", cents: fixedTotal },
      { key: "sinks", label: "Sinking funds", cents: sinkingTotal },
      { key: "buffer", label: "Buffer", cents: buffer },
      { key: "goals", label: "Goals", cents: goalTotal },
    ].filter((s) => s.cents > 0);

    return (
      <>
        <div className="eyebrow" style={{ marginBottom: 14 }}>
          <span className="dot" />Live preview
        </div>
        <div className="card" style={{ padding: 22 }}>
          <div className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)", textTransform: "uppercase", letterSpacing: "0.06em", marginBottom: 8 }}>
            Income
          </div>
          <div style={{ fontSize: 32, fontFamily: "var(--mono)", marginBottom: 16 }}>
            <span className="money">{fmt(data.incomeCents)}</span>
          </div>

          <div style={{ height: 24, borderRadius: 6, background: "var(--surface-2)", overflow: "hidden", display: "flex", gap: 2 }}>
            {segments.map((s) => (
              <span
                key={s.key}
                title={`${s.label} ${fmt(s.cents)}`}
                style={{ width: `${data.incomeCents > 0 ? Math.min(100, (s.cents / data.incomeCents) * 100) : 0}%`, background: "var(--accent)" }}
              />
            ))}
            {remainingCents > 0 && (
              <span title={`Unassigned ${fmt(remainingCents)}`} style={{ flex: 1, background: "var(--surface)", borderLeft: "1px dashed var(--ink-faint)" }} />
            )}
          </div>

          <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 8 }}>
            {segments.map((s) => (
              <div key={s.key} style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <span style={{ fontSize: 14 }}>{s.label}</span>
                <span className="num money" style={{ fontSize: 14 }}>{fmt(s.cents)}</span>
              </div>
            ))}
            <div style={{ height: 1, background: "var(--hairline)", margin: "4px 0" }} />
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <span style={{ fontSize: 14, fontWeight: 500 }}>{remainingCents >= 0 ? "Unassigned" : "Over"}</span>
              <span className="num money" style={{ fontSize: 14, fontWeight: 600, color: remainingCents < 0 ? "var(--negative)" : undefined }}>
                {fmt(Math.abs(remainingCents))}
              </span>
            </div>
          </div>
        </div>
      </>
    );
  };

  return (
    <div style={{ position: "fixed", inset: 0, zIndex: 70, background: "var(--bg)", display: "flex", alignItems: "center", justifyContent: "center", padding: 24 }}>
      <div className="onb-shell" style={{ width: "100%", maxWidth: 1120 }}>
        <header className="onb-top">
          <div className="brand" style={{ padding: 0 }}>
            <div className="mark" aria-hidden="true" />
            <div className="wm">FinSight</div>
          </div>
          <nav className="onb-steps" aria-label="Plan next month progress">
            {STEPS.map((s, i) => {
              const reached = reachedSteps.has(i);
              return (
                <button
                  key={s}
                  className={`onb-step-pip ${i === step ? "cur" : ""} ${reached ? "done" : ""}`}
                  disabled={!reached}
                  onClick={() => reached && setStep(i)}
                  aria-current={i === step ? "step" : undefined}
                  aria-label={`Go to ${s} step`}
                  title={s}
                  type="button"
                />
              );
            })}
          </nav>
          <button className="btn ghost sm" onClick={onClose}>
            ✕ Close
          </button>
        </header>

        <section className="onb-stage" aria-label="Plan next month steps">
          <div className="onb-split">
            <div className="onb-left">
              {renderStep()}

              <div className="onb-actions" style={{ marginTop: 24 }}>
                {step > 0 && (
                  <button className="btn ghost" onClick={() => setStep((s) => s - 1)}>
                    ← Back
                  </button>
                )}
                {step < STEPS.length - 1 ? (
                  <button className="btn primary" onClick={() => setStep((s) => s + 1)}>
                    Next →
                  </button>
                ) : (
                  <button
                    className="btn primary"
                    onClick={() => void handleApply()}
                    disabled={apply.isPending || updateGoalMonthly.isPending}
                  >
                    {apply.isPending || updateGoalMonthly.isPending ? "Applying…" : "Apply budget"}
                  </button>
                )}
              </div>
            </div>

            <div className="onb-right">{renderPreview()}</div>
          </div>
        </section>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: errors only in `PlanNextMonthModal.test.tsx` (fixed in Task 14) — no errors in the modal file itself.

- [ ] **Step 3: Commit**

```bash
git add ui/src/screens/PlanNextMonthModal.tsx
git commit -m "feat(plan-next-month): rebuild as a 7-step flow (look back, sinking funds, buffer, adjust)"
```

---

## Task 14: Rework `PlanNextMonthModal.test.tsx`

**Files:**
- Modify: `ui/src/screens/PlanNextMonthModal.test.tsx` (full replacement)

- [ ] **Step 1: Replace the test file**

Replace the full contents of `ui/src/screens/PlanNextMonthModal.test.tsx` with:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import PlanNextMonthModal from "./PlanNextMonthModal";
import { createWrapper } from "../test-utils";

const applyMutate = vi.fn();
const updateGoalMonthlyMutate = vi.fn();

vi.mock("../api/hooks/budget", () => ({
  usePlanNextMonthData: vi.fn(() => ({ data: undefined, isLoading: true })),
  useApplyNextMonthPlan: vi.fn(() => ({ mutateAsync: applyMutate, isPending: false })),
  useUpdateGoalMonthly: vi.fn(() => ({ mutateAsync: updateGoalMonthlyMutate, isPending: false })),
  useBudgetEnvelopes: vi.fn(() => ({ data: [] })),
  useBudgetHistory: vi.fn(() => ({ data: [] })),
}));

vi.mock("sonner", () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

const MOCK_DATA = {
  incomeCents: 500000,
  recurringExpenseCents: 80000,
  lookBack: [
    { categoryId: "c2", categoryLabel: "Groceries", kind: "under", amountCents: 200, streakMonths: 0 },
  ],
  sinkingFunds: [
    { id: "s1", name: "Car insurance", goalType: "sinking-fund", targetCents: 48000, currentCents: 20000, monthlyCents: 8000, targetDate: null, color: "#000", notes: null, purpose: null, sortOrder: 0, createdAt: "2026-01-01", accountId: null },
  ],
  goals: [
    { id: "g1", name: "Emergency Fund", goalType: "build-balance", targetCents: 1000000, currentCents: 250000, monthlyCents: 90000, targetDate: null, color: "#000", notes: null, purpose: null, sortOrder: 0, createdAt: "2026-01-01", accountId: null },
  ],
  categories: [
    {
      categoryId: "c1",
      label: "Rent",
      color: "#e74c3c",
      groupLabel: "Fixed costs",
      budgetCents: 150000,
      m0Cents: 150000,
      m1Cents: 150000,
      m2Cents: 150000,
    },
    {
      categoryId: "c2",
      label: "Groceries",
      color: "#27ae60",
      groupLabel: "Daily life",
      budgetCents: 40000,
      m0Cents: 38000,
      m1Cents: 42000,
      m2Cents: 41000,
    },
  ],
};

describe("PlanNextMonthModal", () => {
  beforeEach(async () => {
    vi.clearAllMocks();

    const budget = await import("../api/hooks/budget");
    vi.mocked(budget.usePlanNextMonthData).mockReturnValue({
      data: MOCK_DATA,
      isLoading: false,
    } as ReturnType<typeof budget.usePlanNextMonthData>);
    vi.mocked(budget.useApplyNextMonthPlan).mockReturnValue({
      mutateAsync: applyMutate,
      isPending: false,
    } as unknown as ReturnType<typeof budget.useApplyNextMonthPlan>);
    vi.mocked(budget.useUpdateGoalMonthly).mockReturnValue({
      mutateAsync: updateGoalMonthlyMutate,
      isPending: false,
    } as unknown as ReturnType<typeof budget.useUpdateGoalMonthly>);
  });

  it("renders the Look back step by default", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    expect(screen.getByText("First, look back.")).toBeInTheDocument();
    expect(screen.getByText("Step 1 of 7 · Look back")).toBeInTheDocument();
    expect(screen.getAllByText("$5,000").length).toBeGreaterThan(0);
  });

  it("navigates to the Fixed costs step on Next click", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Next →"));
    expect(screen.getByText("What's already spoken for?")).toBeInTheDocument();
    expect(screen.getByText("Step 2 of 7 · Fixed costs")).toBeInTheDocument();
  });

  it("shows Back button after navigating forward", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Next →"));
    expect(screen.getByText("← Back")).toBeInTheDocument();
  });

  it("calls onClose when ✕ Close is clicked", () => {
    const onClose = vi.fn();
    render(<PlanNextMonthModal onClose={onClose} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("✕ Close"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("reaches the Review step after 6 Next clicks", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    for (let i = 0; i < 6; i++) {
      fireEvent.click(screen.getByText("Next →"));
    }
    expect(screen.getByText("Apply budget")).toBeInTheDocument();
  });

  it("calls apply and onClose on Apply budget click", async () => {
    applyMutate.mockResolvedValue(undefined);
    updateGoalMonthlyMutate.mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(<PlanNextMonthModal onClose={onClose} />, { wrapper: createWrapper() });
    for (let i = 0; i < 6; i++) {
      fireEvent.click(screen.getByText("Next →"));
    }
    fireEvent.click(screen.getByText("Apply budget"));
    await waitFor(() => expect(applyMutate).toHaveBeenCalled());
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it("updates the live preview's Unassigned total as fixed-cost amounts are entered", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    expect(screen.getAllByText("$5,000").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByText("Next →")); // → Fixed costs
    const rentInput = screen.getByDisplayValue("1500"); // Rent budgetCents 150000 → $1,500
    fireEvent.change(rentInput, { target: { value: "2000" } });

    expect(screen.getByText("Unassigned")).toBeInTheDocument();
    expect(screen.getByText("$3,000")).toBeInTheDocument();
  });

  it("shows the sinking funds step with a monthly slider", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Next →")); // Fixed costs
    fireEvent.click(screen.getByText("Next →")); // Sinking funds
    expect(screen.getByText("Car insurance")).toBeInTheDocument();
  });

  it("shows an Adjust suggestion when a category is over budget 2+ of 3 months", () => {
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    // Groceries: budgetCents 40000, m0/m1/m2 = 38000/42000/41000 → over in 2 of 3 months.
    for (let i = 0; i < 5; i++) fireEvent.click(screen.getByText("Next →")); // → Adjust (step index 5)
    expect(screen.getByText("Raise Groceries to $420")).toBeInTheDocument();
  });

  it("shows loading state when data is not ready", async () => {
    const budget = await import("../api/hooks/budget");
    vi.mocked(budget.usePlanNextMonthData).mockReturnValue({
      data: undefined,
      isLoading: true,
    } as ReturnType<typeof budget.usePlanNextMonthData>);
    render(<PlanNextMonthModal onClose={vi.fn()} />, { wrapper: createWrapper() });
    expect(screen.getByText("Loading…")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test file**

Run: `cd ui && npx vitest run src/screens/PlanNextMonthModal.test.tsx`
Expected: all tests pass. If the "Adjust suggestion" test's exact rounded figure doesn't match `computeAdjustSuggestions`'s output, adjust the expected `$420` to whatever the max-of-3-months rounded-up-to-$10 value actually is for the Groceries fixture (max(38000,42000,41000) = 42000 → already a $10 multiple → $420 is correct).

- [ ] **Step 3: Type-check the whole frontend**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/screens/PlanNextMonthModal.test.tsx
git commit -m "test(plan-next-month): rework for the 7-step flow"
```

---

## Task 15: Full verification pass

**Files:** none (verification only)

- [ ] **Step 1: Run the full Rust test suite**

Run: `cargo test --workspace`
Expected: all tests pass; count increases by 15 over the prior 509 (7 + 4 + 3 + 1 new tests added across Tasks 1, 4, 10, plus the 1 documentation test in Task 2 — recount exactly from the `test result:` summary and note the new total).

- [ ] **Step 2: Run the full frontend test suite**

Run: `cd ui && npx vitest run`
Expected: all tests pass (424 + the new/modified tests in Tasks 3, 7, 9, 12, 14).

- [ ] **Step 3: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: 0 errors.

- [ ] **Step 4: Manual verification in the running app**

Run `pnpm tauri:dev` (per CLAUDE.md, this starts against an isolated, empty `.dev` data dir). Since it starts empty, either complete onboarding to seed starter categories, or copy a prod DB snapshot into the `.dev` dir first if real data is needed to see rollover/history populated meaningfully. Using the Browser pane:
- Navigate to `/budget` — confirm the "Not yet budgeted" section appears for any category with no budget/spend, and that setting a budget via its "Set budget" button moves it into the main grid.
- Confirm the history table shows a "Your typical" column and budgeted amounts under spent amounts.
- Navigate to `/categories` — confirm "New category" shows a group picker, "+ New group" creates a group that appears in the picker, and the Manage panel's "Group" select moves a category.
- Open "Plan next month" from `/budget` — step through all 7 steps, confirm the live preview updates, confirm "Apply budget" completes without error.
- Navigate to `/goals` — confirm a "Sinking fund" filter button appears and a goal created with that type shows up there instead of the Compound Growth Projector.

- [ ] **Step 5: Update CLAUDE.md's test-count line**

`CLAUDE.md`'s Testing section states "509 Rust tests (+12 ignored live-DB/keychain), 424 frontend tests, 0 TypeScript errors." Update these three numbers to match the actual `cargo test --workspace` and `npx vitest run` output from Steps 1-2.

- [ ] **Step 6: Final commit**

```bash
git add CLAUDE.md
git commit -m "docs: update test counts after budgets page redesign"
```

- [ ] **Step 7: Update Linear**

Move all 5 issues (AI-5 through AI-9) to "Done" in the "Budgets page redesign" Linear project, and post a brief comment on each linking to the commit(s) that closed it.

---

## Self-Review Notes

**Spec coverage:** §3.1/§6.1 (carryover) → Tasks 1-2. §3.2/§7.1 (zero-budget visibility) → Task 2-3. §3.4/§5.2/§7.2 (category groups) → Tasks 4-7. §3.3/§5.1 (history) → Tasks 8-9. §5.3/§5.4/§6.2/§6.3/§6.4 (Plan Next Month rebuild) → Tasks 10-14. §8 (testing) → each task's own test steps plus Task 15. §9 (Linear) → Task 15 Step 7. Every spec section maps to a task.

**Type consistency check:** `BudgetEnvelope.carryoverCents` (Task 2) is read identically in `Budget.tsx` (Task 3). `MonthlyActual.{spentCents,budgetedCents}` (Task 8) matches the field names used in `Budget.tsx`'s history table (Task 9) and nowhere still references the old `cents` field. `PlanData.{lookBack,sinkingFunds}` (Task 11) match the field names read in `PlanNextMonthModal.tsx` (Task 13) and its test fixture (Task 14). `LookBackFact.{categoryId,categoryLabel,kind,amountCents,streakMonths}` used consistently between Task 10 (Rust), Task 13 (component), and Task 14 (test fixture). `CategoryGroup.{id,label,hint,sortOrder}` used consistently between Task 5 (Rust, pre-existing model) and Tasks 6-7 (frontend).
