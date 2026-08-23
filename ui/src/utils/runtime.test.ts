import { describe, it, expect, afterEach, beforeEach, vi } from "vitest";
import { isTauriRuntime, isBackendAvailable } from "./runtime";

const realLocation = window.location;

beforeEach(() => {
  vi.stubEnv("MODE", "production");
  vi.stubEnv("VITEST", "");
  vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)" });
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
  Object.defineProperty(window, "location", { value: realLocation, configurable: true });
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
  delete (window as unknown as { __FINSIGHT_HTTP__?: unknown }).__FINSIGHT_HTTP__;
  delete (window as unknown as { __FINSIGHT_MOCK__?: unknown }).__FINSIGHT_MOCK__;
});

function setLocation(origin: string) {
  Object.defineProperty(window, "location", { value: { origin }, configurable: true });
}

describe("isTauriRuntime — pure PWA (shell deleted)", () => {
  it("always false — the Tauri thin shell was deleted, PWA is the desktop app", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("tauri://localhost");
    expect(isTauriRuntime()).toBe(false);
  });
  it("false even with bridge + internal origin — shell no longer exists", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("http://tauri.localhost");
    expect(isTauriRuntime()).toBe(false);
  });
  it("false on remote server origin", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("https://myhost.ts.net");
    expect(isTauriRuntime()).toBe(false);
  });
  it("false when bridge absent", () => {
    setLocation("tauri://localhost");
    expect(isTauriRuntime()).toBe(false);
  });
});

describe("isBackendAvailable — RPC transport availability (pure PWA)", () => {
  it("true when HTTP shim is installed", () => {
    setLocation("https://myhost.ts.net");
    (window as unknown as { __FINSIGHT_HTTP__?: unknown }).__FINSIGHT_HTTP__ = true;
    expect(isBackendAvailable()).toBe(true);
  });
  it("false when neither HTTP shim nor mock is present", () => {
    setLocation("https://myhost.ts.net");
    expect(isBackendAvailable()).toBe(false);
  });
  it("true when mock harness is installed (design harness / tests)", () => {
    setLocation("http://127.0.0.1:5173");
    (window as unknown as { __FINSIGHT_MOCK__?: unknown }).__FINSIGHT_MOCK__ = true;
    expect(isBackendAvailable()).toBe(true);
  });
  it("true in vitest/jsdom even without shim — hooks remain enabled in tests", () => {
    // No stub — real vitest env has MODE=test / VITEST / jsdom
    vi.unstubAllEnvs();
    vi.unstubAllGlobals();
    expect(isBackendAvailable()).toBe(true);
  });
});
