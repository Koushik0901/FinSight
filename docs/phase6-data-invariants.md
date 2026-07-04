# Phase 6 — Data Invariants

The unifying rule behind the balance/chart/review/insight bugs is a missing
**four-state distinction**. Every derived surface (balances, charts, reports,
review queue, anomalies, insights, Copilot answers) must distinguish:

| State | Meaning | UI/behaviour |
|---|---|---|
| **Known value** | A real, trusted number (incl. a real zero) | Show it |
| **Real zero** | Genuinely zero (e.g. no spend in a period that has data) | Show `$0` honestly |
| **Unknown** | Value not knowable from local data (e.g. seed balance, no confirmed snapshot) | Show "Unknown" / prompt; never render as `$0` |
| **No data** | Nothing imported for this scope | Honest empty state |
| **Query failure** | A query errored | Surface the error; never coerce to empty/zero |

Never collapse *unknown*, *no-data*, or *query-failure* into a misleading `$0`
or an empty chart.

## Accounts & balances
- A balance is **known** only when a non-`seed` `account_balances` snapshot
  exists, OR the account has no transactions (the seed opening balance is then
  trustworthy). Mirrors `AccountSummary.balance_known` (Phase 1‑4).
- A `seed`-sourced balance on an account **with** transaction activity is
  **Unknown**, not `$0`.
- A credit/statement balance **must not** be asserted from summing imported
  transactions: an imported "all-time" statement is not guaranteed to start at
  account opening. Any activity-derived figure is an **estimate** and must be
  labelled as such — never shown as "the balance".
- Net worth excludes unknown-balance accounts and states how many were excluded.

## Transactions
- `amount_cents < 0` = outflow, `> 0` = inflow. Sign convention is per-account
  (credit cards may invert on import) but stored normalized.
- `is_transfer = 1` marks internal transfers / card payments / e‑transfers.
  Transfers are **excluded** from spending charts, income, category totals,
  recurring/subscription detection, and insights.
- `transfer_peer_id` links the two legs of one cross-account transfer
  (withdrawal ↔ matching deposit), written reciprocally by
  `categorize::pair_transfers` (runs after import and on startup). Pairing is
  conservative: exact opposite amounts, different accounts, ≤ 4-day window,
  and either both legs keyword-flagged (rule A) or a Credit-account card
  payment matched to a bill-payment-hinted bank leg (rule B — this can flag a
  leg keywords missed). Paired legs are transfers by construction: the builtin
  keyword re-run must never un-flag or categorize them. A leg pairs at most
  once; e-transfers to other people stay unpaired (`transfer_peer_id IS NULL`).
- Every transaction belongs to exactly one account; `category_id` may be NULL
  (uncategorized) — NULL is a valid, first-class state, not an error.

## Categories & categorization
- `categorizations.source ∈ {builtin, llm, rule, user}`. `builtin` and `rule`
  are deterministic (confidence 1.0); `llm` carries a model confidence in
  `transactions.ai_confidence`.
- **Needs Review** = transactions whose most-recent categorization is `llm`
  with `ai_confidence < LOW_CONFIDENCE_THRESHOLD`. If none exist, the tab is
  honestly empty (not broken).
- Merchant matching uses a **normalized merchant** (location/store#/URL/
  payment-processor noise stripped), shared with recurring detection.

## Recurring items (bills / subscriptions / income / transfers / repeat purchases)
- A recurrence requires **normalized-merchant grouping**, **≥3 occurrences**,
  **regular cadence** (low gap variance), and — for a *subscription/bill* —
  **stable amount within a tolerance band** (~±15%, FX-aware for USD vendors).
- **Exclusions:** transfers/card-payments/e‑transfers are never subscriptions;
  `dining / groceries / transport / shopping` categories are never
  subscriptions unless the vendor is on an explicit subscription allowlist.
- Every recurring item carries a **kind**, a **confidence**, and human-readable
  **reasons**. Low-confidence items are surfaced as such or hidden.

## Insights
- Every insight is backed by a **deterministic query** or **verified** model
  output, with source / count / date-range metadata where useful.
- Subscription insights derive from the recurring detector above (not the loose
  heuristic).
- "What the agent learned" = **only** real user-approved recategorizations /
  rules (`categorizations.source ∈ {user, rule}` / accepted rule proposals) —
  never model guesses or stale flags.
- No stale/hallucinated/cached insight survives a data change; insights are
  recomputed from live data and React-Query caches invalidated on import/reset.

## Anomalies
- Computed from **actual transaction patterns** (statistical outliers vs a
  merchant/category baseline), never random or stale flags. `is_anomaly`
  reflects a real, current computation.

## Reset / re-import
- `delete_all_data` wipes all of the above derived inputs; every derived surface
  must reset (charts, insights, categories, review, anomalies, recurring,
  Copilot context). Re-import recomputes everything; caches are invalidated.
