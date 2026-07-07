import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { createWrapper } from "../../../test-utils";
import { RecategorizationPreviewCard } from "./RecategorizationPreviewCard";

vi.mock("../../../api/hooks/copilot", () => ({
  useActionBundle: vi.fn(() => ({ data: { id: "bundle-abc", items: [] }, isLoading: false })),
  useApproveActionItem: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useRejectActionItem: vi.fn(() => ({ mutate: vi.fn(), isPending: false })),
  useExecuteActionBundle: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("RecategorizationPreviewCard", () => {
  it("renders each proposed merchant/category/confidence row and a more-count footer", () => {
    render(
      <RecategorizationPreviewCard
        block={{
          kind: "recategorizationPreview",
          count: 23,
          rows: [{ merchant: "Trader Joe's", categoryKey: "Groceries", confidence: 0.99 }],
          more: 18,
          bundleId: "bundle-abc",
        }}
      />,
      { wrapper: createWrapper() }
    );
    expect(screen.getByText("Trader Joe's")).toBeInTheDocument();
    expect(screen.getByText("Groceries")).toBeInTheDocument();
    expect(screen.getByText("99%")).toBeInTheDocument();
    expect(screen.getByText(/\+ 18 more/)).toBeInTheDocument();
  });
});
