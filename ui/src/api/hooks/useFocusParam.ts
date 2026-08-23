import { useEffect } from "react";
import { useSearchParams } from "react-router-dom";

/**
 * Shared focus-param helper replacing 4× duplicated effects in
 * Budget/Recurring/Goals/Accounts.
 *
 * Reads `?focusX=raw` from the URL, calls `handler(raw)` and — unless the
 * handler explicitly returns `false` (meaning "not ready yet, keep the param
 * for the next render") — clears the param with `replace:true` so a stale link
 * does not re-fire.
 *
 * The handler owns the lookup (find in list) and the state transition
 * (e.g. `setEditingId`). This keeps the hook generic while still allowing
 * callers to wait for async data: return `false` while `isLoading`.
 */
export function useFocusParam(
  param: string,
  handler: (raw: string) => boolean | void,
): void {
  const [searchParams, setSearchParams] = useSearchParams();
  useEffect(() => {
    const raw = searchParams.get(param);
    if (!raw) return;
    const result = handler(raw);
    if (result === false) return;
    const next = new URLSearchParams(searchParams);
    next.delete(param);
    setSearchParams(next, { replace: true });
  }, [searchParams, param, setSearchParams, handler]);
}
