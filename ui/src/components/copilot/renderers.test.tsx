import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { FinSightResponseBlock } from "./renderers";

describe("FinSightResponseBlock — existing generic kinds", () => {
  it("renders a table block's rows", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "table", title: "Alternatives", columns: ["Option", "Cost"], rows: [["A", "$10"]] }}
      />
    );
    expect(screen.getByText("Alternatives")).toBeInTheDocument();
    expect(screen.getByText("A")).toBeInTheDocument();
  });

  it("renders a metricGrid block's metrics", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "metricGrid", metrics: [{ label: "Net worth", value: "$20,606", detail: null, tone: null }] }}
      />
    );
    expect(screen.getByText("Net worth")).toBeInTheDocument();
    expect(screen.getByText("$20,606")).toBeInTheDocument();
  });

  it("renders a callout block's title and body", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "callout", tone: "warning", title: "Heads up", body: "Missing APR data." }}
      />
    );
    expect(screen.getByText("Heads up")).toBeInTheDocument();
    expect(screen.getByText("Missing APR data.")).toBeInTheDocument();
  });

  it("renders a barChart block's title and points", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{
          kind: "barChart",
          title: "Spend by category",
          seriesLabel: null,
          data: [{ label: "Dining", value: 420 }],
        }}
      />
    );
    expect(screen.getByText("Spend by category")).toBeInTheDocument();
    expect(screen.getByText("Dining")).toBeInTheDocument();
    expect(screen.getByText("420")).toBeInTheDocument();
  });

  it("colors a Category column cell with a category dot", () => {
    const { container } = render(
      <FinSightResponseBlock
        isRunning={false}
        block={{
          kind: "table",
          title: null,
          columns: ["Category", "Amount"],
          rows: [["Dining", "$10"]],
        }}
      />
    );
    expect(screen.getByText("Dining")).toBeInTheDocument();
    expect(container.querySelector(".cp-dot")).toBeInTheDocument();
  });
});
