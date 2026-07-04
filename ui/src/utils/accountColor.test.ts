import { describe, expect, it } from "vitest";
import { accountTypeColor, DEFAULT_ACCOUNT_TYPE_COLOR } from "./accountColor";
import { CATEGORY_COLOR_CHOICES } from "./categoryColor";

describe("accountTypeColor", () => {
  it("maps every account type the app offers to a stable color", () => {
    // The AccountDrawer type radio list — each must have a distinct color.
    const types = ["Checking", "Savings", "Credit", "Investment", "Cash", "Loan"];
    const colors = types.map((t) => accountTypeColor(t));
    expect(new Set(colors).size).toBe(types.length);
    for (const color of colors) {
      expect(color).toMatch(/^#[0-9A-F]{6}$/i);
      expect(color).not.toBe(DEFAULT_ACCOUNT_TYPE_COLOR);
    }
  });

  it("is case- and whitespace-insensitive", () => {
    expect(accountTypeColor("checking")).toBe(accountTypeColor("Checking"));
    expect(accountTypeColor(" SAVINGS ")).toBe(accountTypeColor("Savings"));
  });

  it("falls back to grey for unknown, empty, and missing types", () => {
    expect(accountTypeColor("Other")).toBe(DEFAULT_ACCOUNT_TYPE_COLOR);
    expect(accountTypeColor("")).toBe(DEFAULT_ACCOUNT_TYPE_COLOR);
    expect(accountTypeColor(null)).toBe(DEFAULT_ACCOUNT_TYPE_COLOR);
    expect(accountTypeColor(undefined)).toBe(DEFAULT_ACCOUNT_TYPE_COLOR);
  });

  it("never collides with the category palette (accounts ≠ categories)", () => {
    for (const t of ["Checking", "Savings", "Credit", "Investment", "Cash", "Loan"]) {
      expect(CATEGORY_COLOR_CHOICES).not.toContain(accountTypeColor(t));
    }
  });
});
