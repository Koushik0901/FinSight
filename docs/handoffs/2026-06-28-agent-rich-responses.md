# Agent Rich Responses Handoff

Date: 2026-06-28

This handoff documents the Copilot/agent rendering upgrade that lets FinSight show ChatGPT-like rich answers without allowing arbitrary HTML or JavaScript from the model.

## Completed work

### Backend response schema

- Extended `crates/finsight-app/src/commands/agent.rs` with typed `AgentResponseBlock` variants:
  - `markdown`
  - `table`
  - `barChart`
  - `lineChart`
  - `metricGrid`
  - `callout`
- Added supporting block structs:
  - `AgentTableBlock`
  - `AgentChartBlock`
  - `AgentChartPoint`
  - `AgentMetricBlock`
- Added `response_blocks` to `AgentAnswer`; regenerated `ui/src/api/bindings.ts`, which exposes this as `responseBlocks`.
- Updated the simple `ask_agent` LLM prompt so models can optionally return `response_blocks` JSON when it improves clarity.
- Added `parse_response_blocks()` and `valid_response_block()` guardrails:
  - invalid blocks are dropped;
  - table columns are capped;
  - table rows are capped;
  - chart points are capped;
  - metric counts are capped.
- Added `enrich_agent_answer()` so existing deterministic/tool-driven answers still render as rich blocks even if they only produce legacy fields.

### Frontend renderer

- Added `ui/src/components/AgentResponseRenderer.tsx`.
- Added dependencies in `ui/package.json` and `pnpm-lock.yaml`:
  - `react-markdown`
  - `remark-gfm`
  - `rehype-sanitize`
- The renderer supports:
  - safe markdown paragraphs/lists/code;
  - GitHub-flavored markdown tables;
  - typed table blocks;
  - metric grids;
  - callouts;
  - bar and line charts using existing Nivo packages.
- Markdown is rendered through `rehype-sanitize`; raw HTML is not trusted.
- Chart blocks use the existing `@nivo/bar` and `@nivo/line` packages already present in the UI stack.
- Metric values containing `$` receive `className="money"` so privacy mode can blur monetary amounts.

### UI integration

- `ui/src/screens/Copilot.tsx`
  - Uses `AgentResponseRenderer` for the main Copilot answer card.
  - Keeps existing trace, data-source, missing-data, assumptions, follow-up, and action-bundle sections.
  - Avoids duplicating the legacy alternatives table when the response already includes a structured table.
- `ui/src/components/CommandPalette.tsx`
  - Uses the same renderer in compact mode for inline ask-agent responses.
  - No-provider fallback now includes `responseBlocks: []` to match the generated type.

### Styling

- Added rich-response styles to `ui/src/styles/app.css`:
  - `.agent-rich`
  - `.agent-rich-markdown`
  - `.agent-rich-table-wrap`
  - `.agent-rich-chart`
  - `.agent-rich-metrics`
  - `.agent-rich-metric`
  - `.agent-rich-callout`
- Styles use existing design tokens such as `var(--ink)`, `var(--ink-mute)`, `var(--line)`, `var(--surface)`, `var(--surface-2)`, `var(--accent)`, `var(--positive)`, `var(--warning)`, and `var(--negative)`.

## Behavioral contract for future agents

1. Do not render model-provided raw HTML.
2. Prefer adding new typed `AgentResponseBlock` variants over parsing prose for UI behavior.
3. Keep `prose` as a compatibility fallback for older answers and tests.
4. Keep rich blocks bounded; do not let models return unbounded tables, chart series, or metric grids.
5. Use `AgentResponseRenderer` anywhere an `AgentAnswer` is shown so Copilot and quick-ask stay consistent.
6. Monetary values rendered by custom blocks must use the `.money` class when they are not plain markdown text.
7. After changing Rust response types, run `cargo run -p finsight-tauri --bin export_bindings` from the repo root.

## Validation completed

- `cargo check -p finsight-app`
- `cargo test -p finsight-app`
- `cargo run -p finsight-tauri --bin export_bindings`
- `cd ui && npx tsc --noEmit`
- `cd ui && npx vitest run src/screens/Copilot.test.tsx src/components/CommandPalette.test.tsx`
- `cd ui && npm run build`

## Important caveats

- Rich chart blocks are currently limited to one series. Add a new typed block shape before supporting multi-series charts.
- The prompt asks the model not to include HTML, but frontend sanitization is still the enforcement layer.
- The Command Palette intentionally skips chart rendering in compact mode; full visual charts belong on the Copilot screen.
