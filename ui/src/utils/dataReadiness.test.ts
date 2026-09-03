import { describe, expect, it } from "vitest";
import type { AccountSummary, MonthSummary } from "../api/openapiClient";
import {
  getBudgetReadiness,
  getReportReadiness,
  normalizeCount,
} from "./dataReadiness";

const month = (
  incomeCents: number,
  expenseCents: number,
  label = "Jan",
): MonthSummary => ({
  month: "2026-01",
  label,
  incomeCents,
  expenseCents,
  netCents: incomeCents - expenseCents,
});

const account = (balanceKnown: boolean | undefined): AccountSummary => ({
  id: "a1",
  owner: "Me",
  bank: "Bank",
  type: "Checking",
  name: "Checking",
  balance_cents: 0,
  balance_known: balanceKnown,
  balance_source: balanceKnown === false ? "seed" : "manual",
  currency: "USD",
  color: "#888888",
  source: "manual",
  goal_earmark: null,
  apy_pct: null,
  simplefin_account_id: null,
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
  promo_apr_expires_on: null,
  post_promo_apr_pct: null,
});

describe("getBudgetReadiness", () => {
  it("treats missing or unfunded envelopes as unavailable", () => {
    expect(getBudgetReadiness({
      envelopeCount: 0,
      fundedEnvelopeCount: 0,
      transactionCount: 0,
      spentCents: 0,
      dayOfMonth: 20,
    })).toBe("unavailable");

    expect(getBudgetReadiness({
      envelopeCount: 3,
      fundedEnvelopeCount: 0,
      transactionCount: 12,
      spentCents: 45000,
      dayOfMonth: 20,
    })).toBe("unavailable");
  });

  it("keeps a funded plan in estimated state until activity exists", () => {
    expect(getBudgetReadiness({
      envelopeCount: 3,
      fundedEnvelopeCount: 2,
      transactionCount: 0,
      spentCents: 0,
      dayOfMonth: 20,
    })).toBe("estimated");
  });

  it("allows directional language only after observed coverage", () => {
    expect(getBudgetReadiness({
      envelopeCount: 3,
      fundedEnvelopeCount: 2,
      transactionCount: 10,
      spentCents: 45000,
      dayOfMonth: 4,
    })).toBe("reliable");
  });
});

describe("getReportReadiness", () => {
  it("does not turn absent data into zero-valued metrics", () => {
    expect(getReportReadiness([], [], null)).toEqual({
      hasActivity: false,
      savingsRate: "unavailable",
      averageSpend: "unavailable",
      netWorth: "unavailable",
      runway: "unavailable",
    });
  });

  it("tracks metric readiness independently", () => {
    const result = getReportReadiness(
      [month(500000, 300000), month(510000, 310000, "Feb")],
      [account(true)],
      75,
    );

    expect(result).toEqual({
      hasActivity: true,
      savingsRate: "reliable",
      averageSpend: "reliable",
      netWorth: "reliable",
      runway: "reliable",
    });
  });

  it("rejects an account balance explicitly marked unknown", () => {
    expect(getReportReadiness([], [account(false)], null).netWorth).toBe("unavailable");
  });
});

describe("normalizeCount", () => {
  it("turns missing, invalid, and negative values into zero", () => {
    expect(normalizeCount(undefined)).toBe(0);
    expect(normalizeCount(Number.NaN)).toBe(0);
    expect(normalizeCount(-2)).toBe(0);
    expect(normalizeCount(2.9)).toBe(2);
  });
});
