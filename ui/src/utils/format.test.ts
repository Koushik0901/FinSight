import { describe, it, expect } from "vitest";
import { money } from "./format";

describe("money", () => {
  it("defaults to USD, 0 decimals, comma-grouped", () => {
    expect(money(30000000)).toBe("$300,000");
  });

  it("rounds to whole dollars at 0 decimals", () => {
    expect(money(1482042)).toBe("$14,820");
  });

  it("renders cent precision with { decimals: 2 }", () => {
    expect(money(50000000, { decimals: 2 })).toBe("$500,000.00");
  });

  it("formats negatives at 2 decimals", () => {
    expect(money(-1599, { decimals: 2 })).toBe("-$15.99");
  });

  it("honors a non-USD currency", () => {
    expect(money(10000, { currency: "EUR" })).toBe("€100");
  });
});
