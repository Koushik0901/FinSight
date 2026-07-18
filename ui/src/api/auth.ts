/**
 * Server-mode auth API client. These hit `/api/auth/*` directly with plain
 * `fetch` — they are NOT Tauri commands and must never go through
 * bindings.ts/client.ts. Only meaningful when `isServerMode()` is true (the
 * app is served by finsight-server, not running inside Tauri).
 *
 * Session cookies are HttpOnly; the default `fetch` credentials mode
 * ("same-origin") already sends them on same-origin requests, so callers
 * never read or set the cookie themselves.
 *
 * Errors are thrown as the plain AppError-shaped object `{code, message}`
 * (never as an `Error` instance) so callers can pattern-match on `.code`
 * (e.g. `auth.bad_credentials`, `auth.already_setup`) the same way the
 * generated bindings do.
 */
type AnyRec = Record<string, unknown>;

export type AuthStatus = {
  needsSetup: boolean;
  authenticated: boolean;
  username: string | null;
  isAdmin: boolean | null;
};

export type AdminUser = {
  id: string;
  username: string;
  isAdmin: boolean;
  createdAt: string;
};

async function throwParsedError(res: Response): Promise<never> {
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    body = { code: "rpc.transport", message: `HTTP ${res.status} with non-JSON body` };
  }
  throw body;
}

async function postJson(path: string, payload: unknown): Promise<unknown> {
  const res = await fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(payload),
  });
  if (!res.ok) return throwParsedError(res);
  return res.status === 204 ? null : res.json();
}

export async function fetchAuthStatus(): Promise<AuthStatus> {
  const res = await fetch("/api/auth/status");
  if (!res.ok) return throwParsedError(res);
  return (await res.json()) as AuthStatus;
}

export async function setup(username: string, password: string): Promise<{ recoveryKey: string }> {
  const out = (await postJson("/api/auth/setup", { username, password })) as { recoveryKey: string };
  markSessionEstablished(username);
  return out;
}

export async function login(username: string, password: string): Promise<void> {
  await postJson("/api/auth/login", { username, password });
  markSessionEstablished(username);
}

/**
 * Password recovery: exchange a username + recovery key for a new password.
 * On success the server issues a NEW recovery key (shown exactly once by
 * RecoverScreen) and establishes a session cookie — the caller is logged in,
 * same as `login()`.
 *
 * Error codes: `auth.bad_recovery_key` (401 — wrong key OR unknown user; the
 * message must stay generic so it never hints which), `auth.weak_password`
 * (400), `auth.too_many_attempts` (429).
 */
export async function recoverAccount(
  username: string,
  recoveryKey: string,
  newPassword: string
): Promise<{ recoveryKey: string }> {
  const out = (await postJson("/api/auth/recover", { username, recoveryKey, newPassword })) as {
    recoveryKey: string;
  };
  markSessionEstablished(username);
  return out;
}

export async function logout(): Promise<void> {
  try {
    const res = await fetch("/api/auth/logout", { method: "POST" });
    if (!res.ok) return throwParsedError(res);
  } finally {
    // Clear the offline-boot marker even if the server call failed — the user
    // asked to end the session, so we must not keep serving cached data.
    clearSessionMarker();
  }
}

// ------------------------------------------------- offline boot marker ---
// A NON-SENSITIVE flag recording only that *some* authenticated session has
// existed on this device. It is never a credential: no token, no key, no
// password. Its sole job is to let AuthGate distinguish "the server is
// unreachable but this user has synced data cached" (→ show the persisted
// cache read-only, per the PWA offline story) from "a stranger opened the app
// and the server happens to be down" (→ show the connection-problem wall).
//
// A real auth failure NEVER consults it — 401 / `auth.*` always routes to the
// login screen, so this cannot weaken authentication.

const SESSION_MARKER_KEY = "finsight.hadSession";
const LAST_USER_KEY = "finsight.lastAuthedUser";

/** Record that an authenticated session exists on this device. */
export function markSessionEstablished(username?: string | null): void {
  try {
    window.localStorage.setItem(SESSION_MARKER_KEY, "1");
    if (username) window.localStorage.setItem(LAST_USER_KEY, username);
  } catch {
    // Private browsing / storage disabled — offline boot simply won't engage.
  }
}

/** Forget the marker (sign-out, 401, or a server that says we're logged out). */
export function clearSessionMarker(): void {
  try {
    window.localStorage.removeItem(SESSION_MARKER_KEY);
    window.localStorage.removeItem(LAST_USER_KEY);
  } catch {
    /* ignore */
  }
}

/** True when this device has previously completed an authenticated session. */
export function hadPriorSession(): boolean {
  try {
    return window.localStorage.getItem(SESSION_MARKER_KEY) === "1";
  } catch {
    return false;
  }
}

/** Username of the last authenticated session, purely for display. */
export function lastAuthedUser(): string | null {
  try {
    return window.localStorage.getItem(LAST_USER_KEY);
  } catch {
    return null;
  }
}

/**
 * True when a rejection means "no answer from the server" rather than "the
 * server answered and said no".
 *
 * `throwParsedError` always produces a `{code: string}` from the response
 * body, falling back to `rpc.transport` when the body isn't JSON (a reverse
 * proxy's 502/503 HTML page). Anything WITHOUT a string `code` never reached
 * an HTTP response at all — that's `fetch` itself rejecting (`TypeError:
 * Failed to fetch`), i.e. genuinely offline.
 */
export function isNetworkFailure(err: unknown): boolean {
  const code = (err as { code?: unknown } | null)?.code;
  return typeof code !== "string" || code === "rpc.transport";
}

/** True when a rejection is an authentication verdict from the server. */
export function isAuthFailure(err: unknown): boolean {
  const code = (err as { code?: unknown } | null)?.code;
  return typeof code === "string" && code.startsWith("auth.");
}

/** True once the httpBackend shim has installed the production HTTP/SSE transport. */
export function isServerMode(): boolean {
  return Boolean((window as unknown as AnyRec).__FINSIGHT_HTTP__);
}

// ------------------------------------------------------- admin: users ---
// Admin-only user management. The backend 403s with `auth.admin_required`
// for non-admin callers; screens should gate visibility on
// `fetchAuthStatus().isAdmin` rather than relying on that 403 for UX.

export async function listUsers(): Promise<AdminUser[]> {
  const res = await fetch("/api/auth/users");
  if (!res.ok) return throwParsedError(res);
  return (await res.json()) as AdminUser[];
}

export async function createUser(username: string, password: string): Promise<{ recoveryKey: string }> {
  return (await postJson("/api/auth/users", { username, password })) as { recoveryKey: string };
}

export async function deleteUser(id: string): Promise<void> {
  const res = await fetch(`/api/auth/users/${id}`, { method: "DELETE" });
  if (!res.ok) return throwParsedError(res);
}
