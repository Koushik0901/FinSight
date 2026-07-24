import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { blurAmounts } from "./blurAmounts";

/**
 * Render a fragment in its own container (scoped per call so several renders in
 * one test don't collide on a shared body query) and return the blurred amounts
 * plus the full flattened text.
 */
function inspect(node: React.ReactNode) {
  const { container } = render(<>{node}</>);
  return {
    blurred: [...container.querySelectorAll("span.blurable")].map((s) => s.textContent),
    text: container.textContent,
  };
}

describe("blurAmounts", () => {
  it("wraps a single embedded amount in a .blurable span, leaving prose intact", () => {
    const r = inspect(blurAmounts("NETFLIX.COM (about $18) is due on 2026-09-03"));
    expect(r.blurred).toEqual(["$18"]);
    // Surrounding prose (merchant + date) stays as plain text, not blurred.
    expect(r.text).toBe("NETFLIX.COM (about $18) is due on 2026-09-03");
  });

  it("wraps every amount when several appear in one string", () => {
    expect(inspect(blurAmounts("$48,000 in · +$21,234 growth")).blurred).toEqual(["$48,000", "$21,234"]);
  });

  it("handles compact and cents forms ($35k, $3.50, CA$1.2M)", () => {
    expect(inspect(blurAmounts("worth $35k")).blurred).toEqual(["$35k"]);
    expect(inspect(blurAmounts("about $3.50 each")).blurred).toEqual(["$3.50"]);
    expect(inspect(blurAmounts("CA$1.2M portfolio")).blurred).toEqual(["CA$1.2M"]);
  });

  it("does NOT blur bare numbers — dates, counts, percentages stay visible", () => {
    const r = inspect(blurAmounts("due 2026-09-03, 7% a year, in 30 years"));
    expect(r.blurred).toEqual([]);
    expect(r.text).toBe("due 2026-09-03, 7% a year, in 30 years");
  });

  it("returns a plain string (no spans) when there is no amount", () => {
    const r = inspect(blurAmounts("Nothing else stands out in this window."));
    expect(r.blurred).toEqual([]);
    expect(r.text).toBe("Nothing else stands out in this window.");
  });
});
