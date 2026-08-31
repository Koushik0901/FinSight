# Debt Snowball

> Pay the smallest balance first, for momentum. — *Ramsey*

## How FinSight orders debt

`wellness_context.debt_snowball` lists liabilities ordered by **remaining balance ascending**, smallest first. The order is deterministic from the ledger — no LLM ranking.

- Each debt entry shows name, remaining, and rate (when known).
- The smallest is surfaced as “pay this first” in Today’s next-action when debt exists.

## Snowball vs avalanche

Avalanche (highest rate first) is mathematically cheaper; snowball is behaviourally more robust. FinSight implements snowball as the default order and explains the trade-off when asked. If you prefer avalanche, order debts by rate mentally — the ledger supports either.

## Using it

- **Journey → Debt free** — tracks non-mortgage debt cleared against the snowball.
- **Scenarios** — “What if I add $300/month to the smallest debt while holding the others at minimums?” projects payoff date under your actual balances.
- **Copilot** — “Which debt should I pay first?” returns the snowball with balances and a plain explanation.

Clearing a debt removes it from the snowball; the next-smallest becomes the focus automatically.
