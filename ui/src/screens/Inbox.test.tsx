import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Inbox from "./Inbox";
import { createWrapper } from "../test-utils";
import type { ActionItem } from "../api/client";

vi.mock("../api/hooks/inbox", () => ({
  useActionItems: vi.fn(() => ({ data: undefined, isLoading: true, error: null, dataUpdatedAt: 0 })),
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
