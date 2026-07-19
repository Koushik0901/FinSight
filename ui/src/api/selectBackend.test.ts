import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { selectBackend } from "./selectBackend";
import { isTauriRuntime } from "../utils/runtime";

// selectBackend()'s whole point is to gate on isTauriRuntime() (origin-aware),
// not raw __TAURI_INTERNALS__ presence. Mock isTauriRuntime so each case
// controls "is this a real desktop-IPC context" directly, exactly the way
// httpBackend.test.ts does — otherwise vitest's own MODE==="test" short-circuit
// would force it true everywhere and the matrix couldn't be exercised.
vi.mock("../utils/runtime", () => ({ isTauriRuntime: vi.fn(() => false) }));
const mockedIsTauri = vi.mocked(isTauriRuntime);

type AnyRec = Record<string, unknown>;
const w = window as unknown as AnyRec;

describe("selectBackend — transport gate (Phase 4)", () => {
  beforeEach(() => {
    delete w.__TAURI_INTERNALS__;
    mockedIsTauri.mockReturnValue(false);
  });
  afterEach(() => {
    delete w.__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  it("browser/PWA (no bridge, not Tauri runtime) → http", () => {
    expect(selectBackend(new URLSearchParams(""))).toBe("http");
  });

  it("REGRESSION: thin shell after navigating to a remote server " +
     "(bridge STILL present, but isTauriRuntime() false at the remote origin) → http", () => {
    // This is the exact case main.tsx's old raw `!__TAURI_INTERNALS__` gate got
    // wrong: the Tauri bridge persists at the remote origin, so a raw check
    // skipped installHttpBackend() and left the shell talking to a dead local
    // bridge. Gating on isTauriRuntime() (origin-aware) fixes it.
    w.__TAURI_INTERNALS__ = {};
    mockedIsTauri.mockReturnValue(false);
    expect(selectBackend(new URLSearchParams(""))).toBe("http");
  });

  it("thin shell pre-navigation (bridge present, on Tauri's own origin) → none " +
     "(keep the native bridge for the local config commands)", () => {
    w.__TAURI_INTERNALS__ = {};
    mockedIsTauri.mockReturnValue(true);
    expect(selectBackend(new URLSearchParams(""))).toBe("none");
  });

  it("DEV ?mock design harness (no bridge) → mock, taking precedence over http", () => {
    // import.meta.env.DEV is true under vitest, so the mock branch is reachable.
    expect(selectBackend(new URLSearchParams("mock=rich"))).toBe("mock");
  });

  it("?mock is ignored when a real bridge is present (never shadow a real runtime)", () => {
    w.__TAURI_INTERNALS__ = {};
    mockedIsTauri.mockReturnValue(true);
    // bridge present → mock branch's !__TAURI_INTERNALS__ guard fails → falls
    // through to the isTauriRuntime() check → none.
    expect(selectBackend(new URLSearchParams("mock=rich"))).toBe("none");
  });
});
