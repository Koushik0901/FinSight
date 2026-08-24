import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useNeedsReviewCount, useTriggerCategorize } from "./agent";

vi.mock("../openapiClient", () => ({
  unwrap: async (p: Promise<{ status: "ok" | "error"; data?: unknown; error?: { message: string } }>) => { const r = await p; if (r.status === "error") throw new Error(r.error?.message ?? "command failed"); return r.data; },
  api: {
    getNeedsReviewCount: vi.fn().mockResolvedValue({ status: "ok", data: 3 }),
    triggerCategorize: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("useNeedsReviewCount", () => {
  it("returns count from command", async () => {
    const { result } = renderHook(() => useNeedsReviewCount(), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toBe(3);
  });
});

describe("useTriggerCategorize", () => {
  it("calls triggerCategorize", async () => {
    const { result } = renderHook(() => useTriggerCategorize(), { wrapper: createWrapper() });
    result.current.mutate();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});
