import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import ConnectScreen from "./ConnectScreen";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function fillUrl(url: string) {
  fireEvent.change(screen.getByPlaceholderText(/finsight.example.ts.net/), { target: { value: url } });
}

describe("ConnectScreen", () => {
  afterEach(() => {
    vi.clearAllMocks();
    vi.unstubAllGlobals();
  });

  it("successful health check: calls set_server_url then onConnected with the normalized URL", async () => {
    vi.stubGlobal("fetch", vi.fn(async () =>
      new Response(JSON.stringify({ status: "ok" }), { status: 200 })));
    vi.mocked(invoke).mockResolvedValue(undefined);
    const onConnected = vi.fn();

    render(<ConnectScreen onConnected={onConnected} />);
    fillUrl("https://myhost.ts.net/");
    fireEvent.click(screen.getByRole("button", { name: /connect/i }));

    await waitFor(() => expect(onConnected).toHaveBeenCalledWith("https://myhost.ts.net"));
    expect(invoke).toHaveBeenCalledWith("set_server_url", { url: "https://myhost.ts.net" });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("non-ok health check: shows an error and does not call onConnected", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => new Response("", { status: 503 })));
    const onConnected = vi.fn();

    render(<ConnectScreen onConnected={onConnected} />);
    fillUrl("https://myhost.ts.net");
    fireEvent.click(screen.getByRole("button", { name: /connect/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/couldn't reach/i);
    expect(onConnected).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("network failure: shows an error and does not call onConnected", async () => {
    vi.stubGlobal("fetch", vi.fn(async () => { throw new TypeError("Failed to fetch"); }));
    const onConnected = vi.fn();

    render(<ConnectScreen onConnected={onConnected} />);
    fillUrl("https://myhost.ts.net");
    fireEvent.click(screen.getByRole("button", { name: /connect/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/couldn't reach/i);
    expect(onConnected).not.toHaveBeenCalled();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("empty URL: shows an error without hitting fetch", async () => {
    vi.stubGlobal("fetch", vi.fn());
    const onConnected = vi.fn();

    render(<ConnectScreen onConnected={onConnected} />);
    fireEvent.click(screen.getByRole("button", { name: /connect/i }));

    expect(await screen.findByRole("alert")).toHaveTextContent(/enter your server/i);
    expect(fetch).not.toHaveBeenCalled();
    expect(onConnected).not.toHaveBeenCalled();
  });
});
