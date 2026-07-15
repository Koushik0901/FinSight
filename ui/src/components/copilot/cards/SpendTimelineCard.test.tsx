import { render, screen } from "@testing-library/react";
import { SpendTimelineCard } from "./SpendTimelineCard";

const block = {
  kind: "spendTimeline" as const,
  title: "Monthly spend",
  subtitle: null,
  points: [
    { label: "Jan", amountCents: 360000, highlight: false, annotation: null, projected: false },
    { label: "Apr", amountCents: 570000, highlight: false, annotation: "LISBON", projected: false },
    { label: "Jul", amountCents: 440000, highlight: true, annotation: null, projected: true },
  ],
};

test("renders bars with labels and an annotation", () => {
  render(<SpendTimelineCard block={block} />);
  expect(screen.getByText("Jan")).toBeInTheDocument();
  expect(screen.getByText("Jul")).toBeInTheDocument();
  expect(screen.getByText("LISBON")).toBeInTheDocument();
});
