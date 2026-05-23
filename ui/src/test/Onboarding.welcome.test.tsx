import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import Onboarding from "../screens/Onboarding";
import { useOnboardingStore } from "../state/onboarding";

vi.mock("../api/client", () => ({
  commands: {
    getOnboardingState: vi.fn().mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: false },
    }),
    seedSampleHousehold: vi.fn().mockResolvedValue({
      status: "ok",
      data: { accounts_created: 6, transactions_created: 250, import_id: "abc" },
    }),
    markOnboardingComplete: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

function renderOnboarding() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter initialEntries={["/onboarding"]}>
        <Routes>
          <Route path="/onboarding" element={<Onboarding />} />
          <Route path="/" element={<div>TODAY ROUTE</div>} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("Onboarding · Welcome step", () => {
  beforeEach(() => {
    useOnboardingStore.getState().reset();
    vi.clearAllMocks();
  });

  it("renders the welcome heading", () => {
    renderOnboarding();
    expect(screen.getByRole("heading", { name: /quiet way/i })).toBeInTheDocument();
  });

  it("Try sample seeds, marks complete, navigates to /", async () => {
    renderOnboarding();
    fireEvent.click(screen.getByTestId("try-sample-data"));
    await waitFor(() => {
      expect(screen.getByText("TODAY ROUTE")).toBeInTheDocument();
    });
    const { commands } = await import("../api/client");
    expect(commands.seedSampleHousehold).toHaveBeenCalledOnce();
    expect(commands.markOnboardingComplete).toHaveBeenCalledOnce();
  });

  it("Get started advances to Connect step", () => {
    renderOnboarding();
    fireEvent.click(screen.getByRole("button", { name: /get started/i }));
    expect(screen.getByRole("heading", { name: /connect your money/i })).toBeInTheDocument();
  });
});
