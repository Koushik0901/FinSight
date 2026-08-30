import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import { axe } from "vitest-axe";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import MobileReports from "./MobileReports";

vi.mock("../../api/hooks/budget", () => ({
  useBudgetHistory: vi.fn(() => ({ data: [] })),
}));

function wrap(node: React.ReactNode) {
  const qc = new QueryClient();
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("MobileReports a11y", () => {
  it("has no axe violations with empty history", async () => {
    const { container } = render(wrap(<MobileReports />));
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("renders 6-month placeholder when no data", async () => {
    const { getByText } = render(wrap(<MobileReports />));
    expect(getByText("6-month spent")).toBeInTheDocument();
  });
});
