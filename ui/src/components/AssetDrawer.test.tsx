import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import AssetDrawer from "./AssetDrawer";
import { createWrapper } from "../test-utils";

vi.mock("react-focus-lock", () => ({ default: ({ children }: any) => <>{children}</> }));
vi.mock("../api/hooks/settings", () => ({
  useDefaultCurrency: vi.fn(() => ({ data: "CAD" })),
}));
vi.mock("../api/hooks/assets", () => ({
  useCreateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn() })),
  useUpdateManualAsset: vi.fn(() => ({ mutateAsync: vi.fn() })),
  useDeleteManualAsset: vi.fn(() => ({ mutateAsync: vi.fn(), isPending: false })),
}));
vi.mock("../api/hooks/household", () => ({
  useHouseholdMembers: vi.fn(() => ({ data: [] })),
  useAssetOwners: vi.fn(() => ({ data: [] })),
  useSetAssetOwners: vi.fn(() => ({ mutateAsync: vi.fn() })),
}));

describe("AssetDrawer", () => {
  it("labels a new asset value with the configured household currency", () => {
    render(<AssetDrawer open onClose={() => {}} />, { wrapper: createWrapper() });

    expect(screen.getByRole("spinbutton", { name: "Value (CAD)" })).toBeInTheDocument();
    expect(screen.queryByRole("spinbutton", { name: "Value ($)" })).not.toBeInTheDocument();
  });
});
