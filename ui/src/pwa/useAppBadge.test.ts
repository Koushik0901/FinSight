import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../test-utils";
import { useAppBadge } from "./useAppBadge";

const getInboxBadgeCount = vi.fn();
const notificationUnreadCount = vi.fn();

vi.mock("../api/client", () => ({
  commands: {
    get getInboxBadgeCount() {
      return getInboxBadgeCount;
    },
    get notificationUnreadCount() {
      return notificationUnreadCount;
    },
  },
}));

const setAppBadge = vi.fn().mockResolvedValue(undefined);
const clearAppBadge = vi.fn().mockResolvedValue(undefined);

beforeEach(() => {
  setAppBadge.mockClear();
  clearAppBadge.mockClear();
  const n = navigator as unknown as Record<string, unknown>;
  n.setAppBadge = setAppBadge;
  n.clearAppBadge = clearAppBadge;
  getInboxBadgeCount.mockResolvedValue({
    status: "ok",
    data: {
      total: 6,
      actionItems: 3,
      alerts: 1,
      transferSuggestions: 1,
      importReview: 1,
      unresolvedCounterparties: 0,
    },
  });
  // No unread notifications by default — existing expectations stay at the
  // inbox total until a test opts into a non-zero unread count.
  notificationUnreadCount.mockResolvedValue({ status: "ok", data: 0 });
});

describe("useAppBadge", () => {
  it("puts the inbox total on the app icon", async () => {
    renderHook(() => useAppBadge(), { wrapper: createWrapper() });
    await waitFor(() => expect(setAppBadge).toHaveBeenCalledWith(6));
  });

  it("does not touch the badge before the count has loaded", () => {
    // A premature setAppBadge(0) would visibly clear a badge the OS is already
    // showing, then re-add it once the query settles.
    renderHook(() => useAppBadge(), { wrapper: createWrapper() });
    expect(setAppBadge).not.toHaveBeenCalled();
    expect(clearAppBadge).not.toHaveBeenCalled();
  });

  it("clears the badge when the inbox is empty", async () => {
    getInboxBadgeCount.mockResolvedValue({
      status: "ok",
      data: {
        total: 0,
        actionItems: 0,
        alerts: 0,
        transferSuggestions: 0,
        importReview: 0,
        unresolvedCounterparties: 0,
      },
    });
    renderHook(() => useAppBadge(), { wrapper: createWrapper() });
    await waitFor(() => expect(clearAppBadge).toHaveBeenCalled());
    expect(setAppBadge).not.toHaveBeenCalled();
  });

  it("clears the badge on unmount so a signed-out device shows no count", async () => {
    const { unmount } = renderHook(() => useAppBadge(), { wrapper: createWrapper() });
    await waitFor(() => expect(setAppBadge).toHaveBeenCalledWith(6));
    clearAppBadge.mockClear();
    unmount();
    expect(clearAppBadge).toHaveBeenCalledTimes(1);
  });

  it("sums the inbox total and unread notifications onto the icon", async () => {
    notificationUnreadCount.mockResolvedValue({ status: "ok", data: 4 });
    renderHook(() => useAppBadge(), { wrapper: createWrapper() });
    await waitFor(() => expect(setAppBadge).toHaveBeenCalledWith(10));
  });

  it("leaves the badge alone when both count queries fail", async () => {
    // Both sources share a transport; a real outage fails both, and only then
    // is the count fully unknown — writing anything would clobber a live badge.
    getInboxBadgeCount.mockResolvedValue({
      status: "error",
      error: { code: "rpc.transport", message: "offline" },
    });
    notificationUnreadCount.mockResolvedValue({
      status: "error",
      error: { code: "rpc.transport", message: "offline" },
    });
    renderHook(() => useAppBadge(), { wrapper: createWrapper() });
    // Give the queries a chance to reject and settle.
    await new Promise((r) => setTimeout(r, 20));
    expect(setAppBadge).not.toHaveBeenCalled();
    expect(clearAppBadge).not.toHaveBeenCalled();
  });
});
