import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Inbox from "./Inbox";
import { createWrapper } from "../test-utils";
import type { ActionItem } from "../api/client";

vi.mock("../api/hooks/inbox", () => ({
  useActionItems: vi.fn(() => ({ data: undefined, isLoading: true, error: null, dataUpdatedAt: 0 })),
  useUnresolvedCounterparties: vi.fn(() => ({ data: [], isLoading: false })),
  useApplyCounterpartyVerdict: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
}));

vi.mock("../api/hooks/simplefin", () => ({
  useSimpleFinAlerts: vi.fn(() => ({ data: [] })),
  useAcknowledgeSimpleFinAlert: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useSimpleFinTransferSuggestions: vi.fn(() => ({ data: [] })),
  useConfirmSimpleFinTransfer: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useRejectSimpleFinTransfer: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useImportReviewCandidates: vi.fn(() => ({ data: [] })),
  useAcceptImportCandidateMatch: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useCreateImportCandidateTransaction: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useDismissImportCandidate: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
}));

vi.mock("../api/hooks/notifications", () => ({
  useNotifications: vi.fn(() => ({ data: [] })),
  useMarkNotificationRead: vi.fn(() => ({ mutate: vi.fn() })),
  useMarkAllNotificationsRead: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
}));

vi.mock("@tanstack/react-query", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@tanstack/react-query")>();
  return { ...actual, useQueryClient: vi.fn(() => ({ invalidateQueries: vi.fn() })) };
});

const MOCK_ITEMS = [
  {
    id: "uncategorized-transactions",
    category: "review",
    priority: "high",
    title: "12 transactions need categorizing",
    detail: "Uncategorized transactions make your budget reports unreliable.",
    actionLabel: "Review transactions",
    actionRoute: "/transactions?filter=no_category",
    badgeCount: 12,
    amountCents: null,
  },
  {
    id: "anomalies-flagged",
    category: "review",
    priority: "high",
    title: "2 unusual transactions flagged",
    detail: "The agent spotted spending that looks out of the ordinary.",
    actionLabel: "Review anomalies",
    actionRoute: "/transactions?filter=anomalies",
    badgeCount: 2,
    amountCents: null,
  },
  {
    id: "savings-rate-low",
    category: "savings",
    priority: "medium",
    title: "Savings rate is 7% — below the 10% minimum",
    detail: "The Richest Man in Babylon's first rule: pay yourself first.",
    actionLabel: "Plan with Copilot",
    actionRoute: "/copilot",
    badgeCount: null,
    amountCents: null,
  },
];

describe("Inbox screen — loading and error states", () => {
  it("shows loading stub while fetching", async () => {
    const { useActionItems } = await import("../api/hooks/inbox");
    vi.mocked(useActionItems).mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
      dataUpdatedAt: 0,
    } as ReturnType<typeof useActionItems>);

    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText(/Scanning/i)).toBeInTheDocument();
  });

  it("shows error stub on fetch failure", async () => {
    const { useActionItems } = await import("../api/hooks/inbox");
    vi.mocked(useActionItems).mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error("fail"),
      dataUpdatedAt: 0,
    } as ReturnType<typeof useActionItems>);

    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText(/Error loading inbox/i)).toBeInTheDocument();
  });
});

describe("Inbox screen — empty state", () => {
  it("shows all-clear state when no items", async () => {
    const { useActionItems } = await import("../api/hooks/inbox");
    vi.mocked(useActionItems).mockReturnValue({
      data: [] as ActionItem[],
      isLoading: false,
      error: null,
      dataUpdatedAt: Date.now(),
    } as unknown as ReturnType<typeof useActionItems>);

    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText(/All clear/i)).toBeInTheDocument();
    expect(screen.getByText(/0 items/i)).toBeInTheDocument();
  });
});

describe("Inbox screen — with items", () => {
  beforeEach(async () => {
    const { useActionItems } = await import("../api/hooks/inbox");
    vi.mocked(useActionItems).mockReturnValue({
      data: MOCK_ITEMS,
      isLoading: false,
      error: null,
      dataUpdatedAt: Date.now(),
    } as ReturnType<typeof useActionItems>);
  });

  it("renders screen heading and item count", () => {
    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText(/What needs your attention/i)).toBeInTheDocument();
    expect(screen.getByText(/3 items/i)).toBeInTheDocument();
  });

  it("renders high priority section with 2 items", () => {
    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText(/High priority — 2 item/i)).toBeInTheDocument();
  });

  it("renders medium priority section with 1 item", () => {
    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText(/Medium priority — 1 item/i)).toBeInTheDocument();
  });

  it("renders action item titles", () => {
    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText("12 transactions need categorizing")).toBeInTheDocument();
    expect(screen.getByText("2 unusual transactions flagged")).toBeInTheDocument();
    expect(screen.getByText(/Savings rate is 7%/i)).toBeInTheDocument();
  });

  it("renders CTA buttons for each item", () => {
    render(<Inbox />, { wrapper: createWrapper() });
    const ctaBtns = screen.getAllByText(/Review transactions →|Review anomalies →|Plan with Copilot →/i);
    expect(ctaBtns.length).toBeGreaterThanOrEqual(3);
  });

  it("badge count chip is visible for counted items", () => {
    render(<Inbox />, { wrapper: createWrapper() });
    // badge count 12 and 2 should appear as chips
    expect(screen.getByText("12")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
  });

  it("refresh button triggers query invalidation", async () => {
    const { useQueryClient } = await import("@tanstack/react-query");
    const invalidate = vi.fn();
    vi.mocked(useQueryClient).mockReturnValue({ invalidateQueries: invalidate } as never);

    render(<Inbox />, { wrapper: createWrapper() });
    const refreshBtn = screen.getByTitle("Refresh inbox");
    fireEvent.click(refreshBtn);
    expect(invalidate).toHaveBeenCalled();
  });

  it("navigates when CTA is clicked", () => {
    render(<Inbox />, { wrapper: createWrapper() });
    const ctaBtns = screen.getAllByText(/Review transactions →/i);
    expect(ctaBtns.length).toBeGreaterThanOrEqual(1);
    // Should not throw on click (navigation mocked by MemoryRouter in createWrapper)
    fireEvent.click(ctaBtns[0]!);
  });
});

describe("Inbox screen — notifications section", () => {
  beforeEach(async () => {
    // Only notifications present — no action items, alerts, transfers, etc.
    const { useActionItems } = await import("../api/hooks/inbox");
    vi.mocked(useActionItems).mockReturnValue({
      data: [] as ActionItem[],
      isLoading: false,
      error: null,
      dataUpdatedAt: Date.now(),
    } as unknown as ReturnType<typeof useActionItems>);
  });

  it("renders the notification history, held badge, and suppresses All clear when only notifications exist", async () => {
    const { useNotifications } = await import("../api/hooks/notifications");
    vi.mocked(useNotifications).mockReturnValue({
      data: [
        { id: "n1", category: "cashflow_risk", urgency: "critical", title: "Balance dips below buffer", body: "Projected to go negative", sensitive: "-$142", route: "/cashflow", createdAt: "2026-07-20T00:00:00Z", deliveredAt: "2026-07-20T00:00:00Z", readAt: null, resolvedAt: null },
        { id: "n2", category: "subscription_change", urgency: "normal", title: "Subscription renews soon", body: "Renews in 3 days", sensitive: null, route: null, createdAt: "2026-07-19T00:00:00Z", deliveredAt: null, readAt: null, resolvedAt: null },
      ],
    } as never);

    render(<Inbox />, { wrapper: createWrapper() });
    expect(screen.getByText(/Notifications · 2/i)).toBeInTheDocument();
    expect(screen.getByText("Balance dips below buffer")).toBeInTheDocument();
    // n2 was withheld overnight (deliveredAt null) → held badge.
    expect(screen.getByText(/Held · quiet hours/i)).toBeInTheDocument();
    // Notifications count toward the inbox total, so the empty-state must not show.
    expect(screen.queryByText(/All clear/i)).toBeNull();
    expect(screen.getByText(/2 items/i)).toBeInTheDocument();
  });

  it("marks a notification read and navigates when its card is clicked", async () => {
    const markRead = vi.fn();
    const { useNotifications, useMarkNotificationRead } = await import("../api/hooks/notifications");
    vi.mocked(useMarkNotificationRead).mockReturnValue({ mutate: markRead } as never);
    vi.mocked(useNotifications).mockReturnValue({
      data: [
        { id: "n1", category: "cashflow_risk", urgency: "critical", title: "Balance dips below buffer", body: "Projected to go negative", sensitive: null, route: "/cashflow", createdAt: "2026-07-20T00:00:00Z", deliveredAt: "2026-07-20T00:00:00Z", readAt: null, resolvedAt: null },
      ],
    } as never);

    render(<Inbox />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByText("Balance dips below buffer"));
    expect(markRead).toHaveBeenCalledWith("n1");
  });
});
