import { render, screen } from "@testing-library/react";
import { StatLine, TagPill, ActionChecklist } from "./shared";

test("StatLine joins parts with a middot", () => {
  render(<StatLine parts={["$4,086 spent", "8 of 10 envelopes under"]} />);
  expect(screen.getByText(/\$4,086 spent · 8 of 10 envelopes under/)).toBeInTheDocument();
});

test("TagPill renders label and tone data attr", () => {
  render(<TagPill label="planned" tone="planned" />);
  const el = screen.getByText("planned");
  expect(el).toHaveAttribute("data-tone", "planned");
});

test("ActionChecklist renders each item and its title", () => {
  render(<ActionChecklist title="Action plan" items={["Do X", "Do Y"]} />);
  expect(screen.getByText("Action plan")).toBeInTheDocument();
  expect(screen.getByText("Do X")).toBeInTheDocument();
  expect(screen.getByText("Do Y")).toBeInTheDocument();
});
