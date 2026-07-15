# Robust Copilot Blocks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Copilot generative-UI blocks robust to weak/inconsistent model output via three layers — (c) a heal/tolerant-parse net, (a) server-side synthesis of the marquee blocks from grounded core data, and (b) probe-gated structured outputs.

**Architecture:** Build order **(c) → (a) → probe → (b)**. (c) and (a) are provider-independent and load-bearing; (b) is gated on a real probe and largely redundant with (a). Server-synthesis hydrates data-bearing fields at a single `conn`-carrying funnel (`reasoning_result_to_agent_answer`) so no path can skip it.

**Tech Stack:** Rust (serde_json, rusqlite, specta), TypeScript, Zod, vitest, OpenAI-compatible provider (OpenRouter).

**Reference spec:** `docs/superpowers/specs/2026-07-15-robust-copilot-blocks-design.md`

**Conventions:** Rust tests `cargo test -p finsight-app --lib commands::agent`; frontend `cd ui && npx vitest run <file>`; type-check `cd ui && npx tsc --noEmit`; after any `AgentResponseBlock` change run `cargo run -p finsight-tauri --bin export_bindings`. Commit after each task.

---

## Phase C — Heal net (provider-independent, first)

### Task C1: Per-element tolerant parse + conservative coercion

**Files:** Modify `crates/finsight-app/src/commands/agent.rs` (`parse_response_blocks` + new `coerce_block_value`; tests).

- [ ] **Step 1: Failing test** (agent.rs tests module)

```rust
#[test]
fn parse_response_blocks_keeps_valid_block_despite_a_malformed_neighbor() {
    // One valid table + one structurally-broken block (rows not matching columns).
    let raw = serde_json::json!({ "response_blocks": [
        { "kind": "table", "title": "Ok", "columns": ["A","B"], "rows": [["1","2"]] },
        { "kind": "table", "title": "Bad", "columns": ["A","B"], "rows": [["only-one"]] },
    ]});
    let blocks = parse_response_blocks(&raw);
    assert_eq!(blocks.len(), 1, "the valid block must survive its malformed neighbor");
    assert!(matches!(blocks[0], AgentResponseBlock::Table(_)));
}

#[test]
fn parse_response_blocks_coerces_quoted_integer_amounts() {
    let raw = serde_json::json!({ "response_blocks": [
        { "kind": "accountsOverview", "title": "1 account", "subtitle": null,
          "rows": [{ "name":"Chq", "subtitle":null, "typeLabel":"Checking",
                     "amountCents":"14820", "badge":null }] }
    ]});
    let blocks = parse_response_blocks(&raw);
    assert_eq!(blocks.len(), 1, "a quoted integer amount must be coerced, not dropped");
}
```

- [ ] **Step 2: Run to verify fail**

Run: `cargo test -p finsight-app --lib commands::agent::tests::parse_response_blocks_keeps 2>&1 | tail -12`
Expected: FAIL (current all-or-nothing parse drops both; coercion absent).

- [ ] **Step 3: Rewrite `parse_response_blocks` + add coercion**

```rust
pub(crate) fn parse_response_blocks(raw: &serde_json::Value) -> Vec<AgentResponseBlock> {
    raw.get("response_blocks")
        .or_else(|| raw.get("responseBlocks"))
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| {
                    let coerced = coerce_block_value(v.clone());
                    match serde_json::from_value::<AgentResponseBlock>(coerced) {
                        Ok(b) => Some(b),
                        Err(e) => {
                            eprintln!("copilot: dropping unparseable response block: {e}");
                            None
                        }
                    }
                })
                .filter(valid_response_block)
                .take(8)
                .collect()
        })
        .unwrap_or_default()
}

/// Conservative, logged coercion for common model foibles. Only safe,
/// unambiguous fixes: quoted integers -> integers on any key ending in "Cents".
/// No field-name aliasing (guessing intent renders wrong data). serde already
/// ignores unknown fields and treats missing Options as None.
fn coerce_block_value(mut v: serde_json::Value) -> serde_json::Value {
    fn walk(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Object(map) => {
                for (k, child) in map.iter_mut() {
                    if k.ends_with("Cents") {
                        if let serde_json::Value::String(s) = child {
                            if let Ok(n) = s.parse::<i64>() {
                                eprintln!("copilot: coerced {k} \"{s}\" -> {n}");
                                *child = serde_json::Value::from(n);
                                continue;
                            }
                        }
                    }
                    walk(child);
                }
            }
            serde_json::Value::Array(arr) => arr.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut v);
    v
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p finsight-app --lib commands::agent::tests::parse_response_blocks`
Expected: PASS (both new tests + existing parse tests).

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs
git commit -m "fix(copilot): per-element tolerant parse + quoted-cents coercion for response blocks"
```

### Task C2: Rust ⇔ Zod parity fixture + tests

**Files:** Create `crates/finsight-app/tests/fixtures/response_blocks.json`; create `crates/finsight-app/tests/response_block_parity.rs`; create `ui/src/components/copilot/agUi/parity.test.ts`.

- [ ] **Step 1: Create the shared fixture corpus**

```json
[
  { "expectValid": true,  "block": { "kind": "accountsOverview", "title": "2 accounts", "subtitle": "$1 tracked", "rows": [{ "name": "Chq", "subtitle": null, "typeLabel": "Checking", "amountCents": 100, "badge": null }] } },
  { "expectValid": false, "block": { "kind": "accountsOverview", "title": null, "subtitle": null, "rows": [] } },
  { "expectValid": true,  "block": { "kind": "spendingReview", "months": [{ "label": "May 2026", "spentCents": 100, "subtitle": null, "categories": [{ "label": "Housing", "amountCents": 50, "tag": "fixed" }], "summary": null, "actions": [] }] } },
  { "expectValid": false, "block": { "kind": "spendingReview", "months": [{ "label": "May", "spentCents": 1, "subtitle": null, "categories": [{ "label": "X", "amountCents": 1, "tag": "bogus" }], "summary": null, "actions": [] }] } },
  { "expectValid": true,  "block": { "kind": "watchList", "title": "W", "items": [{ "label": "A", "detail": "d", "amountDisplay": null }] } },
  { "expectValid": false, "block": { "kind": "watchList", "title": "W", "items": [] } }
]
```

- [ ] **Step 2: Rust parity test** (`tests/response_block_parity.rs`)

```rust
//! Parity guard: the Rust validation (valid_response_block + artifact bounds)
//! must agree with the frontend Zod schema over the shared fixture corpus.
//! The Rust side is checked here; the TS side in ui/.../parity.test.ts loads the
//! SAME file. A disagreement pinpoints the exact drift.
use serde_json::Value;

#[test]
fn rust_verdicts_match_the_fixture_corpus() {
    let raw = std::fs::read_to_string("tests/fixtures/response_blocks.json").unwrap();
    let cases: Vec<Value> = serde_json::from_str(&raw).unwrap();
    for case in cases {
        let expect = case["expectValid"].as_bool().unwrap();
        let wrapped = serde_json::json!({ "response_blocks": [ case["block"].clone() ] });
        // parse_response_blocks applies typed parse + valid_response_block; a valid
        // block yields exactly 1, an invalid one yields 0.
        let n = finsight_app::commands::agent::parse_response_blocks_for_test(&wrapped).len();
        assert_eq!(n == 1, expect, "Rust verdict mismatch for {}", case["block"]["kind"]);
    }
}
```
(Add `pub fn parse_response_blocks_for_test(v:&Value)->Vec<AgentResponseBlock>{parse_response_blocks(v)}` re-export, or make `parse_response_blocks` reachable; simplest: `#[cfg(test)]`-free `pub` on the module path used by the integration test.)

- [ ] **Step 3: vitest parity test** (`ui/src/components/copilot/agUi/parity.test.ts`)

```ts
import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { CopilotResponseBlockSchema } from "./artifacts";

const cases = JSON.parse(
  readFileSync(resolve(__dirname, "../../../../../crates/finsight-app/tests/fixtures/response_blocks.json"), "utf8"),
) as { expectValid: boolean; block: unknown }[];

describe("Rust/Zod parity corpus", () => {
  it.each(cases)("Zod verdict matches expectValid for $block.kind", ({ expectValid, block }) => {
    expect(CopilotResponseBlockSchema.safeParse(block).success).toBe(expectValid);
  });
});
```

- [ ] **Step 4: Run both**

Run: `cargo test -p finsight-app --test response_block_parity` and `cd ui && npx vitest run src/components/copilot/agUi/parity.test.ts`
Expected: BOTH PASS. If a case disagrees, fix the drifting bound (Rust or Zod) until identical, then re-run.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/tests/fixtures/response_blocks.json crates/finsight-app/tests/response_block_parity.rs ui/src/components/copilot/agUi/parity.test.ts crates/finsight-app/src/commands/agent.rs
git commit -m "test(copilot): Rust<->Zod parity corpus for response-block validation"
```

---

## Phase A — Server-synthesis (single conn funnel)

### Task A1: Thread `conn` through the single hydration funnel (no-op registry)

**Files:** Modify `crates/finsight-app/src/commands/agent.rs` (`reasoning_result_to_agent_answer` signature + `hydrate_response_blocks` scaffold); modify all 4 call sites in `copilot_chat.rs` (534, 579, 1203) and `agent.rs` (1817, 1842) to pass `conn` via `run(&db, …)`.

- [ ] **Step 1: Add the hydration scaffold + conn param**

In `agent.rs`, change the signature and add hydration (registry empty for now = pure passthrough, so behavior is unchanged):
```rust
pub(crate) fn reasoning_result_to_agent_answer(
    result: ReasoningResult,
    bundle_id: Option<String>,
    conn: &mut rusqlite::Connection,
) -> AgentAnswer {
    // …existing body…
    let mut response_blocks = parse_response_blocks(&serde_json::json!({
        "response_blocks": result.response_blocks,
    }));
    hydrate_response_blocks(conn, &mut response_blocks); // single funnel
    // …build AgentAnswer with response_blocks…
}

/// The ONE place model-requested marquee blocks get their data-bearing fields
/// rebuilt from grounded core data. Kinds not registered here pass through
/// untouched. Every answer path funnels through reasoning_result_to_agent_answer,
/// which now requires `conn`, so no path can skip this.
fn hydrate_response_blocks(_conn: &mut rusqlite::Connection, _blocks: &mut [AgentResponseBlock]) {
    // Registry filled in A2/A3.
}
```

- [ ] **Step 2: Update all 4 call sites** to run inside a `conn` closure. Pattern (copilot_chat.rs:534, inside the existing bundle `run` closure or a new one):
```rust
let mut answer = run(&db, {
    let result = result; let bundle_id = bundle_id.clone();
    move |conn| Ok::<_, finsight_core::CoreError>(reasoning_result_to_agent_answer(result, bundle_id, conn))
}).await.map_err(AppError::from)?;
```
Apply the equivalent at copilot_chat.rs:579, copilot_chat.rs:1203 (deep answer), agent.rs:1817, agent.rs:1842. Update the test call sites (agent.rs:2209 etc.) to pass a test `conn`.

- [ ] **Step 3: Build + existing tests green**

Run: `cargo test -p finsight-app --lib commands::agent`
Expected: compiles; all existing tests pass (hydration is a no-op).

- [ ] **Step 4: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs crates/finsight-app/src/commands/copilot_chat.rs
git commit -m "refactor(copilot): single conn-carrying hydration funnel for response blocks"
```

### Task A2: `accountsOverview` synthesizer (pure data)

**Files:** Modify `agent.rs` (`hydrate_response_blocks` + `synthesize_accounts_overview` + `type_label`; tests).

- [ ] **Step 1: Failing test**

```rust
#[test]
fn accounts_overview_is_hydrated_from_core_ignoring_model_data() {
    let (_dir, db) = fresh_db_with_two_accounts(); // one known balance, one unknown
    let mut conn = db.get().unwrap();
    // Model emits a THIN request (bogus rows the server must ignore).
    let mut blocks = vec![AgentResponseBlock::AccountsOverview(AgentAccountsOverviewBlock {
        title: None, subtitle: None,
        rows: vec![AgentAccountRow { name: "HALLUCINATED".into(), subtitle: None, type_label: "?".into(), amount_cents: Some(999), badge: None }],
    })];
    hydrate_response_blocks(&mut conn, &mut blocks);
    let AgentResponseBlock::AccountsOverview(b) = &blocks[0] else { panic!() };
    assert!(b.rows.iter().all(|r| r.name != "HALLUCINATED"), "model rows replaced by core data");
    assert!(b.rows.iter().any(|r| r.amount_cents.is_none() && r.badge.is_some()), "unknown-balance account gets a badge");
    assert!(b.title.as_deref().unwrap().contains("account"));
}
```
(Add a `fresh_db_with_two_accounts` helper inserting one `balance_known` account and one unknown-balance account.)

- [ ] **Step 2: Run to verify fail** — Expected: FAIL (rows still HALLUCINATED; no synthesizer).

- [ ] **Step 3: Implement synthesizer + registry arm**

```rust
fn hydrate_response_blocks(conn: &mut rusqlite::Connection, blocks: &mut [AgentResponseBlock]) {
    for block in blocks.iter_mut() {
        if let AgentResponseBlock::AccountsOverview(_) = block {
            if let Ok(fresh) = synthesize_accounts_overview(conn) {
                *block = AgentResponseBlock::AccountsOverview(fresh);
            }
        }
    }
}

fn type_label(t: finsight_core::models::AccountType) -> String {
    use finsight_core::models::AccountType::*;
    match t { Checking => "Checking", Savings => "Savings", Credit => "Credit",
              Investment => "Investment", _ => "Account" }.to_string()
}

fn synthesize_accounts_overview(
    conn: &mut rusqlite::Connection,
) -> finsight_core::error::CoreResult<AgentAccountsOverviewBlock> {
    let summaries = finsight_core::repos::accounts::list_summaries(conn)?;
    let bal = finsight_core::metrics::balance_breakdown(conn).ok();
    let missing = summaries.iter().filter(|s| !s.balance_known).count();
    let rows = summaries.iter().map(|s| AgentAccountRow {
        name: s.name.clone(),
        subtitle: Some(s.bank.clone()),
        type_label: type_label(s.r#type),
        amount_cents: if s.balance_known { Some(s.balance_cents) } else { None },
        badge: if s.balance_known { None } else { Some("needs a balance set".to_string()) },
    }).collect();
    let total = bal.map(|b| format!("${} tracked", b.net_worth_cents / 100)).unwrap_or_default();
    Some(()).map(|_| AgentAccountsOverviewBlock {
        title: Some(format!("{} account{}", summaries.len(), if summaries.len()==1 {""} else {"s"})),
        subtitle: Some(if missing > 0 { format!("{total} · {missing} missing a balance") } else { total }),
        rows,
    }).ok_or(finsight_core::error::CoreError::InvalidState("unreachable".into()))
}
```
(Adjust `balance_breakdown` field name to the real total; grep `BalanceBreakdown` for the net-worth/tracked field.)

- [ ] **Step 4: Run to verify pass** — `cargo test -p finsight-app --lib commands::agent::tests::accounts_overview_is_hydrated` → PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-app/src/commands/agent.rs
git commit -m "feat(copilot): synthesize accountsOverview from core (grounded, always valid)"
```

### Task A3: `spendingReview` synthesizer (hybrid)

**Files:** Create `crates/finsight-core/src/spending_review.rs` (extract per-month category breakdown from the logic in `crates/finsight-agent/src/reasoning/tools/read.rs:144 get_spending_breakdown`, returning `Vec<(period, spent_cents, Vec<(category, amount_cents, is_fixed, over_budget)>)>`); modify `agent.rs` (`synthesize_spending_review` merging model prose with core data by `period`; tests).

- [ ] **Step 1: Extract the core helper** `finsight_core::spending_review::per_month_breakdown(conn, months: u32)` from the existing tool query so both the tool and the synthesizer share one grounded source. Add a Rust test that it returns per-month category totals on the seeded DB (mirror `read.rs:1210 spending_breakdown_reports_categories`).

- [ ] **Step 2: Failing synthesizer test**

```rust
#[test]
fn spending_review_merges_model_prose_with_core_numbers_by_period() {
    let (_dir, db) = fresh_db_with_spending(); // seeded transactions across 2 months
    let mut conn = db.get().unwrap();
    let mut blocks = vec![AgentResponseBlock::SpendingReview(AgentSpendingReviewBlock {
        months: vec![AgentReviewMonth {
            label: "IGNORED".into(), spent_cents: 999, subtitle: Some("IGNORED".into()),
            categories: vec![], summary: Some("My insight".into()),
            actions: vec!["Do X".into()],
            // period carried in a model-only field — see step 3 for the join key.
        }],
    })];
    hydrate_response_blocks(&mut conn, &mut blocks);
    let AgentResponseBlock::SpendingReview(b) = &blocks[0] else { panic!() };
    let m = &b.months[0];
    assert_eq!(m.summary.as_deref(), Some("My insight"), "model prose preserved");
    assert!(m.spent_cents != 999 && !m.categories.is_empty(), "numbers replaced from core");
    assert_ne!(m.label, "IGNORED", "label derived from period");
}
```

- [ ] **Step 3: Add the `period` join key + implement.** Add `pub period: Option<String>` (serde camelCase `period`) to `AgentReviewMonth` (model supplies "YYYY-MM"; server keys on it). In `synthesize_spending_review(conn, model_block)`: for each model month, look up that period's core breakdown; overwrite `label` (from period), `spent_cents`, `subtitle` (budget stats), `categories` (label/amountCents + `over`/`fixed` tags derived from budget + spending_type; keep the model's `lever` hint if present); preserve `summary`, `actions`. Drop months with no core data. Register the `SpendingReview` arm in `hydrate_response_blocks`. Regenerate bindings (new `period` field).

- [ ] **Step 4:** `cargo test -p finsight-app --lib commands::agent::tests::spending_review_merges` → PASS. `cargo run -p finsight-tauri --bin export_bindings`. Add `period` to the Zod `spendingReview.months` object (nullable) in `artifacts.ts`.

- [ ] **Step 5: Commit**

```bash
git add crates/finsight-core/src/spending_review.rs crates/finsight-core/src/lib.rs crates/finsight-app/src/commands/agent.rs ui/src/api/bindings.ts ui/src/components/copilot/agUi/artifacts.ts
git commit -m "feat(copilot): synthesize spendingReview numbers from core, keep model prose by period"
```

### Task A4: Prompt — server-rendered kinds + no over-searching

**Files:** Modify `crates/finsight-agent/src/reasoning/engine/mod.rs` (`build_system_prompt`).

- [ ] **Step 1: Edit the accountsOverview + spendingReview usage lines** to say these are server-rendered: emit ONLY the `kind` for accountsOverview; for spendingReview emit only `period`, `summary`, `actions` per month (never the numbers). Append to the spendingReview guidance: `get_spending_breakdown(months:N) already returns per-month category totals — do NOT call search_transactions per month.`

- [ ] **Step 2:** `cargo test -p finsight-agent --lib reasoning::engine` → PASS (compile check).

- [ ] **Step 3: Commit** `feat(copilot): tell the model accountsOverview/spendingReview are server-rendered`.

### Task A5: Live + eval verification

- [ ] **Step 1: Eval subset** — `cargo run -p finsight-eval -- --benchmark eval/subset_genui.jsonl --out eval/runs/robust.jsonl`; confirm the accounts + review flows still emit the kinds AND that the review no longer emits 5 `search_transactions` (fewer tools in the trace).
- [ ] **Step 2: Live app** — with the running Tauri app, run the accounts question on **gemma** and confirm the `accountsOverview` card now renders (the failure that motivated this). Screenshot.

---

## Phase B — Structured outputs (probe-gated, last)

### Task B1: PROBE (go/kill gate) — DO THIS BEFORE ANY (b) APPARATUS

**Files:** Create `crates/finsight-eval/src/bin/probe_structured.rs` (or a one-off script) that reads the OpenRouter key (env → keychain, like `finsight-eval`) and issues one `chat/completions` per model with `response_format:{type:"json_schema",strict:true,json_schema:{name:"answer",schema:{…minimal envelope with a response_blocks array…}}}` AND a non-empty `tools` array, mirroring the final-answer turn.

- [ ] **Step 1:** Probe `z-ai/glm-5.2:exacto` and `google/gemma-4-31b-it`. Record per model: HTTP status (accepted?), whether the returned content parses as schema-conforming JSON, and whether tools + response_format coexisted (no 4xx). Write the verdict into this plan / a short `docs/…` note.
- [ ] **Step 2: GATE.**
  - **PASS** → proceed to B2.
  - **FLAKY** → B2 but restrict to a preset allowlist (e.g. `openai`, `google`), fallback elsewhere.
  - **FAIL** → **STOP. Skip B2.** (a)+(c) carry reliability; record the decision and finish at Phase D.

### Task B2 (only if B1 passes): schemars-generated schema + response_format

**Files:** Add `schemars` to `crates/finsight-app` (+ derive on the answer-envelope + block structs); new schema-export path; modify `crates/finsight-agent/src/providers/openai_compat.rs` (`complete_final_answer_turn*` to send `response_format` with try-then-fallback); a capability flag on `OpenAiCompatProvider`.

- [ ] **Step 1: Failing provider test** — a unit test asserting the final-answer request body includes `response_format.json_schema` when the capability flag is on, and omits it when off. (Refactor the body-building into a testable fn.)
- [ ] **Step 2: Implement** — derive `schemars::JsonSchema` on the block/answer structs (note: they live in finsight-app; the schema must be reachable by the provider in finsight-agent — pass the schema JSON *value* down from finsight-app into the provider config, since finsight-agent can't depend on finsight-app types). Add `response_format` to the final-answer body behind the flag; on a 4xx/format error, retry once without it.
- [ ] **Step 3:** Wire the capability flag from provider config (per §5.1 gate outcome). Tests green.
- [ ] **Step 4: Commit** `feat(copilot): structured-output json_schema on the final-answer turn (gated + fallback)`.

---

## Phase D — Green bar

- [ ] **Step 1:** `cargo test -p finsight-app -p finsight-agent -p finsight-core`; `cd ui && npx vitest run && npx tsc --noEmit`. Fix any failures.
- [ ] **Step 2:** `cargo run -p finsight-tauri --bin export_bindings && git diff --stat ui/src/api/bindings.ts` (no unstaged drift).
- [ ] **Step 3: Commit** any final fixes.

---

## Self-review notes
- **Spec coverage:** (c)→C1(per-element+coercion)+C2(parity); (a)→A1(funnel)+A2(accountsOverview)+A3(spendingReview)+A4(prompt)+A5(verify); (b)→B1(probe gate)+B2(schemars, conditional); grounding via core in A2/A3; over-searching fix in A4.
- **Type consistency:** `AgentAccountRow`/`AgentAccountsOverviewBlock`/`AgentReviewMonth` (+ new `period`) match the existing bindings; `type_label` maps `AccountType`.
- **Risk gates:** A1 keeps hydration a no-op so the refactor is verified independently; B1 is a hard gate before any schema apparatus.
- **Open detail to resolve during impl:** exact `BalanceBreakdown` total field (grep before A2 step 3); the `over`/`fixed` tag derivation source in A3 (budget breach + `spending_type`).
