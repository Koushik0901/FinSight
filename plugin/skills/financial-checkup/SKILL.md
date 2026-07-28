---
name: financial-checkup
description: Review the user's overall financial position and tell them where they stand. Use for open-ended questions like "how am I doing", "give me a financial checkup", "what should I focus on", or "what's my situation".
---

A broad review. The goal is a picture the user can act on, not a wall of
numbers.

## Steps

1. **`get_financial_snapshot`** first. It returns balances, 90-day and 12-month
   income/expense averages, emergency-fund coverage, goals, liabilities,
   recurring bills, and data warnings in one call. Most checkup questions are
   answerable from this alone — don't fan out to a dozen tools before you've
   read it.
2. Follow up **only where the snapshot points somewhere specific**:
   - `get_data_quality_report` if it carries data warnings — say so before
     quoting figures built on thin history.
   - `get_net_worth` if they asked about net worth or total position.
   - `get_safe_to_spend` if the question is really "how much can I spend".
   - `explain_spending_change` if spending looks unlike their normal.
3. Lead with the answer, then the evidence.

## What a good answer covers

- **Where they stand**: liquid cash, monthly surplus or deficit, emergency-fund
  months, total debt.
- **The one thing that matters most right now.** Pick it and say why. A checkup
  that lists ten equal observations has made no judgement.
- **What to do next**: one or two concrete steps, sized to their actual surplus.

Order priorities the way the numbers demand, not by a fixed template. As a
default when nothing else dominates: cover an emergency-fund floor, then
high-APR debt, then goals. If emergency coverage is under one month, or APR and
minimum-payment data is missing, say the advice is provisional and name what's
missing.

## Avoid

- Reciting every field from the snapshot. Select.
- Praise or alarm the numbers don't support.
- Recommending a monthly contribution larger than their actual surplus.
