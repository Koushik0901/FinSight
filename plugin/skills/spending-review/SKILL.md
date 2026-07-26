---
name: spending-review
description: Explain where the user's money is going and what changed versus their normal. Use for "where is my money going", "why did I spend so much", "what changed this month", "what's driving the increase", or "how do I cut back".
---

Two different questions live here. Answer the one asked.

## "What changed?" (a comparison)

1. **`explain_spending_change`** with the period as `YYYY-MM`. It decomposes the
   change against a trailing baseline and returns per-driver deltas.
2. `classify_spending_period` if you need to characterise the month overall
   (was it unusual at all, or a normal month the user is anxious about?).

Report each driver with its own delta string, copied verbatim — do not compute
or re-sum them. Distinguish **a one-off** (a trip, an annual renewal) from **a
trend** (creeping subscriptions, higher grocery prices). They call for opposite
advice, and calling a one-off a trend makes the user cut something that was
never going to recur.

If the period looks normal, say so plainly. "Nothing unusual happened" is a
useful, honest answer.

## "Where does it all go?" (a breakdown)

1. **`get_spending_breakdown`** — per-month totals, top categories, and top
   merchants in one call. For a multi-month review this already returns each
   month; do not loop `search_transactions` per month.
2. `get_top_spending_categories` for a single-period category ranking.
3. `search_transactions` only when they want the actual line items (a merchant,
   a date range, an amount threshold).

Separate **fixed** costs (rent, insurance) from **controllable** ones. Naming
rent as their biggest expense is true and useless; the lever is the largest
discretionary category.

## "How do I cut back?"

**`plan_spending_reduction`** with a target. Report what it identifies rather
than inventing your own cuts, and be honest when the target isn't reachable
without touching something fixed.

Tie each suggestion to a real number from the data ("dining averaged $412/mo
over six months"), and prefer a few meaningful cuts to a long list of trivial
ones.

## Recording a verdict

If the user tells you a driver is a known one-off or an accepted cost
("that was the wedding", "the gym is deliberate"), call
`annotate_spending_driver` with their verdict so it stops resurfacing as a
lever. This **writes immediately** — only call it when they've actually said it,
never on your own inference.
