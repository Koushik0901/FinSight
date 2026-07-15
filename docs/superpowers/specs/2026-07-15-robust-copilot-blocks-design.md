# Robust Copilot Blocks — Design

**Date:** 2026-07-15
**Status:** Design — pending user review
**Depends on:** `2026-07-15-copilot-generative-ui-blocks-design.md` (the block system this hardens)

## 1. Problem

Model-emitted generative-UI blocks are fragile. Live-testing in the real app (gemma-4-31b-it) exposed three concrete weaknesses:

1. **No structure enforcement at generation time.** The final answer's JSON is enforced *only* by the system prompt (`openai_compat.rs` sends no `response_format`). A weak — or occasionally a strong — model emits JSON the schema rejects.
2. **Duplicated, drift-prone validation.** A Rust check (`valid_response_block` + artifact bounds) and a *separately hand-written* Zod schema (`CopilotResponseBlockSchema`) must agree. When they don't, a block that passes Rust fails the frontend and renders a generic fallback. This is the class of failure the gemma accounts run hit (exact field unconfirmed — see §4.3).
3. **One bad block drops them all.** `parse_response_blocks` deserializes the whole `Vec` at once — a single malformed element nukes every block.

Plus a related efficiency issue: heavy flows (3-month review) over-search and hit the reasoning time budget.

## 2. Approach & sequencing

Three layers, deliberately **unequal**:

- **(a) server-synthesis** and **(c) heal net** are the load-bearing, provider-independent reliability work.
- **(b) structured outputs** is narrow and *gated on a probe* — and largely **redundant with (a)**: for any block (a) synthesizes, the model no longer emits the data, so constrained decoding constrains nothing there. (b)'s value is only over blocks the model still emits in full.

**Build order: (c) → (a) → probe → (b) only if the probe passes.** This front-loads provider-independent, high-value work and makes the highest-risk piece last and optional. Keep the spec and each layer tight; do not let (b)'s schema apparatus grow before the probe says it is usable.

## 3. Layer (c): heal net — *first*

Provider-independent, cheapest, highest immediate value. Per-element parse alone would have let gemma's other blocks survive.

### 3.1 Per-element tolerant parse
`parse_response_blocks` (`agent.rs`) currently does `from_value::<Vec<AgentResponseBlock>>(...).ok()` — all-or-nothing. Change to iterate the array and parse **each element independently**, keeping the ones that succeed:
```rust
raw.get("response_blocks").or_else(|| raw.get("responseBlocks"))
   .and_then(Value::as_array)
   .map(|arr| arr.iter().filter_map(|v| coerce_and_parse_block(v)).filter(valid_response_block).take(8).collect())
   .unwrap_or_default()
```

### 3.2 Conservative, logged coercion
`coerce_and_parse_block(&Value)` normalizes only **safe, unambiguous** issues before typed deserialize, logging each coercion:
- string→int for numeric fields that arrive quoted (e.g. `"amountCents":"185000"` → `185000`);
- fill a missing *optional* field with `null`.

**No speculative field-name aliasing in v1** (advisor: guessing intent renders wrong data). serde already ignores unknown fields. Every coercion is logged so drift is observable.

### 3.3 Rust/Zod parity test (find the *real* drift)
We never confirmed which field failed gemma's block — "malformed" was an inference. Instead of guessing, build a **parity test** over a shared fixture corpus so drift is found systematically:
- `crates/finsight-app/tests/fixtures/response_blocks.json`: an array of `{ block, expectValid }` cases — at least one valid + one invalid per kind, plus any real captured failing block.
- **Rust test** loads the file, runs each block through `valid_response_block` + `response_block_within_artifact_bounds`, asserts the verdict equals `expectValid`.
- **vitest test** loads the *same* file, runs each block through `CopilotResponseBlockSchema.safeParse`, asserts the verdict equals `expectValid`.
- Any Rust/Zod disagreement fails one side → the drift is pinpointed and fixed at the field level.

## 4. Layer (a): server-synthesis — *second*

The model emits a **thin block request**; the server hydrates the data-bearing fields from the **same core** that grounds the prose snapshot, so a block's numbers can never diverge from the answer's numbers.

### 4.1 Single hydration funnel (the crux)
`AgentResponseBlock` lives in `finsight-app`; the reasoning engine (`finsight-agent`) can't reference it, and `reasoning_result_to_agent_answer` — the one function all 4 model-block call sites funnel through — has **no `conn`** today. Fix: **add `conn: &mut rusqlite::Connection` to `reasoning_result_to_agent_answer`** and hydrate inside it. The compiler then forces every caller (stream, stream-fallback, deep-answer, ask_agent) to supply `conn` → **no path can skip hydration** (kills the "one path renders, another doesn't" drift-bug class we already hit). Callers wrap the call in the existing `run(&db, move |conn| …)` pattern.

Fallback paths (`planner_answer_to_agent_answer`) don't emit these blocks and are unaffected.

### 4.2 Synthesizer registry (opt-in per kind)
`hydrate_response_blocks(conn, &mut Vec<AgentResponseBlock>)`: for each block whose `kind` is in the registry, **rebuild its data-bearing fields from core**; kinds not in the registry pass through unchanged (existing model-emitted behavior). v1 registers exactly two:

**`accountsOverview` (pure data):**
- Model emits: `{kind:"accountsOverview"}` (may include nothing else).
- Server derives from `accounts::list_summaries` + `metrics::balance_breakdown`: `rows` (name, subtitle, typeLabel, amountCents-or-null, badge for unknown balance), `title` ("N accounts"), `subtitle` ("$X tracked · M missing a balance"). All model-supplied data fields are ignored/overwritten.

**`spendingReview` (hybrid — explicit per-field split):**

| Field | Source |
|---|---|
| `months[].period` ("YYYY-MM") | model (join key) |
| `months[].summary` | model (generative insight) |
| `months[].actions[]` | model (generative) |
| `months[].label` ("May 2026") | **server** (from period) |
| `months[].spentCents` | **server** (spending core for that period) |
| `months[].subtitle` ("N of M envelopes under") | **server** (budget stats) |
| `months[].categories[]` (label, amountCents) | **server** (top categories for period) |
| category `tag`: `over` (budget breach), `fixed` (spending_type) | **server** (derived from data) |
| category `tag`: `lever` | model hint, optional |

Server drops a requested month with no data. Numbers come from the same spending/metrics core the snapshot uses (`memory/project_metrics_layer`) — block totals == prose totals by construction.

### 4.3 Prompt changes
Tell the model these two kinds are **server-rendered**: emit only the `kind` (+ `period`/`summary`/`actions` for a review), never the numbers. And: for a review, `get_spending_breakdown(months:N)` returns everything needed — **do not** search transactions per month (fixes the over-searching that blew the time budget).

## 5. Layer (b): structured outputs — *gated, last*

### 5.1 Phase-0 probe (go/kill gate — run before any (b) apparatus)
One real `chat/completions` call to **glm-5.2 and gemma-4-31b-it** with `response_format:{type:"json_schema", strict:true, json_schema:{…answer envelope…}}` **and the `tools` array present**, mirroring the actual final-answer turn. Record for each: request accepted (no 4xx)? output conforms to schema? do tools + response_format coexist? Outcomes:
- **PASS** → proceed (§5.2).
- **FLAKY/partial** → apply only to known-supporting presets (e.g. `openai`, `google`); fallback for the rest.
- **FAIL** → drop (b); (a)+(c) carry reliability. Document and stop.

OpenRouter structured-output support is per-model and inconsistent, and `tools` + `response_format` coexistence is a known landmine — hence the gate.

### 5.2 If the probe passes
- Add `schemars` derive to the answer-envelope + block structs and **generate** the JSON schema (never hand-maintain 19 variants — that reintroduces the drift we're fighting).
- Pass it as `response_format` on the **final-answer turn only** (`complete_final_answer_turn*`), with try-then-fallback: on a 4xx/format error, retry the same turn without `response_format`.
- Gate per provider capability (config flag or preset allowlist).
- **Redundancy note:** this constrains only blocks the model still emits in full — i.e. *not* `accountsOverview`/`spendingReview` (synthesized by (a)). Its value is the remaining model-emitted kinds.

If (b) lands, the generated schema can later become the single source for the frontend validator too (retiring hand-written Zod) — but that unification is **deferred**, not part of this project.

## 6. Non-goals / deferred
- Full Rust/Zod schema unification (parity test covers drift meanwhile; unification falls out of (b) if it lands).
- Server-synthesizing the other four blocks (`spendTimeline`/`spendingDrivers`/`watchList`/`actionPlan`) — the registry pattern extends; v1 proves it on the two marquee kinds.
- A model repair round-trip (resend the validation error) — per-element parse + coercion + synthesis cover the common cases without the extra latency.

## 7. Acceptance criteria
- **(c)** A `response_blocks` array with one malformed + one valid block renders the valid one (Rust test). Parity test green (Rust ⇔ Zod verdicts identical over the fixture corpus). Coercions logged.
- **(a)** `accountsOverview` + `spendingReview` render in the live app on **both gemma and glm-5.2** from a thin model request — the gemma accounts flow that motivated this now renders a real card. Block numbers equal the snapshot numbers. The review no longer over-searches.
- **(b)** Probe result documented. If passed: malformed model-emitted blocks eliminated for supporting models, with clean fallback otherwise.
- Full green bar (Rust + vitest + `tsc`); bindings regenerated and committed.
