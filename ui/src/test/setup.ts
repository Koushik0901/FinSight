import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Provide a default mock for tauri invoke so Vitest doesn't error on import.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (_cmd: string, _args?: unknown) => {
    throw new Error("invoke not mocked — set per-test with vi.mocked(invoke).mockResolvedValue(...)");
  }),
}));
