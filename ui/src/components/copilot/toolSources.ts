/**
 * Maps a tool name prefix/substring to the mockup's source-rail label.
 * Matching is first-wins: if a tool name matches multiple patterns, the
 * first match in array order is used. Known cases where this matters:
 *   - "compare_debt_vs_goal" → Liabilities (not Goals)
 *   - "list_uncategorized_transactions" → Transactions (not Categories)
 */
const TOOL_TO_SOURCE: Array<[match: RegExp, label: string]> = [
  [/transaction/i, "Transactions"],
  [/account|liquid|balance/i, "Accounts"],
  [/liabilit|debt/i, "Liabilities"],
  [/goal/i, "Goals"],
  [/budget/i, "Budget"],
  [/recurring/i, "Subscriptions"],
  [/categor/i, "Categories"],
];

/**
 * Derives the ordered, de-duplicated list of data-source labels touched
 * this turn from `MessageMeta.toolTrace` entries shaped like
 * "Called tool: search_transactions" (see engine/mod.rs's `trace.push`).
 */
export function sourcesFromToolTrace(trace: string[] | undefined): string[] {
  if (!trace || trace.length === 0) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const line of trace) {
    const m = /^Called tool: (\S+)/.exec(line);
    if (!m) continue;
    const toolName = m[1]!;
    const hit = TOOL_TO_SOURCE.find(([re]) => re.test(toolName));
    if (!hit) continue;
    if (seen.has(hit[1])) continue;
    seen.add(hit[1]);
    out.push(hit[1]);
  }
  return out;
}
