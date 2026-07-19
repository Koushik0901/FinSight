import { get, del, update } from "idb-keyval";

/**
 * At-rest encryption for the PWA's offline query cache.
 *
 * ## Threat model — read this before claiming more than it does
 *
 * PROTECTS AGAINST: someone with access to the browser's on-disk storage
 * reading cached financial data — a stolen/lost laptop or phone, a shared
 * computer, a copied browser profile, a filesystem backup. Today that data sits
 * in IndexedDB's LevelDB files as plainly-readable JSON: balances, transaction
 * history, merchant names. After this, those files hold AES-GCM ciphertext.
 *
 * DOES NOT PROTECT AGAINST: code running in FinSight's own origin. The key is
 * non-extractable, so script cannot read its bytes — but script can still ask
 * the browser to decrypt with it. XSS or a malicious extension with host access
 * defeats this, as it defeats any browser-side at-rest scheme.
 *
 * Also honest about the limit of non-extractability: it is enforced at the
 * WebCrypto API layer. The browser still persists the key material somewhere on
 * disk, protected by the browser's own storage protections rather than by
 * cryptography we control. A determined forensic attacker with full filesystem
 * access may recover it. This raises the bar substantially over plaintext; it is
 * not equivalent to the server's SQLCipher-at-rest story, where the key is
 * wrapped by the user's password and never stored.
 *
 * ## Why the key is not derived from the user's password
 *
 * That would be strictly stronger, and it was rejected deliberately: the whole
 * point of the offline cache is that the app opens and shows data when the
 * server is unreachable. Password-derived keys require a login the user cannot
 * perform while offline, so binding the cache to one would mean the cache is
 * readable exactly when it is useless. The key is therefore device-scoped,
 * generated once, and destroyed on logout.
 */

/** IndexedDB slot holding the non-extractable AES-GCM CryptoKey handle. */
export const CACHE_KEY_HANDLE = "finsight-cache-key";

/** Bump when the envelope shape changes; older envelopes are then discarded. */
const ENVELOPE_VERSION = 1;

/** AES-GCM standard nonce length. Never reuse one with the same key. */
const IV_BYTES = 12;

type Envelope = {
  v: number;
  iv: Uint8Array;
  ct: ArrayBuffer;
};

/**
 * Duck-typed rather than `instanceof`, deliberately. Binary values can arrive
 * from another JS realm — structured clone across contexts, or Node's WebCrypto
 * under the test runner — where `x instanceof ArrayBuffer` is false for a
 * genuine ArrayBuffer. `Object.prototype.toString` and `ArrayBuffer.isView` are
 * realm-agnostic and draw exactly the same line: a real envelope versus a
 * legacy plaintext string or junk.
 */
function isBinary(value: unknown): boolean {
  return Object.prototype.toString.call(value) === "[object ArrayBuffer]";
}

function isEnvelope(value: unknown): value is Envelope {
  if (typeof value !== "object" || value === null) return false;
  const e = value as Partial<Envelope>;
  return typeof e.v === "number" && ArrayBuffer.isView(e.iv) && isBinary(e.ct);
}

/**
 * True when the browser can actually encrypt.
 *
 * `crypto.subtle` exists only in a SECURE CONTEXT (https, or localhost). That
 * is a live case for this app: FinSight is self-hosted, and someone may serve
 * it over plain http on a LAN IP. Callers must treat `false` as "do not persist
 * anything" rather than "persist in the clear" — see persist.ts.
 *
 * Be clear that this is a REAL trade, not a free win. IndexedDB persistence
 * needs no service worker, so an http-only LAN deployment does have a working
 * query cache today, and refusing to write it costs those users warm-start
 * paint (instant cached render, then refetch). Full offline launch was already
 * unavailable there — service workers are secure-context-only too — but the
 * warm start was not. We accept that cost rather than keep writing plaintext
 * financial data to disk; self-hosters who want both should terminate TLS
 * (docs/self-hosting.md covers Tailscale and Caddy).
 */
export function cacheEncryptionAvailable(): boolean {
  return (
    typeof crypto !== "undefined" &&
    typeof crypto.subtle !== "undefined" &&
    typeof crypto.subtle.encrypt === "function"
  );
}

// Single-flight per tab so concurrent persister writes don't each generate a key.
let keyPromise: Promise<CryptoKey | null> | null = null;

async function generateKey(): Promise<CryptoKey> {
  return crypto.subtle.generateKey(
    { name: "AES-GCM", length: 256 },
    // extractable: false — the raw bytes can never be read back out by script,
    // so a compromised page cannot exfiltrate the key itself, only use it.
    false,
    ["encrypt", "decrypt"]
  );
}

/**
 * Fetch the device's cache key, creating it on first use.
 *
 * Uses idb-keyval's `update`, which runs get-then-put inside ONE IndexedDB
 * readwrite transaction. That matters: two tabs booting together would
 * otherwise both generate a key and the second `set` would clobber the first,
 * making the first tab's already-written ciphertext permanently unreadable.
 * The atomic update makes the loser adopt the winner's key instead.
 */
async function getOrCreateKey(): Promise<CryptoKey | null> {
  if (!cacheEncryptionAvailable()) return null;
  if (keyPromise) return keyPromise;

  const attempt = (async () => {
    try {
      const existing = await get(CACHE_KEY_HANDLE);
      if (existing) return existing as CryptoKey;

      const fresh = await generateKey();
      await update(CACHE_KEY_HANDLE, (current) => current ?? fresh);
      // Re-read rather than returning `fresh`: if another tab won the race, the
      // stored key is theirs and ours was discarded by the updater above.
      const stored = await get(CACHE_KEY_HANDLE);
      return (stored as CryptoKey | undefined) ?? null;
    } catch {
      // Private browsing, blocked IndexedDB, quota. Caller degrades to "no
      // persistence" — never to plaintext.
      return null;
    }
  })();

  keyPromise = attempt;
  const key = await attempt;

  // Do NOT memoise a failure. IndexedDB failures here are usually transient —
  // a DB blocked by another tab's version change, a momentary quota refusal —
  // and caching the null would silently disable cache persistence for the rest
  // of the session, long after the cause cleared. Releasing the slot costs one
  // retry per write attempt and restores persistence as soon as it can work.
  if (key === null) keyPromise = null;
  return key;
}

/**
 * Encrypt a persister payload. Returns `null` when encryption is impossible,
 * which the caller MUST treat as "skip the write" — never as "write the plain
 * string". A fresh random IV per call keeps AES-GCM safe across the many
 * rewrites the query cache performs.
 */
export async function encryptCachePayload(plaintext: string): Promise<Envelope | null> {
  const key = await getOrCreateKey();
  if (!key) return null;
  try {
    const iv = crypto.getRandomValues(new Uint8Array(IV_BYTES));
    const ct = await crypto.subtle.encrypt(
      { name: "AES-GCM", iv },
      key,
      new TextEncoder().encode(plaintext)
    );
    return { v: ENVELOPE_VERSION, iv, ct };
  } catch {
    return null;
  }
}

/**
 * Decrypt a stored value back to the persister's string.
 *
 * Returns `null` — a cache miss, from which the app just refetches — for every
 * failure path, and they are all expected at least once:
 *
 * - a plaintext string left by a pre-encryption build (see `purgeLegacyPlaintext`)
 * - an envelope from a future/older `v`
 * - GCM authentication failure, i.e. the key was rotated or the bytes were
 *   tampered with. Never fall back to trusting the bytes here.
 */
export async function decryptCachePayload(stored: unknown): Promise<string | null> {
  if (!isEnvelope(stored) || stored.v !== ENVELOPE_VERSION) return null;
  const key = await getOrCreateKey();
  if (!key) return null;
  try {
    // Copy into this realm's views before handing them to WebCrypto — same
    // cross-realm caveat as `isEnvelope`, and the copies are a few dozen bytes
    // plus one cache-sized buffer, paid once per app boot.
    const plain = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: new Uint8Array(stored.iv) },
      key,
      new Uint8Array(stored.ct)
    );
    return new TextDecoder().decode(plain);
  } catch {
    return null;
  }
}

/** True for a value written by a pre-encryption build of the app. */
export function isLegacyPlaintext(stored: unknown): boolean {
  return typeof stored === "string";
}

/**
 * Destroy the device cache key.
 *
 * This is crypto-shredding, and it is the reason logout purges the key as well
 * as the cache: even if a ciphertext blob survives somewhere the delete didn't
 * reach (a browser-internal copy, an unflushed page of the LevelDB log), losing
 * the key makes it permanently undecryptable.
 */
export async function purgeCacheKey(): Promise<void> {
  keyPromise = null;
  try {
    await del(CACHE_KEY_HANDLE);
  } catch {
    // Nothing actionable — the cache purge alongside this is the primary path.
  }
}

/** Test seam: drop the memoised key so each test starts from a clean slot. */
export function __resetKeyCacheForTests(): void {
  keyPromise = null;
}
