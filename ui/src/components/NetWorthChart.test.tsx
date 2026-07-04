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

  it("draws pixel-space gridlines, per-point markers, and a compact end-value callout", () => {
    const { container } = render(<NetWorthChart points={POINTS} />);

    // Never stretch with preserveAspectRatio="none" — that smears the stroke.
    const svg = container.querySelector("svg")!;
    expect(svg.getAttribute("preserveAspectRatio")).toBeNull();

    // 4 horizontal gridlines.
    expect(container.querySelectorAll("line")).toHaveLength(4);

    // A marker per point: 2 small dots + 1 emphasized end dot.
    expect(container.querySelectorAll("circle")).toHaveLength(3);

    // Compact end-value callout ($1,400.00 → "$1.4K").
    expect(screen.getByText("$1.4K")).toBeInTheDocument();
  });

  it("skips per-point markers on dense ranges but keeps the end dot", () => {
    const dense = Array.from({ length: 24 }, (_, i) => ({
      date: `2024-${String((i % 12) + 1).padStart(2, "0")}-15`,
      totalCents: 100000 + i * 5000,
    }));
    const { container } = render(<NetWorthChart points={dense} />);
    expect(container.querySelectorAll("circle")).toHaveLength(1);
  });
});
