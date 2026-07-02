import { describe, expect, it } from "vitest";
import { Box, Bulb, Car, Cart, Fork, Gift, Heart, House, Plane, Tag } from "../components/Icons";
import { DEFAULT_CATEGORY_COLOR, iconFor, paletteFor } from "./categoryColor";

describe("paletteFor", () => {
  it("returns the canonical color for known starter ids", () => {
    expect(paletteFor("housing")).toBe("#A78BFA");
    expect(paletteFor("groceries")).toBe("#34D399");
    expect(paletteFor("dining")).toBe("#FB923C");
    expect(paletteFor("transport")).toBe("#60A5FA");
    expect(paletteFor("utilities")).toBe("#FACC15");
    expect(paletteFor("subscriptions")).toBe("#F472B6");
    expect(paletteFor("subs")).toBe("#F472B6");
    expect(paletteFor("health")).toBe("#2DD4BF");
    expect(paletteFor("shopping")).toBe("#FCA5A5");
    expect(paletteFor("travel")).toBe("#818CF8");
    expect(paletteFor("gifts")).toBe("#FDE68A");
  });

  it("falls back to the default grey for unknown ids", () => {
    expect(paletteFor("unknown")).toBe(DEFAULT_CATEGORY_COLOR);
    expect(paletteFor("")).toBe(DEFAULT_CATEGORY_COLOR);
  });

  it("contains only well-formed uppercase hex colors", () => {
    // Re-import the private palette indirectly through paletteFor checks.
    const ids = [
      "housing", "groceries", "dining", "transport", "utilities",
      "subscriptions", "subs", "health", "shopping", "travel", "gifts",
    ];
    for (const id of ids) {
      expect(paletteFor(id)).toMatch(/^#[0-9A-F]{6}$/);
    }
  });
});

describe("iconFor", () => {
  it("returns the canonical semantic icon for known starter ids", () => {
    expect(iconFor("housing")).toBe(House);
    expect(iconFor("groceries")).toBe(Cart);
    expect(iconFor("dining")).toBe(Fork);
    expect(iconFor("transport")).toBe(Car);
    expect(iconFor("utilities")).toBe(Bulb);
    expect(iconFor("subscriptions")).toBe(Box);
    expect(iconFor("subs")).toBe(Box);
    expect(iconFor("health")).toBe(Heart);
    expect(iconFor("shopping")).toBe(Tag);
    expect(iconFor("travel")).toBe(Plane);
    expect(iconFor("gifts")).toBe(Gift);
  });

  it("falls back to the generic tag icon for unknown ids", () => {
    expect(iconFor("unknown")).toBe(Tag);
    expect(iconFor("")).toBe(Tag);
  });
});
