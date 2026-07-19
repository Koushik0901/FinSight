import { createAsyncStoragePersister } from "@tanstack/query-async-storage-persister";
import { get, set, del } from "idb-keyval";
import {
  cacheEncryptionAvailable,
  decryptCachePayload,
  encryptCachePayload,
  isLegacyPlaintext,
  purgeCacheKey,
} from "./cacheCrypto";

export const PERSIST_KEY = "finsight-rq-cache";

/** IndexedDB-backed persister for the tanstack-query cache.
 *
 *  Financial data is device-local here, so it gets two protections:
 *
 *  1. It is ENCRYPTED AT REST (AES-GCM, non-extractable device key — see
 *     cacheCrypto.ts for the threat model and its honest limits). Before this,
 *     balances and transaction history sat in IndexedDB as readable JSON for
 *     anyone with access to the device's browser storage.
 *  2. It is purged on logout/401 (purgePersistedCache) so a shared browser
 *     can't leak a prior user's cached balances.
 */
export function createIdbPersister() {
  return createAsyncStoragePersister({
    key: PERSIST_KEY,
    storage: {
      getItem: async (k) => {
        const stored = await get(k);

        // A cache written by a pre-encryption build. Don't read it and don't
        // migrate it — it is exactly the plaintext this feature exists to
        // remove, so delete it and take the refetch. The cache is a cache.
        if (isLegacyPlaintext(stored)) {
          await del(k);
          return null;
        }

        // null on any failure (wrong key, tampered bytes, unsupported context)
        // reads as a cache miss, and the app refetches from the server.
        return decryptCachePayload(stored);
      },

      setItem: async (k, v) => {
        const envelope = await encryptCachePayload(v);
        if (!envelope) {
          // Encryption unavailable (insecure origin — no crypto.subtle — or
          // blocked IndexedDB). Persisting `v` here would silently write the
          // plaintext this module promises not to write, so skip the write
          // entirely and drop any existing blob rather than leaving a stale one
          // that can never be refreshed.
          await del(k);
          return;
        }
        await set(k, envelope);
      },

      removeItem: (k) => del(k),
    },
    throttleTime: 1000,
  });
}

/** True when this browser/origin can encrypt the offline cache. False means
 *  nothing is persisted at all — never that data is persisted in the clear. */
export function offlineCacheEncrypted(): boolean {
  return cacheEncryptionAvailable();
}

/**
 * Drop the persisted cache. Removes the device key too, so any ciphertext that
 * outlives the delete (a browser-internal copy, an unflushed LevelDB page)
 * becomes permanently undecryptable rather than merely unlinked.
 */
export async function purgePersistedCache(): Promise<void> {
  await del(PERSIST_KEY);
  await purgeCacheKey();
}
