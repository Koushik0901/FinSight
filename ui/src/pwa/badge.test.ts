import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { badgingSupported, syncAppBadge, clearAppBadge } from "./badge";

const setAppBadge = vi.fn().mockResolvedValue(undefined);
const clearBadge = vi.fn().mockResolvedValue(undefined);

/** Install a Badging-API-capable navigator; `undefined` removes support. */
function installBadging(impl: { set?: unknown; clear?: unknown } | null) {
  const n = navigator as unknown as Record<string, unknown>;
  if (impl === null) {
    delete n.setAppBadge;
    delete n.clearAppBadge;
    return;
  }
  n.setAppBadge = impl.set;
  n.clearAppBadge = impl.clear;
}

beforeEach(() => {
  setAppBadge.mockClear().mockResolvedValue(undefined);
  clearBadge.mockClear().mockResolvedValue(undefined);
  installBadging({ set: setAppBadge, clear: clearBadge });
});

afterEach(() => installBadging(null));

describe("badgingSupported", () => {
  it("is true when the API is present", () => {
    expect(badgingSupported()).toBe(true);
  });

  it("is false on a browser without the API (e.g. Firefox)", () => {
    installBadging(null);
    expect(badgingSupported()).toBe(false);
  });
});

describe("syncAppBadge", () => {
  it("sets the badge to the given count", async () => {
    await syncAppBadge(7);
    expect(setAppBadge).toHaveBeenCalledWith(7);
    expect(clearBadge).not.toHaveBeenCalled();
  });

  it("clears rather than setting zero, so an empty inbox shows no badge", async () => {
    await syncAppBadge(0);
    expect(clearBadge).toHaveBeenCalledTimes(1);
    expect(setAppBadge).not.toHaveBeenCalled();
  });

  it("treats a negative count as empty", async () => {
    await syncAppBadge(-4);
    expect(clearBadge).toHaveBeenCalledTimes(1);
    expect(setAppBadge).not.toHaveBeenCalled();
  });

  it("degrades a non-finite count to no badge instead of throwing", async () => {
    await syncAppBadge(Number.NaN);
    expect(clearBadge).toHaveBeenCalledTimes(1);
    expect(setAppBadge).not.toHaveBeenCalled();
  });

  it("floors a fractional count", async () => {
    await syncAppBadge(3.9);
    expect(setAppBadge).toHaveBeenCalledWith(3);
  });

  // The two that matter most: a badge must never take down a render.
  it("swallows a rejection from setAppBadge (not installed / permission denied)", async () => {
    setAppBadge.mockRejectedValue(new Error("not installed"));
    await expect(syncAppBadge(2)).resolves.toBeUndefined();
  });

  it("is a no-op on a browser without the API", async () => {
    installBadging(null);
    await expect(syncAppBadge(2)).resolves.toBeUndefined();
  });
});

describe("clearAppBadge", () => {
  it("clears the badge", async () => {
    await clearAppBadge();
    expect(clearBadge).toHaveBeenCalledTimes(1);
  });

  it("swallows a rejection", async () => {
    clearBadge.mockRejectedValue(new Error("nope"));
    await expect(clearAppBadge()).resolves.toBeUndefined();
  });

  it("is a no-op on a browser without the API", async () => {
    installBadging(null);
    await expect(clearAppBadge()).resolves.toBeUndefined();
  });
});
