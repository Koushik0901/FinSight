# Task 9 Report: Extend ask_agent command with mode and router

## Status: DONE

## Changes Made

### `crates/finsight-app/src/commands/agent.rs`

1. **Updated imports** — Added `reasoning::{engine::ReasoningEngine, tools::{act, read, ToolSet}}` from `finsight_agent`.

2. **Added `AgentChange` struct** — Local struct with `kind: String` and `description: String`, with `#[derive(Serialize, Type)]` and `#[serde(rename_all = "camelCase")]` for specta binding generation.

3. **Extended `AgentAnswer` struct** — Added three new fields:
   - `reasoning: String` — chain-of-thought explanation from the LLM
   - `trace: Vec<String>` — tool calls made during deep reasoning
   - `changes: Vec<AgentChange>` — autonomous actions taken (goal updates, planned transactions)

4. **Added `build_toolset()` function** — Registers all 11 tools (9 read-only + 2 action) into a `ToolSet`.

5. **Added `router_classify()` async function** — Uses the provider's `complete_json` to classify a question as "simple" or "deep" based on complexity. Falls back to "simple" on error.

6. **Rewrote `ask_agent` command** — New signature includes `mode: Option<String>`:
   - `mode = Some("deep")` → forces deep reasoning path
   - `mode = Some("quick")` → forces simple path
   - `mode = None` → uses router_classify for auto-detection
   - **Deep path**: builds toolset, runs `ReasoningEngine::run` with up to 10 iterations, persists an executed action bundle if changes were made
   - **Simple path**: preserves existing single-shot LLM call logic, returns new `AgentAnswer` shape with empty reasoning/trace/changes

### `crates/finsight-app/src/lib.rs`

No changes needed — `ask_agent` was already registered in `build_specta_builder()`. The specta registration picks up the new function signature automatically.

## Verification

- `cargo test -p finsight-app` — all 12 tests pass (0 failures)
- Compilation succeeds with no errors
- The new `mode: Option<String>` parameter and `AgentAnswer` type with new fields will generate correct TypeScript bindings via specta

## Design Decisions

- **Local `AgentChange` vs reusing `finsight_agent::AgentChange`**: Created a local struct in the command module to keep the Tauri command surface independent from agent internals. Both structs have identical shape.
- **Runtime inside `run()` closure**: Since `run()` uses `spawn_blocking` (offloads to a non-tokio thread), creating a `tokio::runtime::Builder::new_current_thread()` inside the closure is safe and doesn't conflict with Tauri's runtime.
- **Error handling**: Runtime build errors and reasoning engine errors are mapped to `CoreError::InvalidState` with descriptive messages, which converts cleanly through the `AppError` chain.
- **Bundle persistence**: Only persists if the reasoning engine made changes (non-empty `changes` vector). Uses `copilot_actions::insert_bundle` + `insert_item` + `set_bundle_status("executed")`.
