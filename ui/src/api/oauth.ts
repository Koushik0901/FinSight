/**
 * OAuth consent-screen client. Backs `/oauth/authorize`, the one interactive
 * step of the authorization-code flow a cloud connector (claude.ai, ChatGPT)
 * runs. Plain `fetch` against REST endpoints, same conventions as `auth.ts`.
 */
import type { TokenScope } from "./tokens";

export type OAuthConsentRequest = {
  clientId: string;
  redirectUri: string;
  scope: TokenScope;
  state: string | null;
  codeChallenge: string;
  codeChallengeMethod: string;
};

async function throwParsedError(res: Response): Promise<never> {
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    body = { error: "server_error", error_description: `HTTP ${res.status}` };
  }
  throw body;
}

/**
 * The display name of the app requesting access, so the consent card can say
 * who is asking. The server deliberately returns only the name.
 */
export async function fetchOAuthClientName(clientId: string): Promise<string> {
  const res = await fetch(`/api/oauth/client?client_id=${encodeURIComponent(clientId)}`);
  if (!res.ok) return throwParsedError(res);
  return ((await res.json()) as { clientName: string }).clientName;
}

/**
 * Grant access. Returns the URL to send the browser to — the server builds it
 * so the authorization code never has to be assembled client-side.
 */
export async function approveOAuth(req: OAuthConsentRequest): Promise<string> {
  const res = await fetch("/api/oauth/approve", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(req),
  });
  if (!res.ok) return throwParsedError(res);
  return ((await res.json()) as { redirectTo: string }).redirectTo;
}

/** Human-readable text from an OAuth-shaped error body. */
export function oauthErrorMessage(err: unknown, fallback: string): string {
  const body = err as { error_description?: unknown; error?: unknown } | null;
  if (typeof body?.error_description === "string") return body.error_description;
  if (typeof body?.error === "string") return body.error;
  return fallback;
}
