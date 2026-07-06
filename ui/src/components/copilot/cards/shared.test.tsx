import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { SegmentBar, ConfidenceBadge } from "./shared";

describe("SegmentBar", () => {
  it("renders the label, a proportional-width fill, and the formatted amount", () => {
    render(<SegmentBar label="Housing" amountCents={185_000} maxCents={200_000} color="#A78BFA" />);
    expect(screen.getByText("Housing")).toBeInTheDocument();
    expect(screen.getByText("$1,850")).toBeInTheDocument();
    const fill = screen.getByTestId("segment-bar-fill");
    expect(fill.style.width).toBe("92.5%");
    expect(fill.style.background).toBe("rgb(167, 139, 250)"); // #A78BFA in rgb, sanity-checks the color prop reached the DOM
  });

  it("renders an optional tag chip", () => {
    render(<SegmentBar label="Dining" amountCents={41_200} maxCents={100_000} color="#FB923C" tag={{ text: "lever" }} />);
    expect(screen.getByText("lever")).toBeInTheDocument();
  });

  it("renders a muted tag distinctly (e.g. 'fixed' vs 'lever')", () => {
    render(<SegmentBar label="Housing" amountCents={185_000} maxCents={200_000} color="#A78BFA" tag={{ text: "fixed", muted: true }} />);
    expect(screen.getByText("fixed").className).toContain("muted");
  });
});

describe("ConfidenceBadge", () => {
  it("renders a percentage-filled track and the numeric percentage", () => {
    render(<ConfidenceBadge confidence={0.99} color="#34D399" />);
    expect(screen.getByText("99%")).toBeInTheDocument();
    const fill = screen.getByTestId("confidence-fill");
    expect(fill.style.width).toBe("99%");
  });
});
