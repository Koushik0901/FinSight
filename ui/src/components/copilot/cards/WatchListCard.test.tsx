import { render, screen } from "@testing-library/react";
import { WatchListCard } from "./WatchListCard";

const block = {
  kind: "watchList" as const,
  title: "Watch out for these",
  items: [
    { label: "The Amex balance", detail: "revolving at 24.9%", amountDisplay: "−$50/mo" },
    { label: "MasterClass trial", detail: "flips to $180/yr on the 26th", amountDisplay: null },
  ],
};

test("renders numbered watch items", () => {
  render(<WatchListCard block={block} />);
  expect(screen.getByText("The Amex balance")).toBeInTheDocument();
  expect(screen.getByText("MasterClass trial")).toBeInTheDocument();
  expect(screen.getByText("1")).toBeInTheDocument();
  expect(screen.getByText("2")).toBeInTheDocument();
});
