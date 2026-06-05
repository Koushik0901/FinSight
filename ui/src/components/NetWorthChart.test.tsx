import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import NetWorthChart from "./NetWorthChart";

const POINTS = [
  { date: "2026-01-15", totalCents: 100000 },
  { date: "2026-02-15", totalCents: 150000 },
  { date: "2026-03-15", totalCents: 140000 },
];

describe("NetWorthChart", () => {
  it("renders an SVG path when there are ≥2 points", () => {
    const { container } = render(<NetWorthChart points={POINTS} />);
    expect(container.querySelector("path")).toBeTruthy();
  });

  it("shows a building-history stub with fewer than 2 points", () => {
    render(<NetWorthChart points={[{ date: "2026-03-15", totalCents: 140000 }]} />);
    expect(screen.getByText(/still building/i)).toBeInTheDocument();
  });

  it("renders with exactly 2 points", () => {
    const { container } = render(<NetWorthChart points={[
      { date: "2026-01-15", totalCents: 100000 },
      { date: "2026-02-15", totalCents: 200000 },
    ]} />);
    expect(container.querySelector("path")).toBeTruthy();
  });

  it("renders without crashing when all values are equal", () => {
    const { container } = render(<NetWorthChart points={[
      { date: "2026-01-15", totalCents: 100000 },
      { date: "2026-02-15", totalCents: 100000 },
      { date: "2026-03-15", totalCents: 100000 },
    ]} />);
    expect(container.querySelector("path")).toBeTruthy();
  });

  it("renders a negative net worth without crashing", () => {
    const { container } = render(<NetWorthChart points={[
      { date: "2026-01-15", totalCents: -500000 },
      { date: "2026-02-15", totalCents: -300000 },
    ]} />);
    expect(container.querySelector("path")).toBeTruthy();
  });
});
