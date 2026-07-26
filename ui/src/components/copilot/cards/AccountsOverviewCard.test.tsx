import { render, screen } from "@testing-library/react";
import { AccountsOverviewCard } from "./AccountsOverviewCard";

const block = {
  kind: "accountsOverview" as const,
  title: "7 accounts",
  subtitle: "$137,515 tracked · 1 missing a balance",
  rows: [
    { name: "Amex Gold", subtitle: "Amex ····1006", typeLabel: "Credit", amountCents: -241800, badge: null },
    { name: "Vanguard Brokerage", subtitle: "manual", typeLabel: "Investment", amountCents: null, badge: "needs a balance set" },
  ],
};

test("renders header, a negative balance, and the needs-balance badge", () => {
  render(<AccountsOverviewCard block={block} />);
  expect(screen.getByText("7 accounts")).toBeInTheDocument();
  expect(screen.getByText(/1 missing a balance/)).toBeInTheDocument();
  expect(screen.getByText("Amex Gold")).toBeInTheDocument();
  expect(screen.getByText("Credit")).toBeInTheDocument();
  expect(screen.getByText("needs a balance set")).toBeInTheDocument();
});

/**
 * This kind is server-rendered: the model emits a bare
 * `{"kind":"accountsOverview"}` and the server fills the rows from the ledger.
 * An unfilled block therefore means "the server declined", which covers a failed
 * accounts read as well as a genuinely account-less user — so it must render
 * nothing rather than an empty table that asserts "you have no accounts".
 */
test("renders nothing when the server did not fill in any rows", () => {
  const { container } = render(
    <AccountsOverviewCard block={{ kind: "accountsOverview", title: "Accounts", subtitle: null, rows: [] }} />,
  );
  expect(container).toBeEmptyDOMElement();
});
