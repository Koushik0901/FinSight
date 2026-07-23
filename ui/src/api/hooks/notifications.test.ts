import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useSetNotificationPrefs } from "./notifications";
import type { NotificationPrefsDto } from "../client";

const setNotificationPrefs = vi.fn();
vi.mock("../client", () => ({
  commands: {
    get setNotificationPrefs() {
      return setNotificationPrefs;
    },
  },
}));

const basePrefs: NotificationPrefsDto = {
  masterEnabled: true,
  categories: [],
  quietHours: { start: 22, end: 7 },
  utcOffsetMinutes: 0,
  privacy: "full",
  snoozeUntil: null,
  digestFrequency: "off",
};

describe("useSetNotificationPrefs", () => {
  beforeEach(() => {
    setNotificationPrefs.mockResolvedValue({ status: "ok", data: null });
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("stamps the client's current UTC offset so quiet hours resolve in local time", async () => {
    // getTimezoneOffset returns minutes BEHIND UTC; +480 == UTC−8.
    vi.spyOn(Date.prototype, "getTimezoneOffset").mockReturnValue(480);
    const { result } = renderHook(() => useSetNotificationPrefs(), { wrapper: createWrapper() });

    result.current.mutate(basePrefs);

    await waitFor(() => expect(setNotificationPrefs).toHaveBeenCalled());
    const sent = setNotificationPrefs.mock.calls[0]![0] as NotificationPrefsDto;
    // Negated to express `local = UTC + offset`, overriding whatever the DTO carried.
    expect(sent.utcOffsetMinutes).toBe(-480);
  });

  it("stamps a positive offset for east-of-UTC clients", async () => {
    vi.spyOn(Date.prototype, "getTimezoneOffset").mockReturnValue(-330); // UTC+5:30
    const { result } = renderHook(() => useSetNotificationPrefs(), { wrapper: createWrapper() });

    result.current.mutate({ ...basePrefs, utcOffsetMinutes: -480 });

    await waitFor(() => expect(setNotificationPrefs).toHaveBeenCalled());
    const sent = setNotificationPrefs.mock.calls[0]![0] as NotificationPrefsDto;
    expect(sent.utcOffsetMinutes).toBe(330);
  });
});
