import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import StepCategories from "./StepCategories";

vi.mock("../../api/openapiClient", () => ({
  api: { commitStarterCategories: vi.fn() },
  unwrap: vi.fn(),
}));

describe("StepCategories", () => {
  it("keeps each row compact and exposes color plus icon controls", () => {
    render(<StepCategories onNext={() => {}} />);

    expect(screen.getAllByRole("textbox", { name: /category \d+ label/i })).toHaveLength(10);
    expect(screen.getAllByLabelText(/choose color for/i)).toHaveLength(10);
    expect(screen.getAllByRole("button", { name: /choose icon for/i })).toHaveLength(10);
    expect(screen.queryByRole("radiogroup")).not.toBeInTheDocument();
  });

  it("opens an icon picker for a row and keeps the chosen icon selected", () => {
    render(<StepCategories onNext={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "Choose icon for Housing" }));
    const goalIcon = screen.getByRole("button", { name: "Use Goal icon" });
    expect(goalIcon).toHaveAttribute("aria-pressed", "false");

    fireEvent.click(goalIcon);

    expect(screen.queryByRole("group", { name: /icons for housing/i })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose icon for Housing" })).toHaveAttribute("aria-expanded", "false");
  });
});
