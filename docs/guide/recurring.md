# Recurring

Find subscriptions and bills before they surprise you.

## Two views

- **Calendar** — recurring items placed on the dates they are expected. Useful for the next 30-day outlook.
- **List** — every detected recurring group, with cadence, typical amount, and history count.

Source: `finsight-core::subscriptions` and `finsight-core::recurring`. Detection is deterministic over merchant + amount tolerance + interval regularity; the LLM is never asked “is this recurring?”.

## Detection

A group becomes recurring when it hits the cadence and regularity thresholds (e.g. three charges from the same merchant within ±10% amount and ±3 days of a 30-day cadence). Variable-amount bills (utilities) match on merchant and cadence, not exact amount.

## Managing

- **Confirm** — mark a group as a known subscription (adds it to the calendar even if amount drifts).
- **Dismiss** — hide a spurious group; it will not reappear unless the cadence changes.
- **Edit** — adjust merchant grouping or cadence. Edits are local; no data leaves the machine.

## Sync with Cash Flow

Confirmed recurring items feed the cash-flow calendar and the safe-to-spend calculation as scheduled outflows. A skipped month does not break the model — the forecast exposes the assumption and shows the warning.

See also: [Cash Flow](/guide/cashflow), [Insights](/guide/insights).
