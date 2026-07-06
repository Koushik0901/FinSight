import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { splitReasoningIntoSteps, ThinkingBlock } from "./Copilot";

vi.mock("@assistant-ui/react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@assistant-ui/react")>();
  return {
    ...actual,
    useMessage: () => ({ status: { type: "running" } }),
  };
});

describe("ThinkingBlock plan section", () => {
  it("renders a Plan section with numbered steps when a plan is present", () => {
    render(
      <ThinkingBlock
        reasoningText=""
        toolCalls={null}
        plan={["Find the income that just landed", "Rank every debt by interest rate"]}
      />
    );
    expect(screen.getByText("Plan")).toBeInTheDocument();
    expect(screen.getByText("Find the income that just landed")).toBeInTheDocument();
    expect(screen.getByText("Rank every debt by interest rate")).toBeInTheDocument();
  });

  it("omits the Plan section when no plan is present", () => {
    render(<ThinkingBlock reasoningText="" toolCalls={null} />);
    expect(screen.queryByText("Plan")).not.toBeInTheDocument();
  });

  it("omits the Plan section when the plan is an empty array", () => {
    render(<ThinkingBlock reasoningText="" toolCalls={null} plan={[]} />);
    expect(screen.queryByText("Plan")).not.toBeInTheDocument();
  });
});

describe("splitReasoningIntoSteps", () => {
  it("splits on sentence-ending punctuation followed by a space and a capital letter", () => {
    const text = "Housing is fixed. Dining is the lever. It's 13% over average.";
    expect(splitReasoningIntoSteps(text)).toEqual([
      "Housing is fixed.",
      "Dining is the lever.",
      "It's 13% over average.",
    ]);
  });

  it("returns a single-element array for text with no sentence breaks", () => {
    expect(splitReasoningIntoSteps("Just one clause")).toEqual(["Just one clause"]);
  });

  it("returns an empty array for empty/whitespace-only input", () => {
    expect(splitReasoningIntoSteps("")).toEqual([]);
    expect(splitReasoningIntoSteps("   ")).toEqual([]);
  });
});
