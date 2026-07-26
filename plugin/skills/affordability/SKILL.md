---
name: affordability
description: Decide whether the user can afford a specific purchase, and how much is safe to spend. Use for "can I afford X", "should I buy this", "how much can I spend this month", or "what's my safe-to-spend".
---

## A specific purchase

**`run_purchase_affordability`**, passing the amount as
`purchase_amount_cents` (an integer number of cents — $450 is `45000`).

If the user hasn't given an amount, **ask for it**. Do not pick a plausible
number and compute on it; the whole answer is a function of that figure.

Base the verdict on emergency cash, monthly surplus, upcoming obligations, and
high-interest debt. **Be cautious**: don't approve a purchase that would drop
them below their emergency floor or lean on high-APR debt.

A good answer is a clear yes or no, the one number that decides it, and the
caveat. "Yes — $450 is about 3% of your liquid cash, and your emergency fund
stays above four months" beats a hedged paragraph.

When the answer is no, say what *would* make it yes: waiting a month, funding it
from a specific underspent category, or a smaller amount.

## "How much can I spend?"

**`get_safe_to_spend`** — a near-term daily projection of dated obligations plus
everyday burn, minus a buffer.

- `horizon_days` (7–90, default 30) for the window.
- `buffer_cents` for a floor they want to keep untouched.
- `extra_expense_cents` to test a hypothetical on top.

This figure is deliberately conservative and can *understate* what's available.
That's the correct direction to err — say so rather than talking the number up.
Nothing here is saved; the buffer and hypothetical are what-if inputs only.

## Before recommending they spend a surplus

Check `plan_sinking_funds` and upcoming recurring bills. A surplus that hasn't
had a known annual bill netted out of it isn't really free.
