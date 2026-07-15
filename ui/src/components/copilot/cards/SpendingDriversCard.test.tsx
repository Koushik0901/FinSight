import { render, screen } from "@testing-library/react";
import { SpendingDriversCard } from "./SpendingDriversCard";

const block = {
  kind: "spendingDrivers" as const,
  title: "Drivers",
  subtitle: null,
  drivers: [{ label: "Travel", tag: "planned" as const, amountDisplay: "+$213/mo", note: "Italy deposits" }],
};

test("renders a driver row with tag and amount", () => {
  render(<SpendingDriversCard block={block} />);
  expect(screen.getByText("Travel")).toBeInTheDocument();
  expect(screen.getByText("planned")).toBeInTheDocument();
  expect(screen.getByText("+$213/mo")).toBeInTheDocument();
});
