import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import VersionBanner from "./VersionBanner";

type AnyRec = Record<string, unknown>;

function mockAbout(body: Record<string, unknown>, status = 200) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => new Response(JSON.stringify(body), { status }))
  );
}

describe("VersionBanner", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    delete (window as unknown as AnyRec).__FINSIGHT_HTTP__;
  });

  it("desktop mode (no __FINSIGHT_HTTP__): renders nothing and never fetches", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);

    const { container } = render(<VersionBanner />);

    await waitFor(() => expect(container).toBeEmptyDOMElement());
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("server mode + client up to date: renders nothing", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    mockAbout({ version: "0.1.0", protocol: 1, minClientProtocol: 1 });

    const { container } = render(<VersionBanner />);

    await waitFor(() => expect(fetch).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });

  it("server mode + client outdated: renders the banner with a working Reload button", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    mockAbout({ version: "0.2.0", protocol: 2, minClientProtocol: 2 });

    render(<VersionBanner />);

    expect(await screen.findByRole("status")).toBeInTheDocument();
    const reloadButton = screen.getByRole("button", { name: /reload/i });

    const reloadSpy = vi.fn();
    const originalLocation = window.location;
    Object.defineProperty(window, "location", {
      configurable: true,
      value: { ...originalLocation, reload: reloadSpy },
    });

    reloadButton.click();
    expect(reloadSpy).toHaveBeenCalledTimes(1);

    Object.defineProperty(window, "location", { configurable: true, value: originalLocation });
  });

  it("server mode + fetch failure: renders nothing (fails quiet, no crash)", async () => {
    (window as unknown as AnyRec).__FINSIGHT_HTTP__ = true;
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new TypeError("Failed to fetch");
      })
    );

    const { container } = render(<VersionBanner />);

    await waitFor(() => expect(fetch).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
  });
});
