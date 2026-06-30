import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useUpdateAccount, useArchiveAccount } from "./accounts";

vi.mock("../client", () => ({
  commands: {
    updateAccount: vi.fn().mockResolvedValue({ status: "ok", data: { id: "a1", name: "Updated", bank: "Chase", type: "Checking", last4: null, currency: "USD", color: "#fff", archived_at: null, created_at: "2024-01-01T00:00:00Z", owner: "Me", liquidity_type: "liquid", emergency_fund_eligible: true, goal_earmark: null, apy_pct: null, simplefin_account_id: null, last_synced_at: null, nickname: null, connection_id: null, institution_id: null, external_account_id: null, official_name: null, mask: null, subtype: null, account_group: "cash", available_balance_cents: null, balance_date: null, extra_json: null, raw_json: null, import_pending: false } }),
    archiveAccount: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("useUpdateAccount", () => {
  it("calls updateAccount and invalidates queries", async () => {
    const { result } = renderHook(() => useUpdateAccount(), { wrapper: createWrapper() });
    result.current.mutate({ id: "a1", patch: { name: "Updated", bank: null, account_type: null, color: null, last4: null, currency: null, liquidity_type: null, emergency_fund_eligible: null, goal_earmark: null, apy_pct: null, nickname: null, official_name: null, subtype: null, account_group: null, import_pending: null } });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.name).toBe("Updated");
  });
});

describe("useArchiveAccount", () => {
  it("calls archiveAccount", async () => {
    const { result } = renderHook(() => useArchiveAccount(), { wrapper: createWrapper() });
    result.current.mutate("a1");
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});
