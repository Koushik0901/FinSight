# Categorization

How FinSight decides what category a transaction belongs to.

## Pipeline

`finsight-core::categorize` runs a strict priority:

1. **Rules** — pattern rules first; if one matches, category is set, no LLM call.
2. **LLM** — only for the uncategorized remainder, if a provider is configured and **Agent categorization** is enabled. Sends the redacted merchant description and amount — not the whole ledger — via `finsight-agent::categorizer`.
3. **Centroid fallback** — semantic similarity to category centroids (`repos/category_centroids`) learned from your past categorizations.
4. **Heuristics** — keyword and amount heuristics when nothing above fits.
5. **Uncategorized** — left for you in the review queue.

This means a rule always beats the model, and the model only sees what no rule covers.

## Confidence & review

Each LLM suggestion carries a confidence score (0–1, clamped). Low-confidence rows land in **Insights → Needs Review** and **Transactions → Review** instead of being auto-applied. Fixing them trains the centroid for next time.

## Local vs remote

| Mode | Data leaves server? |
|---|---|
| Ollama (local) | No — same payload over your LAN |
| Cloud provider | Yes — merchant + amount of uncategorized rows only |

The provider key is per-user inside `data.sqlcipher`. Disabling Agent categorization stops all LLM calls immediately.

## Tuning

- Create a **rule** for any merchant that is consistently mis-categorized — that merchant will never hit the LLM again.
- Correct a low-confidence row manually; the centroid learns the correction.
- Review **Rules → Proposals** to promote a repeated correction into a rule.

See also: [Rules](/automation/rules), [Provider config](/configuration/ollama).
