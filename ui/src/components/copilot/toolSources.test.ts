import { describe, it, expect } from "vitest";
import { sourcesFromToolTrace } from "./toolSources";

describe("sourcesFromToolTrace", () => {
  it("maps known tool names to source labels, de-duplicated and in first-seen order", () => {
    const trace = [
      "Called tool: search_transactions",
      "Called tool: get_goals",
      "Called tool: search_transactions",
    ];
    expect(sourcesFromToolTrace(trace)).toEqual(["Transactions", "Goals"]);
  });

  it("ignores trace lines that aren't tool calls and unknown tool names", () => {
    const trace = ["Tool error: some_unknown_tool", "Called tool: get_budgets"];
    expect(sourcesFromToolTrace(trace)).toEqual(["Budget"]);
  });

  it("returns an empty array for an empty or undefined trace", () => {
    expect(sourcesFromToolTrace(undefined)).toEqual([]);
    expect(sourcesFromToolTrace([])).toEqual([]);
  });
});
