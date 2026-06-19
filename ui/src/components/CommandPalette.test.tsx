import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { CommandPalette } from "./CommandPalette";
import type { ReactNode } from "react";

const askMutate = vi.fn();
vi.mock("../api/hooks/agent", () => ({
  useAskAgent: vi.fn(() => ({
    mutate: askMutate,
    isPending: false,
  })),
}));

function wrap(node: ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter>{node}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("CommandPalette — Ask the agent mode", () => {
  it("shows 'Ask the agent' section when query is typed", async () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "am I over budget?" } });
    await waitFor(() => {
      expect(screen.getByText("Ask the agent")).toBeInTheDocument();
      expect(screen.getByText(/Ask: am I over budget\?/)).toBeInTheDocument();
    });
  });

  it("switches to answer mode when ask item is clicked", async () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "what is my net worth?" } });
    await waitFor(() => screen.getByText(/Ask: what is my net worth\?/));
    fireEvent.click(screen.getByText(/Ask: what is my net worth\?/));
    await waitFor(() => {
      expect(screen.getByText(/← Back/)).toBeInTheDocument();
    });
  });

  it("returns to list mode when Back is clicked", async () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "top spending?" } });
    await waitFor(() => screen.getByText(/Ask: top spending\?/));
    fireEvent.click(screen.getByText(/Ask: top spending\?/));
    await waitFor(() => screen.getByText(/← Back/));
    fireEvent.click(screen.getByText(/← Back/));
    await waitFor(() => {
      expect(screen.queryByText(/← Back/)).not.toBeInTheDocument();
    });
  });

  it("shows 'Run a what-if scenario' action", () => {
    render(wrap(<CommandPalette open={true} onClose={() => {}} />));
    expect(screen.getByText("Run a what-if scenario")).toBeInTheDocument();
  });

  it("Escape in answer mode returns to list without closing", async () => {
    const onClose = vi.fn();
    render(wrap(<CommandPalette open={true} onClose={onClose} />));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "savings rate?" } });
    await waitFor(() => screen.getByText(/Ask: savings rate\?/));
    fireEvent.click(screen.getByText(/Ask: savings rate\?/));
    await waitFor(() => screen.getByText(/← Back/));
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByText(/← Back/)).not.toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();
    });
  });
});
