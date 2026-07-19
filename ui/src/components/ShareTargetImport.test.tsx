import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { IDBFactory } from "fake-indexeddb";
import ShareTargetImport from "./ShareTargetImport";
import { SHARE_DB_NAME, SHARE_STORE, SHARE_KEY, SHARE_MAX_AGE_MS } from "../pwa/shareTarget";

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

/** Park a CSV the way the service worker does on a real share.
 *  `receivedAt` must be recent — shareTarget.ts refuses records older than
 *  SHARE_MAX_AGE_MS so an abandoned share cannot linger as plaintext. */
async function stashSharedCsv(name = "statement.csv", receivedAt = Date.now()) {
  const db = await new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(SHARE_DB_NAME, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(SHARE_STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(SHARE_STORE, "readwrite");
    tx.objectStore(SHARE_STORE).put(
      {
        name,
        type: "text/csv",
        buffer: new TextEncoder().encode("Date,Merchant,Amount\n").buffer,
        receivedAt,
      },
      SHARE_KEY
    );
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
  db.close();
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
