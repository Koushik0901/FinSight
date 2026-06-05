import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import Accounts from "./Accounts";
import { createWrapper } from "../test-utils";

vi.mock("../api/hooks/accounts", () => ({
  useAccounts: vi.fn(() => ({ data: [], isLoading: false, error: null })),
  useCreateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useArchiveAccount: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

vi.mock("../api/hooks/assets", () => ({
  useManualAssets: vi.fn(() => ({ data: [
    { id: "a1", name: "House", assetType: "property", valueCents: 50000000, currency: "USD", notes: null, createdAt: "2026-06-01T00:00:00Z", updatedAt: "2026-06-01T00:00:00Z" },
  ], isLoading: false })),
  useCreateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useLiabilities: vi.fn(() => ({ data: [] })),
  useCreateLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useUpdateLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
  useDeleteLiability: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));

describe("Accounts — manual assets", () => {
  it("renders the manual assets section with an asset row", () => {
    render(<Accounts />, { wrapper: createWrapper() });
    expect(screen.getByText("Manual assets")).toBeInTheDocument();
    expect(screen.getByText("House")).toBeInTheDocument();
    expect(screen.getByText("$500000.00")).toBeInTheDocument();
  });
});
