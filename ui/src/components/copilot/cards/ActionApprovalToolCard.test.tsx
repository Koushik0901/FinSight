import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { ActionApprovalToolCard } from "./ActionApprovalToolCard";
import { createWrapper } from "../../../test-utils";
import * as copilotHooks from "../../../api/hooks/copilot";

/**
 * The approval flow used to end in a toast — the user was told "1 action
 * applied" and left in the chat with no way to see the result.
 *
 * These tests pin the offer that replaced that dead end: it appears only after
 * a successful execution, it is an offer rather than an automatic redirect,
 * and it never invents a destination the backend did not supply.
 */

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

vi.mock("../../../api/hooks/copilot", () => ({
  useActionBundle: vi.fn(),
  useApproveActionItem: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useRejectActionItem: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useExecuteActionBundle: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

const approvedBundle = {
  id: "bundle-1",
  sessionId: null,
  title: "Budget change",
  summary: "",
  rationale: "",
  confidence: 0.9,
  status: "approved",
  providerId: null,
  modelId: null,
  createdAt: "2026-07-19T00:00:00Z",
  updatedAt: "2026-07-19T00:00:00Z",
  items: [
    {
      id: "item-1",
      bundleId: "bundle-1",
      actionKind: "set_budget",
      payloadJson: '{"categoryId":"cat-1","month":"2026-07","amountCents":45000}',
      previewJson: null,
      rationale: "Groceries is trending over plan.",
      confidence: 0.9,
      status: "approved",
      validationErrors: null,
      sortOrder: 0,
      createdAt: "2026-07-19T00:00:00Z",
      updatedAt: "2026-07-19T00:00:00Z",
    },
  ],
};

/** Wires the execute mutation to resolve with a given summary. */
function mockExecution(summary: Record<string, unknown>) {
  const mutateAsync = vi.fn().mockResolvedValue(summary);
  vi.mocked(copilotHooks.useExecuteActionBundle).mockReturnValue({
    mutateAsync,
    isPending: false,
  } as unknown as ReturnType<typeof copilotHooks.useExecuteActionBundle>);
  return mutateAsync;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(copilotHooks.useActionBundle).mockReturnValue({
    data: approvedBundle,
    isLoading: false,
  } as unknown as ReturnType<typeof copilotHooks.useActionBundle>);
  vi.mocked(copilotHooks.useApproveActionItem).mockReturnValue({
    mutate: vi.fn(),
    isPending: false,
  } as unknown as ReturnType<typeof copilotHooks.useApproveActionItem>);
  vi.mocked(copilotHooks.useRejectActionItem).mockReturnValue({
    mutate: vi.fn(),
    isPending: false,
  } as unknown as ReturnType<typeof copilotHooks.useRejectActionItem>);
});

describe("ActionApprovalToolCard post-execution navigation", () => {
  it("offers no destination before anything has been executed", () => {
    mockExecution({ bundleId: "bundle-1", succeeded: 0, failed: 0, results: [], navigation: [] });
    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    expect(screen.queryByText("See the change")).not.toBeInTheDocument();
  });

  it("offers the affected screen after a successful execution", async () => {
    mockExecution({
      bundleId: "bundle-1",
      succeeded: 1,
      failed: 0,
      results: [],
      navigation: [{ label: "View in Budget", path: "/budget?focusCategory=cat-1" }],
    });

    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /execute approved actions/i }));

    await waitFor(() => {
      expect(screen.getByText("See the change")).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /view in budget/i })).toBeInTheDocument();
  });

  it("navigates only when the user clicks — never automatically", async () => {
    mockExecution({
      bundleId: "bundle-1",
      succeeded: 1,
      failed: 0,
      results: [],
      navigation: [{ label: "View in Budget", path: "/budget?focusCategory=cat-1" }],
    });

    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /execute approved actions/i }));

    const cta = await screen.findByRole("button", { name: /view in budget/i });
    // Being yanked out of a conversation mid-thread is worse than clicking.
    expect(mockNavigate).not.toHaveBeenCalled();

    fireEvent.click(cta);
    expect(mockNavigate).toHaveBeenCalledWith("/budget?focusCategory=cat-1");
  });

  it("offers every affected screen when a bundle spans more than one", async () => {
    mockExecution({
      bundleId: "bundle-1",
      succeeded: 2,
      failed: 0,
      results: [],
      navigation: [
        { label: "View in Budget", path: "/budget" },
        { label: "View in Goals", path: "/goals?focusGoal=g-1" },
      ],
    });

    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /execute approved actions/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /view in budget/i })).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: /view in goals/i })).toBeInTheDocument();
  });

  it("offers nothing when the backend supplies no destination", async () => {
    // e.g. every executed kind was one with no screen to show.
    mockExecution({ bundleId: "bundle-1", succeeded: 1, failed: 0, results: [], navigation: [] });

    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /execute approved actions/i }));

    await waitFor(() => {
      expect(screen.getByText(/proposed action/i)).toBeInTheDocument();
    });
    expect(screen.queryByText("See the change")).not.toBeInTheDocument();
  });

  it("survives a server that predates the navigation field", async () => {
    // A self-hosted server may be older than the client served to the browser.
    mockExecution({ bundleId: "bundle-1", succeeded: 1, failed: 0, results: [] });

    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /execute approved actions/i }));

    await waitFor(() => {
      expect(screen.queryByText("See the change")).not.toBeInTheDocument();
    });
  });

  it("does not offer a destination when execution throws", async () => {
    const mutateAsync = vi.fn().mockRejectedValue(new Error("db locked"));
    vi.mocked(copilotHooks.useExecuteActionBundle).mockReturnValue({
      mutateAsync,
      isPending: false,
    } as unknown as ReturnType<typeof copilotHooks.useExecuteActionBundle>);

    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /execute approved actions/i }));

    await waitFor(() => expect(mutateAsync).toHaveBeenCalled());
    expect(screen.queryByText("See the change")).not.toBeInTheDocument();
  });

  it("still offers the screens that succeeded in a partial failure", async () => {
    // The backend only includes items that actually applied, so a partial
    // failure should still let the user verify what did land.
    mockExecution({
      bundleId: "bundle-1",
      succeeded: 1,
      failed: 1,
      results: [],
      navigation: [{ label: "View in Budget", path: "/budget?focusCategory=cat-1" }],
    });

    render(<ActionApprovalToolCard bundleId="bundle-1" />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByRole("button", { name: /execute approved actions/i }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: /view in budget/i })).toBeInTheDocument();
    });
  });
});
