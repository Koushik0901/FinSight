/**
 * MCP access-token management. Like `auth.ts`, these hit REST endpoints
 * directly with plain `fetch` — they are NOT Tauri commands and must never go
 * through bindings.ts/client.ts. Only meaningful in server mode.
 *
 * Errors are thrown as the plain AppError-shaped object `{code, message}` so
 * callers can match on `.code`, matching the convention in `auth.ts`.
 */

/** A token as the list view sees it — never includes the secret itself. */
export type ApiTokenSummary = {
  id: string;
  name: string;
  scope: TokenScope;
  createdAt: string;
  lastUsedAt: string | null;
};

/**
 * `read` exposes only the analysis tools. `full` additionally lets the assistant
 * draft changes and — after the user agrees in the conversation — apply them.
 */
export type TokenScope = "read" | "full";

/** The one and only time the token value is returned by the server. */
export type CreatedApiToken = ApiTokenSummary & { token: string };

async function throwParsedError(res: Response): Promise<never> {
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    body = { code: "rpc.transport", message: `HTTP ${res.status} with non-JSON body` };
  }
  throw body;
}

export async function listApiTokens(): Promise<ApiTokenSummary[]> {
  const res = await fetch("/api/auth/tokens");
  if (!res.ok) return throwParsedError(res);
  return (await res.json()) as ApiTokenSummary[];
}

export async function createApiToken(name: string, scope: TokenScope): Promise<CreatedApiToken> {
  const res = await fetch("/api/auth/tokens", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ name, scope }),
  });
  if (!res.ok) return throwParsedError(res);
  return (await res.json()) as CreatedApiToken;
}

export async function revokeApiToken(id: string): Promise<void> {
  const res = await fetch(`/api/auth/tokens/${id}`, { method: "DELETE" });
  if (!res.ok) return throwParsedError(res);
}

/** Where an MCP client should point. Same origin as the app itself. */
export function mcpEndpointUrl(): string {
  return `${window.location.origin}/mcp`;
}
