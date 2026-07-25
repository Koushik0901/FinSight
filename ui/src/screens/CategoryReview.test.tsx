import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { axe } from "vitest-axe";
import CategoryReview from "./CategoryReview";
import { createWrapper } from "../test-utils";

/**
 * These tests drive the REAL hooks (`useCategoryProposals`, `useTransactions`,
 * `useCategories`) through a mocked `commands` object, rather than stubbing the
 * hooks out. That is deliberate: the thing most worth protecting here is the
 * wiring — that queue membership comes from `listCategoryProposals` and that
 * each action calls its command with the PROPOSAL id, not the transaction id.
 */

const listCategoryProposals = vi.fn();
const acceptCategoryProposal = vi.fn();
const correctCategoryProposal = vi.fn();
const rejectCategoryProposal = vi.fn();
const listTransactions = vi.fn();
const listCategories = vi.fn();
const listAccounts = vi.fn();

vi.mock("../api/client", async () => {
  const actual = await vi.importActual("../api/client");
  return {
    ...actual,
    commands: {
      listCategoryProposals: () => listCategoryProposals(),
      acceptCategoryProposal: (id: string) => acceptCategoryProposal(id),
      correctCategoryProposal: (id: string, categoryId: string) =>
        correctCategoryProposal(id, categoryId),
      rejectCategoryProposal: (id: string) => rejectCategoryProposal(id),
      listTransactions: (filter: unknown) => listTransactions(filter),
      listCategories: () => listCategories(),
      listAccounts: () => listAccounts(),
    },
  };
});

const CATEGORIES = [
  { id: "cat-coffee", label: "Coffee", color: "#a3e635", group_id: "g1", group_label: "Food", spending_type: "Want" },
  { id: "cat-groceries", label: "Groceries", color: "#4ade80", group_id: "g1", group_label: "Food", spending_type: "Need" },
];

function txn(overrides: Record<string, unknown> = {}) {
  return {
    id: "txn-1",
    account_id: "acc-1",
    posted_at: "2026-07-14T00:00:00Z",
    amount_cents: -1842,
    merchant_raw: "BEANS CAFE #114",
    merchant_id: null,
    merchant_label: "Beans Cafe",
    merchant_color: null,
    merchant_initials: null,
    category_id: "cat-coffee",
    category_label: "Coffee",
    category_color: "#a3e635",
    status: "cleared",
    notes: null,
    ai_confidence: 0.42,
    ai_explanation: null,
    is_anomaly: false,
    created_at: "2026-07-14T00:00:00Z",
    is_reimbursable: false,
    settle_up: false,
    is_split: false,
    is_transfer: false,
    ...overrides,
  };
}

function proposal(overrides: Record<string, unknown> = {}) {
  return {
    id: "prop-1",
    txnId: "txn-1",
    proposedCategoryId: "cat-coffee",
    source: "llm",
    confidence: 0.42,
    rationale: "Looks like a cafe purchase",
    candidatesJson: null,
    status: "pending",
    applied: true,
    model: "test-model",
    createdAt: "2026-07-14T01:00:00Z",
    reviewedAt: null,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  listCategories.mockResolvedValue({ status: "ok", data: CATEGORIES });
  // A non-USD account, so the amount assertion proves the currency comes from
  // the transaction's own account rather than a hardcoded default.
  listAccounts.mockResolvedValue({
    status: "ok",
    data: [{ id: "acc-1", owner: "You", bank: "Bank", type: "Checking", name: "Chequing", balance_cents: 100000, currency: "CAD" }],
  });
  listCategoryProposals.mockResolvedValue({ status: "ok", data: [proposal()] });
  listTransactions.mockResolvedValue({ status: "ok", data: [txn()] });
  acceptCategoryProposal.mockResolvedValue({ status: "ok", data: { transaction: txn(), proposed_rule: null } });
  correctCategoryProposal.mockResolvedValue({ status: "ok", data: { transaction: txn(), proposed_rule: null } });
  rejectCategoryProposal.mockResolvedValue({ status: "ok", data: null });
});

describe("CategoryReview — the queue itself", () => {
  it("lists a pending proposal with its transaction, proposed category and confidence", async () => {
    render(<CategoryReview />, { wrapper: createWrapper() });

    expect(await screen.findByText("Beans Cafe")).toBeInTheDocument();
    expect(screen.getByTestId("proposed-category")).toHaveTextContent("Coffee");
    expect(screen.getByText(/42% confident/)).toBeInTheDocument();
    expect(screen.getByText(/Looks like a cafe purchase/)).toBeInTheDocument();
    expect(screen.getByText("1 categorization to confirm.")).toBeInTheDocument();
  });

  it("renders the amount in the account's own currency, tagged for privacy blurring", async () => {
    render(<CategoryReview />, { wrapper: createWrapper() });
    // CA$, not $ — the currency is read from the transaction's account.
    const amount = await screen.findByText(/CA\$18\.42/);
    expect(amount).toHaveClass("money");
  });

  it("draws membership from category_proposals, not from the transactions it fetches", async () => {
    // The enrichment fetch returns an EXTRA low-ai_confidence transaction that
    // has no proposal. Under the old `ai_confidence < 0.6` predicate it would
    // have been a queue item; sourced from `category_proposals` it must not be.
    listTransactions.mockResolvedValue({
      status: "ok",
      data: [txn(), txn({ id: "txn-2", merchant_label: "Ghost Merchant", ai_confidence: 0.1 })],
    });

    render(<CategoryReview />, { wrapper: createWrapper() });

    expect(await screen.findByText("Beans Cafe")).toBeInTheDocument();
    expect(screen.queryByText("Ghost Merchant")).not.toBeInTheDocument();
    expect(screen.getByText("1 categorization to confirm.")).toBeInTheDocument();
  });

  it("still renders a proposal whose transaction the enrichment fetch missed", async () => {
    listTransactions.mockResolvedValue({ status: "ok", data: [] });

    render(<CategoryReview />, { wrapper: createWrapper() });

    expect(await screen.findByText(/Transaction details unavailable/)).toBeInTheDocument();
    // The item is still actionable — a queue that hides rows can never be cleared.
    expect(screen.getByRole("button", { name: /^Confirm Coffee/ })).toBeInTheDocument();
  });

  it("sizes the enrichment fetch to cover the whole queue, not one default page", async () => {
    // 140 pending proposals: the backend's default limit is 100, so a fetch
    // that did not scale with the queue would leave 40 rows without their
    // transaction — the exact silent-truncation this sizing exists to avoid.
    const many = Array.from({ length: 140 }, (_, i) =>
      proposal({ id: `prop-${i}`, txnId: `txn-${i}` })
    );
    listCategoryProposals.mockResolvedValue({ status: "ok", data: many });
    listTransactions.mockResolvedValue({
      status: "ok",
      data: many.map((p, i) => txn({ id: p.txnId, merchant_label: `Merchant ${i}` })),
    });

    render(<CategoryReview />, { wrapper: createWrapper() });

    await waitFor(() => {
      const last = listTransactions.mock.calls.at(-1)?.[0] as { limit: number };
      expect(last.limit).toBeGreaterThanOrEqual(many.length);
    });
    const filter = listTransactions.mock.calls.at(-1)?.[0] as { limit: number; filterPreset: string };
    expect(filter.filterPreset).toBe("needs_review");
  });

  it("shows an all-caught-up empty state when nothing is pending", async () => {
    listCategoryProposals.mockResolvedValue({ status: "ok", data: [] });

    render(<CategoryReview />, { wrapper: createWrapper() });

    expect(await screen.findByText("You're all caught up")).toBeInTheDocument();
    expect(screen.getByText("Nothing to review.")).toBeInTheDocument();
  });
});

describe("CategoryReview — actions", () => {
  it("accepts with the PROPOSAL id, not the transaction id", async () => {
    render(<CategoryReview />, { wrapper: createWrapper() });

    fireEvent.click(await screen.findByRole("button", { name: /^Confirm Coffee/ }));

    await waitFor(() => expect(acceptCategoryProposal).toHaveBeenCalledWith("prop-1"));
    expect(acceptCategoryProposal).not.toHaveBeenCalledWith("txn-1");
  });

  it("corrects to the category the user picks", async () => {
    render(<CategoryReview />, { wrapper: createWrapper() });

    fireEvent.click(await screen.findByRole("button", { name: /Change category/ }));
    const list = await screen.findByRole("listbox", { name: "Category" });
    fireEvent.click(within(list).getByRole("option", { name: /Groceries/ }));

    await waitFor(() => expect(correctCategoryProposal).toHaveBeenCalledWith("prop-1", "cat-groceries"));
    expect(acceptCategoryProposal).not.toHaveBeenCalled();
  });

  it("rejects with the proposal id and says the applied category is kept", async () => {
    render(<CategoryReview />, { wrapper: createWrapper() });

    expect(
      await screen.findByText(/Dismiss keeps Coffee on this transaction — it only clears the item from this queue\./)
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Dismiss the suggestion/ }));
    await waitFor(() => expect(rejectCategoryProposal).toHaveBeenCalledWith("prop-1"));
  });
});

describe("CategoryReview — status vs applied are two separate axes", () => {
  it("tells the user an applied proposal is ALREADY affecting budgets", async () => {
    render(<CategoryReview />, { wrapper: createWrapper() });

    expect(await screen.findByText("Already applied")).toBeInTheDocument();
    expect(screen.getByText(/already live/)).toBeInTheDocument();
    expect(screen.getByText(/counts toward your budgets and reports right now/)).toBeInTheDocument();
  });

  it("tells the user a not-yet-applied proposal has written nothing", async () => {
    listCategoryProposals.mockResolvedValue({
      status: "ok",
      data: [proposal({ applied: false, source: "ml" })],
    });
    listTransactions.mockResolvedValue({ status: "ok", data: [txn({ category_id: null, category_label: null })] });

    render(<CategoryReview />, { wrapper: createWrapper() });

    expect(await screen.findByText("Not applied")).toBeInTheDocument();
    expect(screen.getByText(/Nothing has been written to this transaction yet/)).toBeInTheDocument();
    expect(screen.getByText(/Dismiss leaves this transaction uncategorized\./)).toBeInTheDocument();
    // The "already live" reassurance must NOT leak onto a suggestion-only row.
    expect(screen.queryByText(/already live/)).not.toBeInTheDocument();
  });
});

describe("CategoryReview — accessibility", () => {
  it("has no detectable axe violations", async () => {
    const { container } = render(<CategoryReview />, { wrapper: createWrapper() });
    await screen.findByText("Beans Cafe");
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });
});
