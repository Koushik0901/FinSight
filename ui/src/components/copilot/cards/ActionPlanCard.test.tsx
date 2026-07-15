import { render, screen } from "@testing-library/react";
import { ActionPlanCard } from "./ActionPlanCard";

test("renders standalone action plan items", () => {
  render(<ActionPlanCard block={{ kind: "actionPlan", title: "Action plan", items: ["Sweep $168 into House Fund"] }} />);
  expect(screen.getByText("Action plan")).toBeInTheDocument();
  expect(screen.getByText("Sweep $168 into House Fund")).toBeInTheDocument();
});
