import { createAsyncStoragePersister } from "@tanstack/query-async-storage-persister";
import { get, set, del } from "idb-keyval";

export const PERSIST_KEY = "finsight-rq-cache";

/** IndexedDB-backed persister for the tanstack-query cache. Financial data is
 *  device-local here — it is purged on logout/401 (purgePersistedCache) so a
 *  shared browser can't leak a prior user's cached balances. */
export function createIdbPersister() {
  return createAsyncStoragePersister({
    key: PERSIST_KEY,
    storage: {
      getItem: (k) => get(k),
      setItem: (k, v) => set(k, v),
      removeItem: (k) => del(k),
    },
    throttleTime: 1000,
  });
}

export async function purgePersistedCache(): Promise<void> {
  await del(PERSIST_KEY);
}
