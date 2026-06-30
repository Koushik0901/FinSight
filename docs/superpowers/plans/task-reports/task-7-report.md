# Task 7: Implement provider-native tool calling for Ollama — Complete

**Status:** DONE

**Summary:** Added `complete_tool_turn` implementation to `OllamaProvider` in `crates/finsight-agent/src/providers/ollama.rs`.

**Changes:**
- Added import for `AssistantTurn`, `ChatMessage`, `ToolCall`, `ToolDefinition` from `crate::reasoning::messages`
- Added 4 new deserialization structs: `OllamaToolCall`, `OllamaFunction`, `OllamaMessageWithTools`, `OllamaRespWithTools`
- Implemented `complete_tool_turn` method that:
  - Converts `ChatMessage` enum variants to Ollama API message format (system, user, assistant with tool_calls, tool)
  - Converts `ToolDefinition` to Ollama tools format (type: function, function: {name, description, parameters})
  - Sends POST to `/api/chat` with `stream: false` and tools array
  - Parses response — if `tool_calls` present and non-empty, returns `AssistantTurn::ToolCalls`; otherwise `AssistantTurn::FinalAnswer`

**Tests:** All 19 finsight-agent tests pass (including existing `request_body_has_format_json`).

**File modified:** `crates/finsight-agent/src/providers/ollama.rs`
