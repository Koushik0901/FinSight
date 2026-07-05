# Copilot UI Visual Redesign — Design Spec

**Date:** 2026-07-05
**Status:** Approved
**Scope:** Port the visual language of the Claude Design "Plutus" mockup (`components/copilot.jsx`, `components/copilot-cards.jsx`, `copilot.css` — project `fdbc4798-c6d0-41df-9499-e6ca4294d142`) into the real Copilot screen (`ui/src/screens/Copilot.tsx` and `ui/src/components/copilot/`), backed entirely by real data. The mockup is a visual reference only — its scripted scenarios, fake reveal timers, and hardcoded numbers are never ported; only the look is.

---

## 1. Problem statement

The current Copilot screen is already a real, working assistant-ui chat (streaming, reasoning groups, tool-call rendering, functional follow-up chips, real action-bundle approval) — it is not a prototype. But visually it is much plainer than the Plutus mockup: a flat generic tool-call card instead of a rich collapsible trace, a bare empty state instead of a glowing hero, and only 6 generic chart/table block kinds instead of the mockup's 7 purpose-built card types. This spec closes that visual gap without regressing any of the real functionality already in place, and without inventing fake data or scripted behavior to match the mockup's cinematic reveal.

## 2. Goals

- Visually match the mockup's hero/empty state, turn header (agent mark, name, model, source-rail), collapsible thinking block (plan → tool calls → reasoning), result cards, composer, and footer/follow-ups.
- Add a real upfront "Plan" step to the reasoning engine (the mockup's 3-part trace has no backend analog today).
- Extend `AgentResponseBlock` with 7 new semantic artifact kinds mapped conceptually (not 1:1) to the mockup's card types, each a strict Rust domain-data schema with presentation left entirely to the frontend.
- Build the two mockup action buttons ("Export as CSV", to a lesser extent "Save as a filtered view") as real features where they map onto real backend capability; explicitly drop what doesn't (see §9).
- Introduce Recharts as FinSight's new chart primitive for genuinely chart-shaped visuals, wrapped in a themed design layer — never raw/default Recharts styling. Everything else stays hand-rolled CSS/SVG, matching the existing app convention.

## 3. Non-goals

- No new standalone Transactions route (an explicit prior decision this codebase already made).
- No "saved filtered view" persistence layer — out of scope per explicit decision (§9).
- No migration of the existing Nivo-based `AgentResponseRenderer.tsx` (used by CommandPalette's quick-ask, a separate surface) — audited, left untouched.
- No changes to the legacy `CopilotLocalRuntime` streaming plumbing beyond the shared visual restyle (see §7).
- No fabricated/scripted data anywhere — every number in the redesigned UI comes from a real Tauri command or tool result.

---

## 4. Backend: Reasoning engine — Plan step

**File:** `crates/finsight-agent/src/reasoning/engine/mod.rs`

`ReasoningResult` currently carries one joined `reasoning: String` and a flat `trace: Vec<String>`, with no upfront plan. Add:

```rust
pub struct ReasoningResult {
    pub content: String,
    pub reasoning: String,
    pub plan: Vec<String>,       // NEW
    pub trace: Vec<String>,
    // ...existing fields
}

pub enum ReasoningEngineEvent {
    PlanReady { steps: Vec<String> },   // NEW — fires before ToolCallStart
    ToolCallStart { call: ToolCall },
    ToolCallResult { .. },
}
```

**Timing (corrected during design review):** the plan cannot come from the structured final answer — that arrives after tools run, which would invert the mockup's plan → tools → reasoning chronology. Instead, the system prompt (`build_system_prompt`) requires the model's **first** assistant turn to open with a short (3–5 step) numbered plan, whether or not that same turn also requests tool calls. The plan is parsed off that first turn's content and stripped before tool-call dispatch; `PlanReady` fires immediately, before any tool call in that turn executes.

This threads through to a new AG-UI stream frame:

```ts
{ type: "plan"; conversationId: string; runId: string; steps: string[] } & CopilotStreamFrameMeta
```

and is persisted in `agUiMetadataJson` alongside the existing `toolTrace`/`followUpQuestions` so it survives reload.

**Runtime scope:** this frame is emitted and consumed only on the AG-UI path (`TauriAgUiRuntime.ts`, the default runtime). The legacy `TauriRuntime.ts`/`CopilotLocalRuntime` path is not touched — its `MessageMeta.plan` is simply absent, and the frontend thinking block omits the Plan section when that field is missing (see §7).

## 5. Backend: AgentResponseBlock — 7 semantic artifact kinds

Extend the Rust `AgentResponseBlock` enum (`markdown | table | barChart | lineChart | metricGrid | callout`) with 7 new variants. Each is a **strict, validated, domain-data-only** schema — no colors, coordinates, SVG/Recharts config, or layout instructions. Category/tone colors are resolved entirely on the frontend from semantic keys (reusing the existing `colorForCategoryLabel`).

```rust
AllocationSplit {
    total_cents: i64,
    segments: Vec<AllocationSegment>,   // { label, amount_cents, rationale, category_key }
}
RankedOptions {
    title: String,
    options: Vec<RankedOption>,          // { rank_tone: primary|neutral|muted, label, detail, rationale }
}
CategoryBreakdown {
    period_label: String,
    rows: Vec<CategoryRow>,              // { category_key, amount_cents, is_fixed, is_lever }
}
AffordabilityVerdict {
    can_afford: bool,
    headline: String,
    sub: String,
    caveat: Option<String>,
    funding_source: Option<FundingSource>,  // { label, detail }
}
TransactionTable {
    count: i64,
    total_cents: i64,
    rows: Vec<TxRow>,                    // { date, merchant, category_key, amount_cents, flag }
    more: i64,
}
RecategorizationPreview {
    count: i64,
    rows: Vec<RecatRow>,                 // { merchant, category_key, confidence }
    more: i64,
    bundle_id: String,
}
ComparisonBars {
    title: String,
    current: MoneyPoint,                 // { label, amount_cents }
    prior: MoneyPoint,
}
```

**Tool mapping is many-to-many** (not 1:1), and the model selects the artifact kind based on the shape of its analysis, not phrase-matching:

| Example producer | Typical artifact |
|---|---|
| `search_transactions` | `TransactionTable` |
| `run_purchase_affordability` | `AffordabilityVerdict` |
| spending analysis (`get_top_spending_categories`, `get_month_totals`) | `CategoryBreakdown` or `ComparisonBars` |
| debt/goal scenario comparisons | `RankedOptions` or `ComparisonBars` |
| `analyze_cash_inflow` | `AllocationSplit` |

**`RecategorizationPreview` is the one exception — it is never model-chosen.** `draft_recategorization` (`crates/finsight-agent/src/reasoning/tools/act.rs:222`) pushes an `AgentDraftAction` into `ctx.draft_actions`, but the `bundle_id` doesn't exist until the bundle is persisted after the turn completes — the model cannot know it when picking an artifact. Instead, whichever backend code finalizes the turn automatically synthesizes a `RecategorizationPreview` block whenever `draft_actions` contains a `recategorize_bulk` entry, using the tool's own already-computed preview data (proposed count, dropped count, top-5 `merchant → category` labels, confidence) plus the bundle_id once assigned. The card's approve/reject/execute controls are the existing `ActionBundlePanel`, keyed on that `bundle_id` — never a standalone mutation.

**Validation & fallback:** every new variant is validated in Rust (non-empty required fields, bounded row counts) before being wrapped in the `render_finance_artifact` envelope. If validation fails, the command returns a tool error — which flows through the **existing** `isError` path already handled by `CopilotToolCard` (renders a labeled "tool returned an error" card). A malformed artifact never produces a blank assistant bubble.

**No 1:1 payload duplication:** `TransactionTable` does not carry the query that produced it — the frontend's Export action reads the original tool-call `args` (already flowing to `CopilotToolCard`/`ToolCallMessagePartProps`) rather than storing a redundant `query` field in the domain payload.

## 6. Backend: real CSV export for TransactionTable

`export_transactions_csv` (`crates/finsight-app/src/commands/transactions.rs:545`) only accepts `TxnFilterInput` (account_id, search, filter_preset, date range) — it cannot express `min_amount_cents`/`direction`/account-name-substring the way `search_transactions` can. Extract `search_transactions`'s SQL-building logic (`crates/finsight-agent/src/reasoning/tools/read.rs:388`) into a shared `finsight-core` function used by both the tool and a new command, `export_search_transactions_csv(query: TxnQueryArgs)`, so the Copilot's "Export as CSV" button re-runs the *exact* query that produced the table (no row cap) and writes it via the existing save-dialog + `csv_escape` pattern. One canonical filter-building implementation, not a duplicate.

## 7. Frontend: message shell

**Files:** `ui/src/screens/Copilot.tsx`, `ui/src/styles/app.css` (extending the existing ~400 `.copilot-*` rules), new files under `ui/src/components/copilot/`.

- **Hero/empty state:** port the glow orbs, avatar mark, personalized greeting, and pill-style suggestion chips (replacing today's `.copilot-prompt-card` grid). Keep the existing honest `CopilotGroundingStats` (real txn/account counts, explicit "no data yet" message) — never the mockup's hardcoded "1,247 transactions" row.
- **Turn header:** add an agent mark (pulses while running), a "Copilot" name label, inline provider/model text (`meta.providerId`/`meta.modelId`, already available — just needs header placement), and a source-rail of pills derived client-side from which tool names were called this turn via a small `TOOL_TO_SOURCE` lookup table (e.g. `search_transactions → Transactions`). No backend change needed for the rail itself.
- **Thinking block:** restyle `ReasoningGroup`'s plain `<details>` into a collapsible with three subsections: **Plan** (new — numbered, only rendered when `meta.plan` is present), **Tool calls** (restyle `ToolFallbackCard`/`copilotToolkit` cards into expandable rows showing the real args/result already flowing through tool-call parts), **Reasoning** (existing reasoning text, numbered/connected visually). On the legacy runtime path, Plan is simply absent.
- **Composer:** restyle to match (rounded box, model-dot badge, glowing send button on active state) — same `ComposerPrimitive` structure, CSS/markup only.
- **Footer/follow-ups:** keep the existing real `.copilot-msg-meta` and follow-up chips (already functionally correct), restyle to match the mockup's `cp-turn-ft`/`cp-fu-chip` visual language.

## 8. Frontend: card renderer architecture

**Files:** `ui/src/components/copilot/renderers.tsx` (extended), new artifact components, new `ui/src/components/copilot/charts/` module.

Shared internal primitives — `MetricRow`, `SegmentBar`, `ComparisonBar`, `FinancialTable`, `ConfidenceBadge`, `VerdictHeader` — plus one component per new `AgentResponseBlock` kind, avoiding 7 fully independent implementations. Shared loading/error/empty states for all artifact cards.

**Charting:** Recharts is added as a dependency, used only for genuinely chart-shaped visuals (`ComparisonBars`, and any future time-series card). Per explicit direction, **default Recharts styling is never shipped** — a themed wrapper module (`ui/src/components/copilot/charts/FinSightChart.tsx`) provides shared tooltip, typography, axes/grid, gradient/glow, currency formatting, animation, and empty/loading-state primitives, built once and reused by every Recharts-based card. `AllocationSplit`, `CategoryBreakdown`, `AffordabilityVerdict`, `TransactionTable`, and `RecategorizationPreview` remain hand-rolled CSS/SVG (matching both the mockup's own approach for these and the existing app convention — e.g. `NetWorthChart.tsx`'s manually-measured SVG). The existing 6 generic block kinds (`table`, `barChart`, `lineChart`, `metricGrid`, `callout`, `markdown`) are restyled (colored dots, mono tabular figures) to match the mockup's visual language, without new backend surface.

**Risk flag (early implementation checkpoint, not deferred to the end):** Recharts' `ResponsiveContainer`-style components render blank at width:0 and re-animate on every reflow — the exact reason this codebase hand-rolled `NetWorthChart` with manual measurement instead of a library. Verify `ComparisonBars` renders correctly while mounted inside a still-streaming, reflowing message bubble before building further cards on that foundation. If it misbehaves mid-stream, mount the chart only once the assistant message has finished streaming (`isRunning === false`) — consistent with the mockup's own reveal order, where cards only appear after the answer finishes.

**`RecategorizationPreview`** integrates directly with the existing `ActionBundlePanel` via its `bundle_id` — it is read-only presentation of a mutation proposal, never a standalone mutation control, and must remain visually/structurally distinct from `TransactionTable` (no shared "mode" prop between them).

## 9. Explicitly dropped from the mockup

- **"Save as a filtered view"** — no standalone cross-account Transactions screen exists to anchor a saved-filter concept, and inventing one is out of scope. Only "Export as CSV" ships as a real action.
- Scripted/cinematic reveal timers, fake tool latencies, and the mockup's hardcoded numbers (`"1,247 transactions"`, etc.) — never ported; all real.

## 10. Testing plan

- **Rust:** unit tests for the Plan-parsing step (first-turn extraction, stripped before tool dispatch), artifact validation + rejection fallback for each new `AgentResponseBlock` variant, `RecategorizationPreview` synthesis (bundle_id wiring), and the shared `search_transactions`/export SQL builder.
- **Frontend (Vitest):** component tests rendering each new artifact component from a fixture payload — the renderers are pure functions of block → JSX, so no live backend is needed to verify structure/content. Existing tests for follow-ups, action-bundle approval, and the empty state continue to pass unmodified except where markup intentionally changed.
- **Live verification:** the browser preview tool cannot drive Tauri IPC (`window.__TAURI_INTERNALS__` is absent outside the native webview — confirmed earlier this session). Final visual/streaming confirmation requires the native `pnpm tauri:dev` app; this is called out as a manual step, not claimed automatically.

## 11. Recommended implementation order

Not a scope cut — an ordering that de-risks the parts most likely to misbehave:

1. Frontend message-shell restyle + restyled existing 6 block kinds (independently shippable and verifiable via Vitest fixtures).
2. `ComparisonBars` + `FinSightChart` themed Recharts wrapper, verified mid-stream per §8's risk flag.
3. Remaining 6 new `AgentResponseBlock` kinds + their frontend components.
4. Reasoning-engine Plan step + AG-UI plan frame.
5. `RecategorizationPreview` synthesis wiring + real CSV export command.
