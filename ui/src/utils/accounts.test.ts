import { describe, it, expect } from "vitest";
import { getAccountDisplayName, getAccountTypeColor } from "./accounts";
import type { AccountSummary } from "../api/client";

function makeAccount(overrides: Partial<AccountSummary> = {}): AccountSummary {
  return {
    id: "a1",
    owner: "Me",
    bank: "Bank",
    type: "Checking",
    name: "ACT-db288194-14e1-4b85-b63c-68f109943901",
    balance_cents: 0,
    currency: "USD",
    color: "#fff",
    source: "simplefin",
    liquidity_type: "liquid",
    emergency_fund_eligible: true,
    goal_earmark: null,
    apy_pct: null,
    simplefin_account_id: "db288194-14e1-4b85-b63c-68f109943901",
    last_synced_at: null,
    nickname: null,
    connection_id: null,
    institution_id: null,
    external_account_id: null,
    official_name: null,
    mask: null,
    subtype: null,
    account_group: "cash",
    available_balance_cents: null,
    balance_date: null,
    extra_json: null,
    raw_json: null,
    import_pending: false,
    apr_pct: null,
    min_payment_cents: null,
    payoff_date: null,
    limit_cents: null,
    original_balance_cents: null,
    started_at: null,
    ...overrides,
  };
}

describe("getAccountDisplayName", () => {
  it("prefers nickname over official_name and name", () => {
    const account = makeAccount({
      nickname: "Daily Checking",
      official_name: "Chase Total Checking",
      name: "ACT-123",
    });
    expect(getAccountDisplayName(account)).toBe("Daily Checking");
  });

  it("falls back to official_name when nickname is absent", () => {
    const account = makeAccount({
      nickname: null,
      official_name: "Chase Total Checking",
      name: "ACT-123",
    });
    expect(getAccountDisplayName(account)).toBe("Chase Total Checking");
  });

  it("falls back to name when nickname and official_name are absent", () => {
    const account = makeAccount({
      nickname: null,
      official_name: null,
      name: "ACT-123",
    });
    expect(getAccountDisplayName(account)).toBe("ACT-123");
  });
});

describe("getAccountTypeColor", () => {
  it("returns a CSS variable tied to the account type", () => {
    expect(getAccountTypeColor("Checking")).toBe("var(--c-checking)");
    expect(getAccountTypeColor("Savings")).toBe("var(--c-savings)");
    expect(getAccountTypeColor("Credit")).toBe("var(--c-credit)");
    expect(getAccountTypeColor("Investment")).toBe("var(--c-investment)");
    expect(getAccountTypeColor("Cash")).toBe("var(--c-cash)");
    expect(getAccountTypeColor("Loan")).toBe("var(--c-loan)");
    expect(getAccountTypeColor("Other")).toBe("var(--c-other)");
  });
});
