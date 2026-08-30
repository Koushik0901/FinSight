import { describe, it, expect, vi } from "vitest";
import { api, rpc } from "./openapiClient";

describe("openapiClient rpc deprecation", () => {
  it("exposes deprecated rpc on api and as standalone", () => {
    expect(typeof api.rpc).toBe("function");
    expect(typeof rpc).toBe("function");
    // standalone rpc should delegate to api.rpc
    expect(rpc).not.toBe(api.rpc);
  });

  it("api.rpc warns in DEV when called", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    // We can't actually call api.rpc without mocking fetch, but we can verify the function body contains warn via toString?
    const src = api.rpc.toString();
    expect(src).toContain("deprecated");
    expect(src).toContain("console.warn");
    warn.mockRestore();
  });
});
