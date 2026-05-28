# FinSight Phase 3 — Agent Foundation, Multi-Provider, Edit/Archive

## 0. Decisions log

| # | Question | Decision |
|---|---|---|
| 1 | Vector nearest-neighbor (sqlite-vec)? | **Skip** — defer to Phase 5 alongside anomaly detection and ⌘K. Rules → LLM batch only. |
| 2 | Edit/archive in Phase 3? | **Yes** — users need to correct categories immediately; design together with categorizer write paths. |
| 3 | Provider scope | **Multi-provider**: Ollama (local) + `OpenAiCompatProvider` (OpenAI, OpenRouter, Google, Mistral, …) + `AnthropicProvider` (direct — different API format). |
| 4 | Categorizations audit table | **Yes** — append-only `categorizations` table tracks every assignment by source (`rule` / `llm` / `user`). |

---

## 1. Scope

### In scope
- **V003 migration** — `categorizations` + `rules` tables
- **`finsight-agent` crate** — `CompletionProvider` trait with `complete_json`, three impls (`OllamaProvider`, `OpenAiCompatProvider`, `AnthropicProvider`)
- **Agent task** — long-lived `tokio::spawn` task, `mpsc` job queue, categorizer pipeline (rules → LLM batch)
- **Edit/archive** — update + archive accounts; update + delete transactions; category picker; auto-rule proposal on user correction
- **Settings AI Provider panel** — provider tiles, API key input (keychain), test connection, re-categorize all
- **Today screen additions** — "Needs a glance" count chip, agent activity feed
- **Onboarding StepAgent** — multi-provider aware

### Explicitly out of scope (deferred)
- `sqlite-vec` / vector nearest-neighbor — Phase 5
- Anomaly detection — Phase 5
- ⌘K command palette — Phase 5
- Rules editor UI (list, edit, delete rules) — Phase 4
- Budget envelopes — Phase 4
- Anthropic provider for *embeddings* — no embedding provider needed until Phase 5
- OFX/QIF import — separate spec when needed
- Multi-turn chat / conversational memory — Phase 5

---

## 2. Architecture overview

```
AppState
├── db: Db                        (existing)
├── agent: AgentHandle            (NEW)
│   └── tx: mpsc::Sender<AgentJob>
└── ...

AgentTask (tokio::spawn)
├── rx: mpsc::Receiver<AgentJob>
├── provider: Arc<dyn CompletionProvider>   (swappable at runtime)
└── db: Db

finsight-agent crate
├── lib.rs          — CompletionProvider + EmbeddingProvider traits
├── agent.rs        — AgentHandle, AgentJob, AgentTask, run loop
├── categorizer.rs  — pipeline: rules → LLM batch
└── providers/
    ├── mod.rs
    ├── ollama.rs
    ├── openai_compat.rs
    └── anthropic.rs
```

The agent task is started once in `finsight_app::AppState::new()`. Tauri commands enqueue jobs via `state.agent.tx.send(job)`. Long-running progress is reported via Tauri events. On provider config change (`set_completion_provider` command), the task replaces its `Arc<dyn CompletionProvider>` atomically — no restart needed.

---

## 3. Database — V003 migration

```sql
-- V003: categorizations audit trail + rules engine

CREATE TABLE categorizations (
  id          TEXT PRIMARY KEY,
  txn_id      TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  category_id TEXT REFERENCES categories(id),  -- NULL means "category cleared by user"
  source      TEXT NOT NULL,      -- 'rule' | 'llm' | 'user'
  confidence  REAL NOT NULL DEFAULT 1.0,
  model       TEXT,               -- NULL for rule/user assignments
  at          TEXT NOT NULL
);
CREATE INDEX idx_cat_txn ON categorizations(txn_id, at DESC);

CREATE TABLE rules (
  id          TEXT PRIMARY KEY,
  pattern     TEXT NOT NULL,      -- matched against merchant_raw using lower(merchant_raw) LIKE lower(pattern)
  category_id TEXT NOT NULL REFERENCES categories(id),
  enabled     INTEGER NOT NULL DEFAULT 1,
  source      TEXT NOT NULL DEFAULT 'user',  -- 'user' | 'agent-proposed'
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_rules_enabled ON rules(enabled) WHERE enabled = 1;
```

`transactions.category_id` remains the live value for fast reads. Every write (rule engine, LLM, user) appends a `categorizations` row **and** updates `transactions.category_id` + `ai_confidence` + `ai_explanation` in one transaction.

`rules` is intentionally simple: a single `LIKE` pattern on `merchant_raw`. No JSON condition/action system. A user-facing rules editor arrives in Phase 4.

---

## 4. Provider layer

### 4.1 Trait

```rust
// finsight-agent/src/lib.rs
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn provider_id(&self) -> &str;   // e.g. "ollama", "openai", "anthropic"
    fn model_id(&self) -> &str;
    /// Send a system + user prompt; expect a JSON-parseable response.
    async fn complete_json(
        &self,
        system: &str,
        user: &str,
    ) -> anyhow::Result<serde_json::Value>;
}
```

`EmbeddingProvider` trait is retained (stubbed) for Phase 5.

### 4.2 OllamaProvider (`providers/ollama.rs`)

- POST `{base_url}/api/chat` with `"format": "json"`
- Model from config
- No auth
- Also used for model listing: GET `{base_url}/api/tags`

### 4.3 OpenAiCompatProvider (`providers/openai_compat.rs`)

- POST `{base_url}/chat/completions`
- `Authorization: Bearer {api_key}`
- `response_format: { type: "json_object" }`
- Covers: OpenAI, OpenRouter, Google (`https://generativelanguage.googleapis.com/v1beta/openai/`), Mistral, Groq, any OpenAI-compatible endpoint
- `list_provider_models`: returns `Ok(vec![])` — the Settings UI falls back to a free-text model input for all OpenAiCompat providers (no live model dropdown)

### 4.4 AnthropicProvider (`providers/anthropic.rs`)

- POST `https://api.anthropic.com/v1/messages`
- Headers: `x-api-key: {api_key}`, `anthropic-version: 2023-06-01`
- Uses tool-use (function calling) to elicit structured JSON:
  ```json
  {
    "tools": [{"name": "classify", "input_schema": {...}}],
    "tool_choice": {"type": "tool", "name": "classify"}
  }
  ```
- Response extracted from `content[0].input`
- `list_provider_models`: returns `Ok(vec![])` — free-text model input in Settings UI

### 4.5 Config schema

Stored in `settings` KV under key `"completion_provider"`. API keys are **never** stored in settings — they live in the OS keychain.

The existing `keychain.rs` exports only `load_or_create_key` (generates a random key) and `delete_key`. Phase 3 adds two new exports to `finsight-core/src/keychain.rs`:

```rust
/// Store a user-supplied string value in the OS keychain.
pub fn set_key(service: &str, user: &str, value: &str) -> CoreResult<()>;

/// Retrieve a previously stored value. Returns None if not found.
pub fn get_key(service: &str, user: &str) -> CoreResult<Option<String>>;
```

API keys are stored with `service = "com.finsight.llm"`, `user = provider_id` (e.g. `"openai"`, `"anthropic"`, `"openrouter"`). The `save_provider_api_key(provider_id, key)` command calls `set_key("com.finsight.llm", provider_id, &key)`. Provider impls call `get_key("com.finsight.llm", provider_id)` at construction time.

```rust
// finsight-app/src/commands/agent.rs
#[serde(tag = "kind")]
pub enum CompletionProviderConfig {
    #[serde(rename = "unconfigured")]
    Unconfigured,
    #[serde(rename = "ollama")]
    Ollama { base_url: String, model: String },
    #[serde(rename = "openai_compat")]
    OpenAiCompat {
        preset: String,    // "openai" | "openrouter" | "google" | "custom" — display hint only
        base_url: String,
        model: String,
    },
    #[serde(rename = "anthropic")]
    Anthropic { model: String },
}
```

### 4.6 Migration from Phase 2 LlmProviderConfig

On first `AppState::new()` after the Phase 3 update, if settings key `"completion_provider"` is absent but `"llm_provider"` is present, migrate:
- `{ kind: "ollama", base_url, completion_model }` → `CompletionProviderConfig::Ollama { base_url, model: completion_model }`
- `{ kind: "unconfigured" }` → `CompletionProviderConfig::Unconfigured`

Save under `"completion_provider"`, leave old key in place (harmless). The `embedding_model` field present in the old `LlmProviderConfig::Ollama` is intentionally discarded — embedding providers are deferred to Phase 5.

### 4.7 Frontend provider presets

Baked into the Settings UI (no backend enum):

| Tile label | `preset` value | `base_url` |
|---|---|---|
| OpenAI | `"openai"` | `https://api.openai.com/v1` |
| OpenRouter | `"openrouter"` | `https://openrouter.ai/api/v1` |
| Google | `"google"` | `https://generativelanguage.googleapis.com/v1beta/openai/` |
| Custom | `"custom"` | (user-typed) |

Anthropic has its own tile (not an OpenAiCompat preset); selecting it renders the Anthropic config form.

---

## 5. Agent task + categorizer pipeline

### 5.1 AgentHandle / AgentJob

```rust
// finsight-agent/src/agent.rs
pub enum AgentJob {
    CategorizeImport { import_id: String },
    CategorizeAll,
}

pub struct AgentHandle {
    pub tx: mpsc::Sender<AgentJob>,
}

impl AgentHandle {
    /// Spawn the agent task and return a handle.
    pub fn spawn(db: Db, provider: Arc<dyn CompletionProvider>, window: tauri::Window) -> Self { ... }

    /// Replace the completion provider at runtime (no task restart).
    pub fn set_provider(&self, provider: Arc<dyn CompletionProvider>) { ... }
}
```

`AppState` grows:
```rust
pub agent: AgentHandle,
pub agent_provider: Arc<RwLock<Arc<dyn CompletionProvider>>>,
```

### 5.2 Categorizer pipeline (`categorizer.rs`)

Called from the agent task for each job. Steps:

**Step 1 — Rule pass:**
```
load all enabled rules from DB
for each uncategorized transaction in the batch:
  for each rule:
    if lower(merchant_raw) LIKE lower(pattern):  -- SQLite has no ILIKE; use lower() on both sides
      write categorizations row (source='rule', confidence=1.0, model=NULL)
      update transactions.category_id
      mark transaction as categorized
      break
emit categorization.progress
```

**Step 2 — LLM batch:**
```
collect remaining uncategorized transactions
for each batch of ≤20:
  build prompt (see §5.3)
  call provider.complete_json(system, user)
  parse JSON array response:
    [{txn_id, category_id, confidence, rationale}]
  for each result:
    write categorizations row (source='llm', confidence=..., model=provider.model_id())
    update transactions.category_id
    update transactions.ai_confidence + ai_explanation
  emit categorization.progress { import_id, done, total }
```

**Step 3 — Done:**
```
emit categorization.complete { import_id, categorized, skipped }
```

**Error handling:** if a provider call fails, transactions in that batch stay with `category_id = NULL` and `ai_confidence = NULL` (null = "not attempted yet"). Emit `agent.error { message }`. Task continues with next batch/job.

**Low-confidence threshold:** `ai_confidence < 0.6` → surfaced in Today "Needs a glance" count. This applies only to `source = 'llm'` rows; rule and user assignments always have `confidence = 1.0`.

### 5.3 LLM prompt (categorizer)

**System:**
```
You are a personal finance transaction categorizer. Classify each transaction into
exactly one of the provided categories. Respond with a valid JSON array only —
no markdown, no explanation outside the array.

Categories:
[{id, label, group_label, hint?}, ...]

Recent examples from this user (for calibration):
[{merchant_raw, category_label}, ...up to 5]
-- sourced from: SELECT t.merchant_raw, c.label FROM categorizations ca
--   JOIN transactions t ON t.id = ca.txn_id
--   JOIN categories c ON c.id = ca.category_id
--   WHERE ca.source = 'user'
--   ORDER BY ca.at DESC LIMIT 5
-- Falls back to empty list if user has no corrections yet.
```

**User:**
```
Classify these transactions:
[{txn_id, merchant_raw, amount_cents, day_of_week}, ...]

Respond:
[{"txn_id":"...","category_id":"...","confidence":0.0–1.0,"rationale":"one sentence"}]
```

### 5.4 Auto-rule proposal

When `update_transaction` is called with a changed `category_id`:
- The backend checks `SELECT 1 FROM rules WHERE pattern = ? AND enabled = 1` (exact match on `merchant_raw`)
- If none found, command returns `proposed_rule: Some(ProposedRule { pattern, category_id, category_label })`
- Frontend shows sonner toast: *"Always categorize «[merchant]» as [Category]? [Create rule] [Skip]"*
- [Create rule] calls `create_rule` command
- [Skip] or timeout → dismissed, no rule created

---

## 6. Edit + archive

### 6.1 New repo methods

**`repos/accounts.rs`:**
```rust
pub fn update(conn: &mut Connection, id: &str, patch: AccountPatch) -> CoreResult<Account>;
pub fn archive(conn: &mut Connection, id: &str) -> CoreResult<()>;
  // sets archived_at = now(); deletes csv_import_mappings row for this account_id

pub struct AccountPatch {
    pub name: Option<String>,
    pub bank: Option<String>,
    pub account_type: Option<String>,
    pub color: Option<String>,
    pub last4: Option<String>,
    pub currency: Option<String>,
}
```

**`repos/transactions.rs`:**
```rust
pub fn update(conn: &mut Connection, id: &str, patch: TxnPatch) -> CoreResult<(Transaction, Option<ProposedRule>)>;
pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()>;

pub struct TxnPatch {
    pub notes: Option<String>,
    pub category_id: Option<Option<String>>,  // Some(Some(id)) = set; Some(None) = clear
    pub amount_cents: Option<i64>,
    pub merchant_raw: Option<String>,
}
```

When `TxnPatch.category_id` is `Some(Some(id))` (set a category):
1. Append `categorizations` row: `category_id=id`, `source='user'`, `confidence=1.0`, `model=NULL`
2. Update `transactions.category_id = id`, clear `ai_confidence` + `ai_explanation`
3. Check `SELECT 1 FROM rules WHERE lower(pattern) = lower(merchant_raw) AND enabled=1` → if none found, return `proposed_rule`

When `TxnPatch.category_id` is `Some(None)` (clear the category):
1. Append `categorizations` row: `category_id=NULL`, `source='user'`, `confidence=1.0`, `model=NULL`
2. Update `transactions.category_id = NULL`, clear `ai_confidence` + `ai_explanation`
3. No rule proposal (no target category to propose)

`proposed_rule` is never returned when the category was cleared.

**`repos/categorizations.rs`** (new file):
```rust
pub fn insert(conn: &mut Connection, row: NewCategorization) -> CoreResult<()>;
pub fn list_for_txn(conn: &mut Connection, txn_id: &str) -> CoreResult<Vec<Categorization>>;
```

**`repos/rules.rs`** (new file):
```rust
pub fn list_active(conn: &mut Connection) -> CoreResult<Vec<Rule>>;
pub fn insert(conn: &mut Connection, rule: NewRule) -> CoreResult<Rule>;
pub fn set_enabled(conn: &mut Connection, id: &str, enabled: bool) -> CoreResult<()>;
```

### 6.2 New Tauri commands

```rust
// commands/accounts.rs additions
update_account(state, id: String, patch: AccountPatch) -> AppResult<Account>
archive_account(state, id: String) -> AppResult<()>

// commands/transactions.rs additions
update_transaction(state, id: String, patch: TxnPatch) -> AppResult<UpdateTxnResult>
delete_transaction(state, id: String) -> AppResult<()>
create_rule(state, pattern: String, category_id: String) -> AppResult<Rule>

// commands/agent.rs (new file)
set_completion_provider(state, config: CompletionProviderConfig) -> AppResult<()>
save_provider_api_key(state, provider_id: String, key: String) -> AppResult<()>
list_provider_models(state, config: CompletionProviderConfig) -> AppResult<Vec<String>>
test_completion_provider(state, config: CompletionProviderConfig, api_key: Option<String>) -> AppResult<ProviderTestResult>
get_needs_review_count(state) -> AppResult<u32>
trigger_categorize(state) -> AppResult<()>  // enqueues CategorizeAll

pub struct UpdateTxnResult {
    pub transaction: Transaction,
    pub proposed_rule: Option<ProposedRule>,
}

pub struct ProposedRule {
    pub pattern: String,
    pub category_id: String,
    pub category_label: String,
}

pub struct ProviderTestResult {
    pub ok: bool,
    pub error: Option<String>,
    pub latency_ms: u64,
}
```

### 6.3 Frontend — drawer edit mode

**AccountDrawer** gains an optional `account?: Account` prop:
- When present: form pre-filled, title "Edit Account", submit calls `updateAccount` mutation
- "Archive account" button at drawer bottom (two-click confirm: first click → "Confirm archive?" state)
- On archive success: close drawer, invalidate `accounts` query, navigate away if on this account

**TransactionDrawer** gains an optional `transaction?: Transaction` prop:
- When present: form pre-filled, title "Edit Transaction"
- **Category picker** — new `CategoryPicker` component: scrollable list of categories grouped by `category_groups.label`, each row shows color swatch + label. Selected item highlighted. Searchable with a text filter.
- On save: if `proposed_rule` returned, show sonner toast with Create rule / Skip actions
- "Delete transaction" button (two-click confirm pattern)
- On delete: close drawer, invalidate `transactions` + `today-summary` queries

**Screens:**
- Accounts screen: clicking an account row → `AccountDrawer` with that account (edit mode)
- Transactions screen: clicking a transaction row → `TransactionDrawer` with that transaction (edit mode)
- Both screens already have "Add" paths opening the drawers in create mode (unchanged)

---

## 7. Settings screen additions

A new **"AI Provider"** card section in `Settings.tsx`:

**Display state (provider configured):**
```
AI Provider
[Ollama — llama3.2]   [Edit]
```

**Display state (unconfigured):**
```
AI Provider
Not configured — categories won't be assigned automatically.
[Configure]
```

**Edit panel (inline expansion, not a drawer):**

1. Provider type row: `[Ollama] [OpenAI] [OpenRouter] [Anthropic] [Google] [Custom]`
2. Fields by type:
   - Ollama: base_url + model dropdown (live probe)
   - OpenAiCompat: base_url (pre-filled, editable for Custom) + model text input + API key input (masked)
   - Anthropic: model text input + API key input (masked)
3. `[Test connection]` — calls `test_completion_provider`, shows ✓ `{latency_ms}ms` or error inline
4. `[Save]` — calls `set_completion_provider` + (if key changed) `save_provider_api_key`
5. `[Re-categorize all]` — calls `trigger_categorize`, shows quiet progress via `categorization.progress` event

API key field: placeholder `••••••••`. If a key is already saved in keychain, show `[key saved — click to replace]`. Clearing and saving stores the new value; leaving unchanged does not re-write keychain.

---

## 8. Today screen additions

### 8.1 "Needs a glance" chip

Shown between the date header and the transaction list when `needsReviewCount > 0`:

```
⚠  4 transactions need review   →
```

Clicking navigates to `/transactions?filter=needs_review`.

The Transactions screen adds a `needs_review` filter mode. A transaction is "needs review" when:
1. `transactions.ai_confidence < 0.6` (denormalized fast check), AND
2. The most-recent row in `categorizations` for that transaction has `source = 'llm'` (i.e. a user correction clears the flag)

SQL intent:
```sql
SELECT t.* FROM transactions t
WHERE t.ai_confidence < 0.6
  AND (
    SELECT source FROM categorizations c
    WHERE c.txn_id = t.id
    ORDER BY c.at DESC LIMIT 1
  ) = 'llm'
```

`get_needs_review_count` uses the same predicate wrapped in `SELECT COUNT(*)`. `useNeedsReviewCount` invalidates on `categorization.complete` event and on `update_transaction` mutation success.

Query: `useNeedsReviewCount()` hook — polls every 30s, invalidated on `categorization.complete` event.

### 8.2 Agent activity feed

A single line below the date header (above the "Needs a glance" chip):

```
Categorizing…  12 / 47
```

Subscribes to `categorization.progress` and `categorization.complete` Tauri events. Fades out 3 seconds after `categorization.complete`. Invisible when idle. `aria-live="polite"`.

Implemented as a standalone `AgentActivityFeed` component that manages its own event listener lifecycle.

---

## 9. Onboarding StepAgent updates

Current StepAgent: probes Ollama only. Phase 3 update:

**Two-path layout:**

```
How do you want to power AI categorization?

[Local (Ollama)]                    [Cloud provider]
Install-free if already running.    OpenAI, Anthropic, OpenRouter, etc.
```

**Ollama path:** unchanged — probe, model picker, nomic warning, Save.

**Cloud path:**
- Provider tile row (OpenAI / OpenRouter / Anthropic / Google / Custom)
- base_url (pre-filled per tile)
- API key input
- Model text input
- [Test & Save] button — calls `test_completion_provider` inline; on success saves config + key and calls `markOnboardingComplete`

**Skip:** unchanged — `[Configure later →]` saves `Unconfigured` and completes onboarding.

---

## 10. Tauri events (new in Phase 3)

| Event | Payload | When |
|---|---|---|
| `categorization.progress` | `{ import_id: string, done: number, total: number }` | After each rule match or LLM batch write |
| `categorization.complete` | `{ import_id: string, categorized: number, skipped: number }` | Job finished |
| `agent.error` | `{ message: string }` | Provider call failure |

---

## 11. New frontend hooks

```ts
// api/hooks/agent.ts
useNeedsReviewCount()                         // GET count of low-confidence txns
useSetCompletionProvider()                    // mutation: set_completion_provider
useSaveProviderApiKey()                       // mutation: save_provider_api_key
useListProviderModels(config)                 // query: list_provider_models
useTestCompletionProvider()                   // mutation: test_completion_provider
useTriggerCategorize()                        // mutation: trigger_categorize

// api/hooks/accounts.ts additions
useUpdateAccount()                            // mutation: update_account
useArchiveAccount()                           // mutation: archive_account

// api/hooks/transactions.ts additions
useUpdateTransaction()                        // mutation: update_transaction
useDeleteTransaction()                        // mutation: delete_transaction
useCreateRule()                               // mutation: create_rule
```

---

## 12. Testing strategy

**`finsight-agent` crate:**
- `MockCompletionProvider` in `src/providers/mock.rs` — returns canned JSON for given input patterns
- Unit tests for `categorizer.rs`: rule pass (match + no-match), LLM batch parse, write-back correctness
- Tests for each provider's request-building logic (no network calls — mock `reqwest` responses)

**`finsight-app` integration tests (in `tests/`):**
- `categorization_cmd.rs` — end-to-end: import fixture CSV → trigger categorize (mock provider) → assert `categorizations` rows + `transactions.category_id` updated
- `edit_account_cmd.rs` — update fields + archive + verify csv_import_mappings cleanup
- `edit_transaction_cmd.rs` — update category → proposed_rule returned → create_rule → rule appears in list

**Frontend:**
- `CategoryPicker.test.tsx` — renders groups/items, filters on search, fires onChange
- `AccountDrawer.test.tsx` — edit mode pre-fills form, calls `updateAccount` on submit
- `TransactionDrawer.test.tsx` — edit mode pre-fills, category picker changes fire update
- `Settings.test.tsx` — provider config panel renders, test-connection result displayed
- `AgentActivityFeed.test.tsx` — listens to mock events, renders progress, fades after complete
- a11y sweep extended to cover new components

---

## 13. Open follow-ups (tracked for Phase 4+)

- Rules editor UI (list, toggle, delete rules) — Phase 4 alongside budget categories
- `sqlite-vec` + vector nearest-neighbor — Phase 5
- Anomaly detection — Phase 5
- ⌘K palette — Phase 5
- `agent_insights` table — Phase 5
- Inline styles in `Today.tsx` + `Transactions.tsx` → CSS classes (carried from Phase 1)
- `TweaksPanel.tsx` theme toggle surface (carried from Phase 2)
- Mixed snake_case/camelCase in tauri-specta types (carried from Phase 2)
