import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Journey from "./Journey";
import { createWrapper } from "../test-utils";

// ── Mock hooks ────────────────────────────────────────────────────────────────

vi.mock("../api/hooks/journey", () => ({
  useJourneyStatus: vi.fn(() => ({ data: undefined, isLoading: true, error: null })),
}));

// ── Data fixtures ─────────────────────────────────────────────────────────────

const makeMilestone = (stage: number, status: "completed" | "current" | "upcoming", pct = 0) => ({
  stage,
  name: `Stage ${stage} Name`,
  description: `Description for stage ${stage}`,
  status,
  progressPct: pct,
  detail: `Detail for stage ${stage}`,
  actionPrompt: `Action prompt for stage ${stage}`,
});

const mockJourneyData = {
  currentStage: 2,
  completedCount: 1,
  milestones: [
    makeMilestone(1, "completed", 100),
    makeMilestone(2, "current", 45),
    makeMilestone(3, "upcoming", 0),
    makeMilestone(4, "upcoming", 0),
    makeMilestone(5, "upcoming", 0),
    makeMilestone(6, "upcoming", 0),
    makeMilestone(7, "upcoming", 0),
  ],
};

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("Journey screen — loading and error states", () => {
  it("shows loading stub while data is fetching", async () => {
    const { useJourneyStatus } = await import("../api/hooks/journey");
    vi.mocked(useJourneyStatus).mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    } as ReturnType<typeof useJourneyStatus>);

    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.getByText(/Loading journey/i)).toBeInTheDocument();
  });

  it("shows error stub on fetch failure", async () => {
    const { useJourneyStatus } = await import("../api/hooks/journey");
    vi.mocked(useJourneyStatus).mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error("failed"),
    } as ReturnType<typeof useJourneyStatus>);

    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.getByText(/Error loading journey/i)).toBeInTheDocument();
  });
});

describe("Journey screen — rendering with data", () => {
  beforeEach(async () => {
    const { useJourneyStatus } = await import("../api/hooks/journey");
    vi.mocked(useJourneyStatus).mockReturnValue({
      data: mockJourneyData,
      isLoading: false,
      error: null,
    } as ReturnType<typeof useJourneyStatus>);
  });

  it("renders the heading", () => {
    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.getByRole("heading", { name: /Your Financial Journey/i })).toBeInTheDocument();
  });

  it("shows milestone count in the eyebrow", () => {
    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.getByText(/1 of 7 milestones completed/i)).toBeInTheDocument();
  });

  it("renders all 7 milestone cards", () => {
    render(<Journey />, { wrapper: createWrapper() });
    for (let i = 1; i <= 7; i++) {
      expect(screen.getByText(`Stage ${i}`)).toBeInTheDocument();
    }
  });

  it("marks completed milestone with 'Completed' chip", () => {
    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.getByText("Completed")).toBeInTheDocument();
  });

  it("marks current milestone with 'Current focus' chip", () => {
    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.getByText("Current focus")).toBeInTheDocument();
  });

  it("renders motivational quote at the bottom", () => {
    render(<Journey />, { wrapper: createWrapper() });
    // currentStage=2 → QUOTES.early (Lao Tzu)
    expect(screen.getByText(/Lao Tzu/i)).toBeInTheDocument();
  });

  it("each milestone has a 'Get guidance' Copilot entry point", () => {
    render(<Journey />, { wrapper: createWrapper() });
    const guidanceButtons = screen.getAllByRole("button", { name: /Get guidance/i });
    expect(guidanceButtons.length).toBe(7);
  });

  it("clicking 'Get guidance' sets sessionStorage prefill and navigates", () => {
    render(<Journey />, { wrapper: createWrapper() });
    const firstBtn = screen.getAllByRole("button", { name: /Get guidance/i })[0]!;
    fireEvent.click(firstBtn);
    expect(sessionStorage.getItem("copilot.prefill")).toBe("Action prompt for stage 1");
  });
});

describe("Journey screen — stage 1 callout", () => {
  it("shows 'Add your first account' callout when stage 1 is current and progress < 50%", async () => {
    const { useJourneyStatus } = await import("../api/hooks/journey");
    vi.mocked(useJourneyStatus).mockReturnValue({
      data: {
        currentStage: 1,
        completedCount: 0,
        milestones: [
          makeMilestone(1, "current", 20),
          ...Array.from({ length: 6 }, (_, i) => makeMilestone(i + 2, "upcoming", 0)),
        ],
      },
      isLoading: false,
      error: null,
    } as ReturnType<typeof useJourneyStatus>);

    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.getByText(/Start by linking your first account/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Add your first account/i })).toBeInTheDocument();
  });

  it("does NOT show the callout when stage 1 progress >= 50%", async () => {
    const { useJourneyStatus } = await import("../api/hooks/journey");
    vi.mocked(useJourneyStatus).mockReturnValue({
      data: {
        currentStage: 1,
        completedCount: 0,
        milestones: [
          makeMilestone(1, "current", 70),
          ...Array.from({ length: 6 }, (_, i) => makeMilestone(i + 2, "upcoming", 0)),
        ],
      },
      isLoading: false,
      error: null,
    } as ReturnType<typeof useJourneyStatus>);

    render(<Journey />, { wrapper: createWrapper() });
    expect(screen.queryByText(/Start by linking your first account/i)).not.toBeInTheDocument();
  });
});
