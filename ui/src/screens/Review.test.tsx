import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { createWrapperWithEntries } from "../test-utils";
import Review from "./Review";

vi.mock("./Inbox", () => ({ default: ({ embedded }: { embedded?: boolean }) => <div data-testid="attention-pane">Attention {String(embedded)}</div> }));
vi.mock("./CategoryReview", () => ({ default: ({ embedded }: { embedded?: boolean }) => <div data-testid="category-pane">Categories {String(embedded)}</div> }));

function renderAt(path: string) {
  const Wrapper = createWrapperWithEntries([path]);
  return render(<Wrapper><Review /></Wrapper>);
}

describe("Review hub", () => {
  it("opens the combined attention view by default", () => {
    renderAt("/inbox");
    expect(screen.getByRole("heading", { name: "Decisions waiting for you." })).toBeInTheDocument();
    expect(screen.getByTestId("attention-pane")).toHaveTextContent("true");
    expect(screen.queryByTestId("category-pane")).not.toBeInTheDocument();
  });

  it("switches to category decisions without leaving Review", () => {
    renderAt("/inbox");
    fireEvent.click(screen.getByRole("tab", { name: "Category decisions" }));
    expect(screen.getByTestId("category-pane")).toHaveTextContent("true");
    expect(screen.queryByTestId("attention-pane")).not.toBeInTheDocument();
  });

  it("honors a direct category-decisions link", () => {
    renderAt("/inbox?view=categories");
    expect(screen.getByRole("tab", { name: "Category decisions" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByTestId("category-pane")).toBeInTheDocument();
  });
});
