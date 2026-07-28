import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import EmptyState from "./EmptyState";

describe("EmptyState", () => {
  it("uses a section-level heading by default", () => {
    render(<EmptyState title="Nothing here" />);

    expect(screen.getByRole("heading", { level: 2, name: "Nothing here" })).toBeInTheDocument();
  });

  it("can provide the page heading when the empty state is the whole screen", () => {
    render(
      <EmptyState headingLevel={1} title="No accounts yet" description="Add an account to begin." />,
    );

    expect(screen.getByRole("heading", { level: 1, name: "No accounts yet" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { level: 2 })).not.toBeInTheDocument();
  });

  it("can explain what becomes available without rendering fake preview metrics", () => {
    render(
      <EmptyState
        title="Add financial history"
        details={<ul><li>Monthly spending patterns</li></ul>}
      />,
    );

    expect(screen.getByText("Monthly spending patterns")).toBeInTheDocument();
  });
});
