---
name: debt-payoff
description: Compare debt payoff strategies and build a payoff plan. Use for "snowball vs avalanche", "which debt should I pay first", "how fast can I be debt free", "should I pay off debt or save", or questions about extra payments.
---

## Which debt first

**`rank_debt_payoff`** returns the ordering. It defaults to the strategy the
user has already chosen in FinSight — respect that default rather than
substituting your own preference.

## Comparing strategies — report both sides

For "snowball vs avalanche", "which method is better", or any question weighing
one order against another, call **`compare_payoff_strategies`** and report
**both**:

- the total interest difference, and
- when each clears its *first* debt.

Never present one strategy as the only answer. Avalanche wins on arithmetic;
snowball wins on momentum, and the user is the one who has to keep going. Say
which one their setting selects and why the other might still suit them. The
tradeoff is theirs to make.

To model a hybrid — clear a small nuisance balance first, then optimise the rest
by APR — pass those account ids as `custom_order`.

## Timelines and extra payments

**`run_debt_payoff_scenarios`** with `extra_monthly_payment_cents` for "what if
I put another $200 at it". Report the payoff date and total interest saved, and
sanity-check the extra against their actual monthly surplus — a plan funded by
money they don't have is not a plan.

## Debt versus saving

**`compare_debt_vs_goal`** for "should I pay down the card or fund the goal".
Note where the emergency fund stands first: draining it to clear debt usually
just recreates the debt on the next surprise. A small buffer, then high-APR
debt, is the safer default.

## Sinking funds

**`plan_sinking_funds`** covers known amounts due on known dates (insurance,
property tax). Report the monthly figure as a **requirement, not a suggestion** —
the date is fixed, so the number is arithmetic and missing it has a real
consequence. Lead with anything overdue.

Check these before recommending what to do with spare money. They're
commitments against the same surplus that debt payments and goals compete for,
so ignoring them overstates what's actually free.

## Caveats worth stating

If APR or minimum-payment data is missing for any account, say the ranking is
provisional and name what's missing — payoff order is driven by exactly those
fields.
