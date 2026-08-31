# Cash Flow

When money is expected to arrive and leave, and what is safe to spend.

## Safe-to-spend

Safe-to-spend = current balance + scheduled income − scheduled expenses − reserve.

- **Scheduled** comes from confirmed recurring items and planned transactions.
- **Reserve** is your emergency-fund buffer (or a manual cushion). The screen lets you tune it and see the effect live.
- The figure is shown with segments: Today, Horizon, and Safe. Changing the horizon (7/14/30 days) re-slices the same calculation; no new data is fetched.

Source: `finsight-core::forecast` + `finsight-core::cashflow`. Deterministic math; the Copilot explains the inputs when asked.

## Calendar & events

The calendar renders scheduled inflows/outflows by date. Each event shows merchant, amount, and source (recurring vs planned transaction). A row without a firm date is rendered as “around the 15th” with the tolerance noted.

## Inputs & assumptions

The inspector lists:

- Definition of safe-to-spend
- Inputs (balances, recurring, planned)
- Exclusions (unconverted currencies, incomplete history)
- Assumptions (cadence, reserve)
- Period and data-quality warnings

If the figure looks wrong, the inspector is the place to find out why — values come from the same metrics layer as every other screen.

## Planned transactions

Create a future transaction (e.g. quarterly tax) to model its effect without booking it yet. Planned items feed Cash Flow and Scenarios; they are not real transactions until you convert them.

See also: [Recurring](/guide/recurring), [Scenarios](/guide/scenarios).
