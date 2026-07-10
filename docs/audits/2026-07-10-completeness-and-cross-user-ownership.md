# FinSight Completeness & Cross-User Ownership Roadmap — 2026-07-10 (v2)

**Mandate (user goal):** make FinSight a complete, production-quality personal
finance app the user can use daily *and* hand to a girlfriend/family members who
each run their own private local app. Audit the whole product for bugs, weak
workflows, misleading calculations, unfinished features, poor UX, and high-value
gaps; then autonomously prioritize, implement, integrate, test, and polish until
functionally complete. **New architectural pillar:** shared accounts, assets,
liabilities, and expenses must be represented correctly across separate users'
apps *without double-counting ownership*.

**Relationship to the prior audit:** `2026-07-10-finsight-product-audit.md` (P0–P2)
is **implemented and committed**. This document is the *next* cycle: the
cross-user ownership model + a fresh regression/UX/correctness sweep on top of the
now-fixed pipeline.

---

## Part 1 — Cross-user shared ownership (central new pillar)

### 1.0 The key insight (why this is small, not new architecture)

The metrics layer already enforces a reconciliation contract (metrics.rs:296–309):

> `Σ(every member's slice) + unassigned_residual == household total`

In the cross-app world, the **residual is exactly "the share owned by people who
live in other apps."** If the operator owns 50% of a joint account, their slice is
50% and the residual is 50%; combining partner A's app (operator slice 50%) with
partner B's app (operator slice 50%) counts the account once. **No sync, no shared
IDs, no double-counting** — it falls out of the existing contract the moment the
share is explicit rather than hard-coded `1/n`.

So the whole pillar reduces to three additive changes:

1. Make the ownership share **explicit** (`share_bps`, basis points) instead of
   the implicit `1/owner_count`, defaulting to equal-split when unset.
2. Define **"my finances"** = the operator member's slice-sum (the `member_id`
   metrics filter from P0-2 already computes this).
3. Extend ownership from accounts to **`manual_assets`** (and debts, which are
   already accounts) so a shared house/car/mortgage apportions too.

### 1.1 DECISION — shares weight both stock and flow (documented)

A share can weight **balances only** or **balances + transaction flows**. FinSight's
existing model already weights *both* by `1/owner_count`
(`weighted_income_expense` splits income/expense by the same account weight as
`balance_breakdown`). We **keep that**: `share_bps` weights balances *and* flows.

- **Net worth (stock):** operator's share of each shared account/asset/liability.
  Unambiguously correct.
- **Cashflow (flow):** operator's share of a shared account's income/expense. This
  is an *approximation* — if A pays rent and B pays groceries from a joint account,
  a fixed share misrepresents both. It is, however, the *same* approximation the
  app already ships, made *better* by explicit shares (70/30 instead of forced
  50/50). **Per-transaction attribution override** (mark one joint-account
  transaction as 100% A's) is the precise refinement and is **deferred** — noted
  in §3 as future work, not built now.

### 1.2 Invariants (must hold)

- **Solo-user safety:** a user with **zero** household members behaves *exactly*
  as today. "Self"/operator is **inferred and optional** — never a mandatory
  onboarding step. Handing the app to a non-technical family member must require
  no identity setup.
- **Migration safety:** additive columns only; existing DBs upgrade to *implicit
  equal-split*; **no recomputation of any persisted number**. `share_bps NULL`
  ⇒ fall back to `1.0/owner_count` (byte-identical to today's math).
- **Reconciliation (the acceptance test):** with explicit shares, `A-slice +
  B-slice + residual == household total` for balances and flows; a 70/30 joint
  account attributes 70/30; a sole account attributes 100%; an empty-member DB is
  identical to today.

### 1.3 Build order (each step ends green + committed)

1. **Unify the weight source.** The `1/n` weight is computed in ≥3 SQL sites
   (`account_weights_for_member`, the inline join in `weighted_income_expense`,
   `get_member_financial_summary` in reasoning/tools/read.rs). Collapse to **one**
   canonical weight expression/CTE *before* changing behavior, so the
   generalization happens in one place and can't drift.
2. **Schema (additive):** `account_owners.share_bps INTEGER NULL`; a new
   `asset_owners(asset_id, member_id, share_bps)` mirroring `account_owners`; an
   optional `household_members.is_self` (or a settings key) to mark the operator.
   Backfill nothing.
3. **Generalize the weight:** `1.0/oc.n` → `COALESCE(share_bps/10000.0, 1.0/oc.n)`
   in the single unified source. Extend `balance_breakdown`/net-worth to include
   `manual_assets` ownership.
4. **Verify:** probe + reconciliation unit tests (70/30, sole, empty-member,
   over/under-100% guard) green; existing per-member tests unchanged.
5. **Surface:** operator/self + share editor in the household/account UI; "Mine
   vs Household" framing on the per-person switcher; Copilot member-awareness
   already exists (P0-2) — feed explicit shares through it.

---

## Part 2 — Fresh audit findings (empirical: probe on samples/ + code/UX sweep)

Probe on `samples/` (3,213 txns, 6 bank accounts; the TFSA is NOT yet importable
— see F1). Headline: rolling-90 **income $6,270/mo, expense $7,725/mo, net
−$1,455, savings −23%**; May/Jun net −$1,490 / −$3,029. These are still fiction —
the user saves money. Ranked by impact:

### F0 — Transfer detection STILL leaks; headline numbers still wrong (highest impact)
The prior P0-1 fix improved flagging (287→**400** legs) but the probe still lists
**$7,302 outflow / $2,000 inflow of unflagged transfer-like rows** leaking into
expense/income, and anomalies are still transfer-dominated. Two tractable,
high-confidence gaps:
- **Bare `Internet Banking INTERNET TRANSFER 000000NNNNNN`** (no "FROM/TO ACCOUNT"
  suffix, just a reference number) is unflagged — the keyword/pair rules miss it.
  Several have equal-amount opposite legs across the user's own accounts (e.g. two
  −$2,000 `INTERNET TRANSFER 135957/135006` show up as *both* top anomalies and
  expenses). Same-reference / equal-amount cross-account pairing should catch these.
- **`INTERAC e-Transfer From: <operator's own name>`** ($1,500, self→self) is
  counted as **income**. A self-name signal (ties into the operator/"self" member
  of Part 1) flags own e-transfers.
- **Genuinely ambiguous** person-to-person e-transfers (swathi / SATHVIK) inflate
  income and need a *user review affordance* ("is this income or a transfer/
  reimbursement?"), not a silent guess. Prior audit's P0-1(d) proposed this.
**Acceptance:** probe expense ≈ real (~$2.5–3.5k/mo), savings rate positive; bare
INTERNET TRANSFER legs flagged/paired; self e-transfers not income; anomaly top-10
free of self-transfers; no regression on the INTERAC-purchase non-transfer test.

### F1 — Investment/brokerage CSV import unsupported (user added a TFSA today)
`samples/wealthsimple-tfsa-all-time-statement.csv` is a **brokerage activity**
statement (`activity_type, symbol, quantity, unit_price, commission,
net_cash_amount`; BUY/SELL trades + MoneyMovement) — a different shape than the
bank CSVs `CsvProvider` handles. Investment accounts only exist via SimpleFin
(`simplefin/holdings.rs`); **local CSV import of investments is unsupported**, so a
real user can't track holdings/market value — a huge part of net worth. `probe
invested_cents: 0`. Needs: a Wealthsimple/brokerage CSV shape, an Investment
account with holdings + market value (not just cash), and correct
liquid/EF/invested classification (TFSA is invested, not liquid, not EF-eligible).

### F2 — Categorization coverage still poor: 879 rows / $141,698 uncategorized
Uncategorized dwarfs every real category (Travel $20.6k, Dining $12.8k). Part is
the F0 transfer leak; the rest is real spend with no builtin match. Extend the
builtin map (Canadian merchant families) and/or make the AI pass more reachable.

### F3 — Housing still invisible: 1 transaction, $192.88 in 3 years
The prior P1-1 e-transfer-disposition fix did not land the user's rent (paid by
e-transfer). "Where does my money go" is still wrong about the biggest line item.
Rides F0's pairing (an e-transfer with no matching own-account leg is a payment,
not a transfer) + a rent-recognition review.

### F4 — Anomalies still transfer-dominated
8+ of the top anomalies are internet transfers / bill pays / a "GLOBAL MONEY
TRANSFER FEE $0.00" row. Heals substantially once F0 lands; re-verify after.

### F5 — Investment-account classification (advisor-flagged, verify after F1)
Once an Investment account exists: confirm it is net-worth *invested* (not
liquid), NOT emergency-fund-eligible, and its contributions aren't counted as
spending. `is_investment_type`/`is_liquid_type` exist; wire the import to set the
type and assert in the probe.

---

## Part 3 — Explicitly deferred (not built this cycle; recorded so they aren't lost)

- Cross-app **shared-item identifier / reconciliation key** — no consumer without
  a sync feature; defer until sync/merge exists.
- **Per-transaction ownership attribution** override on joint accounts (the
  precise refinement to §1.1's flow approximation).
- Envelope-per-person budgets (budgets stay household-level, per P0-2's v1 call).

---

## Regression gates (every step)
`cargo test --workspace --release` · `cd ui && npx vitest run` · `npx tsc --noEmit`
· `audit_probe` on `samples/` (reconciliation + investment-account assertions) ·
`cd eval && python run_eval.py` for anything touching the agent.
