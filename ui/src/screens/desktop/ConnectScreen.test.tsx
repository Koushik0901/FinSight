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

  // Regression guard: the field used to be a raw <input> carrying only a
  // placeholder — no label, no id, no aria-label — and the error was a
  // detached <p> that screen readers never associated with it.
  it("exposes the server-URL field with a real label", () => {
    render(<ConnectScreen onConnected={vi.fn()} />);

    const field = screen.getByLabelText(/server address/i);
    expect(field).toBeInTheDocument();
    expect(field.id).toBeTruthy();
    expect(field).toHaveAttribute("aria-invalid", "false");
  });

  it("wires the error to the field via aria-describedby / aria-invalid", async () => {
    vi.stubGlobal("fetch", vi.fn());
    render(<ConnectScreen onConnected={vi.fn()} />);

    fireEvent.click(screen.getByRole("button", { name: /connect/i }));

    const alert = await screen.findByRole("alert");
    const field = screen.getByLabelText(/server address/i);
    expect(field).toHaveAttribute("aria-invalid", "true");
    expect(field.getAttribute("aria-describedby")).toBe(alert.id);
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
