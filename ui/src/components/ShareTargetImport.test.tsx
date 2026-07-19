import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { IDBFactory } from "fake-indexeddb";
import ShareTargetImport from "./ShareTargetImport";
import {
  SHARE_DB_NAME,
  SHARE_STORE,
  SHARE_KEY,
  SHARE_CRYPTO_KEY,
  SHARE_MAX_AGE_MS,
  MAX_SHARE_MB,
} from "../pwa/shareTarget";

// The real mapping dialog pulls a CSV preview over RPC; this test is about the
// hand-off, so stub it down to "did it open, and with which upload token".
vi.mock("./ImportMappingDialog", () => ({
  default: ({ path }: { path: string }) => <div data-testid="mapping-dialog">{path}</div>,
}));

const uploadCsv = vi.fn();
vi.mock("../api/csvUpload", () => ({ uploadCsv: (f: File) => uploadCsv(f) }));

const serverMode = { value: true };
vi.mock("../api/auth", () => ({ isServerMode: () => serverMode.value }));

const toastError = vi.fn();
vi.mock("sonner", () => ({
  toast: {
    error: (...args: unknown[]) => toastError(...args),
    loading: vi.fn(() => "toast-id"),
    dismiss: vi.fn(),
  },
}));

const originalHref = window.location.href;

/**
 * Park a CSV the way the service worker does on a real share: encrypted under a
 * non-extractable key, in the worker's envelope layout. `receivedAt` must be
 * recent — shareTarget.ts refuses records past SHARE_MAX_AGE_MS.
 */
async function stashSharedCsv(name = "statement.csv", receivedAt = Date.now()) {
  const open = () =>
    new Promise<IDBDatabase>((resolve, reject) => {
      const req = indexedDB.open(SHARE_DB_NAME, 1);
      req.onupgradeneeded = () => req.result.createObjectStore(SHARE_STORE);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
  const put = async (value: unknown, key: string) => {
    const db = await open();
    await new Promise<void>((resolve, reject) => {
      const tx = db.transaction(SHARE_STORE, "readwrite");
      tx.objectStore(SHARE_STORE).put(value, key);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
    db.close();
  };

  const key = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, [
    "encrypt",
    "decrypt",
  ]);
  await put(key, SHARE_CRYPTO_KEY);

  const header = new TextEncoder().encode(JSON.stringify({ name, type: "text/csv" }));
  const body = new TextEncoder().encode("Date,Merchant,Amount\n");
  const plain = new Uint8Array(4 + header.length + body.length);
  new DataView(plain.buffer).setUint32(0, header.length, false);
  plain.set(header, 4);
  plain.set(body, 4 + header.length);

  const iv = crypto.getRandomValues(new Uint8Array(12));
  const ct = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, key, plain);
  await put({ v: 1, iv, ct, receivedAt }, SHARE_KEY);
}

beforeEach(() => {
  globalThis.indexedDB = new IDBFactory();
  uploadCsv.mockReset().mockResolvedValue("abc-123.csv");
  toastError.mockReset();
  serverMode.value = true;
});

afterEach(() => window.history.replaceState(null, "", originalHref));

describe("ShareTargetImport", () => {
  it("renders nothing on a normal app launch", async () => {
    window.history.replaceState(null, "", "/");
    render(<ShareTargetImport />);
    await waitFor(() => expect(uploadCsv).not.toHaveBeenCalled());
    expect(screen.queryByTestId("mapping-dialog")).toBeNull();
  });

  it("uploads the shared CSV and opens the import dialog with the returned token", async () => {
    await stashSharedCsv();
    window.history.replaceState(null, "", "/?shared=1");

    render(<ShareTargetImport />);

    await waitFor(() => expect(screen.getByTestId("mapping-dialog")).toHaveTextContent("abc-123.csv"));
    expect(uploadCsv).toHaveBeenCalledTimes(1);
    expect((uploadCsv.mock.calls[0]![0] as File).name).toBe("statement.csv");
  });

  it("clears the ?shared flag so a refresh doesn't re-run the import", async () => {
    await stashSharedCsv();
    window.history.replaceState(null, "", "/?shared=1");

    render(<ShareTargetImport />);

    await waitFor(() => expect(window.location.search).toBe(""));

    // The flag is cleared SYNCHRONOUSLY, before the upload chain runs. Ending
    // the test here would leave `takeSharedFile()` → `uploadCsv()` in flight;
    // it then lands in the NEXT test, after beforeEach reset the mock, and that
    // test fails with a phantom uploadCsv call. Drain it before finishing.
    await waitFor(() => expect(screen.getByTestId("mapping-dialog")).toBeInTheDocument());
  });

  it("explains a share that carried no file, and opens no dialog", async () => {
    window.history.replaceState(null, "", "/?shared=empty");

    render(<ShareTargetImport />);

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]![0]).toBe("Nothing to import");
    expect(uploadCsv).not.toHaveBeenCalled();
    expect(screen.queryByTestId("mapping-dialog")).toBeNull();
  });

  // Both of these are rejected by the worker before anything is parked, so the
  // app's only job is to explain what happened in terms the user can act on.
  it("explains an over-sized file with the actual limit", async () => {
    window.history.replaceState(null, "", "/?shared=toolarge");

    render(<ShareTargetImport />);

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]![0]).toBe("That file is too large");
    expect(String((toastError.mock.calls[0]![1] as { description: string }).description)).toContain(
      String(MAX_SHARE_MB)
    );
    expect(uploadCsv).not.toHaveBeenCalled();
  });

  // Banks very commonly hand out PDF statements, so this is a first-run path,
  // not an exotic one.
  it("points a non-CSV share (e.g. a PDF statement) at the CSV export", async () => {
    window.history.replaceState(null, "", "/?shared=unsupported");

    render(<ShareTargetImport />);

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]![0]).toBe("FinSight imports CSV statements");
    expect(uploadCsv).not.toHaveBeenCalled();
    expect(screen.queryByTestId("mapping-dialog")).toBeNull();
  });

  it("reports a service-worker failure rather than silently doing nothing", async () => {
    window.history.replaceState(null, "", "/?shared=error");

    render(<ShareTargetImport />);

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]![0]).toBe("Couldn't receive that file");
  });

  it("reports an upload failure instead of leaving a dead dialog", async () => {
    await stashSharedCsv();
    window.history.replaceState(null, "", "/?shared=1");
    uploadCsv.mockRejectedValue({ code: "import.upload_failed", message: "Too large" });

    render(<ShareTargetImport />);

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]![0]).toBe("Couldn't upload that file");
    expect(screen.queryByTestId("mapping-dialog")).toBeNull();
  });

  // A share parked while signed out is never claimed (AuthGate keeps the app
  // tree unmounted), so it must expire rather than import much later.
  it("refuses a share that has been sitting around past its TTL", async () => {
    await stashSharedCsv("statement.csv", Date.now() - SHARE_MAX_AGE_MS - 1);
    window.history.replaceState(null, "", "/?shared=1");

    render(<ShareTargetImport />);

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]![0]).toBe("That shared file is no longer available");
    expect(uploadCsv).not.toHaveBeenCalled();
  });

  it("tells the user when the parked file has already been consumed", async () => {
    // Flag present, nothing in IndexedDB — e.g. the launch was reloaded twice.
    window.history.replaceState(null, "", "/?shared=1");

    render(<ShareTargetImport />);

    await waitFor(() => expect(toastError).toHaveBeenCalled());
    expect(toastError.mock.calls[0]![0]).toBe("That shared file is no longer available");
  });

  it("stays out of the way in the Tauri shell, where uploads have no session", async () => {
    serverMode.value = false;
    await stashSharedCsv();
    window.history.replaceState(null, "", "/?shared=1");

    render(<ShareTargetImport />);

    await waitFor(() => expect(window.location.search).toBe(""));
    expect(uploadCsv).not.toHaveBeenCalled();
  });
});
