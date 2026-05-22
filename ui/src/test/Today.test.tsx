import { render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import Today from "../screens/Today";
import type { ReactNode } from "react";

function wrap(node: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <QueryClientProvider client={qc}>
      <BrowserRouter>{node}</BrowserRouter>
    </QueryClientProvider>
  );
}

describe("Today", () => {
  it("renders the runway number from the first account", async () => {
    vi.mocked(invoke).mockResolvedValue([
      {
        id: "a1",
        owner: "joint",
        bank: "Mercury",
        type: "Checking",
        name: "Joint Checking",
        balance_cents: 1482042,
        currency: "USD",
        color: "#C9F950",
      },
    ]);
    render(wrap(<Today />));
    await waitFor(() => {
      expect(screen.getByText(/\$14,820/)).toBeInTheDocument();
    });
  });
});
