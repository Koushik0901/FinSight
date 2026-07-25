import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { CategoryReviewQueueCard } from "./CategoryReviewQueueCard";

const navigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual("react-router-dom");
  return { ...actual, useNavigate: () => navigate };
});

function wrap(ui: React.ReactElement) {
  return render(<MemoryRouter>{ui}</MemoryRouter>);
}

const beansCafe = {
  merchant: "BEANS CAFE",
  proposedCategory: "Coffee",
  confidence: 0.42,
  amountCents: -1842,
  date: "2026-07-14T00:00:00Z",
  applied: true,
};

const populated = {
  kind: "categoryReviewQueue" as const,
  pendingCount: 9,
  items: [
    beansCafe,
    { merchant: "MYSTERY LLC", proposedCategory: "Shopping", confidence: 0.31, amountCents: null, date: null, applied: true },
  ],
};

describe("CategoryReviewQueueCard", () => {
  it("renders the pending count, the sampled items and the overflow remainder", () => {
    wrap(<CategoryReviewQueueCard block={populated} />);

    expect(screen.getByText("9 categorizations waiting on you")).toBeInTheDocument();
    expect(screen.getByText("BEANS CAFE")).toBeInTheDocument();
    expect(screen.getByText("Coffee")).toBeInTheDocument();
    expect(screen.getByText("42%")).toBeInTheDocument();
    // 9 pending, 2 shown — the card is a pointer at the queue, not the queue.
    expect(screen.getByText("+ 7 more in the queue")).toBeInTheDocument();
  });

  it("blurs amounts in privacy mode and omits an amount it does not have", () => {
    wrap(<CategoryReviewQueueCard block={populated} />);
    expect(screen.getByText("-$18.42")).toHaveClass("money");
    // The second row has a null amount — no fabricated $0.
    expect(screen.queryByText("$0.00")).not.toBeInTheDocument();
  });

  it("says applied categories are already live rather than pending activation", () => {
    wrap(<CategoryReviewQueueCard block={populated} />);
    expect(screen.getByText(/already applied/)).toBeInTheDocument();
    expect(screen.getByText(/confirms them rather than turning them on/)).toBeInTheDocument();
  });

  it("says so when some items have not been written to their transactions", () => {
    wrap(
      <CategoryReviewQueueCard block={{ ...populated, items: [{ ...beansCafe, applied: false }] }} />
    );
    expect(screen.getByText(/have not been written to their transactions yet/)).toBeInTheDocument();
    expect(screen.queryByText(/already applied/)).not.toBeInTheDocument();
  });

  it("hands off to the review screen instead of mutating in the chat", () => {
    wrap(<CategoryReviewQueueCard block={populated} />);

    // No standalone accept/dismiss affordance — those side effects belong on /review.
    expect(screen.queryByRole("button", { name: /confirm/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /dismiss/i })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Open the review queue/ }));
    expect(navigate).toHaveBeenCalledWith("/review");
  });

  it("renders an all-caught-up state for an empty queue", () => {
    wrap(<CategoryReviewQueueCard block={{ kind: "categoryReviewQueue", pendingCount: 0, items: [] }} />);
    expect(screen.getByText("Nothing waiting on your review")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Open the review queue/ })).not.toBeInTheDocument();
  });
});
