/**
 * Server↔client version handshake (server mode only). Mirrors
 * `crates/finsight-server/src/server_info.rs` — `GET /api/server/about` is an
 * open (unauthenticated) REST endpoint, not a Tauri command, so it's a plain
 * `fetch` here like the rest of `api/auth.ts`, never routed through
 * bindings.ts/client.ts.
 */

/** Bumped in lockstep with the server's PROTOCOL_VERSION when the client ships a
 *  breaking change. The server's minClientProtocol vs this decides the banner. */
export const CLIENT_PROTOCOL = 1;

export type ServerAbout = { version: string; protocol: number; minClientProtocol: number };

export async function fetchServerAbout(): Promise<ServerAbout> {
  const res = await fetch("/api/server/about", { credentials: "same-origin" });
  if (!res.ok) throw new Error(`about ${res.status}`);
  return res.json();
}

/** True only when this build is older than the server's oldest-supported
 *  client protocol — i.e. a stale cached PWA that genuinely needs a refresh. */
export function isClientOutdated(about: ServerAbout): boolean {
  return CLIENT_PROTOCOL < about.minClientProtocol;
}
