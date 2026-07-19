import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { BottomNav } from "./BottomNav";
import { createWrapperWithEntries } from "../test-utils";

const needsReviewMock = vi.hoisted(() => ({ count: 0 }));
const agentStatusMock = vi.hoisted(() => ({ lastScanAt: null as string | null }));

vi.mock("../api/hooks/agent", () => ({
  useNeedsReviewCount: vi.fn(() => ({ data: needsReviewMock.count })),
  useAgentStatus: vi.fn(() => ({ data: { lastScanAt: agentStatusMock.lastScanAt } })),
}));

vi.mock("../api/client", () => ({
  commands: {
    listActionBundles: vi.fn(async () => ({ status: "ok", data: [] })),
  },
}));

vi.mock("../utils/runtime", () => ({
  isBackendAvailable: vi.fn(() => true),
}));

vi.mock("../api/prefetch", () => ({
  prefetchRoute: vi.fn(),
}));

function renderAt(path: string) {
  const Wrapper = createWrapperWithEntries([path]);
  return render(
    <Wrapper>
      <BottomNav />
    </Wrapper>
  );
}

describe("BottomNav", () => {
  it("renders the 5 primary tabs plus More", () => {
    renderAt("/");
    for (const label of ["Today", "Inbox", "Accounts", "Budget", "Goals", "More"]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
  });

  it("marks the current route's tab active", () => {
    renderAt("/budget");
    const budgetTab = screen.getByText("Budget").closest("a");
    expect(budgetTab).toHaveClass("active");
    const todayTab = screen.getByText("Today").closest("a");
    expect(todayTab).not.toHaveClass("active");
  });

  it("shows an inbox pulse indicator when review items are pending", () => {
    needsReviewMock.count = 3;
    renderAt("/");
    expect(screen.getByTestId("bottom-nav-inbox-pulse")).toBeInTheDocument();
    needsReviewMock.count = 0;
  });

  it("does not show an inbox pulse when nothing needs review", () => {
    renderAt("/");
    expect(screen.queryByTestId("bottom-nav-inbox-pulse")).not.toBeInTheDocument();
  });

  it("opens the More sheet and lists secondary destinations", () => {
    renderAt("/");
    fireEvent.click(screen.getByText("More"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    for (const label of ["Categories", "Recurring", "Reports", "Settings", "Journey"]) {
      expect(screen.getByText(label)).toBeInTheDocument();
    }
  });

  it("closes the More sheet after selecting a destination", () => {
    renderAt("/");
    fireEvent.click(screen.getByText("More"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Settings"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
