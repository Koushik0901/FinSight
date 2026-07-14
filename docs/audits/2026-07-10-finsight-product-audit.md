# FinSight Product Audit — 2026-07-10

**Scope:** the whole product — README promises, architecture, data model, household
ownership, import, categorization, transfers, recurring, balances, budgets, goals,
reports, insights, automation, Copilot, privacy, durability, and end-to-end UX.
**Mandate:** audit only. No fixes were made. This document is the deliverable: a
ranked, evidence-backed roadmap for an implementation agent.

## How this was audited (evidence base)

1. **Headless real-pipeline probe** — `crates/finsight-app/tests/audit_probe.rs`
   (committed as audit tooling, `#[ignore]`d) drives the REAL import pipeline
   against the user's actual bank CSVs in `samples/` (6 accounts, 3,213
   transactions, Dec 2023 → Jul 2026), then runs the real post-import cascade
   (builtin categorization, transfer pairing, anomaly recompute, balance
   derivation, net-worth backfill) and dumps every derived number.
   Run: `cargo test -p finsight-app --release --test audit_probe -- --ignored --nocapture`
2. **Independent ground truth** — the same CSVs parsed independently (Python) to
   compute per-account sums, transfer legs, payroll, and recurring candidates;
   diffed against the app's numbers.
3. **Code audit** — migrations/schema, metrics layer, finance.rs, categorize.rs,
   anomaly.rs, recurring.rs, import pipeline, commands, screens' data sources.
4. **Copilot** — the 10-iteration eval loop run this month (`eval/FINDINGS.md`,
   v1 → v10: overall 1.36 → 4.49 clean, fabrication 71% → ~9%) is incorporated
   rather than repeated.
5. **Live UI walk** — attempted; the computer-use permission dialog was denied
   during this session, so screen-level verification was substituted with
   code-level tracing of each screen's data sources (which the probe exercises
   end-to-end on the same data). If a future audit pass gets UI access, §E lists
   what to spot-check.
6. **Durability forensics** — the real app-data directory and git history around
   the 2026-07-02 incident.

**Overall verdict:** the architecture is genuinely good (clean crate layering, a
single metrics layer, disciplined `is_transfer` exclusion everywhere downstream,
encrypted DB, draft-approval pattern for every mutation, extensive tests). But on
the user's *real* data, one upstream defect — transfer detection — quietly
poisons almost every headline number the product shows, and the product has no
concept of *who* owns money, which is the user's stated goal (managing a
girlfriend's / family's finances). Fixing a small number of foundational items
converts FinSight from "impressive demo on synthetic data" to "trustworthy daily
financial OS."

---

## P0 — foundational correctness (do these first)

### P0-1. Transfer detection misses ~half of real transfer volume; every headline number is wrong

**User impact (the single biggest issue in the product):** On the user's real
data the app reports **avg monthly income $9,143, expenses $9,580, savings rate
−4%** (rolling 90d). Reality: income ≈ $2.5–7k/mo, expenses ≈ $2.5–3.5k/mo,
savings rate strongly positive. May 2026 "income" of $13,661 is ~60% the user
moving money between their own accounts. Today, Reports, health score, EF
months, runway, budgets-vs-income, Scenarios baselines, and every Copilot answer
consume these numbers. A user who knows their own finances will notice the
numbers are fiction within minutes — and stop trusting everything else.

**Evidence (probe + ground truth):**
- Ground truth transfer legs in `samples/`: 49 Amex payment pairs ($65,343.90),
  138 + 95 CIBC "INTERNET TRANSFER" legs, 29 Tangerine legs, dozens of Interac
  e-transfers. Probe: only **287 rows flagged** `is_transfer`, only **74 pairs**
  peer-linked. Unflagged transfer-like rows total **$13,516 inflow / $16,254
  outflow** (keyword-matchable alone; more via cross-bank legs).
- Top May-2026 "income" rows (probe output): `INTERNET TRANSFER …` $2,750,
  `EFT Deposit from CANADIAN IMPERI` $2,500 (own CIBC → Tangerine),
  `INTERNET TRANSFER … FROM ACCOUNT` $2,000, `PAYMENT THANK YOU/PAIEMEN T MERCI`
  $633.17 (credit-card payment counted as **income on the card**).
- 8 of the top-10 "anomalies" are the user's own transfers
  (`PREAUTHORIZED DEBIT Tangerine` −$6,000, internet transfers…).

**Root cause (precise):** `crates/finsight-core/src/categorize.rs`
`TRANSFER_KEYWORDS` expects contiguous phrases (`"transfer to"`,
`"transfer from"`, `"payment - thank you"`). Real CIBC strings interpose a
reference number — `INTERNET TRANSFER 000000104797 FROM ACCOUNT` — so nothing
matches; `PAYMENT THANK YOU/PAIEMEN T MERCI` has no dash; Tangerine↔CIBC EFT
legs (`EFT Deposit from CANADIAN IMPERI`, `Electronic Funds Transfer
PREAUTHORIZED DEBIT Tangerine`) match nothing. And `pair_transfers` rule A only
pairs legs *already flagged*; rule B only special-cases credit-card hints — an
unflagged↔unflagged equal-amount opposite-sign pair across accounts (even with
the SAME reference number in both strings) is never attempted.

**Downstream chain (all verified):** income/expense/savings-rate/runway/EF-months
(metrics.rs correctly excludes `is_transfer=0` — garbage in), anomaly list
composition (anomaly.rs excludes correctly — garbage in), recurring "bills"
(`internet banking internet` = biweekly $1,000 "Bill" n=73; `eft deposit from`
= biweekly $2,500 "Income"), **and privacy** (see P0-3).

**Direction:** treat transfer detection as a first-class matching problem, not a
keyword list: (a) normalize merchant strings before matching (strip reference
numbers / collapse whitespace) and broaden the vocabulary with real Canadian
bank phrasings; (b) add a pairing rule for unflagged legs: equal amount,
opposite sign, different account, within N days — with extra confidence when a
shared reference number appears in both strings; (c) treat matched pairs as the
source of truth and keyword flags as hints; (d) surface an "is this a transfer?"
review affordance for singles whose peer account isn't imported (Amex payment
legs when chequing isn't imported yet).

**Acceptance criteria:**
- Probe on `samples/`: ≥ 95% of the ground-truth legs flagged (Amex 98 legs,
  CIBC internet-transfer 233 legs, Tangerine 29, e-transfer self-transfers);
  rolling-90 income within ±15% of hand-computed true income (EI + payroll +
  interest + refunds only); May 2026 income ≤ $4,000.
- Anomaly top-10 on samples contains no self-transfer rows.
- Recurring list contains no "internet banking internet"-style Bill/Income items.
- Regression: existing categorize/pair tests still pass; no debit-purchase
  false-flags (`INTERAC` purchase test `88e6012` stays green).

**Test data:** `samples/` (all six files), plus synthetic: same-amount
same-day non-transfer pair (two $50 purchases) that must NOT pair; a transfer
pair 3 days apart that must pair; single-leg Amex payment with no chequing
account imported.

### P0-2. No per-person money model — the product cannot serve the stated goal (partner/family finances)

**User impact:** the explicit objective is managing the user's own + girlfriend's
+ family finances. Today `household_members` exists (V038) with account-owner
assignment (0/1/N owners per account) — but ownership is consumed **nowhere**:
not in metrics, budgets, goals, reports, transactions filtering, Copilot context,
or any screen except an owner chip on Accounts. Every number in the app is a
blind blend of everyone's money. Two people cannot see "my spending vs yours,"
"her savings rate," "our joint account," or ask the Copilot about one person.

**Evidence:** grep of consumers — `household` referenced only by
`AccountDrawer.tsx`, `Accounts.tsx`, `hooks/household.ts`, CRUD commands, reset,
and one diagnostics test. Zero joins from `account_owners` in metrics/reports/
budgets/goals/Copilot context.

**Root cause:** feature was built as metadata-only (V038 comment says shares
"can be added later"); no ownership dimension in the query layer.

**Direction (design decision for the implementation agent, in order):**
1. Add an optional `member_id` filter dimension to the metrics layer
   (balance_breakdown / cashflow / rolling averages via account-ownership joins;
   equal-split for joint accounts in v1, matching the V038 note).
2. Thread a person/“Everyone” switcher through Today, Reports, Transactions,
   and Accounts.
3. Give the Copilot member-awareness (context includes per-member rollups;
   tools accept an optional member filter).
4. Goals: optional owner on a goal.
   Budgets can stay household-level in v1 (envelope-per-person is a later call).

**Acceptance criteria:** with two members and owner assignments on the sample
accounts, Today/Reports numbers filtered per member sum to the household total;
joint accounts split; Copilot answers "what did X spend last month" from tools
without fabrication; zero-member households behave exactly as today.

**Test data:** samples accounts assigned: CIBC+Amex → person A, Tangerine →
person B, one joint; empty-member DB regression.

### P0-3. Privacy promise broken by P0-1: contact names go to the cloud LLM

**User impact:** README promises "no data leaving your machine unless you opt
in." The categorizer deliberately excludes `is_transfer=1` rows from LLM batches
(`categorizer.rs:305-311` — explicit comment). But because P0-1 misses most
e-transfer rows, **Interac e-transfer strings containing real people's full
names** (visible in `samples/`) are uncategorized, therefore sent to OpenRouter
with amounts on every auto-categorize run (auto-categorize defaults ON after
every import). Also: nothing in Settings/onboarding discloses *what* is sent.

**Evidence:** probe — 936 uncategorized rows include unflagged e-transfer legs;
`load_uncategorized` selects `WHERE category_id IS NULL AND is_transfer = 0`;
auto-categorize enqueues on every import (`import.rs:203-220`, default true).

**Direction:** (a) P0-1 fixes the main leak; (b) belt-and-braces: exclude
transfer-*like* strings (normalized keyword match) from LLM batches even when
unflagged, and/or redact person-name tokens in e-transfer descriptors before
sending; (c) add a one-line disclosure + toggle at provider setup ("merchant
descriptions and amounts of uncategorized transactions are sent to <provider>
for categorization"); (d) document exactly what the Copilot sends when asked.

**Acceptance criteria:** on samples, zero rows containing `e-Transfer`/`FULFILL
REQUEST` descriptors appear in any LLM batch (assert in a categorizer test);
provider-setup UI shows the disclosure; README privacy claim matches behavior.

### P0-4. Data durability: corruption incident, silent startup mutations, WAL hygiene

**User impact:** the app-data directory contains a real incident trail:
`data.sqlcipher.corrupt-20260702-222719` (+ corrupt WAL/SHM twins),
`data.sqlcipher.unexplained-reappeared-20260702-222900`, purge/migration
backups. A finance app that has already eaten a database once has one strike
left with a user. Mitigation since (single-instance plugin: "two windows on the
same encrypted DB would deadlock on WAL locks") addresses the presumed cause,
but: (a) the current WAL is 4.5 MB — the same size as the main DB — suggesting
checkpointing isn't reliably happening; (b) app startup silently runs a batch of
best-effort mutations (`lib.rs:343-364`: re-categorization, transfer pairing,
balance recompute, net-worth record+backfill, anomaly recompute) with all errors
discarded via `let _ =`; (c) there is no automatic pre-migration/periodic backup
except the ad-hoc `pre-v039` copy, and no in-app restore.

**Direction:** (a) verify WAL checkpoint on clean shutdown (and
`wal_checkpoint(TRUNCATE)` periodically); (b) log-and-surface (Inbox alert)
instead of discarding startup-cascade errors; (c) automatic timestamped backup
before running any migration + a "restore from backup" affordance in Settings;
(d) a `PRAGMA integrity_check`/`cipher_integrity_check` on startup with a
user-visible warning path.

**Acceptance criteria:** kill -9 during import → reopen clean; WAL shrinks after
checkpoint; migration failure leaves an intact automatic backup + clear restore
path; startup-cascade failure produces a visible Inbox alert, not silence.

---

## P1 — high-value correctness and trust

### P1-1. Housing (typically rent) is invisible — the biggest real expense doesn't exist

**Impact:** on real data, Housing = **1 transaction, $192.88, in 3 years**. The
user pays rent by e-transfer (Canadian norm) — those rows are either flagged as
transfers (correctly excluded from spending!) or uncategorized. Every "where
does my money go" answer, budget suggestion, and Copilot plan is wrong about the
single biggest line item in a real budget.

**Direction:** e-transfers need a disposition: own-account transfer vs payment
to a person (rent, friends). The pairing fix (P0-1b) gives the signal — an
e-transfer leg with **no matching opposite leg in any imported account** is a
real payment, not a transfer. Add a lightweight "who was this to?" review
(Inbox) that lets the user say "this is Rent — always" (creates a rule:
recurring e-transfer of ~$X monthly → Housing). Recurring detection should then
pick rent up as a Bill.

**Acceptance:** on samples, the monthly ~$1,000–1,600 e-transfer/`FULFILL
REQUEST` series lands in Housing after one user confirmation; spending
breakdown puts Housing at the top; unmatched-e-transfer review items appear in
Inbox with a one-click categorize.

### P1-2. Recurring detection: merchant-variant splitting breaks cadence; installment fees misclassified

**Impact:** the Recurring screen and upcoming-bills projections mislead:
- `OPENAI *CHATGPT SUBSCR` vs `CHATGPT SUBSCRIPTION S` split one monthly series
  into two → reported cadence **quarterly** (gap 54d). Same for Claude
  (`claude.ai subscription` quarterly + separate `anthropic` monthly).
- Amex `MEMBERSHIP FEE INSTALLMENT` — 31 perfectly monthly, stable $15.99 —
  classified `RepeatPurchase` conf 0.54 instead of Bill/Subscription.
- (After P0-1 the transfer "bills"/"income" rows disappear; these two remain.)

**Root cause:** `merchant_key` normalization keeps differentiating tokens;
kind-classifier vocabulary lacks "membership/installment".

**Direction:** vendor-aware normalization (strip processor prefixes `OPENAI *`,
`SQ *`, `PAYPAL *`; collapse to a canonical vendor token before grouping);
classify by the merged series. Add `membership|installment|fee` to the
subscription/bill vocabulary. Consider fuzzy-merging keys with edit-distance ≤2
on the leading token.

**Acceptance:** on samples — exactly one ChatGPT series (monthly), one
Claude/Anthropic series (monthly), Amex membership fee = Subscription/Bill with
monthly cadence; no regression on the 17 hand-computed monthly candidates from
the ground-truth script.

### P1-3. Copilot residuals from the eval loop (documented, unfixed)

Carried from `eval/FINDINGS.md` (v10):
1. **Temporal blindness:** `get_spending_breakdown` defaults `months=6` (max 60);
   the model concluded "data only goes back 6 months" on a 10-year history
   (temp-02/06 scored 1). Direction: default wider or instruct retry-wider in
   the tool description before concluding absence.
2. **Deterministic tool inconsistencies:** `run_cashflow_timeline` starts from
   `emergency_fund_balance_cents` instead of liquid (finance.rs ~1255); EF
   months has two definitions (snapshot = EF-eligible ÷ expenses vs scenarios =
   liquid ÷ expenses). Pick one definition each and align facts/tests.
3. **90-day-window surplus distortion** (one-off purchases crush "monthly
   surplus"): consider median/recurring-based expense basis for surplus.
4. **Hard-question latency**: the heaviest planning questions exceed 180 s /
   time out at the provider; acceptable for now, but keep the eval harness in CI
   as the regression detector (`cd eval && run_eval.py`).

### P1-4. Re-importing a statement queues ~7% of identical rows for manual review

**Impact:** monthly workflow friction. Re-importing the *identical* Amex file:
imported=0 ✓, but **145/1,988 rows queued for review** — same-day same-amount
same-merchant rows (two coffees) become "ambiguous matches" the user must
resolve every single re-import. Real users import overlapping statements
monthly; this trains them to ignore the review queue (which then hides real
issues).

**Root cause:** reconciliation matches on (account, amount, date, merchant); K
identical incoming rows × K identical existing rows → ambiguity queued
(`import_candidates` workbench) instead of set-matched.

**Direction:** bipartite set-matching for identical groups (K incoming ↔ K
existing → consume all K silently); only queue when counts differ. Also dedupe
within a re-imported file against the same `import_id` row hashes.

**Acceptance:** re-import of every samples file → imported=0, queued=0;
overlapping-window import (last 30 days re-exported) → only genuinely new rows
imported, 0 queued for identical duplicates.

### P1-5. Balance model correctness depends on an undocumented "opening balance" trap

**Impact:** derived balances were **exactly right** on all six accounts in the
probe — *because* opening=0 + all-time statements. The AccountDrawer just asks
"Opening balance ($)". A user who enters their **current** balance and then
imports full history double-counts; one who imports only 90 days with opening=0
shows garbage. Nothing explains the semantic (balance *before the first
imported transaction*).

**Direction:** label + helper text ("balance before the earliest transaction
you'll import — enter 0 if importing full history"); or better: after first
import, offer "set current balance" and back-solve the opening snapshot.
Surface per-account "balance basis" (derived vs snapshot) in the drawer.

**Acceptance:** both user paths (all-time import with 0; partial import with
known current balance) end with the correct current balance; a unit test on the
back-solve.

---

## P2 — product value and polish

### P2-1. First-run/out-of-box categorization coverage
44% of expense rows ($192k gross) remain uncategorized after the builtin pass
(no LLM configured). With LLM: 936 rows on first import (cost/latency; and
correctness rides P0-1/P1-1). Direction: extend the builtin keyword map with the
top Canadian merchant families observed (groceries chains, telecoms, transit),
and add an import-summary prompt ("936 uncategorized — run AI categorization
now?") instead of silent background enqueue.
**Acceptance:** uncategorized ≤ 20% on samples without LLM; explicit user-visible
choice for the LLM pass.

### P2-2. Onboarding/empty states are inconsistent
Only 7/20 screens use the shared `EmptyState`. Today/Reports/Categories/
Recurring/Goals handle emptiness ad hoc (some render zero-charts). Direction:
empty-state sweep with a consistent CTA (import CSV) per screen.
**Acceptance:** fresh DB → every route renders an intentional empty state
(frontend tests per screen).

### P2-3. Recipes ride the legacy no-tools planner
`recipe_runner.rs` uses `planner::build_system_prompt` + single-shot
`complete_json` — the pre-tool-loop path with the fabrication profile the eval
loop measured (v1–v3 era). Drafts + approval contain the risk, but quality lags
the Copilot. Direction: migrate recipes onto `ReasoningEngine::run` with
`standard_toolset()` (same grounding, `_display` fields, stall handling), keep
the draft-bundle contract.
**Acceptance:** each built-in recipe produces a bundle grounded in tool calls
(trace non-empty) on the samples DB; no invented figures in bundle rationales
(spot eval with the judge).

### P2-4. README/claims hygiene
"No data leaving your machine unless you opt in" (see P0-3), screens table all
✅ while §P0-2 shows household is metadata-only, "feature-complete" framing, and
`docs/TODO.md` is a stale all-done ledger with no live gap list. Direction:
align claims; add a PRIVACY section; keep a live "known gaps" doc (this audit).

### P2-5. Health score composition
`get_financial_health_score` mixes poisoned inputs (savings rate, EF months —
P0-1) with a neutral-50 goals default and budget adherence that ignores
uncategorized spend (44% of rows bypass envelopes → false "under budget"
comfort). After P0-1, re-verify the score on samples; consider counting
uncategorized spend against a virtual "Unbudgeted" envelope.

### P2-6. Anomaly review UX
60 anomalies on real data with no bulk triage ("these are fine") loop beyond
per-transaction drawer edits; after P0-1 the volume drops, but a "dismiss/confirm
anomaly" affordance (and feeding confirmations back to the detector) is needed
to keep Insights trustworthy.

---

## P3 — worth doing, not urgent — ✅ ALL DONE (2026-07-11)

1. **Startup mutation transparency — DONE.** The launch cascade now records a
   positive summary ("Refreshed on launch: categorized N · matched M transfer
   pairs · flagged K unusual charges") to `data.startup_summary`, shown in
   Settings → Data health alongside the existing failure warnings.
2. **Import progress + cascade visibility — DONE.** `ImportResult` carries
   `builtin_categorized` + `transfers_paired`; the post-import toast reads
   "imported 573, categorized 214, matched 12 transfer pairs" instead of hiding
   the cascade. (Uncategorized count + the AI-pass choice were already surfaced.)
3. **Merchant display naming — DONE.** `prettyMerchant()` (ui/src/utils/
   merchant.ts) collapses statement column-alignment space runs and drops
   support-URL tails, without renaming anything (casing/words stay as the bank
   wrote them; domain-style names like TEMU.COM kept). Applied to the
   transactions table and Recurring; the drawer's editable field stays raw.
4. **Currency correctness — DONE.** `money()`/`compactMoney()` already followed
   the configured currency; the real gaps were creation defaults — new accounts
   defaulted the picker to USD and new manual ASSETS were silently stored as
   USD (a CAD house displayed as US$). Both drawers now default to the
   configured currency.
5. **`docs/TODO.md` refresh — DONE** (points at the audits as the live gap
   list; status line kept current).
6. **Copilot chat-history housekeeping — VERIFIED.** `delete_conversation` is
   wired end-to-end (command → hook → thread list), so history is
   user-boundable; no unbounded background growth beyond user-visible threads.

---

## Dependency graph and execution order

```
P0-1 transfers ──┬─► P1-1 housing/e-transfer disposition
                 ├─► P2-5 health score re-verify
                 ├─► P2-6 anomaly UX (volume drops first)
                 ├─► P0-3 privacy (main leak closed; consent UI independent)
                 └─► (heals: income/expense/savings/runway/EF, anomalies,
                      recurring transfer-noise, Scenarios baselines, Copilot inputs)

P0-2 household model ──► Copilot member-awareness ──► partner onboarding
      (independent of P0-1; can run in parallel)

P0-4 durability (independent; small, do alongside)

P1-2 recurring normalization (independent; shares merchant-normalizer with P3-3)
P1-4 re-import set-matching (independent)
P1-5 opening-balance UX (independent, small)
P1-3 Copilot residuals (independent; eval harness is the regression gate)

P2-1 builtin coverage ── after P0-1 (transfer rows leave the uncategorized pool)
P2-2 empty states (anytime)
P2-3 recipes → tool loop (after Copilot residuals P1-3.2 so tools are consistent)
P2-4 README (after P0-3 consent lands)
```

**Recommended order of execution:**
1. **P0-1** transfer detection (biggest single win; unlocks half the list)
2. **P0-4** durability guards (small, independent, trust-critical)
3. **P0-3** privacy exclusion + consent surface (rides P0-1)
4. **P1-1** e-transfer disposition / housing (rides P0-1 pairing)
5. **P1-4** re-import set-matching + **P1-5** opening-balance UX (import loop
   becomes routinely safe)
6. **P1-2** recurring/merchant normalization
7. **P0-2** household member model (the strategic feature; parallelizable with
   5–6 by a second agent)
8. **P1-3** Copilot residuals + **P2-3** recipes on the tool loop
9. **P2-x** polish sweep (empty states, builtin coverage, health score, README)

**Regression gates for every step:** `cargo test --workspace` (341+),
`cd ui && npx vitest run` (363), the audit probe on `samples/`
(acceptance numbers above), and `cd eval && run_eval.py` for anything touching
the agent (compare to v10 baseline in MLflow).

---

## Appendix A — probe ground-truth reference (samples/)

| Account | Rows | Net sum | Derived balance (✓ verified) |
|---|---:|---:|---:|
| Amex | 1,988 | −$1,139.23 | −$1,139.23 |
| CIBC Chequing | 573 | +$1,070.66 | $1,070.66 |
| CIBC Credit | 376 | −$778.95 | −$778.95 |
| CIBC Savings | 220 | +$3,003.18 | $3,003.18 |
| Tangerine Chequing | 29 | $0.00 | $0.00 |
| Tangerine Savings | 27 | +$8,036.60 | $8,036.60 |

Liquid $12,110.44 · debt $1,918.18 · net worth $10,192.26 — **all exact** ✓.
Transfer legs (ground truth): Amex payments 49×2; CIBC internet-transfer 233;
Tangerine 29; plus e-transfers. Payroll (Infoblox) starts 2026-06-15 (2
deposits, $7,029.45); before that EI $1,186 biweekly.

## Appendix B — ✅ DONE (2026-07-13): real-app UI validation pass

Ran unattended against the ACTUAL compiled Tauri binary + a real encrypted
SQLCipher DB — not a mock, not just the probe. Method: `tauri:dev` launched
with an isolated `--config` (`{"identifier": "com.finsight.uiprobe"}` →
separate app-data dir, zero risk to the real DB) and
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223`, driven
over CDP (`window.__TAURI_INTERNALS__.invoke` for direct backend calls +
native DOM events for clicking). No computer-use/OS-dialog approval needed.

**Verified working, real data:** first-run onboarding on an empty DB → manual
account/transaction creation via real IPC → Today's derived numbers (net
worth, liquid, "spent so far") update correctly and consistently → the
(previously-missing) `/transactions` route renders with all five filter chips
→ marking a transaction a transfer removes it from Today's spending and from
the Inbox's uncategorized count, exactly per the F0/metrics-layer invariants →
imported the REAL `cibc-savings-all-time-statements.csv` (220 rows, matching
the Appendix A ground truth) and confirmed `builtinCategorized`/
`transfersPaired` populate correctly, own-account transfers get flagged while
ambiguous person e-transfers correctly stay `Uncategorized`, and the
"Possible transfers" surface lists exactly the ambiguous rows (swathi ×11,
matching the earlier probe finding) → the bulk-verdict "Also mark N more"
offer fires correctly on real data → Delete-All → every screen checked
(Today, `/transactions`) returns to its intentional empty state.

**Found and fixed a real bug this pass alone would catch** (380+ passing unit
tests did not): opening a transaction from a filtered view, then marking it a
transfer, silently flipped the still-open drawer to a blank "Add transaction"
form once the row left the active filter's refetched list — a unit-test-proof
staleness bug in `AccountTransactions.tsx`/`TransactionDrawer.tsx`. Fixed
(commit `08bd0a7`) with a last-known-transaction cache plus a drawer-local
display-state updated from each mutation's own return value. This class of
bug (filtered-list + open-drawer) can recur for other filter/drawer pairings —
worth a spot-check whenever a new filter is added to a screen with an edit
drawer.
