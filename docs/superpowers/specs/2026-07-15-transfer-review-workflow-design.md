# Transfer Review Workflow (Settle-Up) — Design

**Date:** 2026-07-15
**Status:** Approved design, pre-implementation
**Origin:** F0 follow-up. The audit established that FinSight can auto-detect
internal transfers and household e-transfers, but the dominant residual leak on
real data is **genuinely ambiguous bidirectional person-to-person e-transfers**
(friends: joe / vyshnavi / sathvik / jin, ~$10k each, both directions). These
cannot be auto-classified — they need a fast, sticky, confirm-once **review
workflow**, not more detection heuristics.

## Goal

Let the user resolve every ambiguous person-to-person money flow **once per
counterparty**, and have that verdict (a) fix the headline numbers truthfully,
(b) stick across re-imports, and (c) generalize to future rows — without opening
one drawer per transaction.

## What already exists (do NOT rebuild)

The confirm-once-and-generalize machinery for the binary transfer/not verdict is
already built and shipped (V046):

- `transactions::set_transfer_override(id, is_transfer)` — sticky verdict; clears
  category + anomaly on mark; unlinks the paired peer on unmark. Respected by
  both `apply_builtin_categorization` and `pair_transfers`, so it survives
  re-imports and re-categorization.
- `transactions::transfer_verdict_siblings(id)` → `(%name%, count)` — the other
  UNDECIDED rows sharing this counterparty ("also rule the other 11 with Joe").
- `transactions::apply_transfer_override_to_matching(pattern, is_transfer)` — bulk
  apply one verdict to a counterparty's undecided rows.
- Tauri commands `set_transaction_transfer` / `apply_transfer_verdict_to_similar`.
- `categorize::suggested_rule_pattern(merchant_raw)` → `%counterparty%` for
  person-to-person e-transfers, raw string otherwise.
- UI: the `TransactionDrawer` "Transfer" toggle + a "Possible transfers" filter
  tab on `/transactions`, and the `Inbox` needs-review feed (`/inbox`; the old
  `/insights` route now redirects here).

**Two gaps this design closes:** (1) the verdict is binary (transfer / not) and
can't express *settle-up* netting; (2) it's one-row-at-a-time in the drawer with
no counterparty-grouped surface, and no persistence to *future* imports for
non-transfer verdicts (rules only set `category_id`).

## Verdict model — 3 treatments

A per-transaction treatment, ruled once per counterparty and generalized:

| Verdict | Meaning | Metrics effect | Storage |
|---|---|---|---|
| **Transfer** | own money / household-internal | excluded from income & expense | `transfer_override=1, is_transfer=1` (exists) |
| **Reimbursement** (settle-up) | shared costs / lending with a person | signed into **expense only**; never income | `settle_up=1` (new), `transfer_override=0` |
| **Real** | genuine income (+) or expense (−) | counts normally; expense may take a category | `transfer_override=0, settle_up=0` (decided-not-transfer) |

"Real" auto-labels income or expense by the amount sign — the user picks the
*treatment*, the sign picks the label.

### Settle-up math (the one novel rule)

A `settle_up=1` row contributes its **signed negation to expense and nothing to
income**: an outflow (`amount_cents<0`) adds `−amount_cents` to expense; an
inflow (`amount_cents>0`) subtracts `amount_cents` from expense. Worked example —
Joe (`$11,475` out, `$3,000` in):

```
expense += 11,475 − 3,000 = 8,475      income += 0
```

This is the true "we split costs" model: a repayment reduces what you spent, it
is never income.

## Data model — migration V049

`V049__settle_up_and_rule_treatment.sql`:

1. `ALTER TABLE transactions ADD COLUMN settle_up INTEGER NOT NULL DEFAULT 0;`
2. `ALTER TABLE rules ADD COLUMN treatment TEXT NOT NULL DEFAULT 'categorize';`
   — one of `categorize` | `transfer` | `settle_up`. Make `category_id` tolerate
   the non-categorize treatments (keep the column; it is only read when
   `treatment='categorize'`).

`transactions.is_reimbursable` is left untouched (its meaning — "an outflow I
expect to be repaid" — is different and out of scope; it nets nowhere today).

### Undecided predicate (what the review shows)

A row **needs review** when it is transfer-vocabulary / person-e-transfer AND
`transfer_override IS NULL AND settle_up = 0 AND category_id IS NULL AND
transfer_peer_id IS NULL`. Any verdict removes it from the queue:
- Transfer → `transfer_override=1`
- Settle-up → `settle_up=1, transfer_override=0`
- Real → `transfer_override=0`

## Metrics netting (`crates/finsight-core/src/metrics.rs`)

The income/expense split is a single `SUM(CASE …)` gated on `is_transfer = 0`,
in ~4 places (`cashflow` inner query lines ~179–180 / ~201–202 and the
month-weighted `_for` variants ~427–428). Change the CASE so settle-up rows net:

```sql
-- income: exclude settle-up rows entirely
COALESCE(SUM(CASE WHEN amount_cents > 0 AND settle_up = 0 THEN amount_cents ELSE 0 END), 0),
-- expense: normal outflows PLUS the signed negation of settle-up rows
COALESCE(SUM(
  CASE WHEN settle_up = 1 THEN -amount_cents
       WHEN amount_cents < 0 THEN -amount_cents
       ELSE 0 END), 0)
```

Savings rate, runway, and the Copilot context read `cashflow`, so they inherit
the netting for free. **Explicit requirement, not an assumption:** every query
that aggregates *expense* must apply the same rule, or a positive settle-up
inflow filtered by `amount_cents < 0` would be silently dropped instead of
netted. Audit and update each such query — at minimum the category-breakdown /
spending queries (in `repos/categories.rs` / `spending`) alongside `metrics.rs`.
A categorized settle-up inflow then reduces its own category; an uncategorized
one reduces total/uncategorized expense. The plan must enumerate every
expense-aggregating query and give each a test.

## Verdict machinery — generalize to 3 treatments + persist to future rows

1. **Generalize the existing per-verdict funnel.** Replace the binary
   `set_transfer_override` call-path with a 3-way verdict at the repo/command
   layer (`set_counterparty_verdict(id, verdict)` where verdict ∈
   {transfer, settle_up, real}). Transfer keeps today's semantics; settle_up sets
   `settle_up=1, transfer_override=0` and clears anomaly; real sets
   `transfer_override=0, settle_up=0`. `transfer_verdict_siblings` and the bulk
   apply generalize to the chosen verdict.
2. **Persist to future imports via a rule with a treatment.** Ruling a
   counterparty also upserts a `rules` row `{pattern:%name%, treatment}`. The
   categorizer, which already consults rules for categories, gains: on a rule
   match, `treatment='transfer'` → flag transfer; `treatment='settle_up'` → set
   `settle_up=1`; `treatment='categorize'` → today's behavior. **Precedence:** an
   explicit per-row `transfer_override` always wins over a rule treatment (the
   user's direct verdict beats the generalization), mirroring the existing
   sticky-override invariant.
3. Bare `INTERNET TRANSFER <ref>` rows (no counterparty name) have no `%name%` to
   generalize on — they remain per-row (transfer toggle) as today; the grouped
   surface lists them under an "unnamed internal transfers" bucket, not a person.

## The grouped review surface (Inbox)

A new card in the `Inbox` needs-review feed: **"People with unresolved money (N)"**.

```
People with unresolved money            $38,000 undecided · 5 people
  Swathi      11 txns   $19,360 out               [ Transfer ][ Settle-up ][ Real ]
  Joe         12 txns   $11,475 out · $3,000 in   [ Transfer ][ Settle-up ][ Real ]
  Vyshnavi     9 txns   $11,150 out               [ Transfer ][ Settle-up ][ Real ]
  Sathvik      9 txns   $10,927 out · $2,800 in   [ Transfer ][ Settle-up ][ Real ]
  Jin Y        3 txns    $2,555 out               [ Transfer ][ Settle-up ][ Real ]
```

- New read command `list_unresolved_counterparties()` → rows of
  `{pattern, label, txn_count, inflow_cents, outflow_cents}`, grouped by the
  `%name%` pattern over the undecided predicate, ordered by absolute net size.
- Each row's three buttons call the 3-way verdict with the counterparty pattern
  (bulk apply + rule upsert). The row disappears on decision; the Inbox review
  badge decrements.
- "Real" with an expense side offers an optional inline category (reuses the
  existing rule/category path); skippable.
- Amounts honor privacy mode (`className="money"`).

## Testing

Core (Rust):
- Settle-up netting: a `settle_up` inflow reduces expense and adds `0` income;
  Joe fixture nets to `$8,475` expense / `$0` income in `cashflow_between`.
- Verdict transitions: each of transfer/settle_up/real sets the right fields,
  clears category/anomaly appropriately, and removes the row from the undecided
  predicate; re-import keeps the verdict (sticky).
- Rule-treatment on import: a `%joe%` settle_up rule flags a *new* imported Joe
  row `settle_up=1`; an explicit per-row `transfer_override` beats it.
- `list_unresolved_counterparties` groups by counterparty with correct
  in/out/count and excludes decided/categorized/paired rows.
- Precision guards intact: bare `INTERNET TRANSFER` unaffected; no regression in
  the existing transfer/pairing tests.
- **audit_probe**: with the ambiguous friends ruled settle-up, expense drops to
  the netted figure and income sheds the repayments — measured, not asserted as a
  target (the probe stays a general harness; verdicts applied by pattern, not by
  hard-coding names).

Frontend (vitest): the counterparty list renders groups + net in/out; each verb
button fires the verdict mutation and optimistically removes the group; privacy
mode blurs amounts; a11y pass.

Bindings regenerated (`export_bindings`) after the new commands.

## Decisions made (this brainstorm)

- 3 treatments (Transfer / Settle-up / Real), settle-up = net-against-expense.
- New `settle_up` column (V049), not a repurpose of `is_reimbursable`.
- Surface lives in the Inbox needs-review feed, not a new nav item.
- Rules gain a `treatment` so non-category verdicts persist to future imports.

## Out of scope (v1)

- Per-direction verdicts (a counterparty is ruled as a whole).
- A full "who owes whom" ledger / settle-up balance tracker.
- Editing/undoing a counterparty verdict in bulk (per-row drawer still works;
  bulk re-rule is a later nicety).
- Touching `is_reimbursable` semantics.
