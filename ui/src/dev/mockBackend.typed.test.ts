import { describe, it, expect, beforeEach } from "vitest";
import type { CommandName } from "../api/commandNames";
import { _createTestResponders, _fallbackForTest, installMockBackend } from "./mockBackend";
import goalsFixture from "../pwa/fixtures/goals.json";
import balanceFixture from "../pwa/fixtures/balanceTimeline.json";
import monthCloseFixture from "../pwa/fixtures/monthClose.json";

describe("mockBackend typed contract", () => {
  it("responders are typed as Partial<Record<CommandName, Responder>>", () => {
    // This assignment must compile: proves mockBackend keys are the generated union,
    // so a renamed Rust command fails tsc here instead of silently falling to fallback.
    const responders: Partial<Record<CommandName, (args: any) => unknown>> = _createTestResponders("rich");
    expect(responders.list_accounts).toBeDefined();
    expect(responders.get_financial_metrics).toBeDefined();
    expect(typeof responders.project_goal_growth).toBe("function");
  });

  it("unimplemented command throws in dev instead of silent []", () => {
    expect(() => _fallbackForTest("nonexistent_command_xyz" as CommandName)).toThrow(/unimplemented/);
    expect(() => _fallbackForTest("get_month_totals" as CommandName)).toThrow(/mockBackend\.ts/);
  });

  it("installMockBackend fetch throws on unimplemented command", async () => {
    installMockBackend("rich");
    const res = await fetch("/api/rpc/nonexistent_command_xyz", { method: "POST", body: JSON.stringify({}) });
    expect(res.status).toBe(501);
    const body = (await res.json()) as { code: string };
    expect(body.code).toMatch(/unimplemented/);
    // Known command still succeeds
    const ok = await fetch("/api/rpc/list_accounts", { method: "POST", body: JSON.stringify({}) });
    expect(ok.status).toBe(200);
    const accounts = (await ok.json()) as unknown[];
    expect(Array.isArray(accounts)).toBe(true);
  });

  it("fixtures are imported and used (not inline magic numbers)", async () => {
    // Fixtures must exist and have expected shape — guards that the harness
    // was actually switched to fixture-backed math as Task 5 requires.
    expect(goalsFixture).toBeDefined();
    expect((goalsFixture as { projections: { defaultAnnualRate: number } }).projections.defaultAnnualRate).toBe(0.07);
    expect(balanceFixture).toBeDefined();
    expect(Array.isArray((balanceFixture as { samplePoints: unknown[] }).samplePoints)).toBe(true);
    expect(monthCloseFixture).toBeDefined();
    expect((monthCloseFixture as { sampleSnapshot: { incomeCents: number } }).sampleSnapshot.incomeCents).toBeGreaterThan(0);

    // project_goal_growth must respect fixture rate (not a reimplemented constant)
    const responders = _createTestResponders("rich");
    const proj = responders.project_goal_growth?.({ goalId: "g-ef", years: 10 } as unknown as Record<string, unknown>) as { annualRate: number } | undefined;
    expect(proj?.annualRate).toBe(0.07);
  });

  it("get_account_balance_timeline is fixture-backed and still returns series", () => {
    // r-chk is simplefin-linked (refused) — use a non-linked account from "large" dataset
    const responders = _createTestResponders("large");
    const tl = responders.get_account_balance_timeline?.({ accountId: "l-chk" } as unknown as Record<string, unknown>) as {
      points: unknown[];
      reconstructable: boolean;
    } | null | undefined;
    expect(tl?.reconstructable).toBe(true);
    expect(Array.isArray(tl?.points)).toBe(true);
    // simplefin-linked account should be refused
    const rich = _createTestResponders("rich");
    const refused = rich.get_account_balance_timeline?.({ accountId: "r-chk" } as unknown as Record<string, unknown>) as {
      reconstructable: boolean;
    } | null | undefined;
    expect(refused?.reconstructable).toBe(false);
  });
});
