import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import AgentActivityFeed from "./AgentActivityFeed";

// Mock Tauri event listener
const listeners: Record<string, ((payload: unknown) => void)[]> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (payload: unknown) => void) => {
    if (!listeners[event]) listeners[event] = [];
    listeners[event].push(handler);
    return () => {}; // unlisten
  }),
}));

function emitTauriEvent(event: string, payload: unknown) {
  for (const h of listeners[event] ?? []) h({ payload });
}

describe("AgentActivityFeed", () => {
  beforeEach(() => {
    Object.keys(listeners).forEach((k) => delete listeners[k]);
  });

  it("is invisible when idle", () => {
    const { container } = render(<AgentActivityFeed />);
    // aria-live region exists but shows nothing
    const region = container.querySelector("[aria-live]");
    expect(region?.textContent?.trim()).toBe("");
  });

  it("shows progress when categorization.progress fires", async () => {
    render(<AgentActivityFeed />);
    await act(async () => {
      emitTauriEvent("categorization.progress", { done: 12, total: 47 });
    });
    expect(screen.getByText(/12\s*\/\s*47/)).toBeInTheDocument();
  });

  it("shows 'Done' after categorization.complete fires", async () => {
    render(<AgentActivityFeed />);
    await act(async () => {
      emitTauriEvent("categorization.complete", { categorized: 47, skipped: 0 });
    });
    expect(screen.getByText(/done/i)).toBeInTheDocument();
  });
});
