# FinSight Maintainability & Architecture Audit

**Date:** 2026-08-22
**Scope:** Full repository (`crates/*`, `src-tauri`, `ui/src`) — read-only audit, no code modified.
**Method:** Three parallel deep-dive passes (Rust domain/agent, Rust API/server, frontend) followed by targeted verification of every high-severity claim against source.

---

## 1. What this system is

**Architecture:** Self-hosted client/server personal-finance app. An Axum server (`finsight-server`) owns auth (Argon2id + SQLCipher per-user DBs, wrapped keys), serves a React PWA from `ui/dist`, and exposes ~229 RPC commands via `POST /api/rpc/{cmd}` plus SSE at `/api/events`. The shipped Tauri binary (`src-tauri`) is a thin webview shell that just navigates to a server URL. Command bodies live transport-agnostically in `finsight-api`; `finsight-bindings` is a specta codegen wrapper emitting `ui/src/api/bindings.ts`; `ui/src/api/httpBackend.ts` shims the Tauri `invoke`/`event` contract over HTTP+SSE so browser/PWA/shell share one frontend.

**Layers:** `finsight-core` (domain + all SQL, 63 refinery migrations) → `finsight-agent` (Copilot planner/executor/reasoning engine, categorizer LLM pipeline) + `finsight-providers` (CSV parsers, Ollama/OpenAI-compat/Anthropic clients) → `finsight-api` (every command body) → server/bindings transports.

**State:** tanstack-query on the frontend with a genuinely good domain-graph invalidation module (`ui/src/api/hooks/invalidation.ts`) and encrypted 7-day IndexedDB persistence (`ui/src/pwa/persist.ts`); zustand only for theme/density/accent/privacy/currency (`state/tweaks.ts`). Server side: per-user runtimes with single-flight bootstrap in `crates/finsight-server/src/registry.rs` — this design is excellent.

**Testing:** ~136 frontend vitest files, 36 Rust test files, a Rust↔Zod fixture-corpus parity test for Copilot blocks, a regex-based dispatcher↔bindings parity test (`crates/finsight-server/tests/parity.rs`), CI in `.github/workflows/ci.yml`.

The codebase is far more disciplined than typical AI-accreted code: near-zero `any`, near-zero TODO/FIXME, comments encode regression history. The debt is concentrated in **duplication** — the same contract, formula, or runtime re-declared by hand in multiple places that drift silently.

---

## 2. Maintainability problems

### A. The same contract re-declared by hand in N places *(systemic root cause)*

| Contract | Hand-maintained copies |
|---|---|
| Command registration | finsight-api body → bindings wrapper → `collect_commands!` (`finsight-bindings/src/lib.rs:59-289`) → dispatch arm (`dispatch.rs:120-904`, ~126 arms) → `SUPPORTED` array (`dispatch.rs:909-1167`, hand-ordered copy of the match) → bindings.ts regen = **5–7 edits per command** |
| Event names | `"copilot-stream-frame"`, `"import-progress"`, `"categorization.progress"` etc. as string literals in Rust producers (`copilot_chat.rs:1058`, `import.rs:149`, `registry.rs:203`) and TS consumers (`ImportProgress.tsx:19`, `TauriRuntime.ts:453`, `TauriAgUiAgent.ts:449`, `nativeNotify.ts:24`). Rename compiles everywhere, silently kills streaming UX |
| Copilot stream frames | `normalizeFrameType`/`normalizeCopilotFrame` implemented twice: `TauriRuntime.ts:167-292` vs near-verbatim copy in `TauriAgUiAgent.ts:55-179` (~130 lines differing only in `plan` handling) |
| Tool names | planning/mod.rs references tool names as literals 10× matching `fn name()` impls; also hand-copied into `renderers.tsx:30-62` (`ALL_TOOL_NAMES`) and MCP bundle schemas (`mcp.rs:329-385` vs reader `mcp.rs:838-954`) |
| Mock backend | `ui/src/dev/mockBackend.ts` (1,410 lines) re-implements business math (compound growth `project_goal_growth` L1159, balance timeline L1239, month closes) for **~66 of 229 commands**, untyped responders, zero tests, everything else silently returns `[]` |

### B. Two anomaly detectors fighting over one DB column *(correctness bug shaped like architecture debt)*

- Authoritative: `crates/finsight-core/src/anomaly.rs` — median/MAD, honors `anomaly_dismissed` (4 refs), runs at startup/import/household change.
- Second implementation: `crates/finsight-agent/src/anomaly.rs::detect_anomalies` (:38) — IQR fences, raw-string merchant matching, **zero references to `anomaly_dismissed`**, runs after every categorization job (`categorizer.rs:343`, result discarded via `let _ =`).

Core's recompute wipes flags; agent re-flags under different criteria and can resurrect flags the user explicitly dismissed. Also: its writes skip the `ResetBarrier` lease, so it can race a data reset.

Related duplication cluster (classic "AI solved it twice"):

- Rule-pattern LIKE matching in both SQL (`repos/rules.rs:81`) and Rust (`agent/categorizer.rs:100-111`) with *divergent wildcard semantics* despite a doc claiming parity.
- Balance-priority `ORDER BY … CASE source …` SQL fragment copy-pasted **8×** across two crates (`repos/accounts.rs:925`, `finance.rs:2611,2713`, `context.rs:710,1097,1136`, `read.rs:27,940`).
- Money formatting **5 ways** with divergent output (`finance.rs:2816` truncates $19.99→`$19`).
- `contains_any`/`months_until`/date-parsers duplicated across finance.rs/context.rs/planning.
- 4 private `unwrap()` helpers in TS while 178 copies of the unwrap boilerplate are inlined across hooks.

### C. God files with clear seams

- `crates/finsight-agent/src/finance.rs` (**4,664 lines**) — seven responsibilities: question NLP, raw-SQL snapshot loaders (`fn accounts/goals/liabilities/recurring_bills` :2608-2756 — repo-bypassing data access living in the agent crate), debt payoff sim, goal planning, cashflow, affordability, NLG explainers.
- Same repo-bypass pattern in `context.rs` and `reasoning/tools/read.rs` (35 tools each embedding raw SQL + 12 `.filter_map(|r| r.ok())` error swallows).
- `metrics.rs` (2,617) — every metric exists twice (household + `_for` member variant); adding a metric = write both or screens disagree.
- Frontend: `screens/Copilot.tsx` (1,285 — messages, headers ×2 nearly identical, action panel with its own divergent mutation, streaming-markdown healing logic, error boundary), `copilot/TauriRuntime.ts` (858 — queue + transport normalization + error formatting + persistence mapping + React runtime), `Settings.tsx` (855 — 12 sections), plus Scenarios/Recipes/Budget/Goals 600-700 each.
- `styles/app.css` (5,032) mixes base utilities with 284 copilot selectors that belong in `copilot-shell.css`.

### D. Live bug candidates from drift

1. **Execute-bundle implemented twice:** `hooks/copilot.ts:124` (`useExecuteActionBundle`, invalidates agentApply+goals+memory) vs inline raw invoke in `Copilot.tsx:147-153` invalidating **only** `["action-bundles"]` — executing a ledger-mutating bundle from the legacy panel leaves caches stale.
2. `agent/categorizer.rs:377` — `load_uncategorized(conn, _import_id)` ignores `import_id`; `CategorizeImport { import_id }` categorizes everything while UI reports per-import progress.
3. `executor.rs:175` classifies errors by `err.to_string().contains("validation:")`.
4. Query keys duplicated by hand: `["action-bundles","pending",null]` literal in `BottomNav.tsx:55`/`Sidebar.tsx:65` breaks silently if the hook's key gains a segment; `simplefinKeys` re-typed by hand in `Inbox.tsx:437`.

### E. Error handling / robustness

- HTTP mapping: everything except 3 codes → 500 (`dispatch.rs:108-117`), so client-fixable errors pollute server-fault telemetry; `AppError` carries no status hint.
- Silent failure paths: `let _ = recipe_runner::run_due_recipes(...)` (`agent.rs:93`), context builders render empty-on-error (`context.rs:1102,1142`), broadcast lag drops frames uncounted (`events.rs:49`).
- 33× `Mutex<Connection>.lock().unwrap()` in `users.rs` — one panic poisons auth until restart. RFC3339 `unwrap()`s in row mappers (`repos/connections.rs:109,145`, `repos/transfers.rs:43-103`) panic lists on one malformed timestamp. Multi-row writers mostly lack transactions (categorizer per-row, executor bundle statuses) unlike the correct pattern in `rules.rs:90-104`.

### F. Dead code / leftovers

- Untracked husk `crates/finsight-app/tests/fixtures/` on disk; **AGENTS.md instructs contributors to edit `crates/finsight-app/src/commands/` which doesn't exist in git** (real location: `finsight-bindings`). Doc actively misdirects.
- `core/sample.rs` (1,130) + `seed.rs` test fixtures compiled unconditionally into release builds (unlike feature-gated `testing.rs`).
- `src-tauri/Cargo.toml:25-27` declares dialog/opener/notification plugins + capability grants that `main.rs` never registers; `FilePicker.tsx:24` still calls plugin-dialog in a branch that would throw post-Phase-4.
- Unused npm deps: `@assistant-ui/react-markdown` (0 imports), likely `@tauri-apps/plugin-opener`; stale comments reference removed `@nivo/*` (`App.tsx:62`).
- Legacy Copilot runtime (`CopilotLocalRuntime`, `?agui=0`) kept as rollback but doubling every stream-frame change's cost, alongside routed dead route `/copilot/ag-ui-spike` (`CopilotAgUiSpike.tsx`).

---

## 3. Extensibility test (realistic features)

**① New LLM provider (e.g., Gemini).** Touches: new `CompletionProvider` impl in `finsight-providers` (trait is clean — easy), provider factory/construction helpers in `finsight-api`, the provider config state machine in `Settings.tsx:604-686` (+presets :779-810), onboarding `StepAgent.tsx`, structured-output probe gating in `copilot_chat.rs`. Cascades because provider identity is a stringly-typed switch replicated across settings UI, factory, and probe logic. Improvement: derive the Settings provider list from one shared descriptor table.

**② New sync source (e.g., Plaid alongside SimpleFIN).** Touches: `commands/simplefin.rs` (1,027-line provider-specific module — the scheduler assumes one source), account `source` values where `'csv'`-style addition means editing the balance-priority CASE fragment at **8 sites across 2 crates** (miss one → that surface ranks balances differently, silently), vendor hint tables (`merchant.rs` vs `categorize.rs KEYWORD_MAP` list overlapping vendors with different matchers), Settings connection UI. This is the scenario where current architecture bites hardest. Improvement: one `latest_balance_subquery(alias)` helper + unified source enum.

**③ New Copilot response block kind.** Four touchpoints (Rust enum in `agent.rs:851-881`, Zod branch in `artifacts.ts`, card + `renderers.tsx` switch arm, corpus fixture) — well-documented and corpus-tested, but missing the card step degrades to silent `return null` (`renderers.tsx:284`). Improvement: switch-exhaustiveness assert against the Zod union. Mostly fine.

**④ Any new RPC command.** 5–7 hand edits (§2A); parity test catches most omissions *except* "registered but never routed," which passes CI and surfaces as production 404. AGENTS.md's checklist points at a nonexistent crate path. Improvement: `rpc_routes!` macro generating match arms + `SUPPORTED` together; assert count equality.

**⑤ Member-scoped anything (household growth).** Every new metric in `metrics.rs` must be written twice (household + `_for` member variant, e.g. `income_expense_since`:295 vs `_for`:740, `rolling_averages`:413/:787) — the file's dominant shape *is* this cascade. Improvement: parameterize member scope once instead of pairing functions.

---

## 4. Top 10 maintainability risks

### 1. Dual Copilot runtime stacks (legacy TauriRuntime vs AG-UI)
- **Location:** `ui/src/components/copilot/TauriRuntime.ts` vs `agUi/TauriAgUiAgent.ts`/`TauriAgUiRuntime.ts`
- **Problem:** second stack duplicates frame parsing, meta/history/persist helpers, error formatting — already drifted (`parseStoredParts` strictness differs; `backendMessageId` fallback differs).
- **Future impact:** every stream-frame/tooling change costs 2-3×; user-visible inconsistencies between the two chat experiences.
- **Fix:** extract shared `parseCopilotStreamFrame` + one support module; set an explicit sunset date for `?agui=0` legacy path, then delete.
- **Priority:** Critical · **Effort:** Medium

### 2. Competing anomaly detectors writing one column
- **Location:** `finsight-core/src/anomaly.rs` vs `finsight-agent/src/anomaly.rs:38` (+ call site `categorizer.rs:343`)
- **Problem:** two statistics systems, two merchant-identity models; agent ignores dismissals and skips ResetBarrier.
- **Future impact:** users see dismissed anomalies resurrect; any future flag consumer can't trust the column.
- **Fix:** keep core stats engine; demote agent pass to optional confirmation of core candidates, honoring dismissal + barrier.
- **Priority:** Critical · **Effort:** Small-Medium

### 3. Hand-synced command contract surface
- **Location:** `finsight-server/src/dispatch.rs` (match + `SUPPORTED` :909), `finsight-bindings/src/lib.rs`, stale `AGENTS.md:107-109`
- **Problem:** 5-7 edits/command, `SUPPORTED` duplicates the match by convention, parity test regex-couples to rustfmt output, blind spot for registered-but-unrouted.
- **Future impact:** linear tax on every feature; occasional silent production 404s; doc misdirection wastes contributor time.
- **Fix:** declarative `rpc_routes!` macro emitting match + list; typed camelCase arg structs; fix AGENTS.md.
- **Priority:** High · **Effort:** Medium

### 4. `finance.rs` god file + repo-bypass SQL in agent crate
- **Location:** `finsight-agent/src/finance.rs` (loaders :2608-2756), `context.rs`, `reasoning/tools/read.rs`
- **Problem:** domain logic interleaved with raw SQL duplicating repos; balance-priority fragment ×8.
- **Future impact:** schema/source changes require cross-crate lockstep edits; merge conflicts; heavy SQLite-fixture-only testing.
- **Fix:** move loaders into `finsight-core::repos`; split finance.rs along its seven seams; expose shared SQL fragment helper.
- **Priority:** High · **Effort:** Medium-Large

### 5. mockBackend as untyped parallel universe
- **Location:** `ui/src/dev/mockBackend.ts` (1,410 lines)
- **Problem:** reimplements formulas, covers 66/229 commands, no type link to bindings, zero tests.
- **Future impact:** demo/dev mode silently lies; every command addition risks unnoticed drift.
- **Fix:** type responders as `Partial<Record<keyof typeof commands, …>>`; replace reimplemented math with fixtures.
- **Priority:** High · **Effort:** Small

### 6. Cross-boundary magic strings (events, tools, keys)
- **Location:** Rust event emits vs TS listeners (§2A table); tool-name literals; query-key literals
- **Problem:** no shared constants; compiles fine when renamed.
- **Future impact:** silent runtime breakage of import/streaming UX; the hardest class of bug to trace.
- **Fix:** `pub mod event_names` + mirrored TS const + one vitest membership check; tool-name consts; export key factories.
- **Priority:** High · **Effort:** Small

### 7. Error-handling gaps
- **Location:** `dispatch.rs:108-117` catch-all 500; `executor.rs:175` substring classification; `let _ =` discards (`agent.rs:93`, `categorizer.rs:343`); 33 lock-unwraps in `users.rs`; RFC3339 unwraps in row mappers
- **Problem:** misclassified telemetry, swallowed background failures, poisoning/panic paths in auth and row mappers.
- **Future impact:** undiagnosable production issues; rare total-auth-outage mode.
- **Fix:** status hint on `AppError`; typed validation variant; poisoned-lock recovery accessor; graceful date parse in mappers; wrap multi-row writes in transactions.
- **Priority:** High · **Effort:** Small-Medium

### 8. metrics.rs paired-function pattern
- **Location:** `crates/finsight-core/src/metrics.rs` (`_for` family :740-818, repeated income/expense CASE)
- **Problem:** every metric maintained 2-3×; construction logic repeated rather than shared.
- **Future impact:** number drift between household and member views; each new metric doubles.
- **Fix:** single parameterized core per metric family.
- **Priority:** Medium · **Effort:** Medium

### 9. God screens with internal seams
- **Location:** `Copilot.tsx` (incl. duplicated headers :992-1149 and divergent execute mutation), `Settings.tsx`, `Scenarios.tsx`, `Budget.tsx`, `Goals.tsx`; `app.css` 5k lines
- **Problem:** presentation, orchestration, and pure logic co-located; exports-for-tests symptom.
- **Future impact:** highest-churn files become merge bottlenecks and regression magnets.
- **Fix:** cut along existing named inner components (mostly mechanical); unify headers; move `stabilizeStreamingMarkdown` to utils.
- **Priority:** Medium · **Effort:** Medium

### 10. Duplication boilerplate in hooks
- **Location:** 178 unwrap copies + 20 backend-guard copies across `api/hooks/*`; 4 private unwraps; drawer-seed effect ×5 with eslint-disables; focus-param effect ×4; date/money format re-implementations
- **Problem:** mechanical repetition from AI generation without a shared factory.
- **Future impact:** inconsistent error toasts; each contract tweak touches dozens of sites.
- **Fix:** `unwrap<T>()` in `client.ts`, one `mutationWrapper` (guard+onError), `useFocusParam`, shared seed-effect hook.
- **Priority:** Medium · **Effort:** Small

---

## 5. Leave this alone for now

- **Per-user runtime registry** (`finsight-server/src/registry.rs`) — single-flight bootstrap, idle eviction sparing SSE subscribers, documented races. Genuinely good; don't touch.
- **Zod↔Rust block parity via shared fixture corpus** — working well; add only a switch-exhaustiveness assert.
- **PWA persistence layer** (`pwa/persist.ts` + AuthGate purge) — single persister, clean logout semantics.
- **`ResetBarrier`** — exemplary; extend usage (risk #2), don't redesign.
- **specta exact-pinned RCs** — correct choice for codegen determinism.
- **`tokio` features="full", minor Cargo dep-centralization inconsistencies** — churn without payoff.
- **The `(" ramen", "dining")` keyword-table hack** (`categorize.rs:47-51`) — documented and tested; fine.
- **`AppError` refusal of blanket `From<Display>`** — good discipline, keep.

---

## 6. Final assessment

### Overall maintainability — **6/10**
Strong documentation/test culture undermined by pervasive hand-synced duplication.

### Architecture health — **7/10**
Layer boundaries (api/server/bindings split, parity tests, registry) are real and enforced; violations are localized (agent crate bypassing repos).

### Extensibility — **6/10**
Blocks and providers extend cleanly; sync sources, metrics, and commands pay a multi-site tax.

### Testability — **6/10**
Broad coverage on both sides, but time isn't abstracted (189 `Utc::now()` sites), logic+I/O mixed in agent crate, mockBackend untested, and frontend tests mock hooks so contract breaks pass green.

### Technical debt trajectory — **Manageable**
Drifting toward *Concerning* if the dual-runtime and mock-backend duplication lives another quarter.

### Biggest long-term risk
**Contracts without a single source of truth** — command registration, event names, tool names, stream frames, and the mock backend are each declared by hand in multiple places. Every future feature multiplies these copies; they fail silently at runtime, not at compile time or in tests.

### At 2× the feature set
Development stays functional but the per-command checklist and god-file merge conflicts become the daily tax; expect 2-3 "silent drift" bugs per quarter (stale caches, broken streams, resurrected anomalies) that each cost a day to trace.

### At 10×
The hand-synced surfaces dominate: adding features means editing 8-12 locations across languages; Copilot runtime forks diverge into visibly different products; the agent/domain crates become effectively unrefactorable (>4k-line files with embedded SQL), and onboarding requires tribal knowledge the (already stale) docs don't capture.

### Top 5 actions (in order)

1. **Single-source the cross-boundary contracts** (Small): `event_names` consts + TS mirror + guard test; tool-name consts; typed mockBackend responders; export query-key factories. Highest leverage per hour spent.
2. **Consolidate the Copilot runtime** (Medium): shared `parseCopilotStreamFrame`/support module, fix the `execute_action_bundle` divergence now (it's a live cache-staleness bug), schedule legacy-path deletion.
3. **Unify anomaly detection + transactional writes** (Small-Medium): one stats engine honoring dismissals; transactions around categorizer/executor/anomaly writes.
4. **Cut the command-registration tax** (Medium): `rpc_routes!` macro killing the `SUPPORTED` duplicate, typed arg structs retiring the regex parser, fix AGENTS.md's phantom `finsight-app` references, delete the leftover directory and unused plugins/deps.
5. **Split `finance.rs` starting with its SQL loaders** (Medium-Large): loaders → `finsight-core::repos`, shared balance-priority fragment, then seam-split by domain. Do this before adding the next finance feature, not after.
