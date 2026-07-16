import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import Onboarding from "../screens/Onboarding";
import { commands } from "../api/client";
import { useOnboardingStore } from "../state/onboarding";

vi.mock("../api/client", () => ({
  commands: {
    getOnboardingState: vi.fn().mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: false },
    }),
    markOnboardingComplete: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    createHouseholdMember: vi.fn().mockResolvedValue({
      status: "ok",
      data: { id: "member-1", name: "John Doe", color: null, createdAt: "2026-07-16T00:00:00Z" },
    }),
    setSelfMember: vi.fn().mockResolvedValue({ status: "ok", data: null }),
  },
}));

vi.mock("../screens/onboarding/StepAccounts", () => ({
  default: () => <h1>Start with your accounts.</h1>,
}));
vi.mock("../screens/onboarding/StepHistory", () => ({
  default: () => <h1>Bring in your history.</h1>,
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

  it("uses the shared styled field with a generic example name", () => {
    renderOnboarding();
    const nameInput = screen.getByPlaceholderText("e.g. John Doe");
    expect(nameInput.closest("label")).toHaveClass("field", "onb-name-field");
  });

  it("saves a supplied name best-effort and advances to Accounts", async () => {
    renderOnboarding();
    fireEvent.change(screen.getByLabelText("Your name"), { target: { value: "John Doe" } });
    fireEvent.click(screen.getByRole("button", { name: /get started/i }));

    await waitFor(() => {
      expect(commands.createHouseholdMember).toHaveBeenCalledWith("John Doe", null);
      expect(commands.setSelfMember).toHaveBeenCalledWith("member-1");
    });
    expect(screen.getByRole("heading", { name: /start with your accounts/i })).toBeInTheDocument();
  });

  it("marks onboarding complete before Skip setup navigates to Today", async () => {
    renderOnboarding();
    fireEvent.click(screen.getByRole("button", { name: /skip setup/i }));

    await waitFor(() => expect(commands.markOnboardingComplete).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("TODAY ROUTE")).toBeInTheDocument();
  });

  it("stays in onboarding and shows an error when Skip setup cannot be saved", async () => {
    vi.mocked(commands.markOnboardingComplete).mockResolvedValueOnce({
      status: "error",
      error: { code: "onboarding.failed", message: "Could not save completion" },
    });
    renderOnboarding();
    fireEvent.click(screen.getByRole("button", { name: /skip setup/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save completion");
    expect(screen.queryByText("TODAY ROUTE")).not.toBeInTheDocument();
  });
});