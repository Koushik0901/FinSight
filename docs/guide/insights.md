# Insights

Insights surfaces what deserves your attention before it becomes a problem.

## Tabs

- **Anomalies** — outlier spend per category, using rolling baseline + z-score in `finsight-core::anomaly`. A spike from a known one-off (e.g. annual insurance) is flagged as “explainable” when prior-year history matches.
- **Patterns** — spending type drift, subscription creep, and budget forecast variance. Source: `spending/baseline` and `forecast`.
- **Memory** — agent memory items the Copilot has stored (`repos/agent_memory`). Review and delete anything wrong.
- **Needs Review** — the actionable queue: uncategorized, low-confidence categorizations, possible transfers, possible splits.

## Agent Memory

The Copilot can store small, reviewable memory items (e.g. “User prefers grocery budget at $600”). They are rows in your encrypted database, shown on Insights → Memory, and fed into the context only when relevant. Delete any you disagree with — the Copilot will not re-create it without evidence.

## Working the queue

The fastest path through Insights:

1. Clear **Needs Review** first — categorize, merge, or split. Each fix improves tomorrow’s baseline.
2. Read **Anomalies** next — decide if the spike is a one-off or a new normal.
3. Skim **Patterns** before month close — shift one envelope instead of re-budgeting everything.

See also: [Anomalies in code](/developers/crates), [Copilot → Memory](/copilot/memory).
