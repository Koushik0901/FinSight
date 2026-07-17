import { describe, it, expect, vi, afterEach } from "vitest";
import { downloadBlob } from "./downloadBlob";

afterEach(() => vi.restoreAllMocks());

describe("downloadBlob", () => {
  it("creates an object URL, triggers a synthetic download click, and revokes the URL", () => {
    const createUrl = vi.fn(() => "blob:mock-url");
    const revokeUrl = vi.fn();
    vi.stubGlobal("URL", { createObjectURL: createUrl, revokeObjectURL: revokeUrl });
    const clickSpy = vi.fn();
    const origCreateElement = document.createElement.bind(document);
    vi.spyOn(document, "createElement").mockImplementation((tag: string) => {
      const el = origCreateElement(tag);
      if (tag === "a") el.click = clickSpy;
      return el;
    });

    downloadBlob("date,amount\n2026-01-01,10.00\n", "text/csv", "export.csv");

    expect(createUrl).toHaveBeenCalledTimes(1);
    expect(clickSpy).toHaveBeenCalledTimes(1);
    expect(revokeUrl).toHaveBeenCalledWith("blob:mock-url");
  });
});
