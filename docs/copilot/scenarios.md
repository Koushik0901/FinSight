# Scenarios

Scenarios answer “what if?” with math, not vibes.

## Natural language → structured tools

On **Copilot** or **Scenarios**, type:

> “What if I cut dining by $200/month for a year and put it toward the emergency fund?”

The Copilot maps_free text to deterministic tool calls (`adjust_recurring`, `set_monthly_contribution`, `set_expense`, …). Those tools run in `finsight-core::forecast`, which projects balances forward from your current ledger plus recurring assumptions.

Results return as a dated projection with a narrative — the Copilot explains the trade-off, the tools provide the numbers.

## Saving and comparing

- **Save** a scenario to revisit after new imports — it refreshes against the latest ledger.
- **Compare** two saved scenarios to see which ends richer in 12 months.

## What the Copilot will not do

- Predict market returns or invent exchange rates.
- Summarize a single-currency subtotal as your whole position when unconverted holdings exist — the `CurrencyContext` block forbids it.
- Auto-save a scenario you have not approved.

See also: [Guide → Scenarios](/guide/scenarios), [Forecast in code](/developers/crates).
