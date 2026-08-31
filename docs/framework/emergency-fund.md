# Emergency Fund

> 3–6 months of expenses, saved before ambitious plans. — *Ramsey* / *Sethi*

## The metric

`emergency_fund_months = total_balance / average_monthly_expenses`

Computed in `wellness_context` from your ledger. A reliability flag suppresses the headline when history is thin — a brand-new user with nine months of cover from a one-time balance is shown as provisional.

## Using it

- **Goals → Emergency fund** — quick-fill computes the shortfall to the next month of cover and creates a contribution in one tap.
- **Today → Progress disclosure** — the milestone card shows months of cover and the inputs behind it.
- **Copilot** — “How long until my emergency fund is full at $400/month?” runs the contribution projection and cites the months.

## Policy

FinSight does not treat the emergency fund as a category — it is a goal funded from Saving envelopes. Fund it first; the Copilot ranks it above debt payoff only when debt is non-urgent, and the prompt encodes that ordering.

Typical order: stability → one month → three–six months → debt payoff → compounding.
