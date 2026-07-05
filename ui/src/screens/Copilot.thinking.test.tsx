import { describe, it, expect } from "vitest";
import { splitReasoningIntoSteps } from "./Copilot";

describe("splitReasoningIntoSteps", () => {
  it("splits on sentence-ending punctuation followed by a space and a capital letter", () => {
    const text = "Housing is fixed. Dining is the lever. It's 13% over average.";
    expect(splitReasoningIntoSteps(text)).toEqual([
      "Housing is fixed.",
      "Dining is the lever.",
      "It's 13% over average.",
    ]);
  });

  it("returns a single-element array for text with no sentence breaks", () => {
    expect(splitReasoningIntoSteps("Just one clause")).toEqual(["Just one clause"]);
  });

  it("returns an empty array for empty/whitespace-only input", () => {
    expect(splitReasoningIntoSteps("")).toEqual([]);
    expect(splitReasoningIntoSteps("   ")).toEqual([]);
  });
});
