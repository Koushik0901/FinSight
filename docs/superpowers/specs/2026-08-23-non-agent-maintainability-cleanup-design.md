# Non-Agent Maintainability Cleanup — Design

**Date:** 2026-08-23
**Scope:** One spec, 5 sequential phases, non-agent debt only (Bound B)
**Gates:** Gate C — `cargo test --workspace` + `cd ui && npx vitest run` + `npx tsc --noEmit` + parity/corpus tests + 7-day PWA purge; zero silent `[]` fallbacks; typed `mockBackend`
**Style:** Aggressive deletes (one commit per deletion, no flag) + mechanical move commits separate from logic
**Context:** `docs/audits/2026-08-22-maintainability-architecture-audit.md` (overall 6/10, health 7/10) + `eval/FINDINGS.md:216-240` + `docs/phase6-final-report.md` + `TODO.md:11-43` (all feature ✅, debt is duplication)

## 1. Scope & Phasing (Approach A: Correctness-first)

**5 phases, each shippable and independently verified on `samples/` numbers.** Order front-loads user-visible trust, then leverage:

1. **Phase 1 — Metrics correctness** — `metrics.rs:413/787` robust expense, unified EF-months, headline numbers truthful
2. **Phase 2 — Contracts single-source** — `rpc_routes!` kills `SUPPORTED` dupe `dispatch.rs:909-1167`, `sink::event_names` ↔ TS mirror, typed `mockBackend.ts` 66→229
3. **Phase 3 — Household/member scaling** — parametrize `metrics.rs` (`_for` → `scope: Option<MemberId>`), `latest_balance_subquery(alias)` ×8, `Budget.tsx:185` shared overlay
4. **Phase 4 — Robustness** — `AppError::http_status()` (`dispatch.rs:108`), poison-safe `users.rs`, transactional writers, no `let _ =` swallows
5. **Phase 5 — Hygiene / god files / dead code** — split `Settings.tsx:855`/`app.css:5032`/hooks 178×, delete `finsight-app` husk, ghost plugins, `sample.rs` leak, fix `AGENTS.md:107`

**Invariants (hold after every phase):** Gate C green; deletes are one-commit-per-deletion for revert; Bound B — `finsight-agent` touched only where it leaks core correctness (`anomaly.rs` second detector honoring `anomaly_dismissed` + `ResetBarrier`, shared CASE helper, `categorizer.rs:343` barrier); out-of-scope = new sync source (Plaid), new LLM provider, new UI features (only unblock them).

## 2. Architecture Delta

**Before (hand-synced, drift-prone):** `finsight-api` body → `finsight-bindings` wrapper → `collect_commands! lib.rs:59-289` → `dispatch.rs:120-904` match → `SUPPORTED dispatch.rs:909-1167` hand-ordered copy → `bindings.ts` (5-7 edits/cmd; registered-but-unrouted passes CI as 404). Events/tools/keys as string literals in Rust (`copilot_chat.rs:1058`, `import.rs:149`) vs TS (`TauriRuntime.ts:453`) — rename compiles, stream dies. `metrics.rs:2617` every metric ×2 + CASE ×8 (`accounts.rs:925`, `finance.rs:2611,2713`, `context.rs:710,1097,1136`, `read.rs:27,940`). `AppError` typeless → `dispatch.rs:108` maps everything to 500.

**After (single-source + thin seams):** `rpc_routes!(api, events, cmd, p, c: get_month_totals(api,…), set_budget(api,…, arg("categoryId"), arg("amountCents")), …)` one table generates both `match` and `SUPPORTED`; typed `arg("camelCase")` structs retire regex `tests/parity.rs` → `assert_eq!(SUPPORTED.len(), collect_commands!().len())`. `finsight_api::sink::event_names` (`COPILOT_STREAM_FRAME`, `IMPORT_PROGRESS`…) + generated `ui/src/api/eventNames.ts` mirror + vitest membership guard; tool-name consts; query-key factories from `hooks/invalidation.ts` (`actionBundleKeys.pending(null)` replaces literals `BottomNav.tsx:55`/`Sidebar.tsx:65`). `metrics.rs` single `fn metric_core(conn, scope)` + `repos/balance.rs::latest_balance_subquery(alias)` helper; `AppError::http_status()` → `400/401/409/422/500`. Core owns SQL; agent callers via `repos` for touched loaders. No wire change — `invoke("cmd", {camelKeys})` → `httpBackend.ts` → `POST /api/rpc/{cmd}` → `dispatch.rs` generated → `finsight-api` body (`run(db, |conn|…)`) unchanged; events `FrameSink::emit(event_names::X)` → SSE `/api/events`.

**Visual:** Before/After split shown in brainstorm `design-s2-arch.html` — contracts, metrics, robustness, seams.

## 3. Phase 1 — Metrics Correctness

**Goal:** Headline surplus/savings/runway/EF stops lying on a one-off purchase.

- `crates/finsight-core/src/metrics.rs:413 rolling_averages` display path: replace raw `AVG(expense last 90d)` with `robust_monthly_expense_cents(conn, scope)` / `typical_monthly_expense_cents` (median-month, anomaly-excluded — `FINDINGS.md:217` notes it already exists for projections, used by `run_emergency_fund_scenarios` etc.). Keep 90-day mean as fallback when `<2 full months` history, set `metrics.is_estimated = true` so `HealthScoreCard` can show ⓘ.
- Unify EF-months: single `metrics::emergency_fund_months(conn, scope)` (EF-eligible pool — `accounts.is_emergency_fund = 1` / liquid, never total liquid). `Today.tsx:332` runway, `HealthScoreCard.tsx:148`, `build_snapshot`, and `run_emergency_fund_scenarios` all call it — fixes `FINDINGS.md:226-240` duality where `EmergencyFundScenarios` reported EF-eligible balance but measured total liquid (4.5 vs 2.5 months).
- `Today.tsx:246-332` and `useFinancialMetrics` only consume corrected row; once unified remove `HealthScoreCard:148-154` `// deliberately has no ⓘ` hide hack — now the inspector's number matches the card.

**Edge cases:** Cold start `<2 months` → fallback + estimated label; EF-eligible 0 → 0 months + `missingData: "no EF-eligible accounts"` (no div/0); all months anomalous → median still defined; if none, fallback + warning.

**Verification (Gate C):** Add `typical_monthly_expense_ignores_one_off_spike` display-path variant + `emergency_fund_months_unified` test (`assert_eq!(reported_months, pool / robust_expense)`); existing `phase6` anomaly/robust tests stay green; manual `samples/` spot: May income ≤ $4k, savings + runway stable when a $2,500 one-off is inserted; `cargo test --workspace` + `vitest` green.

## 4. Phase 2 — Contracts Single-Source

**Goal:** Kill 5-7 edits/cmd and silent stream/404 drift with no wire change.

- **Macro:** At bottom of `crates/finsight-server/src/dispatch.rs`:
  ```rust
  rpc_routes!(api, events, cmd, p, c:
    get_month_totals(api, events, cmd, p, c: ),
    set_budget(api, events, cmd, p, c: arg("categoryId"), arg("amountCents")),
    record_goal_contribution(api, events, cmd, p, c: arg("id"), arg("amountCents"), opt("note")),
    // spec-audit: verify every generated arg is camelCase per bindings.ts
    // UNSUPPORTED("openPath", reason = "client-filesystem path")
  );
  ```
  Expands to `match cmd { "getMonthTotals" => { let categoryId: String = arg(&p, "categoryId")?; … } }` + `const SUPPORTED: &[&str]`. Hand-ordered `SUPPORTED dispatch.rs:909-1167` deleted. Retire regex `parity.rs` parser → `assert_eq!(SUPPORTED.len(), finsight_bindings::COMMAND_COUNT)` + cmd count equality. Fix `AGENTS.md:107-109` phantom `finsight-app/src/commands/` → `finsight-bindings/src/commands/`.

- **Cross-boundary constants:** `crates/finsight-api/src/sink/event_names.rs` — `pub const COPILOT_STREAM_FRAME: &str = "copilot-stream-frame"` etc.; producers use `event_names::COPILOT_STREAM_FRAME`; CI `rg -n '"copilot-stream-frame"'` fails if literal remains. Generated mirror `ui/src/api/eventNames.ts` (via `export_bindings`) + vitest `expect(Object.values(eventNames)).toContain("import-progress")`. Query keys: export factories from `ui/src/api/hooks/invalidation.ts` — `actionBundleKeys.pending(null)` replaces literals `BottomNav.tsx:55`/`Sidebar.tsx:65` + `simplefinKeys` re-typed `Inbox.tsx:437`.

- **mockBackend:** `ui/src/dev/mockBackend.ts:1410` — change `Record<string, (args:any)=>any>` → `Partial<Record<CommandName, (args: CommandArgs[K])=>Promise<CommandResult[K]>>>` (types from `bindings.ts`). Reimplemented business math (`project_goal_growth:1159`, balance timeline `1239`, month closes) → fixtures `ui/src/pwa/fixtures/*.json`; unimplemented cmds throw in dev (not silent `[]`); add 1 vitest proving typed responder compiles; zero tests → one.

**Review gate:** Spec audit must verify every `arg("…")` is `camelCase` per `bindings.ts` — any `snake_case` drift flagged.

## 5. Phase 3 — Household/Member Scaling

**Goal:** Adding a metric or sync source costs 1 edit, not 2-8.

- **`metrics.rs` parametrization:** `fn income_expense_since(conn, scope: Option<MemberId>)` etc. — `scope=None` household (no join), `Some(id)` → `JOIN account_owners ON …` with `share_bps` split (joint 50%). Replace `income_expense_since:295/740`, `rolling_averages:413/787`, `emergency_fund_months` pairs. Single `fn metric_core` helper, `rolling_averages` delegates. File shrinks ~40%. Tests: household + member + joint-split + zero-member parity + `sum(member views) == household`.
- **Balance helper:** `crates/finsight-core/src/repos/balance.rs::latest_balance_subquery(alias) -> String` returning `ORDER BY CASE alias.source WHEN 'simplefin' THEN 1 WHEN 'ledger_recomputed' THEN 2 ELSE 3 END` — replaces 8 copies in `repos/accounts.rs:925`, `finance.rs:2611,2713` (touched under Bound B), `context.rs:710,1097,1136`, `read.rs:27,940`. Adding Plaid = 1 enum variant + 1 helper edit.
- **UI:** `Budget.tsx:185` `scopeMemberId` member overlay → shared `useMemberScope()` hook; budgets remain household-level in v1 (unchanged), just overlay reuses metric core — prepares future per-envelope-per-person without new SQL.

## 6. Phase 4 — Robustness

**Small, high-leverage, localized:**

- **HTTP:** `crates/finsight-api/src/error.rs::AppError::http_status() -> u16` mapping `Validation→400, Auth→401, Conflict→409, Unprocessable→422, Internal→500` — fixes `dispatch.rs:108-117` catch-all 500 that pollutes telemetry. `crates/finsight-agent/src/executor.rs:175` `contains("validation:")` → typed `AppError::Validation` variant.
- **Poison/parse:** `crates/finsight-server/src/users.rs` 33× `Mutex<Connection>.lock().unwrap()` → `lock().unwrap_or_else(|e| e.into_inner())` + `log::warn!` (auth never poisons until restart). `repos/connections.rs:109,145` + `repos/transfers.rs:43-103` RFC3339 `unwrap()` → `parse().unwrap_or_else(|e| { log::warn!(…); skip row })` — one bad timestamp never panics list.
- **Silent → visible:** `crates/finsight-agent/src/agent.rs:93 let _ = recipe_runner::run_due_recipes` + `categorizer.rs:343` anomaly discard → `log::error!` + Inbox `data.startup_warnings`-style alert; anomaly writes honor `ResetBarrier` lease (Bound B exception). `context.rs:1102,1142` empty-on-error → return `Result` and surface via toast, not blank. Multi-row writers (categorizer per-row, executor bundle statuses) → `BEGIN; … COMMIT;` transactions like `repos/rules.rs:90-104` correct pattern. `events.rs:49` broadcast `Lagged` → count dropped frames, emit `lag_warned` metric once per burst.

## 7. Phase 5 — Hygiene / God Files / Dead Code

**Mechanical splits (pure moves, no logic) then aggressive deletes (one commit each):**

- **God files:** `ui/src/screens/Settings.tsx:855` (12 sections) → `ui/src/screens/settings/*.tsx` slices (Accounts/Currency/Appearance/Data/Shortcuts) — header + state unchanged. Hooks: `api/hooks/*` 178× `unwrap` boilerplate + 20× `getBackend()` guard → shared `ui/src/api/client.ts::unwrap<T>()` + `ui/src/api/hooks/_factory.ts::mutationWrapper` + `useFocusParam` + `useDrawerSeed` (kills 5× `eslint-disable`). `styles/app.css:5032` → `app.css` (utilities) + `copilot-shell.css` (284 copilot selectors). `metrics.rs` already thinned in Phase 3 — no extra split.
- **Dead code deletes:** Untracked `crates/finsight-app/tests/fixtures/` husk + fix `AGENTS.md:107` phantom path; `finsight-core/src/sample.rs:1130` + `seed.rs` → `#[cfg(any(test, feature="dev-seed"))]` (currently compiled into release — unlike `testing.rs`); `src-tauri/Cargo.toml:25-27` ghost plugins/capabilities (dialog/opener/notification declared never registered — `FilePicker.tsx:24` throw branch) → delete; unused `npm: @assistant-ui/react-markdown` (0 imports), stale `@nivo/*` comment `App.tsx:62`, likely `plugin-opener`.
- **Phase 1-5 rollout:** Phase order is rollout — no flag, no migration. `migrations/` unchanged (next = `V064`). Each phase commit Gate C green; add `rg -n 'SUPPORTED.*=' --quiet` fails if dupe returns + `rg -n '"copilot-stream-frame"'` literal check.

**Risks:** Mechanical splits can move bugs — mitigate by pure-move commits (no logic) separate from delete commits; each delete is one commit for revert; Phase 3 metric core change is largest — needs `samples/` manual spot.

## 8. Verification & Acceptance

**Per-phase acceptance (all require Gate C green):**
- P1: `typical_monthly_expense_ignores_one_off_spike` display variant passes; `emergency_fund_months_unified` passes; `samples/` May income stable, headline surplus no longer $163 vs $1,900.
- P2: `SUPPORTED.len() == collect_commands!().len()`; `rg` literal checks pass; `mockBackend` typed vitest passes; unimplemented cmd throws observed in dev.
- P3: `sum(member views) == household` + joint 50% split test passes; `latest_balance_subquery` used at all 8 sites (grep 0 remaining); adding a dummy source is 1 edit in tests.
- P4: `AppError::http_status` unit test (400/401/409/422/500); `users.rs` poison-recover test (lock poisoned → next request succeeds); malformed RFC3339 row skipped test; transaction atomicity test (mid-batch failure → no partial rows).
- P5: `tsc --noEmit` 0 errors after splits; `cargo b --release` no longer contains `sample` symbols ( `nm` check); `AGENTS.md` path correct; `npm prune` removes unused deps.

**Leave alone (audit §5):** `registry.rs` per-user runtime single-flight + idle eviction, Zod↔Rust block parity corpus, `pwa/persist.ts` + `AuthGate` purge, `ResetBarrier`, specta pinned RCs, `AppError` refusal of blanket `From<Display>`, `(" ramen","dining")` hack.

## 9. Alternatives Considered

**B) Contracts-first:** Highest leverage early but delays user-visible fix (surplus lie) to phase 2-3 — rejected. **C) Primitives-first big bang:** Fastest total dedup but largest blast radius / hardest review — rejected. **Chosen A** front-loads trust, keeps each phase <~500 lines, lets contract work ride Gate C harness.

## 10. Open Questions

None — clarified: one spec (Q1 A), Gate C (Q2 C), aggressive deletes (Q3 B), Bound B agent-excluded-ish (Q4 B), sequencing A.
