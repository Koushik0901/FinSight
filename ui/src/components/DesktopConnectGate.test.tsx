import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "../utils/runtime";
import DesktopConnectGate from "./DesktopConnectGate";

// isTauriRuntime() short-circuits to `true` under vitest (see
// utils/runtime.test.ts) — this suite is about DesktopConnectGate's own
// branching, not isTauriRuntime()'s bridge/origin logic, so the module is
// mocked (same pattern as httpBackend.test.ts) with a default of `true`
// (the "real Tauri shell" case) and overridden per-test to `false` for the
// non-Tauri (browser/PWA/post-navigate) scenario.
vi.mock("../utils/runtime", () => ({ isTauriRuntime: vi.fn(() => true) }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../screens/desktop/ConnectScreen", () => ({
  default: ({ onConnected }: { onConnected: (url: string) => void }) => (
    <div>
      <span>CONNECT_SCREEN</span>
      <button onClick={() => onConnected("https://picked.example.ts.net")}>fake-connect</button>
    </div>
  ),
}));

const realLocation = window.location;

function mockLocation() {
  const loc = { href: "" };
  Object.defineProperty(window, "location", { value: loc, configurable: true });
  return loc;
}

describe("DesktopConnectGate", () => {
  beforeEach(() => {
    // vi.clearAllMocks() (afterEach below) clears call history but NOT a
    // mock's configured implementation, so a mockReturnValue(false) from one
    // test would otherwise leak into the next — pin the default back to
    // `true` before every test.
    vi.mocked(isTauriRuntime).mockReturnValue(true);
  });

  afterEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(window, "location", { value: realLocation, configurable: true });
    delete (window as unknown as { __FINSIGHT_MOCK__?: boolean }).__FINSIGHT_MOCK__;
  });

  it("get_server_url resolves a URL: navigates the window there, ConnectScreen never renders", async () => {
    const loc = mockLocation();
    vi.mocked(invoke).mockResolvedValue("https://myhost.ts.net");

    render(
      <DesktopConnectGate>
        <div>APP_CONTENT</div>
      </DesktopConnectGate>
    );

    await waitFor(() => expect(loc.href).toBe("https://myhost.ts.net"));
    expect(screen.queryByText("CONNECT_SCREEN")).toBeNull();
    expect(invoke).toHaveBeenCalledWith("get_server_url");
  });

  it("get_server_url resolves null: renders ConnectScreen, never navigates", async () => {
    const loc = mockLocation();
    vi.mocked(invoke).mockResolvedValue(null);

    render(
      <DesktopConnectGate>
        <div>APP_CONTENT</div>
      </DesktopConnectGate>
    );

    expect(await screen.findByText("CONNECT_SCREEN")).toBeInTheDocument();
    expect(screen.queryByText("APP_CONTENT")).toBeNull();
    expect(loc.href).toBe("");
  });

  it("ConnectScreen's onConnected navigates the window to the newly-connected URL", async () => {
    const loc = mockLocation();
    vi.mocked(invoke).mockResolvedValue(null);

    render(
      <DesktopConnectGate>
        <div>APP_CONTENT</div>
      </DesktopConnectGate>
    );

    const button = await screen.findByText("fake-connect");
    button.click();

    expect(loc.href).toBe("https://picked.example.ts.net");
  });

  it("non-Tauri runtime: renders children immediately, never calls get_server_url", () => {
    vi.mocked(isTauriRuntime).mockReturnValue(false);
    mockLocation();

    render(
      <DesktopConnectGate>
        <div>APP_CONTENT</div>
      </DesktopConnectGate>
    );

    expect(screen.getByText("APP_CONTENT")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("__FINSIGHT_MOCK__ marker set: renders children immediately, never calls get_server_url", () => {
    (window as unknown as { __FINSIGHT_MOCK__?: boolean }).__FINSIGHT_MOCK__ = true;
    mockLocation();

    render(
      <DesktopConnectGate>
        <div>APP_CONTENT</div>
      </DesktopConnectGate>
    );

    expect(screen.getByText("APP_CONTENT")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("get_server_url resolves a non-string truthy value (e.g. []): renders ConnectScreen, never navigates", async () => {
    const loc = mockLocation();
    vi.mocked(invoke).mockResolvedValue([]);

    render(
      <DesktopConnectGate>
        <div>APP_CONTENT</div>
      </DesktopConnectGate>
    );

    expect(await screen.findByText("CONNECT_SCREEN")).toBeInTheDocument();
    expect(screen.queryByText("APP_CONTENT")).toBeNull();
    expect(loc.href).toBe("");
  });

  it("renders nothing while the initial check is in flight (avoids a ConnectScreen flash)", () => {
    mockLocation();
    vi.mocked(invoke).mockReturnValue(new Promise(() => {})); // never resolves

    const { container } = render(
      <DesktopConnectGate>
        <div>APP_CONTENT</div>
      </DesktopConnectGate>
    );

    expect(container).toBeEmptyDOMElement();
  });
});
