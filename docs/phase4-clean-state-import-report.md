# Phase 4 — Clean-State UI Import Validation

Driven entirely through the desktop UI like a real user: reset to a clean state,
import the sample statements, verify parsing and categorization, fix the root
causes that surfaced, and prove repeatability. All fixes are generic and tested —
no hardcoding to specific visible rows. Data used: the real `samples/` CSVs
(`amex-all-time-statement.csv`, `tangerine-chequing-all-time-statement.csv`,
`tangerine-savings-all-time-statement.csv`). No demo/mock/seeded data.

## 1. Clean-state workflow
- **Settings → Delete all data** (typed-confirm) resets to onboarding. Verified the
  honest empty state: net worth `$0` with "N accounts have no balance set —
  excluded", `$0` liquid/invested/credit, "Net worth history is still building" —
  no fabricated values, no stale rows. Repeated across three full cycles.
- Imported each CSV through the real onboarding **Connect → Pick a file → Map
  columns** flow, creating one account per statement.

## 2. Parsing verified in the UI
- Dates (`01 Jul 2026` / `MM/DD/YYYY`), merchants, amounts, and account all land
  correctly. Amex `Date/Description/Amount` and Tangerine `Date/Name/Memo/Amount`
  auto-map right (Name→merchant, Memo→notes).
- Counts are exact and stable: **Amex 1988, Tangerine Chequing 29, Tangerine
  Savings 27 = 2044**, reproduced identically on re-import.

## 3. Sign convention (verified by outcome, not the radio)
- Amex is a credit-card export where **charges are positive, payments negative**;
  the correct setting is **Positive = outflow** (selected manually). Tangerine
  bank files use the standard **Negative = outflow** (auto-detected, correct).
- Confirmed by the resulting ledger: `TIM HORTONS −$9.43` (spending),
  `PAYMENT RECEIVED −> +$2,986.14` (inflow), `Use Points +$213.90` (credit),
  `EFT Deposit +$1,000` / `Internet Withdrawal −$500`.
- **Documented limitation (not a bug):** auto-detect always defaults to
  `negative_is_outflow`. Sign convention is *undecidable from amounts alone* (a
  mostly-positive file is equally a credit card of charges or a chequing account
  of deposits); the only separating signal is the *description* ("PAYMENT / THANK
  YOU"), and keying on that would overfit to visible rows. So the credit-card
  case needs a one-click manual flip, by design. (`ui/src/utils/csvDetection.ts`.)

## 4. Categorization quality (looked, not just counted)
- On the clean import: **builtin 1438 + LLM 606 = 2044 categorized, 0
  uncategorized.** Auto-categorize (Gemma via OpenRouter) ran automatically after
  import — see fix #2 below.
- Same merchant → same category across the file (TIM HORTONS→Dining, EVO CAR
  SHARE→Transport ×all, COMPASS→Transport, ANTHROPIC→Subscriptions).
- **Low-confidence → review, not force-fit:** the AI-confidence spread is healthy
  (~95 items `< 0.6`) and the account view's **"Needs review 95"** filter surfaces
  exactly those. Anomalies filter shows 43 genuine outliers.
- Residual LLM variance (not fixed, would overfit): a few ambiguous
  government/fee merchants land in the nearest default category
  (IMMIGRATION CANADA→Shopping, REVENUE SERVICES BC→Transport). There is no
  "Fees/Government" default; the user can add one + guidance.

## 5. Root-cause fixes (generic + tested)
Five defects surfaced and were fixed at the root, each with a regression test:

1. **Import-before-categories left everything uncategorized** — builtin only
   assigns categories that already exist, and none were seeded if you import
   before onboarding's category step. `ensure_default_categories` now seeds the
   10 defaults on import when the table is empty (idempotent; never overwrites a
   user set). `crates/finsight-core/src/categorize.rs`.
2. **Intra-file dedup dropped real repeated charges** — a first import auto-skipped
   127 and queued 160 as "duplicates" because rows matched *other rows in the same
   file* (e.g. several same-day `OPENROUTER −$5.33` API top-ups). A single
   statement lists each posted charge once; rows this import inserts are now
   excluded from dedup candidacy (`reconcile_excluding_batch`). Cross-import dedup
   unchanged. Result: **1988 imported, 0 skipped, 0 queued.**
   `crates/finsight-providers/src/{csv/mod.rs,simplefin/matcher.rs}`.
3. **Auto-categorize never fired on import** — the setting promises "categorize
   after each import … using your AI provider" but `import_csv` only ran the
   builtin pass. It now enqueues the LLM categorizer when the setting is on.
   `crates/finsight-app/src/commands/import.rs`.
4. **Transfers were sent to the LLM** — card payments / internal transfers are
   flagged `is_transfer=1` but were still handed to the categorizer, which tagged
   `PAYMENT RECEIVED` as "Shopping" and flooded Needs Review. `load_uncategorized`
   now excludes transfers. `crates/finsight-agent/src/categorizer.rs`.
5. **Uncategorized counts would nag forever** (regression caught for #4) — with
   transfers never categorized, every `category_id IS NULL` tally had to also
   exclude `is_transfer=1`, else the status bar/Insights/Today/Copilot would show
   ~83 uncategorized that can never clear. Aligned all four count sites with the
   categorizer and reports. `agent.rs`, `context.rs`, `finance.rs`.

## 6. Calculations validated against ground truth
- **Reports** category totals reconcile exactly to the header: Year scope
  category sum = **$27,955** = "spent this period"; All-time = **$65,151**.
- **Transfers excluded from every spending surface:** with ~$25k of
  `PAYMENT RECEIVED` transfers mislabeled "Shopping" on this session's data,
  Reports Shopping is **$14,468** (all-time) and the Categories screen's June
  Shopping is **$1,779** — i.e. *not* inflated. Both Reports and Categories filter
  `is_transfer = 0`, so the aggregates stay correct even when a label is wrong.
- Per-account net activity (via read-only diagnostic): Amex **−$1,139.23**,
  Tangerine Savings **+$8,036.60**, Tangerine Chequing **$0** (pass-through) —
  all sensible for the source data.

## 7. Charts, filters, empty state
- "Income and expenses over time" populates for the historical window
  (data-anchored, not wall-clock), with **Month / Quarter / Year / All-time**
  scopes all rendering. Net-worth and spending-deep-dive tabs render.
- Account-view **search, date-range, All / Needs review / Anomalies** filters
  work. Empty state after Delete All Data is honest (no crashes, no fake $0).
- **Balance honesty:** all three accounts show "Balance not set" and are excluded
  from net worth — no invented zeros.

## 8. Repeatability & limitations
- Delete → re-import reproduced identical counts (Amex 1988 twice; full 3-file set
  2044) and every derived surface reset and rebuilt cleanly.
- Green bar: full Rust workspace + frontend tests pass; the five fixes ship with
  new regression tests (`repeated_identical_rows_in_one_statement_all_import`,
  `transfers_are_not_sent_to_the_llm_and_stay_uncategorized`, the
  `ensure_default_categories*` trio, and the count-exclusion updates).
- **Mixed-state caveat:** this session's live data was categorized *before* fix
  #4/#5 landed, so its ledger labels and Needs-Review count still reflect the old
  behaviour (transfers tagged "Shopping"). Aggregates are already correct because
  Reports/Categories exclude transfers; a fresh import produces the corrected
  labels, which the new unit tests lock in. The convention auto-flip for
  credit-card files (§3) is a deliberate manual step, not a defect.
