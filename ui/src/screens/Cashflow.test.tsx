import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Cashflow from "./Cashflow";
import type { CashflowForecast } from "../api/client";

const useCashflowForecast = vi.fn();
const useFinancialMetrics = vi.fn();
vi.mock("../api/hooks/cashflow", () => ({
  useCashflowForecast: (params: unknown) => useCashflowForecast(params),
}));
vi.mock("../api/hooks/metrics", () => ({
  useFinancialMetrics: () => useFinancialMetrics(),
}));

function forecast(over: Partial<CashflowForecast> = {}): CashflowForecast {
  const days = Array.from({ length: 30 }, (_, i) => ({
    date: `2026-08-${String(i + 1).padStart(2, "0")}`,
    projectedBalanceCents: 200_000 - i * 5_000,
    eventNetCents: 0,
    burnCents: -5_000,
    belowBuffer: 200_000 - i * 5_000 < 50_000,
  }));
  return {
    asOf: "2026-08-01",
    horizonDays: 30,
    startBalanceCents: 200_000,
    bufferCents: 50_000,
    dailyBurnCents: 5_000,
    days,
    lowestBalanceCents: 55_000,
    lowestDate: "2026-08-29",
    firstBreachDate: "2026-08-31",
    safeToSpendCents: 5_000,
    upcomingEvents: [
      { date: "2026-08-05", label: "Rent", amountCents: -145_000, kind: "bill", confidence: 0.8 },
      { date: "2026-08-15", label: "Employer payroll", amountCents: 400_000, kind: "income", confidence: 0.9 },
    ],
    warnings: [
      { level: "caution", message: "Car insurance (about $680) is due 2026-09-04, just after this window — plan for it." },
      { level: "info", message: "Everyday spending is projected at about $164/day from your recent average." },
    ],
    reliable: true,
    ...over,
  };
}

describe("Cashflow screen", () => {
  beforeEach(() => {
    useCashflowForecast.mockReset();
    useFinancialMetrics.mockReset();
    useFinancialMetrics.mockReturnValue({ data: { currency: "USD" } });
  });

  it("renders safe-to-spend, upcoming events, and warnings", () => {
    useCashflowForecast.mockReturnValue({ data: forecast(), isLoading: false, isError: false });
    render(<Cashflow />);

    // Safe-to-spend figure ($50, whole dollars) and the section.
    expect(screen.getByText("Safe to spend now")).toBeInTheDocument();
    expect(screen.getByText("$50")).toBeInTheDocument();
    // Upcoming dated events.
    expect(screen.getByText("Rent")).toBeInTheDocument();
    expect(screen.getByText("Employer payroll")).toBeInTheDocument();
    // Data-quality warnings surface.
    expect(screen.getByText(/Car insurance/)).toBeInTheDocument();
    expect(screen.getByText(/Everyday spending is projected/)).toBeInTheDocument();
  });

  it("warns about the tight point when the balance breaches the buffer", () => {
    useCashflowForecast.mockReturnValue({ data: forecast(), isLoading: false, isError: false });
    render(<Cashflow />);
    expect(screen.getByText(/dips to/)).toBeInTheDocument();
  });

  it("shows a reassuring message when the balance never breaches the buffer", () => {
    useCashflowForecast.mockReturnValue({
      data: forecast({ firstBreachDate: null, lowestBalanceCents: 120_000, safeToSpendCents: 70_000 }),
      isLoading: false,
      isError: false,
    });
    render(<Cashflow />);
    expect(screen.getByText(/stays above/)).toBeInTheDocument();
    expect(screen.queryByText(/dips to/)).not.toBeInTheDocument();
  });

  it("discloses when the forecast is unreliable", () => {
    useCashflowForecast.mockReturnValue({
      data: forecast({ reliable: false }),
      isLoading: false,
      isError: false,
    });
    render(<Cashflow />);
    expect(screen.getByText(/rough estimate/)).toBeInTheDocument();
  });

  it("lets the user change the horizon (refetches with the new value)", () => {
    useCashflowForecast.mockReturnValue({ data: forecast(), isLoading: false, isError: false });
    render(<Cashflow />);
    fireEvent.click(screen.getByRole("button", { name: "60d" }));
    // The hook is called with the updated horizon.
    expect(useCashflowForecast).toHaveBeenCalledWith(expect.objectContaining({ horizonDays: 60 }));
  });
});
