import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useUpdateAccount, useArchiveAccount } from "./accounts";

vi.mock("../client", () => ({
  commands: {
    updateAccount: vi.fn().mockResolvedValue({ status: "ok", data: { id: "a1", name: "Updated", bank: "Chase", type: "Checking", last4: null, currency: "USD", color: "#fff", archived_at: null, created_at: "2024-01-01T00:00:00Z", owner: "Me" } }),
    archiveAccount: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("useUpdateAccount", () => {
  it("calls updateAccount and invalidates queries", async () => {
    const { result } = renderHook(() => useUpdateAccount(), { wrapper: createWrapper() });
    result.current.mutate({ id: "a1", patch: { name: "Updated", bank: null, account_type: null, color: null, last4: null, currency: null, nickname: null } });
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
