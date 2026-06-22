import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import Copilot from "./Copilot";
import { createWrapper } from "../test-utils";

// ── Mock hooks ────────────────────────────────────────────────────────────────

vi.mock("../api/hooks/copilot", () => ({
  useActionBundles: vi.fn(() => ({ data: [], isLoading: false })),
  useActionBundle: vi.fn(() => ({ data: null, isLoading: false })),
  useApproveActionItem: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useRejectActionItem: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useExecutionLog: vi.fn(() => ({ data: [], isLoading: false })),
}));

vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
  }),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

const mockPlanResult = {
  bundleId: "bundle-1",
  prose: "Here is your financial plan.",
  reasoning: "Projected savings: $12,000 by end of year.",
  trace: ["Called tool: analyze_cash_inflow"],
  changes: [],
  actionLabel: null,
  actionPath: null,
  assumptions: ["Assumes current income stays constant"],
  dataSources: ["Accounts and liabilities"],
  missingData: ["Add APR for Loan"],
  alternatives: [],
  followUpQuestions: ["How much debt do you have?"],
};

function getTextarea() {
  return screen.getByPlaceholderText(/Ask your financial analyst anything/i);
}

function getSubmitBtn() {
  return screen.getByRole("button", { name: /Ask Copilot/i });
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  sessionStorage.clear();
});

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("Copilot screen — rendering", () => {
  it("renders the heading and input area", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByRole("heading", { name: /copilot/i })).toBeInTheDocument();
    expect(getTextarea()).toBeInTheDocument();
  });

  it("renders suggested prompts", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(screen.getByText(/Plan next month's budget/i)).toBeInTheDocument();
    expect(screen.getByText(/What can I cut/i)).toBeInTheDocument();
  });

  it("submit button is disabled when input is empty", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    expect(getSubmitBtn()).toBeDisabled();
  });

  it("submit button is enabled when input has text", () => {
    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.change(getTextarea(), { target: { value: "Help me" } });
    expect(getSubmitBtn()).not.toBeDisabled();
  });
});

describe("Copilot screen — asking a question", () => {
  it("calls ask_agent in deep mode and shows the answer", async () => {
    vi.mocked(invoke).mockResolvedValue(mockPlanResult);

    render(<Copilot />, { wrapper: createWrapper() });

    fireEvent.change(getTextarea(), { target: { value: "What should I do with $500?" } });
    fireEvent.click(getSubmitBtn());

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("ask_agent", {
        question: "What should I do with $500?",
        mode: "deep",
      });
    });

    await waitFor(() => {
      expect(screen.getByText("Here is your financial plan.")).toBeInTheDocument();
    });
  });

  it("shows assumptions when returned", async () => {
    vi.mocked(invoke).mockResolvedValue(mockPlanResult);

    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.change(getTextarea(), { target: { value: "Plan my budget" } });
    fireEvent.click(getSubmitBtn());

    await waitFor(() => {
      expect(screen.getByText(/Assumes current income stays constant/i)).toBeInTheDocument();
    });
  });

  it("shows reasoning as forecast summary when returned", async () => {
    vi.mocked(invoke).mockResolvedValue(mockPlanResult);

    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.change(getTextarea(), { target: { value: "Forecast my savings" } });
    fireEvent.click(getSubmitBtn());

    await waitFor(() => {
      expect(screen.getByText(/Projected savings/i)).toBeInTheDocument();
    });
  });

  it("shows tool trace and missing data", async () => {
    vi.mocked(invoke).mockResolvedValue(mockPlanResult);

    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.change(getTextarea(), { target: { value: "Plan my debts" } });
    fireEvent.click(getSubmitBtn());

    await waitFor(() => {
      expect(screen.getByText(/analyze_cash_inflow/i)).toBeInTheDocument();
      expect(screen.getByText(/Add APR for Loan/i)).toBeInTheDocument();
    });
  });

  it("shows scenario alternatives in a comparison table", async () => {
    vi.mocked(invoke).mockResolvedValue({
      ...mockPlanResult,
      alternatives: [
        {
          name: "Keep savings intact",
          summary: "Use $0 from car savings; estimated interest $1,200.",
          tradeoff: "Preserves liquidity but keeps the loan longer.",
        },
        {
          name: "Safe partial payoff",
          summary: "Use $3,000 from car savings; estimated interest $700.",
          tradeoff: "Saves interest while protecting emergency cash.",
        },
      ],
    });

    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.change(getTextarea(), { target: { value: "Compare car savings and loan" } });
    fireEvent.click(getSubmitBtn());

    await waitFor(() => {
      expect(screen.getByRole("table", { name: /Scenario alternatives compared/i })).toBeInTheDocument();
      expect(screen.getByText(/Keep savings intact/i)).toBeInTheDocument();
      expect(screen.getByText(/Safe partial payoff/i)).toBeInTheDocument();
      expect(screen.getByText(/Saves interest/i)).toBeInTheDocument();
    });
  });
  it("shows follow-up question chips", async () => {
    vi.mocked(invoke).mockResolvedValue(mockPlanResult);

    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.change(getTextarea(), { target: { value: "Help" } });
    fireEvent.click(getSubmitBtn());

    await waitFor(() => {
      expect(screen.getByText(/How much debt do you have\?/i)).toBeInTheDocument();
    });
  });

  it("shows error toast when provider is not configured", async () => {
    const { toast } = await import("sonner");
    vi.mocked(invoke).mockRejectedValue({ message: "no_provider" });

    render(<Copilot />, { wrapper: createWrapper() });
    fireEvent.change(getTextarea(), { target: { value: "Budget help" } });
    fireEvent.click(getSubmitBtn());

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        "AI provider not configured",
        expect.objectContaining({ description: expect.stringContaining("Settings") }),
      );
    });
  });
});

describe("Copilot screen — suggested prompt chips", () => {
  it("clicking a suggested prompt fills the textarea", () => {
    render(<Copilot />, { wrapper: createWrapper() });

    const chip = screen.getByText(/Plan next month's budget/i);
    fireEvent.click(chip);

    expect(getTextarea()).toHaveValue("Plan next month's budget");
  });
});

describe("Copilot screen — sessionStorage prefill", () => {
  it("reads copilot.prefill from sessionStorage and auto-submits", async () => {
    vi.mocked(invoke).mockResolvedValue(mockPlanResult);
    sessionStorage.setItem("copilot.prefill", "Auto-filled question");

    render(<Copilot />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("ask_agent", {
        question: "Auto-filled question",
        mode: "deep",
      });
    });

    // sessionStorage entry should be cleared after reading
    expect(sessionStorage.getItem("copilot.prefill")).toBeNull();
  });
});
