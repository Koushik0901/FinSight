import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
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

  it("does not expose sample-data loading", () => {
    renderOnboarding();
    expect(screen.queryByText(/sample data/i)).not.toBeInTheDocument();
  });

  it("Get started advances to Connect step", () => {
    renderOnboarding();
    fireEvent.click(screen.getByRole("button", { name: /get started/i }));
    expect(screen.getByRole("heading", { name: /connect your money/i })).toBeInTheDocument();
  });
});
