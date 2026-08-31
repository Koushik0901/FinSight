# Budget

Envelope planning — decide what each dollar is for before the month spends it.

## Concepts

- **Envelope** — a category with a budgeted amount for the month. Budgets live in `crates/finsight-core::repos/budgets`.
- **To Budget** — income assigned vs unassigned. When To Budget is negative, you have planned more than you have.
- **Carryover** — unspent envelope amount that rolls to next month. Configure per envelope; useful for groceries, not for “guilt-free” where you want a reset.
- **Conscious Spending allocation** — every category is tagged as Need / Want / Saving / Investment. Budget shows the allocation donut and flags when wants creep past needs (Sethi).

## Planning a month

1. Open **Budget** and pick the month.
2. Set an amount per envelope. The “To Budget” header counts down.
3. Adjust until To Budget is 0 or positive with Savings ≥10% of income.
4. During the month, each envelope shows spent / budgeted, with a thin progress bar. Extra spend pulls from To Budget, not from other envelopes, unless you move it.
5. At month close, carryover is applied per envelope per `repos/month_close`.

## Per-person scope

Households can scope the “spent” side to one member’s ownership-weighted share via the scope pills. Budgets themselves stay household-level — only actuals are rescoped.

## Budget vs forecast

The “forecast” card shows deterministic end-of-month projection from burn rate and planned transactions. It is math from `finsight-core::forecast`, explained by the Copilot — not an LLM guess.

## Keyboard & accessibility

Envelopes are not draggable cards — the primary control is the amount input inside each row. The whole-budget grid lifts subtly on hover for reading focus, not to imply “this row is clickable”.

See also: [Categories](/guide/categories), [Cash Flow](/guide/cashflow), [Scenarios](/guide/scenarios).
