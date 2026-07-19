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
 * The parked file is ENCRYPTED AT REST. A shared bank statement is exactly the
 * data ui/src/pwa/cacheCrypto.ts exists to keep off disk in the clear, and it
 * can sit here unclaimed: the worker accepts a share with no idea whether
 * anyone is signed in, but AuthGate renders the login screen INSTEAD of the app
 * when they are not, so nothing claims it until someone signs in.
 *
 * Encryption is unconditional here, with no plaintext fallback, because a
 * service worker only ever registers in a SECURE CONTEXT — `crypto.subtle` is
 * therefore always available inside one. (The query cache needs a fallback
 * because plain IndexedDB works on http; this does not.)
 *
 * KEEP IN SYNC with ui/src/pwa/shareTarget.ts — the DB name, store name, record
 * keys, and envelope layout below are a contract between the two files. There
 * is a test that pins them (ui/src/pwa/shareTarget.test.ts).
 */

const FINSIGHT_SHARE_DB = "finsight-share-target";
const FINSIGHT_SHARE_STORE = "incoming";
const FINSIGHT_SHARE_KEY = "pending";

/**
 * Largest share we will park. MIRRORS `MAX_CSV_UPLOAD_BYTES` in
 * crates/finsight-server/src/uploads.rs, which is the authority — this is a
 * guard rail, not the real check.
 *
 * It exists because the OS hands us whatever the user picked. Without it, a
 * mis-shared video or a decade-long export would be read fully into memory,
 * encrypted (a second copy), and written to IndexedDB, only to be rejected by
 * the server afterwards. Rejecting up front turns that into an immediate,
 * explainable "too large" instead of a stall followed by a vague failure.
 */
const FINSIGHT_MAX_SHARE_BYTES = 25 * 1024 * 1024;

/**
 * Whether the OS-supplied filename looks like a CSV.
 *
 * Extension, not MIME, deliberately: MIME for `.csv` is wildly inconsistent
 * across platforms and file managers (text/csv, application/vnd.ms-excel,
 * application/octet-stream, text/plain all occur), and the server's own check
 * is on the extension too — so this agrees with the authority rather than
 * inventing a second rule.
 *
 * A share with NO filename is allowed through: we cannot prove it is wrong,
 * and refusing it would break pickers that omit the name. The import preview
 * is the backstop there.
 */
function finsightLooksLikeCsv(name) {
  if (!name) return true;
  return /\.csv$/i.test(name);
}
/** Non-extractable AES-GCM key, in the SAME store — no schema version bump. */
const FINSIGHT_SHARE_CRYPTO_KEY = "key";
const FINSIGHT_SHARE_ENVELOPE_VERSION = 1;

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

/**
 * Fetch this device's share-encryption key, creating it on first use.
 *
 * Non-extractable: script can ask the browser to decrypt with it but can never
 * read its bytes out, so a dump of the browser's storage files yields
 * ciphertext. Get-then-put in ONE readwrite transaction so a second worker
 * generation cannot clobber a key that already has ciphertext written under it.
 */
function finsightShareKey() {
  return finsightOpenShareDb().then(function (db) {
    return new Promise(function (resolve, reject) {
      const tx = db.transaction(FINSIGHT_SHARE_STORE, "readwrite");
      const store = tx.objectStore(FINSIGHT_SHARE_STORE);
      const getReq = store.get(FINSIGHT_SHARE_CRYPTO_KEY);
      let resolved = null;
      getReq.onsuccess = function () {
        if (getReq.result) {
          resolved = getReq.result;
          return;
        }
        // generateKey is async and would outlive this transaction, so settle
        // the transaction first and create the key in a second one below.
        resolved = null;
      };
      tx.oncomplete = function () {
        db.close();
        resolve(resolved);
      };
      tx.onerror = function () {
        db.close();
        reject(tx.error);
      };
    });
  }).then(function (existing) {
    if (existing) return existing;
    return crypto.subtle
      .generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"])
      .then(function (fresh) {
        return finsightOpenShareDb().then(function (db) {
          return new Promise(function (resolve, reject) {
            const tx = db.transaction(FINSIGHT_SHARE_STORE, "readwrite");
            const store = tx.objectStore(FINSIGHT_SHARE_STORE);
            const getReq = store.get(FINSIGHT_SHARE_CRYPTO_KEY);
            let winner = fresh;
            getReq.onsuccess = function () {
              // Another pass beat us to it — adopt theirs, or ours is the first.
              if (getReq.result) winner = getReq.result;
              else store.put(fresh, FINSIGHT_SHARE_CRYPTO_KEY);
            };
            tx.oncomplete = function () {
              db.close();
              resolve(winner);
            };
            tx.onerror = function () {
              db.close();
              reject(tx.error);
            };
          });
        });
      });
  });
}

/**
 * Encrypt the shared file into the envelope shareTarget.ts decrypts.
 *
 * The filename is encrypted along with the bytes — "RBC-chequing-2026.csv" is
 * itself a disclosure. Layout of the plaintext before encryption:
 *
 *   [4-byte big-endian header length][UTF-8 JSON {name,type}][raw file bytes]
 *
 * `receivedAt` deliberately stays OUTSIDE the envelope, in the clear, so the
 * app's TTL sweep can discard a stale share without needing the key at all.
 */
function finsightEncryptShare(file) {
  return Promise.all([finsightShareKey(), file.arrayBuffer()]).then(function (parts) {
    const key = parts[0];
    const body = new Uint8Array(parts[1]);
    const header = new TextEncoder().encode(
      JSON.stringify({
        name: file.name || "shared.csv",
        type: file.type || "text/csv",
      })
    );

    const plain = new Uint8Array(4 + header.length + body.length);
    new DataView(plain.buffer).setUint32(0, header.length, false);
    plain.set(header, 4);
    plain.set(body, 4 + header.length);

    const iv = crypto.getRandomValues(new Uint8Array(12));
    return crypto.subtle
      .encrypt({ name: "AES-GCM", iv: iv }, key, plain)
      .then(function (ct) {
        return {
          v: FINSIGHT_SHARE_ENVELOPE_VERSION,
          iv: iv,
          ct: ct,
          receivedAt: Date.now(),
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
        // A zero-byte file is as useless as no file, and the server rejects it
        // too — say so now rather than after an upload round trip.
        if (file.size === 0) {
          return Response.redirect("/?shared=empty", 303);
        }
        // Reject before reading the bytes: the whole point is not to pull a
        // huge file into memory.
        if (typeof file.size === "number" && file.size > FINSIGHT_MAX_SHARE_BYTES) {
          return Response.redirect("/?shared=toolarge", 303);
        }
        // Bank statements are very often shared as PDFs. Catch that here
        // instead of encrypting, parking, and uploading something the server
        // will certainly refuse.
        if (!finsightLooksLikeCsv(file.name)) {
          return Response.redirect("/?shared=unsupported", 303);
        }
        // Encrypted before it ever touches disk. Bytes rather than the Blob
        // itself: an ArrayBuffer round-trips through IndexedDB identically
        // everywhere, whereas Blob storage has historically been patchy.
        await finsightStashSharedFile(await finsightEncryptShare(file));
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
