# Copilot Chat Redesign — Design Spec

**Date:** 2026-06-29  
**Status:** Approved  
**Scope:** Full redesign of the Copilot screen using `@assistant-ui/react` as the UI backbone, with streaming, persistent thread history, message branching, and all modern AI chat features — wired to the existing Rust/Tauri agent backend.

---

## 1. Problem Statement

The current Copilot screen is a single-question-at-a-time interface: the user asks, one response card appears, and the next question replaces it. There is no conversation history, no streaming, and no message threading. Users of ChatGPT, Claude, and Gemini expect a full threaded chat experience. FinSight's Copilot should match that bar while preserving its unique financial-action workflow (approve/reject/execute actions on live data).

---

## 2. Goals

- Persistent, multi-turn conversation threads saved locally to SQLite
- Streaming responses: tokens appear in real time as the LLM generates them
- Message branching: edit any past message and explore alternative responses
- Full markdown rendering in AI messages (code blocks, lists, bold/italic, tables)
- Action items (approve / reject / execute) preserved and embedded inline in AI message bubbles
- Thread history sidebar with search and deletion
- Auto-generated thread titles (4–6 words, from first message)
- Suggested prompts on an empty thread
- Follow-up suggestion chips after each AI response
- Tool call indicators showing which data sources were queried
- Copy and feedback (👍/👎) on every message
- Plutus design system throughout — no shadcn component overrides

**Out of scope:** file attachments, voice input, assistant-ui Cloud, thread sharing.

---

## 3. Architecture

### 3.1 Overview

```
┌──────────────────────────────────────────────────────────┐
│  Copilot Screen (React)                                  │
│                                                          │
│  ConversationSidebar      Thread (assistant-ui)          │
│  ├─ thread list            ├─ Message bubbles            │
│  ├─ search                 ├─ Streaming tokens           │
│  ├─ new conversation       ├─ ActionItemMessage (custom) │
│  └─ delete thread          ├─ ToolCallIndicator (custom) │
│                            ├─ FollowUpSuggestions (custom)│
│                            └─ Composer                   │
│                                                          │
│             ExternalStoreRuntime (bridge)                │
│     owns message state · feeds assistant-ui UI layer     │
└─────────────────────┬────────────────────────────────────┘
                      │  Tauri IPC + Window Events
┌─────────────────────▼────────────────────────────────────┐
│  Rust Backend                                            │
│                                                          │
│  stream_copilot_message  →  AgentHandle (modified)       │
│    app.emit("copilot-token", {token, conversation_id})   │
│    app.emit("copilot-done", {conversation_id,            │
│               bundle_id, tool_trace, follow_ups})        │
│                                                          │
│  list_conversations                                      │
│  get_conversation_messages(conversation_id)              │
│  delete_conversation(id)                                 │
│                                                          │
│  SQLite via finsight-core (V027 migration)               │
│    conversations table                                   │
│    conversation_messages table                           │
└──────────────────────────────────────────────────────────┘
```

### 3.2 Runtime Bridge

`TauriRuntime.ts` implements `ExternalStoreRuntime` from `@assistant-ui/react`:

- **`messages`** — React state array of `ThreadMessage[]`, populated from `useConversationMessages(activeConversationId)` on load and appended in real time during streaming
- **`isRunning`** — true while Tauri streaming session is active
- **`onNew(message)`** — invoked by assistant-ui's Composer on send:
  1. Appends user message optimistically to local state
  2. Calls `invoke("stream_copilot_message", { conversationId, text, history })`
  3. Registers `listen("copilot-token")` to append tokens to the pending AI message
  4. Registers `listen("copilot-done")` to finalise the message, attach action bundle, invalidate queries
- **Branching** — `onEdit(messageId, newContent)` re-invokes `stream_copilot_message` with the truncated history up to the edited message; the branched response is stored as a sibling message

### 3.3 Streaming Flow (sequence)

```
User types → Composer.onSend()
  → ExternalStoreRuntime.onNew()
    → [optimistic] append UserMessage to state
    → invoke("stream_copilot_message", {conversationId, text, history})
    → Rust: AgentHandle plans + calls LLM in streaming mode
      → for each token: app.emit("copilot-token", {token, conversationId})
      → [frontend] listen("copilot-token") → append to pendingAssistantMessage
      → Rust: LLM done, agent finalises action bundle
      → app.emit("copilot-done", {conversationId, bundleId, toolTrace, followUps})
    → [frontend] listen("copilot-done")
      → finalise AssistantMessage (attach bundleId, toolTrace, followUps)
      → persist to SQLite via Rust (already done server-side on emit)
      → unlisten both event channels
      → invalidate ["conversations"] and ["conversation-messages", conversationId] queries
      → isRunning = false
```

---

## 4. Database (V027 Migration)

File: `crates/finsight-core/migrations/V027__copilot_conversations.sql`

```sql
CREATE TABLE conversations (
  id         TEXT PRIMARY KEY,
  title      TEXT NOT NULL DEFAULT 'New conversation',
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE conversation_messages (
  id               TEXT PRIMARY KEY,
  conversation_id  TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  role             TEXT NOT NULL CHECK(role IN ('user', 'assistant')),
  content          TEXT NOT NULL,
  tool_trace       TEXT,             -- JSON array: ["spending_by_category", "budget_envelopes"]
  action_bundle_id TEXT,             -- FK into existing action_bundles table (nullable)
  branch_parent_id TEXT,             -- ID of the message this branches from (nullable)
  created_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_conv_messages_conv ON conversation_messages(conversation_id);
CREATE INDEX idx_conversations_updated ON conversations(updated_at DESC);
```

**Auto-title:** After the first `stream_copilot_message` in a new conversation completes, a second short LLM call (`max_tokens: 12`) generates a title from the first user message. Updated via `UPDATE conversations SET title=?, updated_at=? WHERE id=?`.

---

## 5. Rust Changes

### 5.1 New commands — `crates/finsight-app/src/commands/copilot_chat.rs`

| Command | Signature | Returns |
|---|---|---|
| `list_conversations` | `(state)` | `Vec<ConversationSummary>` |
| `get_conversation_messages` | `(state, conversation_id: String)` | `Vec<ConversationMessage>` |
| `delete_conversation` | `(state, id: String)` | `()` |
| `stream_copilot_message` | `(app, state, conversation_id: String, text: String, history: Vec<ChatMessage>)` | `String` (conversation_id) |

All commands are `pub async fn` with `#[tauri::command]` + `#[specta::specta]`.

### 5.2 Streaming in the agent pipeline

Modify `crates/finsight-agent/src/planner.rs` (or the provider layer in `crates/finsight-providers`):

- Add a `StreamSink` trait/enum: `AppHandleSink(AppHandle)` | `NoopSink`
- Each LLM provider's streaming response is iterated; each token is emitted via `sink.emit_token(token)`
- When complete, emit `copilot-done` with the finalised payload
- Non-streaming callers (background agent, categorizer) pass `NoopSink` — no behaviour change

### 5.3 Tauri events emitted

| Event name | Payload |
|---|---|
| `copilot-token` | `{ conversation_id: String, token: String }` |
| `copilot-done` | `{ conversation_id: String, bundle_id: Option<String>, tool_trace: Vec<String>, follow_up_questions: Vec<String> }` |

---

## 6. Frontend Changes

### 6.1 New files

| Path | Purpose |
|---|---|
| `ui/src/components/copilot/TauriRuntime.ts` | `ExternalStoreRuntime` adapter wiring Tauri events to assistant-ui state |
| `ui/src/components/copilot/ConversationSidebar.tsx` | Thread list, search input, new/delete, grouped by Today / This week / Earlier |
| `ui/src/components/copilot/ActionItemMessage.tsx` | Custom assistant-ui message renderer: displays approve/reject/execute action items inline |
| `ui/src/components/copilot/ToolCallIndicator.tsx` | "Analysed N sources" chips from `tool_trace` |
| `ui/src/components/copilot/FollowUpSuggestions.tsx` | Clickable suggestion pills below AI messages |
| `ui/src/components/copilot/EmptyThreadState.tsx` | Suggested-prompt grid shown when no messages in active thread |
| `ui/src/api/hooks/copilotChat.ts` | `useConversations()`, `useConversationMessages(id)`, `useDeleteConversation()` |

### 6.2 Modified files

| Path | Change |
|---|---|
| `ui/src/screens/Copilot.tsx` | Full rewrite — mounts `ConversationSidebar` + assistant-ui `Thread` with `TauriRuntime` |
| `ui/src/screens/Copilot.test.tsx` | Updated to reflect new structure |
| `ui/src/styles/app.css` | Add `.copilot-*` CSS using Plutus tokens to style headless primitives |
| `ui/src/api/client.ts` | Re-export new bindings after `export_bindings` |

### 6.3 Packages added

```json
"@assistant-ui/react": "latest",
"@assistant-ui/react-markdown": "latest"
```

Only headless primitives are used (`ThreadPrimitive.*`, `MessagePrimitive.*`, `ComposerPrimitive.*`). No shadcn component imports. All styling via `.copilot-*` CSS classes using existing Plutus design tokens.

---

## 7. Feature Delivery Matrix

| Feature | How delivered |
|---|---|
| Persistent conversation threads | `conversations` + `conversation_messages` tables; sidebar lists via `useConversations()` |
| Streaming token-by-token | `copilot-token` Tauri events → `listen()` in `TauriRuntime.onNew` |
| Message branching (edit → new response) | `ExternalStoreRuntime` branching; `branch_parent_id` stored in DB |
| Regenerate last response | assistant-ui built-in `MessagePrimitive.Reload` |
| Copy message | assistant-ui built-in `MessagePrimitive.Copy` |
| 👍 / 👎 feedback | assistant-ui built-in (logged client-side; can persist later) |
| Markdown rendering | `@assistant-ui/react-markdown` inside `MessagePrimitive.Content` |
| Tool call indicators | `ToolCallIndicator.tsx` renders `tool_trace` from `copilot-done` payload |
| Follow-up suggestions | `FollowUpSuggestions.tsx` renders `follow_up_questions`; click → pre-fill Composer |
| Action items inline | `ActionItemMessage.tsx` — full approve/reject/execute preserved |
| Suggested prompts (empty state) | `EmptyThreadState.tsx` with 8 prompt cards |
| Thread search | Client-side filter on `useConversations()` data |
| Thread deletion | `delete_conversation` command → invalidate query |
| Auto-generated titles | Short LLM call after first message; updates `conversations.title` |
| Multi-turn context | `history: Vec<ChatMessage>` passed to `stream_copilot_message` |
| Plutus design | All assistant-ui primitives unstyled; `.copilot-*` CSS uses `var(--ink)`, `var(--accent)`, etc. |

---

## 8. Integration with the Rest of the App

- **CopilotNudge / CopilotQuickAsk components** (used on Today, Budget, etc.) — clicking "Ask Copilot" navigates to `/copilot` and pre-populates a new thread with the prompt. No structural change to those components beyond updating the navigation target.
- **Existing action bundle hooks** (`useApproveActionItem`, `useRejectActionItem`, `useExecuteActionBundle`) — used unchanged inside `ActionItemMessage.tsx`.
- **AgentActivityFeed on Today screen** — unchanged; it reads from the existing `agent_sessions` table, not the new conversation tables.

---

## 9. Testing Plan

- **Rust unit tests:** `stream_copilot_message` in no-LLM mode (mock sink), `list_conversations`, `get_conversation_messages`, `delete_conversation`
- **Frontend component tests (vitest):** `ConversationSidebar` (renders threads, handles empty state), `ActionItemMessage` (approve/reject/execute flows), `EmptyThreadState` (prompt chips)
- **Full integration:** manual smoke test — start new thread, send message, see streaming, approve action, reload page, confirm thread persists

---

## 10. Open Questions

None. All design decisions are resolved.
