/**
 * App-side half of the OS share-target flow.
 *
 * The service worker (ui/public/share-target-sw.js) answers the share sheet's
 * POST, parks the file in IndexedDB, and redirects here with a `?shared=` flag.
 * This module picks the file back up in the page, where the session cookie
 * actually applies, so the normal authenticated CSV import can run.
 *
 * The constants below are a CONTRACT with that service-worker file, which is
 * plain JS outside the bundle and so cannot import them. `shareTarget.test.ts`
 * pins both sides.
 */

export const SHARE_DB_NAME = "finsight-share-target";
export const SHARE_STORE = "incoming";
export const SHARE_KEY = "pending";

/**
 * How long a parked share may sit unclaimed before it is thrown away.
 *
 * This matters more than it looks. The service worker answers the share POST
 * with no idea whether anyone is signed in — SW fetch handlers run before any
 * app code. If the user shares while logged out, `AuthGate` renders the login
 * screen INSTEAD of the app, so `ShareTargetImport` never mounts and never
 * claims the file. Abandon the login screen and a full bank statement would
 * otherwise sit in cleartext IndexedDB indefinitely — the very thing the
 * encrypted query cache exists to prevent.
 *
 * A hand-off that works takes seconds, so fifteen minutes is generous.
 */
export const SHARE_MAX_AGE_MS = 15 * 60 * 1000;

/** What the service worker writes; `buffer` rather than a Blob — see the SW. */
export type SharedFileRecord = {
  name: string;
  type: string;
  buffer: ArrayBuffer;
  receivedAt: number;
};

/** Outcome encoded in the redirect URL by the service worker. */
export type ShareOutcome = "file" | "empty" | "error" | "none";

/**
 * Read `?shared=` from the current URL.
 *
 * `1` means a file is waiting in IndexedDB; `empty` means the share carried no
 * file (someone shared a URL or plain text); `error` means the worker failed to
 * park it. Anything else is a normal app launch.
 */
export function readShareFlag(search: string = window.location.search): ShareOutcome {
  const value = new URLSearchParams(search).get("shared");
  if (value === "1") return "file";
  if (value === "empty") return "empty";
  if (value === "error") return "error";
  return "none";
}

/**
 * Strip the `?shared=` flag from the address bar without a navigation.
 *
 * Necessary, not cosmetic: leaving it there means a refresh re-enters the
 * share-handling path and shows the import UI again for a file that was already
 * consumed. Other query params are preserved.
 */
export function clearShareFlag(): void {
  try {
    const url = new URL(window.location.href);
    if (!url.searchParams.has("shared")) return;
    url.searchParams.delete("shared");
    window.history.replaceState(
      window.history.state,
      "",
      url.pathname + (url.searchParams.toString() ? `?${url.searchParams}` : "") + url.hash
    );
  } catch {
    // Non-fatal: a stale flag costs a duplicate prompt, not correctness.
  }
}

function openShareDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(SHARE_DB_NAME, 1);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(SHARE_STORE)) {
        req.result.createObjectStore(SHARE_STORE);
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/**
 * Claim the shared file: read it, DELETE it, and return it as a `File`.
 *
 * Read-and-delete in ONE readwrite transaction so an interrupted launch cannot
 * leave a file that gets imported twice. The record is a one-shot hand-off, and
 * a duplicate import of a bank statement is a genuinely bad outcome — the
 * importer dedupes, but a second silent import is still not something to risk
 * on transaction timing.
 *
 * Returns null when nothing is waiting or IndexedDB is unavailable.
 */
export async function takeSharedFile(): Promise<File | null> {
  const record = await claimRecord();
  if (!record?.buffer) return null;

  // Defence in depth alongside the boot-time sweep: never hand a caller a file
  // that has been sitting around, even if the sweep did not get to run.
  if (isExpired(record)) return null;

  return new File([record.buffer], record.name || "shared.csv", {
    type: record.type || "text/csv",
  });
}

function isExpired(record: SharedFileRecord): boolean {
  // A missing/garbled timestamp is treated as expired: we cannot prove the file
  // is fresh, and refusing it costs one re-share while keeping it costs a
  // plaintext statement on disk.
  if (typeof record.receivedAt !== "number" || !Number.isFinite(record.receivedAt)) return true;
  return Date.now() - record.receivedAt > SHARE_MAX_AGE_MS;
}

/**
 * Read-and-delete the parked record in ONE readwrite transaction.
 *
 * Atomic on purpose: an interrupted launch must not leave a file that gets
 * imported twice. The importer dedupes, but a second silent import of a bank
 * statement is not something to risk on transaction timing.
 */
async function claimRecord(): Promise<SharedFileRecord | undefined> {
  let db: IDBDatabase | null = null;
  try {
    db = await openShareDb();
    const handle = db;
    return await new Promise<SharedFileRecord | undefined>((resolve, reject) => {
      const tx = handle.transaction(SHARE_STORE, "readwrite");
      const store = tx.objectStore(SHARE_STORE);
      const getReq = store.get(SHARE_KEY);
      getReq.onsuccess = () => {
        if (getReq.result) store.delete(SHARE_KEY);
      };
      tx.oncomplete = () => resolve(getReq.result as SharedFileRecord | undefined);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } catch {
    return undefined;
  } finally {
    db?.close();
  }
}

/**
 * Throw away any parked share unconditionally.
 *
 * Called when a session ends AND when a new one begins. The second is not
 * redundant: on a shared device, one person can park a statement, abandon the
 * login screen, and someone else can then sign in — without this, that second
 * person's app would silently import the first person's bank statement. This
 * mirrors why `purgePersistedCache` already runs on both transitions.
 */
export async function purgeSharedFiles(): Promise<void> {
  await claimRecord();
}

/**
 * Drop a parked share that has outlived `SHARE_MAX_AGE_MS`, leaving a fresh one
 * alone. Run at app boot REGARDLESS of auth state, so a share abandoned at the
 * login screen is cleaned up on the next launch rather than living forever.
 */
export async function sweepStaleSharedFiles(): Promise<void> {
  let db: IDBDatabase | null = null;
  try {
    db = await openShareDb();
    const handle = db;
    await new Promise<void>((resolve, reject) => {
      const tx = handle.transaction(SHARE_STORE, "readwrite");
      const store = tx.objectStore(SHARE_STORE);
      const getReq = store.get(SHARE_KEY);
      getReq.onsuccess = () => {
        const record = getReq.result as SharedFileRecord | undefined;
        if (record && isExpired(record)) store.delete(SHARE_KEY);
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error);
    });
  } catch {
    // Nothing actionable; the TTL check in takeSharedFile still applies.
  } finally {
    db?.close();
  }
}
