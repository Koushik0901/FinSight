/**
 * OpenAPI-typed fetch client for the RPC surface.
 * Generated from `openapi.json` (via `pnpm openapi:gen`) — this wrapper keeps
 * the `Result<T,AppError>` envelope and the 401 → `FINSIGHT_AUTH_REQUIRED`
 * dispatch that the old `httpBackend.ts` shim provided, so existing hooks keep
 * working while the transport moves off `__TAURI_INTERNALS__`.
 *
 * Usage: `import { openapiClient } from "./openapiClient"` then
 * `openapiClient.POST("/api/rpc/list_accounts", {})`. The `api` object below
 * provides the ergonomic `api.listAccounts()` aliases that mirror `bindings.ts`
 * while the migration completes (Task 4 deletes the shim).
 */
import createClient from "openapi-fetch";
import type { paths } from "./openapi";
import { FINSIGHT_AUTH_REQUIRED } from "./eventNames";

const raw = createClient<paths>({ baseUrl: "" });

async function wrap<T>(p: Promise<{ data?: T; error?: unknown; response: Response }>): Promise<
  { status: "ok"; data: T } | { status: "error"; error: { code: string; message: string } }
> {
  const { data, error, response } = await p as unknown as {
    data?: T;
    error?: unknown;
    response: Response;
  };
  if (!response.ok) {
    const body = (error ?? data ?? {}) as { code?: string; message?: string };
    if (
      response.status === 401 &&
      typeof body === "object" &&
      body !== null &&
      (body as { code?: string }).code === "auth.required"
    ) {
      window.dispatchEvent(new CustomEvent(FINSIGHT_AUTH_REQUIRED));
    }
    return {
      status: "error",
      error: {
        code: (body as { code?: string }).code ?? "rpc.transport",
        message: (body as { message?: string }).message ?? `HTTP ${response.status}`,
      },
    };
  }
  return { status: "ok", data: data as T };
}

// Ergonomic aliases — one per RPC command, matching `bindings.ts` names.
// Initially a thin pass-through; later hooks can import these directly.
// Only a subset is needed for the Task 3 test; the rest are generated on demand.
export const api = {
  // example: list_accounts
  listAccounts: () => wrap(raw.POST("/api/rpc/list_accounts" as never, {} as never)),
  // generic fallback for any command (used by hooks that haven't migrated yet)
  rpc: <T>(cmd: string, body: unknown) =>
    wrap<T>(
      // `as never` because the generated `paths` type is strict and `cmd` is dynamic
      raw.POST(`/api/rpc/${cmd}` as never, { body: body as never } as never) as never,
    ),
};

export { raw as openapiClient };
