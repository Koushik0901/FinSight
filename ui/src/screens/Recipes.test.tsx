import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Recipes from "./Recipes";
import { createWrapper } from "../test-utils";

// ── Mock hooks ────────────────────────────────────────────────────────────────

const mockTrigger = vi.fn();

vi.mock("../api/hooks/recipes", () => ({
  useRecipes: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useCreateRecipe: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateRecipe: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  usePauseRecipe: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useResumeRecipe: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteRecipe: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useTriggerRecipe: vi.fn(() => ({ mutateAsync: mockTrigger, isPending: false })),
  useRecipeRuns: vi.fn(() => ({ data: [], isLoading: false })),
}));

vi.mock("sonner", () => ({
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
  }),
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

const mockRecipe = {
  id: "recipe-1",
  title: "My Custom Recipe",
  description: "Auto budget every month.",
  recipeKind: "monthly_budget_draft",
  promptTemplate: "Draft my budget",
  cadence: "monthly",
  dayOfWeek: null,
  dayOfMonth: 1,
  status: "active",
  runCount: 3,
  lastRunAt: "2026-06-01T09:00:00Z",
  nextRunAt: "2026-07-01T09:00:00Z",
  createdAt: "2026-05-01T00:00:00Z",
  updatedAt: "2026-06-01T00:00:00Z",
};

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("Recipes screen — rendering", () => {
  it("renders the Recipes heading", () => {
    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getByRole("heading", { name: /^Recipes$/i })).toBeInTheDocument();
  });

  it("shows 5 built-in template cards when no recipes exist", () => {
    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getByText("Monthly Budget Draft")).toBeInTheDocument();
    expect(screen.getByText("Weekly Cleanup")).toBeInTheDocument();
    expect(screen.getByText("Goal Progress Check")).toBeInTheDocument();
    expect(screen.getByText("Subscription Review")).toBeInTheDocument();
    expect(screen.getByText("Savings Rate Check")).toBeInTheDocument();
  });

  it("shows intro callout when no recipes are saved yet", () => {
    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getByText(/Start from a trusted template/i)).toBeInTheDocument();
  });

  it("shows New recipe button", () => {
    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getByRole("button", { name: /New recipe/i })).toBeInTheDocument();
  });
});

describe("Recipes screen — saved recipes", () => {
  it("renders a saved recipe title", async () => {
    const { useRecipes } = await import("../api/hooks/recipes");
    vi.mocked(useRecipes).mockReturnValue({
      data: [mockRecipe],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useRecipes>);

    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getByText("My Custom Recipe")).toBeInTheDocument();
  });

  it("shows 'Run now' button for each saved recipe", async () => {
    const { useRecipes } = await import("../api/hooks/recipes");
    vi.mocked(useRecipes).mockReturnValue({
      data: [mockRecipe],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useRecipes>);

    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getAllByRole("button", { name: /Run now/i }).length).toBeGreaterThan(0);
  });
});

describe("Recipes screen — template use", () => {
  it("clicking 'Use template' opens the drawer", () => {
    render(<Recipes />, { wrapper: createWrapper() });

    const useButtons = screen.getAllByRole("button", { name: /Use template/i });
    fireEvent.click(useButtons[0]!);

    // Drawer should open — check for its Create recipe button
    expect(screen.getByRole("button", { name: /Create recipe/i })).toBeInTheDocument();
  });
});

describe("Recipes screen — run now", () => {
  it("run now triggers recipe and shows success toast", async () => {
    const { toast } = await import("sonner");
    const { useRecipes } = await import("../api/hooks/recipes");
    mockTrigger.mockResolvedValue(undefined);

    vi.mocked(useRecipes).mockReturnValue({
      data: [mockRecipe],
      isLoading: false,
      error: null,
    } as ReturnType<typeof useRecipes>);

    render(<Recipes />, { wrapper: createWrapper() });

    const runBtn = screen.getByRole("button", { name: /Run now/i });
    fireEvent.click(runBtn);

    await waitFor(() => {
      expect(mockTrigger).toHaveBeenCalledWith("recipe-1");
    });

    await waitFor(() => {
      expect(toast.success).toHaveBeenCalledWith(
        "Draft bundle sent to Copilot",
        expect.any(Object),
      );
    });
  });
});

describe("Recipes screen — loading and error states", () => {
  it("shows loading stub", async () => {
    const { useRecipes } = await import("../api/hooks/recipes");
    vi.mocked(useRecipes).mockReturnValue({
      data: undefined,
      isLoading: true,
      error: null,
    } as ReturnType<typeof useRecipes>);

    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getByText(/Loading recipes/i)).toBeInTheDocument();
  });

  it("shows error stub", async () => {
    const { useRecipes } = await import("../api/hooks/recipes");
    vi.mocked(useRecipes).mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error("fetch failed"),
    } as ReturnType<typeof useRecipes>);

    render(<Recipes />, { wrapper: createWrapper() });
    expect(screen.getByText(/Error loading recipes/i)).toBeInTheDocument();
  });
});
