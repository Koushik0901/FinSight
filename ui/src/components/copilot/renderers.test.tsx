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

describe("FinSightResponseBlock — composite finance kinds dispatch", () => {
  it("dispatches spendingReview to the SpendingReviewCard", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{
          kind: "spendingReview",
          months: [
            {
              label: "May 2026",
              spentCents: 408600,
              subtitle: "8 of 10 envelopes under",
              categories: [{ label: "Housing", amountCents: 185000, tag: "fixed" }],
              summary: "A steady month.",
              actions: ["Glance at the PG&E bill"],
            },
          ],
        }}
      />
    );
    expect(screen.getByText("May 2026")).toBeInTheDocument();
    expect(screen.getByText("Glance at the PG&E bill")).toBeInTheDocument();
  });

  it("dispatches accountsOverview to the AccountsOverviewCard", () => {
    render(
      <FinSightResponseBlock
        isRunning={false}
        block={{
          kind: "accountsOverview",
          title: "7 accounts",
          subtitle: "$137,515 tracked · 1 missing a balance",
          rows: [{ name: "Vanguard", subtitle: "manual", typeLabel: "Investment", amountCents: null, badge: "needs a balance set" }],
        }}
      />
    );
    expect(screen.getByText("needs a balance set")).toBeInTheDocument();
  });

  it("dispatches spendTimeline, spendingDrivers, watchList, and actionPlan", () => {
    const { rerender } = render(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "spendTimeline", title: "Monthly spend", subtitle: null, points: [{ label: "Jan", amountCents: 1, highlight: false, annotation: null, projected: false }, { label: "Feb", amountCents: 2, highlight: false, annotation: null, projected: false }] }}
      />
    );
    expect(screen.getByText("Jan")).toBeInTheDocument();

    rerender(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "spendingDrivers", title: "Drivers", subtitle: null, drivers: [{ label: "Travel", tag: "planned", amountDisplay: "+$213/mo", note: null }] }}
      />
    );
    expect(screen.getByText("planned")).toBeInTheDocument();

    rerender(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "watchList", title: "Watch out", items: [{ label: "Amex", detail: "24.9% APR", amountDisplay: null }] }}
      />
    );
    expect(screen.getByText("Amex")).toBeInTheDocument();

    rerender(
      <FinSightResponseBlock
        isRunning={false}
        block={{ kind: "actionPlan", title: "Action plan", items: ["Do X"] }}
      />
    );
    expect(screen.getByText("Do X")).toBeInTheDocument();
  });

  it("returns null (no throw) for an unknown block kind, guarding the switch default", () => {
    const { container } = render(
      // @ts-expect-error deliberately invalid kind exercises the default branch
      <FinSightResponseBlock isRunning={false} block={{ kind: "notARealKind" }} />
    );
    expect(container.firstChild).toBeNull();
  });
});
