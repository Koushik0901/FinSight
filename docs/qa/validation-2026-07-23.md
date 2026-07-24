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
