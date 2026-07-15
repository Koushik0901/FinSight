# Copilot Generative-UI Blocks — Design

**Date:** 2026-07-15
**Status:** Design — pending user review
**Owner:** Copilot / finsight-agent + ui

## 1. Goal & honest framing

Make Copilot answers render as polished, native FinSight product surfaces — spending-review month cards, category bars, summary boxes, action-plan checklists, an accounts overview, tagged trend/driver breakdowns, a highlighted spend timeline, and watch-out lists — instead of chatbot prose. Target visual direction: the three attached "Plutus" screenshots (dark-first, elegant bordered cards, compact stat headers, category rows with visual weight, clear next actions).

**Framing correction (important):** the Copilot is **not markdown-only today**. A validated, typed generative-UI system already ships — a `AgentResponseBlock` Rust union of **13 kinds** (markdown, table, barChart, lineChart, metricGrid, callout, transactionTable, affordabilityVerdict, categoryBreakdown, allocationSplit, rankedOptions, comparisonBars, recategorizationPreview) that the model emits via `response_blocks`, which is validated server-side, streamed as a `FinSightResponseBlock` artifact, re-validated by a Zod schema, and rendered by native React cards. The safety/validation machinery the goal asks for **already exists and works**.

So this project is **extend the existing union**, not build a new system. The gap is that today's blocks are *flat and granular* and can't express the *composite* surfaces the screenshots show (a month card = header stats + category bars + summary + action plan, as one cohesive unit). We add a small set of new, richer block kinds that map 1:1 to the screenshot surfaces, reusing the entire existing validate→stream→revalidate→render pipeline.

## 2. Existing pipeline (what we build on)

Model → screen, every hop already in place (see `memory/project_copilot_genui_pipeline.md`):

1. **Prompt contract** — `crates/finsight-agent/src/reasoning/engine/mod.rs::build_system_prompt` enumerates allowed block kinds + inline JSON schema + when-to-use guidance. This is the emission lever.
2. **Server parse/validate** — `crates/finsight-app/src/commands/agent.rs`: `AgentResponseBlock` enum (source of truth, `#[serde(tag="kind")]`) → `parse_response_blocks` (deserialize + `valid_response_block` filter + **`take(8)`** cap) → `enrich_agent_answer` (fallback synthesis: markdown/callout/table only).
3. **Server stream/bounds** — `crates/finsight-app/src/commands/copilot_chat.rs`: `should_emit_response_block` + `response_block_within_artifact_bounds` (size caps) → emits a `ResponseBlock` frame + a `generative-ui` part wrapping a `FinSightResponseBlock` artifact envelope.
4. **Bindings** — `CopilotResponseBlock` TS type auto-generated in `ui/src/api/bindings.ts` from the Rust enum via specta (`cargo run -p finsight-tauri --bin export_bindings`).
5. **Frontend re-validate** — `ui/src/components/copilot/agUi/artifacts.ts`: Zod `CopilotResponseBlockSchema` (discriminated union mirroring Rust bounds) + `COMPONENT_PROP_SCHEMAS` allowlist + 24 KB byte cap. Unknown/oversized rejected.
6. **Render** — `ui/src/components/copilot/renderers.tsx`: `FinSightResponseBlock` switch → cards in `ui/src/components/copilot/cards/`, reusing `cards/shared.tsx` (`SegmentBar`), `colorForCategoryLabel`, `money()`, tokens, and `.cp-*` classes in `ui/src/styles/copilot-shell.css`. Unknown kind → fallback callout.

## 3. Design principles

- **Extend, don't replace.** Every new block flows through the exact pipeline above. No new transport, no new validation framework.
- **Composite when repetition would blow the `take(8)` cap or sub-parts are meaningless alone; primitive otherwise.** A 3-month review is 3×(stats+bars+summary+plan) ≈ 12 elements — must be **one composite block with a `months[]` array** (one intro, cap-safe). A spend timeline or watch-list is useful alone → primitive block.
- **Flat schema, reuse at the React layer.** No block-union nesting (avoids Zod/specta recursion). Composites and standalone primitives render from the *same* shared React sub-components — the `SegmentBar` pattern extended with `StatStrip`, `ActionChecklist`, `TagPill`.
- **Presentational in v1; mutations stay on the existing rails.** New blocks emit no side effects. CTAs like "Set the Vanguard balance" ride the existing `followUpQuestions` chips + single `actionLabel`/`actionPath` deep-link; anything that writes data still goes through the draft→approve→execute bundle flow (`ActionBundlePanel`). Checklist checkboxes are local/decorative toggles, no persistence.
- **Money = integer cents (i64), formatted by the frontend `money()` util** — same as every existing block. The only exception: presentational delta strings ("+$213/mo", "−$50/mo") carried as bounded short strings (`amountDisplay`), matching how `affordabilityVerdict.sub` / `rankedOption.detail` already carry pre-formatted text.
- **Security invariants preserved (explicit):** the model can still only emit allowlisted flat kinds; every new branch is bounded on both Rust and Zod sides; no HTML/JS/component-names/arbitrary props/unvalidated URLs; unknown → fallback. New kinds inherit these guarantees by construction.
- **Reuse tokens/classes, never hardcode colors.** New CSS uses `var(--ink)`, `var(--line)`, `var(--accent)`, `var(--negative)`, category colors via `colorForCategoryLabel`, etc.

## 4. New block kinds (v1 = exactly the three screenshots)

All fields camelCase (serde `rename_all="camelCase"`). `cents` fields are i64. Bounds shown are enforced identically in Rust `valid_response_block` + `response_block_within_artifact_bounds` and in Zod.

### 4.1 `spendingReview` — composite month cards (Screenshot 1, marquee)
```jsonc
{ "kind": "spendingReview",
  "months": [                                  // 1..6
    { "label": "May 2026",                     // shortString
      "spentCents": 408600,
      "subtitle": "8 of 10 envelopes under",   // shortString, free text
      "categories": [                          // 1..10
        { "label": "Housing", "amountCents": 185000, "tag": "fixed" } ],
      // tag?: "over" | "fixed" | "lever" (enum, nullable)
      "summary": "A steady month…",            // ≤ MAX_TEXT, nullable
      "actions": [ "Glance at the PG&E bill…" ]  // 0..6 shortString items
    } ] }
```
Renders one bordered card per month: title + stat strip (`spentCents` + `subtitle`), `SegmentBar` category rows (color via `colorForCategoryLabel`, `over` tag = negative-toned pill), a summary sub-box, and an `ActionChecklist` "Action plan" section. The intro paragraph stays in the answer prose above.

### 4.2 `accountsOverview` — accounts table with badges (Screenshot 2)
```jsonc
{ "kind": "accountsOverview",
  "title": "7 accounts",                        // shortString, nullable
  "subtitle": "$137,515 tracked · 1 missing a balance",  // shortString, nullable
  "rows": [                                     // 1..30
    { "name": "Joint Checking", "subtitle": "Mercury ····4421",
      "typeLabel": "Checking",                  // shortString
      "amountCents": 1482042,                   // nullable → renders the badge instead
      "badge": null } ,                         // shortString, nullable ("needs a balance set")
    { "name": "Vanguard Brokerage", "subtitle": "manual · added Mar 2026",
      "typeLabel": "Investment", "amountCents": null, "badge": "needs a balance set" } ] }
```
Row balance right-aligned, `var(--negative)` when `amountCents < 0`; when `amountCents` is null the `badge` pill shows. Type label is a neutral chip.

### 4.3 `spendTimeline` — monthly bars with highlight/annotation (Screenshot 3, top)
```jsonc
{ "kind": "spendTimeline",
  "title": "Monthly spend · Jan–Jul 2026",     // shortString, nullable
  "subtitle": "last 3 months highlighted · July projected", // nullable
  "points": [                                   // 2..24
    { "label": "Jan", "amountCents": 360000, "highlight": false,
      "annotation": null, "projected": false },
    { "label": "Apr", "amountCents": 570000, "highlight": false,
      "annotation": "LISBON", "projected": false },
    { "label": "Jul", "amountCents": 440000, "highlight": true,
      "annotation": null, "projected": true } ] }
```
Vertical bars scaled to max; `highlight` → accent fill; `annotation` label above the bar; `projected` → dashed/asterisked.

### 4.4 `spendingDrivers` — tagged driver breakdown (Screenshot 3, middle)
```jsonc
{ "kind": "spendingDrivers",
  "title": "What's actually driving the +$728/mo",  // shortString
  "subtitle": "vs your Jan–Feb baseline",            // shortString, nullable
  "drivers": [                                       // 1..8
    { "label": "Travel", "tag": "planned",
      "amountDisplay": "+$213/mo",                   // shortString (presentational delta)
      "note": "Italy flight deposits — funded by the Italy goal" } ] }
  // tag: "planned"|"trend"|"prices"|"anomaly"|"creep"|"mixed" (enum)
```
Colored dot + label + a tag pill (tag → token color) + right-aligned `amountDisplay` + note line. Reuses `TagPill`.

### 4.5 `watchList` — numbered watch-outs (Screenshot 3, bottom)
```jsonc
{ "kind": "watchList",
  "title": "Watch out for these",               // shortString
  "items": [                                     // 1..8
    { "label": "The Amex balance", "detail": "$2,418 revolving at 24.9%…",
      "amountDisplay": "−$50/mo" } ] }           // amountDisplay nullable
```
Numbered rows (renderer supplies the index), label + detail + optional right-aligned `amountDisplay`.

### 4.6 `actionPlan` — standalone checklist
```jsonc
{ "kind": "actionPlan",
  "title": "Action plan",                        // shortString, nullable
  "items": [ "Sweep the unused $168 into the House Fund" ] }  // 1..8 shortString
```
Same `ActionChecklist` sub-component used inside `spendingReview`, exposed standalone because the goal names action-plan checklists as first-class. Local check state only.

**Goal-enumerated coverage:** month cards → `spendingReview`; category bars → existing `categoryBreakdown` + reused in review; summary boxes → existing `callout` + review summary; action-plan checklists → `actionPlan` (+ in review); budget warnings → existing `callout` tone=warning; trend comparisons → existing `comparisonBars` + `spendingDrivers`/`spendTimeline`; goal-impact cards → **deferred** (§7, no screenshot/data yet).

## 5. Change map (per layer, per new kind)

1. `agent.rs` — add block structs + `AgentResponseBlock` variants + `valid_response_block` arms.
2. `copilot_chat.rs` — extend `should_emit_response_block` + `response_block_within_artifact_bounds` (new per-kind bounds constants).
3. `engine/mod.rs::build_system_prompt` — add each kind's compact schema + when-to-use line; add 1–2 few-shot exemplars for `spendingReview` and the analysis trio (emission lever).
4. `export_bindings` — regenerate `bindings.ts`.
5. `artifacts.ts` — add Zod branch per kind, bounds mirrored exactly.
6. `renderers.tsx` + new `cards/*.tsx` (`SpendingReviewCard`, `AccountsOverviewCard`, `SpendTimelineCard`, `SpendingDriversCard`, `WatchListCard`, `ActionPlanCard`) + shared `StatStrip`/`ActionChecklist`/`TagPill` in `cards/shared.tsx`.
7. `copilot-shell.css` — new `.cp-review*`, `.cp-accounts*`, `.cp-timeline*`, `.cp-drivers*`, `.cp-watch*`, `.cp-checklist*`, `.cp-tag*` classes, tokens only, dark-first + theme-aware.
8. Tests — Rust: serde round-trip + `valid_response_block` accept/reject + bounds per kind (mirror existing agent.rs test pattern). Frontend: `renderers.test.tsx` render-a-valid-payload per card; `artifacts.test.ts` accept valid / reject oversized+unknown-tag+malformed.

## 6. Emission reliability (the load-bearing risk)

Schema coverage ≠ the model actually emitting the block. Two forces work against it: (a) small local models (target: llama-3.3) against a strict JSON contract, and (b) **all fallback paths** (planner/deterministic) route through `enrich_agent_answer` and only ever produce markdown/callout/table — so whenever a fallback fires, *no* rich block appears regardless of schema. Graceful degradation to markdown is acceptable, but the happy path must reliably emit.

**Mitigations, built into Phase 1:**
- Composite/flat design chosen partly *because* it is easy for a model to emit (one block, one intent, no nesting).
- Prompt work: explicit when-to-use guidance + 1–2 few-shot exemplars per marquee flow.
- **Acceptance = the eval subset harness** (`memory/feedback_eval_subset_iteration`): it already records `response_block_kinds` per run. Iterate prompt/schema on a tiny subset (harness-only, no judge) until the target flows emit the new kinds; run the full eval only once stable. Needs `OPENROUTER_API_KEY` in `eval/.env` — user-run, not doable unattended here.
- Render path is fully testable without a model (vitest with hand-written payloads), so UI can be verified independently of emission.

## 7. Deferred (post-v1, noted for honesty)

- `goalImpact` card (goal named it, but no screenshot/data yet) — sketch: goal label, target, ETA delta from a proposed change.
- Interactive block CTAs that mutate — route through the bundle approval flow when added.
- Highlight/annotation on the generic `barChart` (superseded by `spendTimeline`).
- Richer `budgetWarning` beyond `callout`.

## 8. Phasing (each phase = one vertical slice: Rust → prompt → bindings → Zod → render → tests)

- **Phase 1 — Foundation + marquee.** Shared sub-components (`StatStrip`, `ActionChecklist`, `TagPill`) + CSS, then `spendingReview` end-to-end incl. prompt+few-shot and eval-subset emission check. Proves the whole vertical on the highest-value surface.
- **Phase 2 — `accountsOverview`** end-to-end.
- **Phase 3 — Analysis trio** (`spendTimeline`, `spendingDrivers`, `watchList`) end-to-end.
- **Phase 4 — `actionPlan` standalone + polish, full green-bar (Rust + vitest + tsc), full eval when stable.**

## 9. Acceptance criteria

- All new kinds validate identically on both sides; oversized/malformed/unknown-tag payloads rejected (Rust + Zod tests prove it).
- The three screenshots reproduce from hand-written payloads in the live app / vitest render tests, using tokens (no hardcoded colors), dark-first + theme-aware.
- The eval subset shows the target flows emitting the new block kinds on the configured model.
- Full green bar maintained (Rust tests, vitest, `tsc --noEmit`); bindings regenerated and committed.
- No new mutation path; CTAs ride existing followups/action-path/bundle flow.
