import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import NotificationPolicySettings from "./NotificationPolicySettings";

const setPrefs = vi.fn();
const useNotificationPrefs = vi.fn();
vi.mock("../api/hooks/notifications", () => ({
  useNotificationPrefs: () => useNotificationPrefs(),
  useSetNotificationPrefs: () => ({ mutate: setPrefs, isPending: false }),
}));

const base = {
  masterEnabled: true,
  categories: [{ key: "cashflow_risk", label: "Cash-flow risk", enabled: true }],
  quietHours: null,
  utcOffsetMinutes: 0,
  privacy: "full",
  snoozeUntil: null,
  digestFrequency: "off",
};

describe("NotificationPolicySettings — digests & snooze (#69)", () => {
  beforeEach(() => {
    setPrefs.mockReset();
    useNotificationPrefs.mockReturnValue({ data: base });
  });

  it("sets a daily digest frequency", () => {
    render(<NotificationPolicySettings />);
    fireEvent.click(screen.getByRole("button", { name: "Daily" }));
    expect(setPrefs).toHaveBeenCalledWith(expect.objectContaining({ digestFrequency: "daily" }));
  });

  it("snoozes notifications for a chosen duration", () => {
    render(<NotificationPolicySettings />);
    fireEvent.click(screen.getByRole("button", { name: "1 hour" }));
    expect(setPrefs).toHaveBeenCalledWith(expect.objectContaining({ snoozeUntil: expect.any(String) }));
  });

  it("offers a resume control while snoozed and clears the snooze", () => {
    useNotificationPrefs.mockReturnValue({
      data: { ...base, snoozeUntil: new Date(Date.now() + 3_600_000).toISOString() },
    });
    render(<NotificationPolicySettings />);
    expect(screen.getByText(/Snoozed until/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Resume now" }));
    expect(setPrefs).toHaveBeenCalledWith(expect.objectContaining({ snoozeUntil: null }));
  });
});
