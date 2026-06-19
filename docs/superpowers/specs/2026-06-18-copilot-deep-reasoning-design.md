# Copilot Deep Reasoning — Design Spec

**Date:** 2026-06-18  
**Status:** Approved  
**Scope:** Upgrade FinSight Copilot so it can reason across multiple data sources, call tools, run what-if projections, and autonomously apply safe planning changes (goals and planned transactions) when the user asks complex questions like *“I just got my pay of $4k. What should I do with it first? Could I still invest $500 in stocks? What about my loans?”*

---

## 1. Goals

- Give Copilot a ChatGPT-like “deep think” capability: simple questions stay fast, complex financial questions trigger a multi-step reasoning loop.
- The agent decides which data it needs, fetches it through provider-native tool-calling APIs, and explains its reasoning in plain language.
- Autonomous actions are limited to safe bookkeeping changes (goal contributions and planned transactions). No real money moves.
- The UI is transparent but not verbose: a collapsed trace of tool names, a final answer with reasoning, and a summary of any changes applied.

---

## 2. User Experience

### 2.1 Input controls

The `/copilot` screen gets a small **“Deep think”** toggle next to the input bar.

- First-time default: **off**.
- The last user choice is remembered in `localStorage` (`finsight.copilot.deepThink`).
- The `ask_agent` command gains a `mode` parameter:
  - `"quick"` — today’s single-shot JSON answer.
  - `"deep"` — always run the full reasoning loop.
  - `"auto"` — a fast router prompt decides whether the question needs deep reasoning.

When the toggle is off, the frontend sends `"auto"`. When on, it sends `"deep"`.

### 2.2 Deep response UI

The response from `ask_agent` in deep/auto mode returns:

```ts
{
  prose: string;
  reasoning: string;
  trace: string[];            // human-readable tool names in order
  changes: AgentChange[];     // optional summary of autonomous changes
  action_label?: string;
  action_path?: string;
}
```

Rendering rules:

- **Collapsed trace pill:** “Used {trace.length} tools”. Clicking expands a vertical list of only the tool display names, e.g.:
  - Checked account balances
  - Listed upcoming bills
  - Ran cashflow projection
  - Updated “Invest” goal
  - Created planned payment
- **Final answer card:** prose + reasoning. Reasoning explains *why* the plan is safe (e.g., emergency fund is full, credit card minimum is covered, projected runway remains positive).
- **Changes applied card:** if `changes` is non-empty, render a compact card below the answer with one line per change, e.g.:
  - “Updated Invest goal to +$500/mo”
  - “Planned $800 credit card payment on 2026-06-25”

### 2.3 Autonomous actions

In deep mode the agent may call action tools directly. There is **no approval step**.
Every autonomous change is persisted as an executed action bundle in the existing `copilot_actions` tables so the user can review it later.

---

## 3. Backend Architecture

### 3.1 New module: `crates/finsight-agent/src/reasoning/`

```
crates/finsight-agent/src/reasoning/
  mod.rs          // public exports
  engine.rs       // ReasoningEngine loop
  messages.rs     // ChatMessage, ToolDefinition, ToolCall, AssistantTurn
  tools/mod.rs    // Tool trait and registry
  tools/read.rs   // read-only tool implementations
  tools/act.rs    // autonomous action tool implementations
```

### 3.2 Extended `CompletionProvider` trait

```rust
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn model_id(&self) -> &str;

    // Existing simple path
    async fn complete_json(&self, system: &str, user: &str) -> anyhow::Result<Value> {
        Ok(Value::Null)
    }

    // New tool-turn path
    async fn complete_tool_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> anyhow::Result<AssistantTurn>;
}
```

Provider-neutral message types:

```rust
pub enum ChatMessage {
    System { content: String },
    User { content: String },
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema object
}

pub enum AssistantTurn {
    ToolCalls(Vec<ToolCall>),
    FinalAnswer {
        content: String,
        reasoning: String,
    },
}

pub struct ReasoningResult {
    pub content: String,
    pub reasoning: String,
    pub trace: Vec<String>,
    pub changes: Vec<AgentChange>,
}
```

`ReasoningResult` is what the engine returns to the command layer. `trace` contains human-readable tool display names in execution order. `changes` is built from the log of mutating tool calls.

Each provider implements `complete_tool_turn` using its native API:

- **OpenAI-compatible providers** use the `tools` / `tool_choice` request fields and parse `choices[0].message.tool_calls`.
- **Anthropic** uses native `tools` and content blocks (`tool_use` / `tool_result`).
- **Ollama** uses `/api/chat` with a `tools` array and parses `message.tool_calls`.

### 3.3 `ReasoningEngine`

```rust
pub struct ReasoningEngine;

impl ReasoningEngine {
    pub async fn run(
        conn: &mut Connection,
        question: &str,
        tools: &ToolSet,
        provider: Arc<dyn CompletionProvider>,
        max_iterations: usize,
    ) -> anyhow::Result<ReasoningResult>;
}
```

Loop behavior:

1. Build the initial system prompt and user message.
2. Call `complete_tool_turn`.
3. If the turn is `FinalAnswer`, return it.
4. If the turn is `ToolCalls`, execute each tool via the `Tool` trait, append `ChatMessage::Tool` messages, and loop.
5. Mutating tools (`update_goal_monthly`, `create_planned_transaction`) log their outcome into a running `changes` list.
6. Stop after `max_iterations` (default 10) and return a graceful timeout message with partial reasoning.

After a successful run, the command layer persists an executed action bundle in `copilot_actions` with one item per mutating tool call.

### 3.4 Tool trait and registry

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value; // JSON Schema object
    fn execute(&self, ctx: &mut ToolContext, args: Value) -> anyhow::Result<Value>;
}

pub struct ToolContext<'a> {
    pub conn: &'a mut Connection,
}

pub struct ToolSet {
    tools: HashMap<String, Arc<dyn Tool>>,
}
```

`ToolSet::definitions()` returns `Vec<ToolDefinition>` for the provider. `ToolSet::execute(name, args)` dispatches to the matching tool.

### 3.5 Initial tool set

**Read-only tools**

| Tool | Description |
|---|---|
| `get_account_balances` | Current balance for every account plus total. |
| `get_month_totals` | This month’s income, expenses, and savings rate. |
| `get_top_spending_categories` | Top N spending categories with amounts. |
| `get_budgets` | Current month budgets with budgeted vs actual. |
| `get_goals` | Goals with current balance, target, monthly contribution, and progress. |
| `get_recurring_bills` | Detected recurring bills with expected next date and amount. |
| `get_liabilities` | Credit cards and loans with balance, APR, limit, and minimum payment. |
| `search_transactions` | Find transactions by merchant, date range, category, or amount. |
| `run_cashflow_projection` | Project runway and end-of-month net under a hypothetical change. |

**Autonomous action tools**

| Tool | Description |
|---|---|
| `update_goal_monthly` | Change a goal’s monthly contribution by a delta. |
| `create_planned_transaction` | Record a future payment, transfer, or investment. |

Action tools are only registered when `mode == "deep"`. In `"auto"` mode the router must classify the question as planning-related before deep mode is invoked.

---

## 4. Data Model Changes

### 4.1 New migration: `V018__planned_transactions.sql`

```sql
CREATE TABLE planned_transactions (
  id           TEXT PRIMARY KEY,
  description  TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  account_id   TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  category_id  TEXT REFERENCES categories(id) ON DELETE SET NULL,
  due_date     TEXT NOT NULL, -- YYYY-MM-DD
  status       TEXT NOT NULL DEFAULT 'planned', -- planned | completed | cancelled
  source       TEXT NOT NULL DEFAULT 'agent',    -- agent | manual
  created_at   TEXT NOT NULL DEFAULT datetime('now')
);

CREATE INDEX idx_planned_txn_due ON planned_transactions(due_date);
CREATE INDEX idx_planned_txn_status ON planned_transactions(status);
```

A separate table keeps planned intentions separate from real, posted transactions. Existing spending reports and category aggregations do not need to filter them out.

### 4.2 Core repo: `crates/finsight-core/src/repos/planned_transactions.rs`

```rust
pub struct PlannedTransaction { ... }
pub struct NewPlannedTransaction { ... }

pub fn list(conn: &mut Connection, filter: PlannedTxnFilter) -> CoreResult<Vec<PlannedTransaction>>;
pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<PlannedTransaction>>;
pub fn insert(conn: &mut Connection, new: NewPlannedTransaction) -> CoreResult<PlannedTransaction>;
pub fn update_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()>;
pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()>;
```

### 4.3 Integration points

- `run_cashflow_projection` includes planned transactions with `status = 'planned'` and `due_date <= projection_end`.
- `get_recurring_bills` may optionally union planned transactions for a fuller upcoming view (future UI work, not required for this spec).

---

## 5. Tauri Commands

### 5.1 Extend `ask_agent`

```rust
#[tauri::command]
#[specta::specta]
pub async fn ask_agent(
    state: tauri::State<'_, AppState>,
    question: String,
    mode: Option<String>, // "auto" | "deep" | "quick"
) -> AppResult<AgentAnswer>;
```

Behavior:

- `mode = None` or `"auto"`:
  - Run a router prompt (single fast LLM call) that classifies the question as `"simple"` or `"deep"`.
  - If `"simple"`, use the existing single-shot path.
  - If `"deep"`, run `ReasoningEngine`.
- `"quick"`: always use the existing single-shot path.
- `"deep"`: always run `ReasoningEngine`.

### 5.2 New response shape

```rust
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentAnswer {
    pub prose: String,
    pub reasoning: String,
    pub trace: Vec<String>,
    pub changes: Vec<AgentChange>,
    pub action_label: Option<String>,
    pub action_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentChange {
    pub kind: String,      // "goal" | "planned_transaction"
    pub description: String,
}
```

### 5.3 Router prompt

The router is a small, fast prompt that returns JSON only:

```json
{"mode": "simple" | "deep"}
```

It classifies anything involving income allocation, debt payoff, investment decisions, bill due dates, or “should I” financial tradeoffs as `"deep"`.

---

## 6. UI Changes

### 6.1 `ui/src/screens/Copilot.tsx`

- Add a **“Deep think”** toggle next to the submit button.
- Persist toggle state with `localStorage`.
- Pass `mode` to the `useAskAgent` mutation.

### 6.2 Message rendering

- If `trace` is non-empty, render a collapsed pill: “Used {trace.length} tools”.
- Expanded trace shows the tool display names only, never raw arguments or values.
- If `changes` is non-empty, render a “Changes applied” card with one line per change.

### 6.3 No new routes

Deep reasoning lives entirely within the existing Copilot screen.

---

## 7. Safety and Guardrails

### 7.1 Mutating tools are whitelisted

Only these tools can change data:

- `update_goal_monthly`
- `create_planned_transaction`

They are **not** available in quick mode. In auto mode they are only available if the router selected deep mode.

### 7.2 No destructive operations

The agent cannot:

- Delete transactions, accounts, goals, or categories.
- Move real money.
- Create categorization rules or new categories autonomously.
- Expose raw account balances or transaction details in the UI trace.

### 7.3 Audit trail

Every autonomous change is persisted as an executed action bundle via the existing `copilot_actions` repo:

- `bundle.status = "executed"`
- One item per `update_goal_monthly` / `create_planned_transaction`

This lets the user review what the agent did, even though there was no pre-approval step.

### 7.4 Error handling

| Scenario | Behavior |
|---|---|
| Provider fails mid-loop | Return partial reasoning plus a friendly “connection issue” message. |
| Tool arguments are invalid | Return a tool-error observation to the LLM; allow retry within iteration limit. |
| Max iterations exceeded | Return a message that the question was too complex, with what was learned so far. |
| No provider configured | Return the existing “Configure an AI provider in Settings → Agent.” error. |

---

## 8. Testing

### 8.1 Unit tests in `finsight-agent`

- `MockCompletionProvider` supports scripted `AssistantTurn` sequences.
- `ReasoningEngine` tests:
  - Single-turn final answer.
  - Multi-turn loop with tool calls.
  - Invalid tool arguments produce tool-error observations.
  - Max-iteration timeout.
- Provider tests extended to verify native tool request/response serialization.

### 8.2 Integration tests

- Seed the dev DB with the “Mira & Adam” dataset.
- Call `ask_agent(question: "I just got my pay of $4k...", mode: "deep")`.
- Assert response contains prose reasoning and at least one autonomous change or planned transaction.

### 8.3 Frontend tests

- Toggle deep think → `useAskAgent` receives `mode: "deep"`.
- Trace pill renders when `trace` is non-empty.
- Changes card renders when `changes` is non-empty.

### 8.4 Manual/provider matrix

- Validate Anthropic and at least one OpenAI-compatible endpoint.
- Ollama support is best-effort and depends on the local model’s tool-calling quality.

---

## 9. Out of Scope

- Provider-agnostic JSON-mode tool calling (Approach A) — deferred.
- Auto-creating categories or rules.
- Moving real money or connecting to banks.
- Full drag-and-drop UI for planned transactions.
- In-app notification when a planned transaction becomes due.

---

## 10. File Map

| File | Action |
|---|---|
| `crates/finsight-agent/src/lib.rs` | Add `reasoning` module, extend `CompletionProvider` trait |
| `crates/finsight-agent/src/reasoning/mod.rs` | Create |
| `crates/finsight-agent/src/reasoning/messages.rs` | Create |
| `crates/finsight-agent/src/reasoning/engine.rs` | Create |
| `crates/finsight-agent/src/reasoning/tools/mod.rs` | Create |
| `crates/finsight-agent/src/reasoning/tools/read.rs` | Create |
| `crates/finsight-agent/src/reasoning/tools/act.rs` | Create |
| `crates/finsight-agent/src/providers/ollama.rs` | Implement `complete_tool_turn` |
| `crates/finsight-agent/src/providers/openai_compat.rs` | Implement `complete_tool_turn` |
| `crates/finsight-agent/src/providers/anthropic.rs` | Implement `complete_tool_turn` |
| `crates/finsight-agent/src/providers/mock.rs` | Implement `complete_tool_turn` |
| `crates/finsight-core/migrations/V018__planned_transactions.sql` | Create |
| `crates/finsight-core/src/models/planned_transaction.rs` | Create |
| `crates/finsight-core/src/repos/planned_transactions.rs` | Create |
| `crates/finsight-core/src/models/mod.rs` | Add planned transaction model |
| `crates/finsight-core/src/repos/mod.rs` | Add planned transactions repo |
| `crates/finsight-app/src/commands/agent.rs` | Extend `ask_agent` with `mode` and router |
| `crates/finsight-app/src/lib.rs` | Re-export/bind commands |
| `ui/src/api/bindings.ts` | Regenerate |
| `ui/src/api/hooks/agent.ts` | Update `useAskAgent` |
| `ui/src/screens/Copilot.tsx` | Add deep-think toggle and trace/changes UI |

---

## 11. Success Criteria

- A user can ask *“I just got my pay of $4k. What should I do with it first? Could I still invest $500 in stocks? What about my loans?”* and receive a reasoned answer that references real balances, upcoming bills, and goals.
- The answer explains *why* the proposed plan is safe.
- If the agent decides to update a goal or create a planned transaction, the change is visible in the app immediately and recorded in the action log.
- Simple greetings still answer instantly without invoking the reasoning loop.
- All existing tests remain green.
