import { render, screen, fireEvent } from "@testing-library/react";
import { vi, describe, it, expect, beforeEach } from "vitest";
import React from "react";

vi.mock("react-focus-lock", () => ({
  default: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

vi.mock("../api/hooks/transactions", () => ({
  useCategories: vi.fn().mockReturnValue({
    data: [
      { id: "cat-1", label: "Groceries", color: "#4ade80", group_id: "g1", group_label: "Food" },
      { id: "cat-2", label: "Restaurants", color: "#f97316", group_id: "g1", group_label: "Food" },
      { id: "cat-3", label: "Rent", color: "#60a5fa", group_id: "g2", group_label: "Housing" },
    ],
    isLoading: false,
  }),
}));

import CategoryPicker from "./CategoryPicker";

describe("CategoryPicker", () => {
  const onChange = vi.fn();

  beforeEach(() => onChange.mockClear());

  it("renders group headers and category items", () => {
    render(<CategoryPicker value={null} onChange={onChange} />);
    expect(screen.getByText("Food")).toBeInTheDocument();
    expect(screen.getByText("Housing")).toBeInTheDocument();
    expect(screen.getByText("Groceries")).toBeInTheDocument();
    expect(screen.getByText("Rent")).toBeInTheDocument();
  });

  it("filters items by search query", () => {
    render(<CategoryPicker value={null} onChange={onChange} />);
    const input = screen.getByRole("searchbox");
    fireEvent.change(input, { target: { value: "groc" } });
    expect(screen.getByText("Groceries")).toBeInTheDocument();
    expect(screen.queryByText("Rent")).not.toBeInTheDocument();
  });

  it("calls onChange when a category is clicked", () => {
    render(<CategoryPicker value={null} onChange={onChange} />);
    fireEvent.click(screen.getByRole("option", { name: /Groceries/i }));
    expect(onChange).toHaveBeenCalledWith("cat-1");
  });

  it("marks the selected item as aria-selected", () => {
    render(<CategoryPicker value="cat-3" onChange={onChange} />);
    const opt = screen.getByRole("option", { name: /Rent/i });
    expect(opt).toHaveAttribute("aria-selected", "true");
    const notOpt = screen.getByRole("option", { name: /Groceries/i });
    expect(notOpt).toHaveAttribute("aria-selected", "false");
  });
});
