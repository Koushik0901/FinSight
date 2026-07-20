import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { createWrapper } from "../test-utils";
import * as metricsHooks from "../api/hooks/metrics";

/**
 * The philosophy preferences are not cosmetic: they set the debt-payoff order
 * the ranking engine uses and the APR above which debt is treated as urgent,
 * and they are stated in the Copilot's prompt.
 *
 * These tests cover the screen's contract — that it shows what is stored,
 * sends what was chosen, and that an untouched profile reads as the previous
 * hard-coded behaviour.
 */

const mutateAsync = vi.fn().mockResolvedValue(undefined);

vi.mock("../api/hooks/metrics", () => ({
  useFinancialMetrics: vi.fn(() => ({ data: null })),
  useSetFinancialAssumptions: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useFinancialPhilosophy: vi.fn(),
  useSetFinancialPhilosophy: vi.fn(),
}));

// Everything below is unrelated to this section but pulled in by the screen.
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

async function renderSection(philosophy: unknown) {
  vi.mocked(metricsHooks.useFinancialPhilosophy).mockReturnValue({
    data: philosophy,
  } as unknown as ReturnType<typeof metricsHooks.useFinancialPhilosophy>);
  vi.mocked(metricsHooks.useSetFinancialPhilosophy).mockReturnValue({
    mutateAsync,
    isPending: false,
  } as unknown as ReturnType<typeof metricsHooks.useSetFinancialPhilosophy>);
  const { PhilosophySection } = await import("./Settings");
  return render(<PhilosophySection />, { wrapper: createWrapper() });
}

beforeEach(() => {
  vi.clearAllMocks();
  mutateAsync.mockResolvedValue(undefined);
});

describe("advice preferences", () => {
  it("shows the stored choice as selected", async () => {
    await renderSection({ debtStrategy: "snowball", riskTolerance: "cautious", highInterestAprPct: 5 });
    await waitFor(() => {
      expect(screen.getByRole("radio", { name: /smallest balance first/i })).toBeChecked();
    });
    expect(screen.getByRole("radio", { name: /debt-averse/i })).toBeChecked();
  });

  it("spells out the consequence of the risk choice rather than just its name", async () => {
    await renderSection({ debtStrategy: "avalanche", riskTolerance: "cautious", highInterestAprPct: 5 });
    await waitFor(() => {
      expect(screen.getByText(/5% APR as urgent/i)).toBeInTheDocument();
    });
  });

  it("sends the chosen values", async () => {
    await renderSection({ debtStrategy: "avalanche", riskTolerance: "balanced", highInterestAprPct: 8 });
    fireEvent.click(await screen.findByRole("radio", { name: /smallest balance first/i }));
    fireEvent.click(screen.getByRole("button", { name: /apply preferences/i }));
    await waitFor(() => {
      expect(mutateAsync).toHaveBeenCalledWith(
        expect.objectContaining({ debtStrategy: "snowball", riskTolerance: "balanced" }),
      );
    });
  });

  it("cannot be applied until something changes", async () => {
    await renderSection({ debtStrategy: "avalanche", riskTolerance: "balanced", highInterestAprPct: 8 });
    expect(await screen.findByRole("button", { name: /apply preferences/i })).toBeDisabled();
  });

  it("falls back to the defaults before the preference loads", async () => {
    // A brand-new user, or a slow load: the untouched profile must read as
    // the behaviour the app had before this setting existed.
    await renderSection(undefined);
    await waitFor(() => {
      expect(screen.getByRole("radio", { name: /highest interest first/i })).toBeChecked();
    });
    expect(screen.getByRole("radio", { name: /balanced/i })).toBeChecked();
  });
});
