# Task 2 Report: Extend CompletionProvider trait and add reasoning messages

**Status:** DONE

## Summary

Created the reasoning module skeleton and extended the `CompletionProvider` trait with tool-calling support.

## Files Created
- `crates/finsight-agent/src/reasoning/mod.rs` — module root
- `crates/finsight-agent/src/reasoning/messages.rs` — `ChatMessage`, `ToolCall`, `ToolDefinition`, `AssistantTurn`, `AgentChange`, `ReasoningResult` types
- `crates/finsight-agent/src/reasoning/engine.rs` — `ReasoningEngine` with `run()` loop and `build_system_prompt()`
- `crates/finsight-agent/src/reasoning/tools/mod.rs` — `Tool` trait, `ToolContext`, `ToolSet` registry
- `crates/finsight-agent/src/reasoning/tools/read.rs` — stub (Task 3)
- `crates/finsight-agent/src/reasoning/tools/act.rs` — stub (Task 4)

## Files Modified
- `crates/finsight-agent/src/lib.rs` — added `pub mod reasoning`, re-exported types, added `complete_tool_turn` default method to `CompletionProvider`
- `crates/finsight-agent/src/providers/mock.rs` — added `tool_turns: Mutex<Vec<AssistantTurn>>` field, implemented `complete_tool_turn`
- `crates/finsight-agent/src/agent.rs` — updated `MockCompletionProvider` constructor
- `crates/finsight-agent/src/anomaly.rs` — updated `MockCompletionProvider` constructors, added `Mutex` import
- `crates/finsight-agent/src/categorizer.rs` — updated `MockCompletionProvider` constructors

## Test Results
All 19 tests in `finsight-agent` pass (0 failed, 0 ignored).
