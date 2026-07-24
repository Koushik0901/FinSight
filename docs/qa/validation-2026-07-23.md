# FinSight full-app QA — 2026-07-23

Validation ledger for the whole-app QA pass. Environment: real `finsight-server`
on `:8674` (throwaway scratchpad data dir), signed in as `tester`, driven through
the in-app browser. One record per screen; each screen graded **Verified**,
**Repaired & Verified**, or **Blocked**.

---

## PR split (2026-07-24)
This session's work is now separated into reviewable PRs:
- **[#83](https://github.com/Koushik0901/FinSight/pull/83)** — QA validation branch (`chore/validate-shipped-issues`): the 14/14-screen validation, auth redesign, the critical `build_baseline` SQL fix, and the responsive / privacy-blur / horizon-label UI fixes.
- **[#84](https://github.com/Koushik0901/FinSight/pull/84)** — `feat/persistent-sessions` off `main`: **persistent sessions + sign out other devices** (below). Isolated from the QA work; `cargo test -p finsight-server --lib` = 59 passed.

## Dead / non-functional element hunt (2026-07-24, ongoing standing goal)

Reported: the Today net-worth time-range selector (1M/3M/6M/1Y/All) "just flickers, nothing changes."

**Root cause (fixed):** the selector *is* wired (setRange → days → `useNetWorthHistory(days)` → chart), and 1M genuinely changes the trend + chart (2 points, "$1,977" vs 4 points, "$6,704"). Two things made it feel dead: (1) **the query had no `placeholderData`, so each range switch dropped the data to `[]` for a frame — the chart blanked and the trend chip flashed "Baseline building" (the flicker)**; (2) the seed only has ~3 months of net-worth snapshots, so 3M/6M/1Y/All coincide (no data beyond 3M — a data limitation, correct behavior, not a bug). Fix: `placeholderData: (prev) => prev` on `useNetWorthHistory`. Verified live: after clicking 1M the chart **retains the previous data** (never blanks to "Baseline building"), then transitions smoothly. Every click now gives feedback (active button + trend label update immediately). Committed f8d1405 on #83.

**Same flicker class found & fixed:** `useAccountBalanceTimeline` drives the account **Balance history** card's 3M/1Y selector and also lacked `placeholderData` — same fix applied. (Cash-flow horizon selector already had it; no other selector-driven chart queries remained.)

**Swept for other dead elements — comprehensive pass, all clean:**
- **Static (reliable, whole codebase):** no empty `onClick={() => {}}` handlers; no dead nav targets (`/onboarding` is special-cased in App.tsx, all sidebar routes defined); **no dead-state controls** — a scan of every `const [v, setV] = useState` across all screens found zero where `v` is set but never read. (`useAccountBalanceHistory` / `useAccountBalanceSparklines` are dead *hooks* — referenced only by test mocks — dead code, not user-facing.)
- **Live per-screen (9 screens):** Today, Reports, Budget, Goals, Cash flow, Recurring, Categories, Inbox, Accounts — **every interactive control functional.** Verified: Reports period + view tabs; Budget 4 sort modes ("By group" was the pre-selected default); Goals filter tabs + what-if goal selector (`setScenarioGoalId`, a no-op with a single goal); Cash flow 30/60/90d; Recurring view toggles + "I cancelled this" (opens the date form — no data mutated); Categories month/year; account cards navigate.
- **Every auto-detector "dead" flag was a false positive** — a pre-selected default, single-item data, or an in-place form the detector's DOM-delta missed. Manual/code verification confirmed each is wired.

**Conclusion:** the reported bug (net-worth range flicker) is fixed, plus one sibling (balance history). No other genuinely dead elements found across 9 screens + whole-codebase static analysis. The "feels dead" cases are data-limited (ranges coincide on <6mo of seed history; single-goal/single-account selectors) — correct behavior, not bugs. Mutating action buttons (create/edit/delete/apply/etc.) were exercised in this session's earlier per-issue functional QA rather than blindly auto-clicked.

## NEW FEATURE (2026-07-24) — Persistent sessions + sign out other devices (PR #84)

User request: *"use sessions … similar to how immich uses, because we don't want to log in every time."* Previously sessions were in-memory only (`sessions.rs`), so every server restart forced a re-login. Implemented server-master-key–wrapped persistent sessions:

- **`crypto.rs`** — `load_or_create_server_key` (SMK: 32 bytes from **`FINSIGHT_SESSION_KEY`** env, else `<data_dir>/session.key`, generated 0600), `wrap/unwrap_key_with_server_key` (XChaCha20-Poly1305), `hash_session_token` (SHA-256).
- **`users.db`** — new `sessions` table (`token_hash` PK = SHA-256(token) so a stolen DB never yields a live cookie; `wrapped_db_key` = DB key wrapped under SMK; `expires_unix`) + CRUD (`persist/recover/slide/delete/delete_user/purge`).
- **`SessionStore`** — optional persistence (in-memory `Default` unchanged for tests). `create` mirrors to disk; `get` recovers on an in-memory miss (unwraps with SMK, **slides expiry**); `remove`/`remove_user` **purge the row** (logout/recover/delete → no resurrection); startup purges expired.
- **Auth handlers unchanged** (persistence is encapsulated). Recover already sweeps sessions → persisted rows invalidated on the compromise path.

**Security tradeoff (documented in code):** SMK + `users.db` together can decrypt an *active* session's DB at rest — the Immich-style posture the user asked for, weaker than the prior "unwrapped keys never touch disk." Mitigation: `FINSIGHT_SESSION_KEY` lets an operator keep the master key off the data volume.

**Tests:** +9 unit tests (crypto ×3, sessions ×6 incl. the load-bearing `removed_session_does_not_resurrect_on_restart`); **`cargo test -p finsight-server --lib` = 58 passed, 0 failed.** Server rebuilt; `session.key` + `sessions` table confirmed created on the live box.

### Sign out other devices (added to PR #84)
Persistent sessions mean other devices stay signed in across restarts, so a "sign out everywhere but here" control is now meaningful:
- **`POST /api/auth/sign-out-others`** ([auth.rs](../../crates/finsight-server/src/auth.rs)) revokes every OTHER session for the user (in-memory + persisted rows) while keeping the caller's — `SessionStore::remove_user_except` + `UsersDb::delete_user_sessions_except`.
- Surfaced as a **"Sign out other devices"** button in Settings → Account ([Settings.tsx](../../ui/src/screens/Settings.tsx), `auth.signOutOtherSessions`).
- +1 backend test (`sign_out_others_keeps_current_and_purges_the_rest_across_restart`), +1 Settings test. Endpoint confirmed live (401 unauthed, not 404); button confirmed in the shipped bundle.

**Follow-up (noted, not built):** password-*change* (vs recover) leaves persisted sessions valid — defensible (SMK-wrapped key is password-independent). Recover already sweeps them (compromise path).

**LIVE PROOF — DONE (2026-07-24):**
- **Persistence:** user logged in (persisted row written: 32-byte token hash, 72-byte SMK-wrapped key). Server killed + restarted as a fresh process (in-memory store empty) → the browser's existing cookie returned **provider 200 / list_accounts 200 with no re-login**. The session was recovered from disk. ✅
- **Sign out other devices:** the button renders in Settings → Account; `POST /api/auth/sign-out-others` → **200 `{signedOut: 0}`** with only this session present, and the current session stays valid afterward. Multi-device removal (2 others purged, current kept, survives restart) is covered by the unit test. ✅

---

Per-screen checklist (applied uniformly):
- Empty / loading / error / populated states
- Data create / edit / delete / persist (reload survives) / refresh
- Navigation, validation, feedback (toasts/inline), error recovery
- Responsive at 375 / 768 / 1280
- A11y & labels; privacy/local-first (amount blur in privacy mode)
- UI/UX polish — hierarchy, spacing, consistent tokens, no slop/placeholder

Screen priority (money-critical first): Today · Accounts · Budget · Recurring ·
Cash flow · Reports · Categories · Goals · Scenarios · Inbox · Path back ·
Copilot · Rules & agents · Settings.

---

## Prior work carried in (already verified this session)

Scenario/explanation features validated against the real backend (see PR #83):
- **#72 apply** — promote→apply wrote a real `-$35,000` planned-transaction row (0→1); scenario unchanged. **Verified (real backend).**
- **#73 revise** — recompute flips verdict when the purchase is dropped; original preserved. **Verified (real backend).**
- **#71 explanations** — goal ETA (75.0 mo) rendered in real UI; scenario explanation returns real `provenance.rs` tradeoffs. **Verified (real backend).**
- **Critical fix** — `build_baseline` SQL (`FROM transactionsWHERE`) 500'd every scenario command; fixed + unit-tested + confirmed 200 live.
- **Auth screens redesign** — setup/login/recovery ported; invisible-input + label-notch + synced/smoothed showcase fixes. **Verified live (desktop/mobile/focus).**
- **Repairs**: #72/#73 stale-panel state, Settings responsive overflow, dev-mock fidelity.

Documented as unit-covered (not live-fired): #75 alert generation (sync-scheduler-only), CSV import file-picker widget click-test.

---

## Data foundation

_(seed plan + import record appended once seeded)_

---

## Per-screen records

_(appended as each screen is validated)_

### Today — `/` — Verified (with notes)
- **Data/workflows:** 3 accounts + 98 seeded txns (4 months), transfers, price step, July anomaly.
- **Numbers correct:** net worth $30,904 (= 11,794.14 + 20,500 − 1,390); Liquid $32,294; Credit $1,390; Runway 260d @ $3,720/mo — all internally consistent. Future-dated (Jul-28) txns correctly excluded from balances.
- **States:** net-worth history shows the legit "still building" empty state (all data same-day; snapshots accrue daily — populated chart not testable with fresh data). Morning briefing, smart sweep, upcoming-bills table render.
- **Items to verify (not yet graded defects):**
  1. Agent "while you were away" says *"Nothing needs your attention"* despite 98 uncategorized manual txns — is manual data auto-categorized, or should it be flagged? (checking Inbox)
  2. "Due in the next two weeks" lists **ACME PAYROLL (income) $5,200** in a table that reads as bills-due — possible mixed-signal UX.
- **Responsive/privacy:** pending (batched pass).

### Inbox — `/inbox` — Verified (with candidate defects)
- **Renders:** prioritized list — HIGH "87 transactions need categorizing" (correctly surfaces uncategorized manual txns), MEDIUM emergency-fund nudge. "Help me work through these" CTA present.
- **CANDIDATE DEFECT A (UX, med):** Today's "Agent · while you were away" says *"Nothing needs your attention right now"* while Inbox simultaneously shows 87 needs-review + EF gap. Likely "while you were away" = new-since-last-visit vs Inbox = standing actions, but the flat "nothing needs attention" contradicts the Inbox and reads wrong. → investigate wording/logic.
- **CANDIDATE DEFECT B (metric, med):** EF nudge = *"covers less than 1 month of expenses"* with $32,294 liquid (incl. $20,500 Savings) and $3,720/mo burn (~8 months of runway). Appears to key off the $0-funded EF **goal** rather than actual liquid savings → potentially misleading. → trace emergency_fund_months source.
- **To do:** click "Review transactions →" (categorization flow), responsive, empty state.

## Candidate defects queue (to reproduce + fix)
1. [UX] Today "while you were away" vs Inbox inconsistency (Inbox A).
2. [metric] EF-months uses $0 goal vs actual liquid (Inbox B).
3. [verify] Marking a cryptic-named recurring txn (e.g. "TFR TO SAVINGS") as a transfer via the UI — does it leave the Recurring list? (transfer_override → is_transfer → detect_recurring path).

### Branch code review (background agent) — resolved
- Result: **0 high / 1 med / 2 low**; no functional/data-loss bugs. SQL fix complete, authScene listener cleanup complete, auth wiring + CSS scoping + Scenarios fixes verified.
- MED (a11y): JS showcase animations ignored prefers-reduced-motion → **fixed** (count-ups/lines/bg/parallax all static under reduced motion).
- LOW: RecoverScreen missing pw length check → **fixed** (≥10). LOW: index.html FOUC → intentional/documented.

### Candidate defect resolutions
- **Inbox B (EF <1 month)** → **seed artifact, not a bug.** metrics.rs:544 only counts `emergency_fund_eligible` accounts; my seeded Savings omitted the flag. Marked Savings EF-eligible; metric now reflects actual liquid. (Advisor's seed-artifact trap — 4th time.)
- Recurring transfer misclassification, Netflix price-change-not-flagged, "Shell as subscription" → all **seed artifacts** (transfer pairing not run on manual data; only 2 old-price charges vs the ≥3 baseline requirement; unrealistically-identical gas). Code correct.
- **Still open — Inbox A:** Today "nothing needs attention" vs Inbox 87-needs-review wording inconsistency → to reproduce/decide next.

### Accounts — `/accounts` — Verified
- Totals correct: Connected $32,294, Manual $0 (empty state handled), Liability $1,390, Net worth $30,904; per-account balances exact.
- **Edit CRUD + persist:** set Checking nickname "Main chequing" → persisted (confirmed via RPC), drawer closed. Minor: no success toast (drawer-close is the only feedback).
- Add-account entry point present; by-owner attribution ("Unassigned / shared") renders.

### Budget — `/budget` — Verified
- Initially showed "$0 spent" — **seed artifact** (manual txns uncategorized; CSV import auto-categorizes). Categorized 79 txns via RPC → screen now correct.
- SPENT SO FAR **$3,305** breaks down exactly: Housing 1,800 / Groceries 822 (incl. July anomaly) / Dining 152 / Transport 156 / Utilities 164 / Subscriptions 49 / Shopping 117 / Health 45. Groups (Daily/Fixed/Lifestyle/Wellbeing) sum correctly.
- 5-month spending history + "YOUR TYPICAL" baseline render; July grocery anomaly visible. Unbudgeted-envelope + empty (Gifts/Travel) states handled.
- **Minor UX:** top "0% spent / $0 left" (of the unset $0 budget) reads oddly beside "$3,305 spent". Budget-set CRUD test pending.

### Recurring — `/recurring` — Verified (1 finding to reproduce)
- Real detected recurring: bills (Rent, Hydro, Bell), subscriptions (Netflix, Spotify, iCloud, Fitness→Health), income (Payroll). Grouping, counts, next-date projection, MONTHLY COMMITTED render. Amounts negative (outflow) correctly.
- **#75 lifecycle (real backend):** "Mark as trial" on Netflix → date input → Save → **"Trial ends Jul 30, 2026" badge** + Edit trial / Clear trial appear. Works end-to-end on a real detected subscription.
- **FINDING (needs real-flow repro):** transfers I marked via `apply_transfer_verdict_to_similar` (RPC) still appear in Recurring — "VISA PAYMENT" shows as a **Subscription**, "TFR TO SAVINGS" as a Bill — inflating MONTHLY COMMITTED ($2,693) and the sub count (5). Root: the RPC set `transfer_override` but `is_transfer` stayed `false`, and `detect_recurring` reads `is_transfer` (recurring.rs:249) gated by a display-name keyword (line 401). In real usage pair_transfers auto-flags cross-account pairs on CSV import (bypassed by manual seed). **To reproduce via the real Transactions "mark as transfer" UI** and confirm whether it re-scans / propagates — if not, it's a real bug (user-confirmed transfer stays in Recurring). Deferred, documented, not filed as shipped yet.
- **Pending:** "Dismiss" affordance test; #58 price-change surfacing (needs ≥3 old-price charges — extend seed); responsive.

## Remaining screens to QA
Cash flow · Reports · Categories · Goals · Scenarios · Path back · Copilot · Rules & agents · Settings. Plus: budget-set CRUD, transfer-verdict real-flow repro, price-change seed extension, responsive/privacy batch.

### Cash flow — `/cashflow` — Verified
- SAFE TO SPEND $32,058 = lowest projected balance (Jul 30) − buffer; consistent.
- **Buffer control:** setting buffer $5,000 → safe-to-spend recomputed to exactly $27,058 (−$5,000). Horizon toggle (30/60/90d), test-purchase input, projected-balance chart, upcoming dated events, and "GOOD TO KNOW" just-after-window bill warnings (#55) all render.
- Transfer mis-classification propagates here (TFR legs in Upcoming; VISA PAYMENT as an obligation) but paired legs net out in the liquid projection — same cross-cutting artifact as Recurring.

### Reports — `/reports` — Verified (1 candidate to trace)
- Savings rate 38%, avg monthly spend $3,601, runway 9mo; monthly overview chart; top categories + top merchants all numerically correct (Housing $7,200 = 4×1,800; Subscriptions $162; Health $180 = 4×45; etc.). Month/Quarter/Year/All-time toggles + Export present.
- **Cross-cutting artifact:** "Uncategorized $2,600 (8 txns)" in top categories = the transfers (TFR $2,000 + VISA PAYMENT $600) counted as spend — same is_transfer root as Recurring/Cash flow.
- **CANDIDATE (trace):** net worth shows **$31,054** here vs **$30,904** on Today — $150 gap (= one VISA PAYMENT). Possibly an as-of-date / future-txn-inclusion difference between the two net-worth surfaces, or a real inconsistency. → trace both sources.

## QA progress: 7/14 screens
Done: Today, Inbox, Accounts, Budget, Recurring, Cash flow, Reports (all Verified; findings queued).
Remaining: Categories, Goals, Scenarios, Path back, Copilot, Rules & agents, Settings.
Open findings: Recurring transfer-verdict real-flow repro; Today/Inbox "nothing needs attention" wording; net-worth Today-vs-Reports $150 gap; budget-set CRUD; #58 price-change seed extension; responsive/privacy batch.

### Categories — `/categories` — Verified
- THIS MONTH $3,305 with per-category vs-June deltas (Groceries $822 vs $362 = anomaly visible), spending-type tags (Fixed/Investments/Savings/Guilt-free — conscious-spending model), Manage + New category, This-month/vs-average/Year toggles. Empty categories (Gifts/Travel $0) handled. PACE 0% (no budget set — consistent with Budget).

### Net-worth Today-vs-Reports candidate — RESOLVED (not a bug)
- Empirically: net worth now = **$31,054** per BOTH the metrics layer AND the accounts sum (they agree). Reloaded Today → also $31,054. The earlier $30,904 was a stale reading taken before an intervening balance change. No source inconsistency. (Verified before filing — advisor discipline.)

## QA progress: 8/14 screens verified
Done: Today, Inbox, Accounts, Budget, Recurring, Cash flow, Reports, Categories.
Remaining: Goals, Scenarios, Path back, Copilot, Rules & agents, Settings.

### Goals — `/goals` — Verified
- Emergency Fund (build-balance, on track): ETA Oct 2032 / 75 months ($30k ÷ $400/mo exact), $0 of $30,000 progress. Horizon timeline, what-if slider (+$0..$1,500), Pause/Explain/Adjust all render.
- **Compound Growth projector works on the real backend** (was em-dashes in the mock — confirmed mock gap): 10yr $69,234 / 20yr $208,371 / 30yr $487,988; annuity math verified ($400/mo @7% → $69,234 at 10y). Type filters (save-by-date/build-balance/etc.) present.
- Note: goal progress $0 (goal_contributions ledger) is distinct from the EF-eligible Savings account — by design (goal balance ≠ account earmark).

### Scenarios — `/scenarios` — Verified
- Composer + quick-start chips render. Saved "Buy a car $35k" shows correct **Stale + Revised** badges and the revised "Stays afloat? Yes / +0d / $0" state (from the earlier revise-to-no-car). Actions Explain/Reopen/Duplicate/Revise/Promote/Archive present. (Core #71/#72/#73 already confirmed on the real backend.)

### Path back — `/path-back` — Verified
- Spending-recovery analysis with real data: RECENT $3,955/mo vs NORMAL $3,478 (12mo median), GAP $477 ("within your normal"). Levers ("trim these" $0) + self-correcting ("leave them" $11) correctly surface Audible (+$10, new recurring) and Netflix price step (+$1). "Ask Copilot to plan it" CTA.

## QA progress: 11/14 screens verified
Remaining: Settings, Rules & agents, Copilot (Copilot AI needs an LLM key — render-only test possible).

### Settings — `/settings` — Verified
- All sections render on the real server (Profile, Financial targets, How you want advice, Privacy & data, Data & backups, Agent, AI Provider, Appearance, Connections, Notifications, Keyboard, About, Account).
- **"treating debt at or above 8% APR as urgent"** shows correctly — confirms the earlier "undefined% APR" was a pure mock gap (real backend supplies 8% for Balanced risk tolerance).
- Server-mode Account section works (Users / Manage users / Sign out with password note). Data integrity "Healthy".
- **#69 notification prefs confirmed on real SQLCipher**: get/set round-trip — digest→weekly + snooze persisted, then restored. (Container-query responsive fix from earlier is in this build.)

### Rules & agents — `/rules` — Verified
- "No rules yet" empty state (I categorized directly, no rule created) with clear explanation. Trust Dial (auto-categorize high autonomy, apply-rules on). Agent activity log correctly lists the 79 categorizations as "user · 100% conf".

### Copilot — `/copilot` — Render Verified · AI Q&A BLOCKED (external dependency)
- Screen renders: greeting, "Copilot ready", 6 suggested prompts, real-data context (99 transactions · 3 accounts · 100% local).
- **Blocked:** the real backend's completion provider is `unconfigured`; the AI planner/tools need an LLM API key that this throwaway server has none of (and I won't add a real key). Grounded generative-UI blocks and Q&A are therefore not exercisable here. Graceful-degradation of a sent query was inconclusive (suggested-prompt click didn't submit) — flagged for the LLM-configured follow-up.

---

## First full pass complete — 14/14 screens
**Verified (14):** Today, Inbox, Accounts, Budget, Recurring, Cash flow, Reports, Categories, Goals, Scenarios, Path back, Settings, Rules & agents, **Copilot**.
**~~Blocked (1): Copilot~~ — RESOLVED 2026-07-24** (see below).

### Copilot AI Q&A — VERIFIED (was blocked on LLM key)
User configured OpenRouter (`deepseek/deepseek-v4-flash`) via Settings → Agent. Sent a live grounded query ("overview of my accounts, savings rate, standout spending") through the real UI:
- **Plan → tool loop → grounded answer**, all real: `Financial Snapshot`, `get spending breakdown`, `find anomalies`, `explain spending change` tool calls all completed.
- **Typed generative-UI blocks rendered** (not markdown): accounts table (Everyday Checking $11,794.14 / Savings $20,500 / Rewards Visa −$1,240 / Total Liquid $32,294.14), a missing-APR caution on the Visa, a metrics table (income $5,200 / expenses $3,000.46 / **surplus $2,199.54 / savings rate 42%** — internally consistent / EF 10.8mo), spending breakdown, and an Upcoming Bills table (Rent $1,800, subscriptions).
- **Cross-feature integration confirmed:** the answer surfaced "a planned car purchase of $35,000 coming up in October" — the exact scenario applied during the #72 QA earlier, proving the applied-scenario planned-transaction flows into the Copilot's context.
- Grounded, framework-aligned advice (emergency fund / savings rate / debt); "no anomalies found." Completed in 33.4s. **No fallback, no `no_provider` error.**
- Minor spot-check flagged (non-blocking): the answer named "July's higher Costco spend" as the watch item — likely from `explain spending change`, worth confirming it's tool-grounded not embellished on a future pass.

**Defects found & FIXED this session (verified):** critical build_baseline SQL (scenario 500s); #72 stale-panel + double-apply; #73 stale-panel; Settings responsive overflow; auth-screen invisible inputs; a11y reduced-motion; RecoverScreen pw validation; + 4 dev-mock fidelity fixes. Full auth redesign shipped.

**Seed artifacts correctly NOT filed as bugs (advisor discipline):** EF "<1 month" (eligibility flag), Netflix price-step not flagged (≥3-charge baseline), transfer misclassification (pair_transfers not run on manual data), "Shell subscription" (identical gas), net-worth Today-vs-Reports (stale reading), Budget "$0 spent" (uncategorized) — all resolved by fixing the seed, not the code.

**Open follow-ups (non-blocking):** transfer-verdict real-UI-flow repro; Today "nothing needs attention" vs Inbox wording; budget-set CRUD; #58 price-change seed extension; systematic responsive/privacy batch; Copilot with an LLM key.

### Transfer-verdict flow (open finding #1) — RESOLVED, not a bug
- Root cause of my earlier confusion: I used `apply_transfer_verdict_to_similar` (bulk) which is **scoped to the transfer-review queue** (`transfer_review_predicate`) — it only rules rows the app already flags as transfer-like, so my arbitrary rows matched 0 (the RPC "succeeded" changing nothing).
- The real per-transaction path `set_transaction_transfer(id, true)` sets `is_transfer=1` (transactions.rs:497). Reproduced: marked all 4 VISA PAYMENT occurrences → **VISA PAYMENT left Recurring** (11→10 items; subs 5→4). Marked the TFR/PAYMENT pairs too → Recurring now lists only real bills/subs/income, no transfers. Net worth unchanged.
- **Verdict: the transfer-verdict → is_transfer → detect_recurring exclusion chain works correctly.** No code change. Cross-cutting transfer "pollution" on Recurring/Reports/Cash flow was purely the manual-seed + wrong-RPC artifact.

### Budget-set CRUD (follow-up) — Verified
- `set_budget(groceries, $600)` → persisted; Budget UI reflects it: BUDGETED $600, Groceries "$822 spent of $600 · Over by $222", "Cover from another envelope" affordance. (Top-level "551% spent" is expected with only one category budgeted.)

### Inbox A (Today "nothing needs attention" vs Inbox) — RESOLVED, not a bug
- Today's panel uses `useNeedsReviewCount()` = transactions **flagged for a decision** (low-confidence/transfer-review/anomaly), gated with anomalyCount. The Inbox's "87 need categorizing" = **uncategorized cleanup** count. Two legitimately different metrics — "nothing flagged for review" ≠ "nothing to categorize". Defensible wording; not misleading given the panel scope. No change. (Advisor discipline — another near-phantom avoided.)

## Responsive sweep (375px, iPhone-SE width) — all 14 routes VERIFIED

Measured document horizontal overflow (`scrollWidth − clientWidth`) on the **real backend** at 375×812 across every route. Method: JS offender-detection walking the DOM for elements whose right edge passes the viewport, distinguishing genuine page-overflow from opt-in internal scrollers (tables, filter pills) and fixed-element victims.

**Defects found & FIXED (verified live, 0 overflow after):**
- **Budget — 26px overflow.** The `.budget-grid` stat cards (`minmax(260px,1fr)`) sat inside a `1.4fr 3fr` hero grid that never collapsed, so the 3fr column was narrower than the 260px min-track. Fix: `.budget-hero-grid` responsive class (collapses to 1 col ≤640px) + `.budget-grid` single-column at ≤640px. Result: the three stat cards now stack cleanly. ([app.css](ui/src/styles/app.css), [Budget.tsx:381](ui/src/screens/Budget.tsx#L381))
- **Goals — 108px→0.** Two inline `gridTemplateColumns` (`1.5fr 1fr 1fr` goal-card row and `1fr 1fr` what-if grid) didn't collapse. Fix: `.goal-card-row` / `.goal-whatif-grid` classes, single-col ≤640px.
- **Goals filter pill — overflowed the viewport.** The 6-segment `.toolbar` (`All / Save by date / … / Sinking fund`) is wider than 375px. Fix: at ≤640px the toolbar becomes a horizontal scroller (`overflow-x:auto`, `min-width:0` flex buttons) — keeps the segmented-pill look, swipes sideways. ([app.css](ui/src/styles/app.css))
- **App shell sub-pixel bleed.** A residual ~13px document jiggle traced to flex min-content rounding in `.screen-*` (no visible content past the edge). Fix: `.main-inner { overflow-x: clip }` at ≤900px — clips the phantom bleed while leaving `overflow-y` visible (so sticky headers/dropdowns still work) and inner opt-in scrollers intact.

**Final sweep — 0 horizontal overflow on all 14 routes** at 375px: `/ · today · inbox · accounts · budget · goals · cashflow · scenarios · recurring · reports · categories · transactions · settings · journey`.

### Goals horizon axis label — mislabel FIXED (data-independent)
- **Defect:** the "When each goal lands" horizon axis formatted ticks ≥12 months out with `year:"2-digit"` → "Feb 28", "Apr 31", "Nov 32" — which read as invalid calendar days (Apr 31 doesn't exist), not years. Any goal >1yr out hit this. ([Goals.tsx:192](ui/src/screens/Goals.tsx#L192))
- **First fix** (`year:"numeric"` → "Feb 2028") removed the ambiguity but the wider labels **collided** at the phone-width right edge.
- **Final fix:** extracted `horizonTickLabel(monthsOut, now)` (exported, unit-tested) using the apostrophe-year convention → "Feb '28 · Sep '29 · Apr '31 · Nov '32" — unambiguously a year, compact enough that 5 ticks clear each other at 375px (verified: adjacent labels 2px apart, no overlap). +3 regression tests in `Goals.test.tsx` (34 pass).

## Privacy-mode (amount blur) sweep — leaks found & FIXED

Enabled privacy (`finsight.tweaks.privacy=true` → ThemeProvider stamps `[data-privacy="on"]` on `<html>`, which blurs `.money/.num/.figure/.blurable`) on the **real backend** and walked every money-bearing route for `$`-amounts whose element wasn't covered by a blur hook. The headline figures were always blurred; the gap was **contextual amounts** — the feature promises "blur all financial numbers," so these were real leaks.

**Verified via screenshot** (Budget, Cash flow, Goals) then closed, re-audited to **0 leaks**:
- **Budget:** "$144/day pace" sub, "Over by $3,855" projected npill, per-envelope "Over by $222" status chip, group subtotals (`.muted mono`), and the "of $600" budgeted sub in the 5-month history table. Fix: wrapped each amount in `.blurable` (chip blurs only when its label carries a `$`, so "On track" stays crisp). ([Budget.tsx](ui/src/screens/Budget.tsx))
- **Goals:** goal subtitle "Auto-moves $400/month", horizon marker "$30,000", compound-projector intro "$0 now plus $400/month", and the projection cards "$48,000 in · +$21,234 growth" (×3). Fix: `.blurable` wraps + `blurAmounts()` on the subtitle prose. ([Goals.tsx](ui/src/screens/Goals.tsx))
- **Cash flow:** the "Good to know" cards leaked "NETFLIX.COM (about **$18**) is due…" etc. Fix: new **`blurAmounts()`** util ([blurAmounts.tsx](ui/src/utils/blurAmounts.tsx)) splits app-generated prose and wraps only `$`-tokens in `.blurable` — so the amount blurs while merchant/date/advice stay readable (screenshot-confirmed). +5 unit tests.

**Intentionally NOT blurred (documented, by design):**
- **User-typed scenario descriptions** ("Buy a car $35k", "Add $500/mo to savings" on Scenarios/Recurring) — free text the user authored; blurring a substring of someone's own words needs a dollar-regex over arbitrary text, which is fragile and semantically wrong. Confirmed user-typed via the save form's free-text input ([Scenarios.tsx:548](ui/src/screens/Scenarios.tsx#L548)).
- **Framework milestone copy** ("Dave Ramsey's Baby Step 1: save $1,000", "Build your first $1,000 buffer" on Inbox/Journey) — fixed generic labels identical for every user, not the user's own balances.
- **What-if slider scale bounds** ($0 / $750 / $1,500 on Goals) — the range control's fixed scale, reveals no private data (the live selected value uses `.figure`, which *is* blurred).

### Pre-existing bug fixed in passing — CashFlow filename casing
While type-checking the privacy fix, found `tsc --noEmit` was **failing** on a casing conflict: the file is `CashFlow.tsx` but `App.tsx` and the test imported `./screens/Cashflow` / `./Cashflow`. Works on Windows (case-insensitive FS) but breaks `tsc` and would break a case-sensitive Linux CI build. (My earlier `tsc | head` checks masked it by reading `head`'s exit code, not tsc's.) Fixed by aligning both imports to the real filename `CashFlow`. tsc now genuinely clean (exit 0).

---

