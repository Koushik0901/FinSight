import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import Onboarding from "../screens/Onboarding";
import { useOnboardingStore } from "../state/onboarding";

vi.mock("../api/openapiClient", () => ({
  unwrap: async (promise: Promise<{ status: string; data?: unknown }>) => (await promise).data,
  api: {
    getOnboardingState: vi.fn().mockResolvedValue({
      status: "ok",
      data: { account_count: 0, category_count: 0, completion_marked: false },
    }),
    listAccounts: vi.fn().mockResolvedValue({ status: "ok", data: [] }),
  },
}));

vi.mock("../components/AccountDrawer", () => ({
  default: ({ open }: { open: boolean }) => open ? <div role="dialog" aria-label="Account editor" /> : null,
}));

vi.mock("../screens/onboarding/SimpleFinDialog", () => ({
  default: ({ open }: { open: boolean }) => open ? <div role="dialog" aria-label="SimpleFIN setup" /> : null,
}));

function renderOnboarding() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={["/onboarding"]}>
        <Routes>
          <Route path="/onboarding" element={<Onboarding />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

describe("Onboarding first-run entry", () => {
  beforeEach(() => {
    useOnboardingStore.getState().reset();
  });

  it("opens on the first useful action instead of a welcome ceremony", async () => {
    renderOnboarding();

    expect(await screen.findByRole("heading", { name: /start with your accounts/i })).toBeInTheDocument();
    expect(document.querySelector(".onb-context-label")).toHaveTextContent("First step · Accounts");
    expect(screen.getByRole("main")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /explore today|continue to today/i })).toBeInTheDocument();
    expect(screen.queryByText(/quiet way to understand your money/i)).not.toBeInTheDocument();
  });
});
