import { describe, it, expect } from "vitest";
import { money, compactMoney } from "./format";

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

  it("accepts a lowercase code, since import sources are inconsistent", () => {
    expect(money(10000, { currency: "eur" })).toBe("€100");
  });

  // Account currencies come from arbitrary CSV imports. `Intl.NumberFormat`
  // throws a RangeError on anything that isn't three letters, which would blank
  // the whole screen rather than one number.
  it.each(["US Dollar", "", "USDD", "12", "$"])(
    "renders rather than throwing on the unusable code %o",
    (code) => {
      expect(() => money(123456, { currency: code })).not.toThrow();
      expect(money(123456, { currency: code })).toContain("1,235");
    },
  );

  it("labels an unusable code with the raw value instead of guessing a symbol", () => {
    // Silently formatting as dollars would assert a currency the data never
    // claimed — the exact failure this guards.
    expect(money(123456, { currency: "GOLD" })).toBe("GOLD 1,235");
  });
});

describe("compactMoney", () => {
  it("survives an unusable currency code too", () => {
    expect(() => compactMoney(123456789, { currency: "not a code" })).not.toThrow();
  });

  it("still abbreviates for a valid code", () => {
    expect(compactMoney(13750000, { currency: "USD" })).toBe("$137.5K");
  });
});
