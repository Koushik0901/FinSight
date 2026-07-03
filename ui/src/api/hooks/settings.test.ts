import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useAutoCategorizeEnabled, useSetAutoCategorizeEnabled } from "./settings";

vi.mock("../client", () => ({
  commands: {
    getAutoCategorizeEnabled: vi.fn().mockResolvedValue({ status: "ok", data: true }),
    setAutoCategorizeEnabled: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

describe("useAutoCategorizeEnabled", () => {
  it("returns the enabled value from the backend", async () => {
    const { result } = renderHook(() => useAutoCategorizeEnabled(), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toBe(true);
  });
});

describe("useSetAutoCategorizeEnabled", () => {
  it("calls setAutoCategorizeEnabled", async () => {
    const { result } = renderHook(() => useSetAutoCategorizeEnabled(), { wrapper: createWrapper() });
    result.current.mutate(false);
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});
