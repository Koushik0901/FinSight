import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import PageHeader from "./PageHeader";

describe("PageHeader", () => {
  it("renders one semantic page heading and optional supporting content", () => {
    render(
      <PageHeader
        eyebrow="Reports · Year"
        title="How money is moving."
        description="See the shape of your money over time."
        actions={<button type="button">Export</button>}
      />,
    );

    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "How money is moving." })).toBeInTheDocument();
    expect(screen.getByText("Reports · Year")).toBeInTheDocument();
    expect(screen.getByText("See the shape of your money over time.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();
  });

  it("supports the ruled variant and an eyebrow without a signal dot", () => {
    const { container } = render(
      <PageHeader eyebrow="Settings" title="Make it yours." variant="ruled" dot={false} />,
    );

    expect(container.querySelector(".page-header-ruled")).toBeInTheDocument();
    expect(container.querySelector(".page-header-eyebrow .dot")).not.toBeInTheDocument();
  });
});
