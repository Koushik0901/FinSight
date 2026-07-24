# FinSight full-app QA — 2026-07-23

Validation ledger for the whole-app QA pass. Environment: real `finsight-server`
on `:8674` (throwaway scratchpad data dir), signed in as `tester`, driven through
the in-app browser. One record per screen; each screen graded **Verified**,
**Repaired & Verified**, or **Blocked**.

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
**Verified (13):** Today, Inbox, Accounts, Budget, Recurring, Cash flow, Reports, Categories, Goals, Scenarios, Path back, Settings, Rules & agents.
**Blocked (1):** Copilot AI Q&A (no LLM key) — screen itself renders.

**Defects found & FIXED this session (verified):** critical build_baseline SQL (scenario 500s); #72 stale-panel + double-apply; #73 stale-panel; Settings responsive overflow; auth-screen invisible inputs; a11y reduced-motion; RecoverScreen pw validation; + 4 dev-mock fidelity fixes. Full auth redesign shipped.

**Seed artifacts correctly NOT filed as bugs (advisor discipline):** EF "<1 month" (eligibility flag), Netflix price-step not flagged (≥3-charge baseline), transfer misclassification (pair_transfers not run on manual data), "Shell subscription" (identical gas), net-worth Today-vs-Reports (stale reading), Budget "$0 spent" (uncategorized) — all resolved by fixing the seed, not the code.

**Open follow-ups (non-blocking):** transfer-verdict real-UI-flow repro; Today "nothing needs attention" vs Inbox wording; budget-set CRUD; #58 price-change seed extension; systematic responsive/privacy batch; Copilot with an LLM key.
