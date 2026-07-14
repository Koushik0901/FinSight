import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import PathBack from "./PathBack";
import { createWrapper } from "../test-utils";
import { usePathBack, useSetSpendingAnnotation } from "../api/hooks/spending";
import type { PathBackView } from "../api/client";

vi.mock("../api/hooks/spending", () => ({
  usePathBack: vi.fn(),
  useSetSpendingAnnotation: vi.fn(),
}));

const VIEW: PathBackView = {
  period: "2026-06",
  assessment: {
    class: "regime_shift",
    period_total_cents: 450000,
    baseline_monthly_cents: 320000,
    upper_band_cents: 360000,
    elevated_recent_months: 3,
    baseline_months: 12,
    mixed_currency: false,
    note: "Spend has stayed elevated for 3 months running.",
  },
  plan: {
    currency: "USD",
    recent_monthly_cents: 450000,
    baseline_monthly_cents: 320000,
    self_correcting_cents: 40000,
    recoverable_recurring_cents: 90000,
    projected_after_levers_cents: 340000,
    levers: [
      {
        merchant_key: "amazon",
        display: "Amazon",
        category: "Shopping",
        delta_cents: 6000,
        recent_monthly_cents: 12000,
        base_monthly_cents: 6000,
        recent_txns_per_month: 4,
        base_txns_per_month: 2,
        mechanism: "frequency_up",
        persistence: "recurring",
        user_verdict: null,
      },
      {
        merchant_key: "netflix",
        display: "Netflix",
        category: "Subscriptions",
        delta_cents: 3000,
        recent_monthly_cents: 3000,
        base_monthly_cents: 0,
        recent_txns_per_month: 1,
        base_txns_per_month: 0,
        mechanism: "new",
        persistence: "recurring",
        user_verdict: null,
      },
    ],
    self_correcting: [
      {
        merchant_key: "airline-x",
        display: "Airline X",
        category: "Travel",
        delta_cents: 40000,
        recent_monthly_cents: 40000,
        base_monthly_cents: 0,
        recent_txns_per_month: 1,
        base_txns_per_month: 0,
        mechanism: "new",
        persistence: "one_off",
        user_verdict: null,
      },
    ],
    target_monthly_cents: 300000,
    structural_gap_cents: 15000,
    note: "Trimming every lever gets you to $3,400/mo — $150 short of your $3,000 target.",
  },
};

beforeEach(() => {
  vi.mocked(usePathBack).mockReturnValue({
    data: VIEW,
    isLoading: false,
    error: null,
  } as unknown as ReturnType<typeof usePathBack>);
});

describe("PathBack screen", () => {
  it("renders the regime-shift banner", () => {
    const mutate = vi.fn();
    vi.mocked(useSetSpendingAnnotation).mockReturnValue({ mutate, isPending: false } as unknown as ReturnType<typeof useSetSpendingAnnotation>);
    render(<PathBack />, { wrapper: createWrapper() });
    expect(screen.getByText("Regime shift — not a blip")).toBeInTheDocument();
  });

  it("renders a lever's display name and delta", () => {
    const mutate = vi.fn();
    vi.mocked(useSetSpendingAnnotation).mockReturnValue({ mutate, isPending: false } as unknown as ReturnType<typeof useSetSpendingAnnotation>);
    render(<PathBack />, { wrapper: createWrapper() });
    expect(screen.getByText("Amazon")).toBeInTheDocument();
    expect(screen.getByText("+$60")).toBeInTheDocument();
  });

  it("renders the structural-gap copy", () => {
    const mutate = vi.fn();
    vi.mocked(useSetSpendingAnnotation).mockReturnValue({ mutate, isPending: false } as unknown as ReturnType<typeof useSetSpendingAnnotation>);
    render(<PathBack />, { wrapper: createWrapper() });
    expect(screen.getByText(/of that is structural/)).toBeInTheDocument();
  });

  it("calls the annotation mutation with the merchant key and verdict when Keep is clicked", () => {
    const mutate = vi.fn();
    vi.mocked(useSetSpendingAnnotation).mockReturnValue({ mutate, isPending: false } as unknown as ReturnType<typeof useSetSpendingAnnotation>);
    render(<PathBack />, { wrapper: createWrapper() });

    const amazonRow = screen.getByText("Amazon").closest("div.row");
    expect(amazonRow).toBeTruthy();
    const keepButton = screen.getAllByRole("button", { name: "Keep" })[0]!;
    fireEvent.click(keepButton);

    expect(mutate).toHaveBeenCalledWith(
      { merchantKey: "amazon", verdict: "expected" },
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    );
  });
});
