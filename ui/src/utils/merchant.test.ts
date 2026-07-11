import { describe, it, expect } from "vitest";
import { prettyMerchant } from "./merchant";

describe("prettyMerchant", () => {
  it("collapses statement column-alignment space runs", () => {
    expect(prettyMerchant("TIM HORTONS #3356       BURNABY")).toBe("TIM HORTONS #3356 BURNABY");
  });

  it("drops support-URL tails but keeps domain-style names", () => {
    expect(prettyMerchant("UBER EATS               HTTPS://HELP.UB")).toBe("UBER EATS");
    expect(prettyMerchant("SPOTIFY www.spotify.com STOCKHOLM")).toBe("SPOTIFY STOCKHOLM");
    // A domain IS the merchant's name — never dropped.
    expect(prettyMerchant("TEMU.COM                VICTORIA")).toBe("TEMU.COM VICTORIA");
  });

  it("never renames — words and casing stay exactly as the bank wrote them", () => {
    expect(prettyMerchant("Internet Banking E-TRANSFER 106001023942 Swathi")).toBe(
      "Internet Banking E-TRANSFER 106001023942 Swathi"
    );
  });

  it("falls back to the raw string rather than returning empty", () => {
    expect(prettyMerchant("https://only-a-url.example")).toBe("https://only-a-url.example");
  });
});
