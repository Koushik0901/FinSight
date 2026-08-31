# Automation — Overview

FinSight automates the repetitive parts and leaves the decisions with you.

| Mechanism | Trigger | Review |
|---|---|---|
| **Rules** — pattern & treatment | Incoming transactions | Enable/disable; counts shown |
| **Categorization** — LLM + deterministic fallbacks | Uncategorized queue | Confidence; needs-review filter |
| **Recipes** — trusted multi-step bundles | You do | Plan shown; confirm once |

All three are deterministic first, LLM second, and never silent — you see the proposal and the effect before it lands.

## Choosing

- Use a **rule** when the same merchant should always get the same category/treatment.
- Trust **categorization** for the long tail where no rule exists — the Copilot explains low-confidence calls.
- Run a **recipe** for one-off, multi-step housekeeping (e.g. “Create next month’s envelopes from actuals”).

Next: [Rules](/automation/rules).
