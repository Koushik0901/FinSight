import { describe, it, expect, vi, afterEach } from "vitest";
import { fetchServerAbout, isClientOutdated, CLIENT_PROTOCOL } from "./serverInfo";

afterEach(() => vi.unstubAllGlobals());

describe("serverInfo", () => {
  it("fetchServerAbout parses /api/server/about", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        new Response(JSON.stringify({ version: "0.0.0", protocol: 1, minClientProtocol: 1 }), { status: 200 })
      )
    );
    const about = await fetchServerAbout();
    expect(fetch).toHaveBeenCalledWith("/api/server/about", expect.anything());
    expect(about.protocol).toBe(1);
  });

  it("throws when the response is not ok", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => new Response("", { status: 500 }))
    );
    await expect(fetchServerAbout()).rejects.toThrow();
  });

  it("isClientOutdated is true only when the client is below the server's minimum", () => {
    expect(isClientOutdated({ version: "x", protocol: 2, minClientProtocol: CLIENT_PROTOCOL + 1 })).toBe(true);
    expect(isClientOutdated({ version: "x", protocol: 1, minClientProtocol: CLIENT_PROTOCOL })).toBe(false);
  });
});
