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
  return (await postJson("/api/auth/setup", { username, password })) as { recoveryKey: string };
}

export async function login(username: string, password: string): Promise<void> {
  await postJson("/api/auth/login", { username, password });
}

export async function logout(): Promise<void> {
  const res = await fetch("/api/auth/logout", { method: "POST" });
  if (!res.ok) return throwParsedError(res);
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
