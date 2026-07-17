import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";

// Regression guard (server-mode): the data hooks must gate on
// isBackendAvailable(), NOT the Phase-4-narrowed isTauriRuntime(). In server /
// PWA / thin-shell-post-navigate mode the HTTP shim is the transport and
// isTauriRuntime() is false at that origin, yet RPC works over HTTP. Here we
// simulate exactly that: isTauriRuntime() mocked false, but the shim's
// __FINSIGHT_HTTP__ flag set — so isBackendAvailable() is true and the query
// must be ENABLED and run. (The real isBackendAvailable() impl — including this
// exact bridge-present-at-remote-origin case — is unit-tested in
// utils/runtime.test.ts; here we only prove the hook consumes it correctly.)
vi.mock("../../utils/runtime", () => ({
  isTauriRuntime: vi.fn(() => false),
  isBackendAvailable: () =>
    Boolean((window as unknown as { __FINSIGHT_HTTP__?: unknown }).__FINSIGHT_HTTP__),
}));

const listAccounts = vi.fn();
vi.mock("../client", () => ({
  commands: { listAccounts: (...args: unknown[]) => listAccounts(...args) },
}));

import { useAccounts } from "./accounts";

beforeEach(() => {
  listAccounts.mockResolvedValue({ status: "ok", data: [] });
});
afterEach(() => {
  delete (window as unknown as { __FINSIGHT_HTTP__?: unknown }).__FINSIGHT_HTTP__;
  vi.clearAllMocks();
});

describe("useAccounts — server mode (HTTP shim, no Tauri IPC)", () => {
  it("is ENABLED and fetches when __FINSIGHT_HTTP__ is set and isTauriRuntime() is false", async () => {
    (window as unknown as { __FINSIGHT_HTTP__?: unknown }).__FINSIGHT_HTTP__ = true;
    const { result } = renderHook(() => useAccounts(), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(listAccounts).toHaveBeenCalledTimes(1);
  });

  it("is DISABLED (no backend) when neither Tauri IPC nor the HTTP shim is present", async () => {
    const { result } = renderHook(() => useAccounts(), { wrapper: createWrapper() });
    // enabled:false → the query never runs; it stays idle and the command is
    // never invoked.
    await new Promise((r) => setTimeout(r, 20));
    expect(listAccounts).not.toHaveBeenCalled();
    expect(result.current.fetchStatus).toBe("idle");
  });
});
