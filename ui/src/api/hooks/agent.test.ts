import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useNeedsReviewCount, useTriggerCategorize } from "./agent";

vi.mock("../client", () => ({
  commands: {
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
