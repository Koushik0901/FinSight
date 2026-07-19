/*
 * Share-target receiver, imported into the generated Workbox service worker via
 * `workbox.importScripts` (see ui/vite.config.ts).
 *
 * WHY THIS RUNS IN THE SERVICE WORKER, not on the server
 * -----------------------------------------------------
 * A share-sheet hand-off is a cross-site-initiated top-level POST navigation.
 * FinSight's session cookie is `SameSite=Lax` (crates/finsight-server/src/auth.rs),
 * and Lax cookies are withheld from cross-site POSTs — so if this POST reached
 * the server it would arrive UNAUTHENTICATED and could not stage the file into
 * the user's directory. Relaxing the cookie to SameSite=None to allow it would
 * trade real CSRF protection for a minor simplification.
 *
 * So the service worker answers the POST itself, parks the file in IndexedDB,
 * and redirects to the app. The app then uploads it from a first-party context
 * where the session cookie flows normally, and runs the ordinary import.
 *
 * Plain JS on purpose: `importScripts` loads this file as-is, outside the Vite
 * bundle. It is deliberately kept small and dependency-free — the interesting
 * logic (and its tests) lives in ui/src/pwa/shareTarget.ts.
 *
 * KEEP IN SYNC with ui/src/pwa/shareTarget.ts — the DB name, store name, key,
 * and record shape below are a contract between the two files. There is a test
 * that pins them (ui/src/pwa/shareTarget.test.ts).
 */

const FINSIGHT_SHARE_DB = "finsight-share-target";
const FINSIGHT_SHARE_STORE = "incoming";
const FINSIGHT_SHARE_KEY = "pending";

function finsightOpenShareDb() {
  return new Promise(function (resolve, reject) {
    const req = indexedDB.open(FINSIGHT_SHARE_DB, 1);
    req.onupgradeneeded = function () {
      if (!req.result.objectStoreNames.contains(FINSIGHT_SHARE_STORE)) {
        req.result.createObjectStore(FINSIGHT_SHARE_STORE);
      }
    };
    req.onsuccess = function () {
      resolve(req.result);
    };
    req.onerror = function () {
      reject(req.error);
    };
  });
}

function finsightStashSharedFile(record) {
  return finsightOpenShareDb().then(function (db) {
    return new Promise(function (resolve, reject) {
      const tx = db.transaction(FINSIGHT_SHARE_STORE, "readwrite");
      tx.objectStore(FINSIGHT_SHARE_STORE).put(record, FINSIGHT_SHARE_KEY);
      tx.oncomplete = function () {
        db.close();
        resolve();
      };
      tx.onerror = function () {
        db.close();
        reject(tx.error);
      };
    });
  });
}

self.addEventListener("fetch", function (event) {
  const request = event.request;
  if (request.method !== "POST") return;

  let url;
  try {
    url = new URL(request.url);
  } catch (_e) {
    return;
  }
  if (url.pathname !== "/share-target") return;

  // Claiming the event here also keeps the POST away from the network. Without
  // it the request would fall through to the server, which has no such route.
  event.respondWith(
    (async function () {
      try {
        const form = await request.formData();
        const file = form.get("file");
        if (!file || typeof file.arrayBuffer !== "function") {
          return Response.redirect("/?shared=empty", 303);
        }
        // Store bytes rather than the Blob itself: an ArrayBuffer round-trips
        // through IndexedDB identically everywhere, whereas Blob storage has
        // historically been patchy across engines.
        await finsightStashSharedFile({
          name: file.name || "shared.csv",
          type: file.type || "text/csv",
          buffer: await file.arrayBuffer(),
          receivedAt: Date.now(),
        });
        // 303 forces the follow-up request to be a GET, so a reload of the
        // landing page never re-submits the share.
        return Response.redirect("/?shared=1", 303);
      } catch (_e) {
        // Never surface a raw error page into the share sheet's launched window
        // — send the user into the app with a flag it can turn into a toast.
        return Response.redirect("/?shared=error", 303);
      }
    })()
  );
});
