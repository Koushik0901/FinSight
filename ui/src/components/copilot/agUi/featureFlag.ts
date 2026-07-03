const AG_UI_RUNTIME_QUERY_PARAM = "agui";
const AG_UI_RUNTIME_STORAGE_KEY = "finsight.copilot.agUiRuntime";

/**
 * The AG-UI runtime is the default Copilot runtime (Phase 5B). The legacy
 * local runtime remains reachable as a rollback via an explicit opt-out:
 *   - `?agui=0` query param, or
 *   - localStorage `finsight.copilot.agUiRuntime = "false"`.
 * A query param, when present, always wins over storage.
 */
export function isCopilotAgUiRuntimeEnabled(
  search = globalThis.location?.search ?? "",
  storage: Pick<Storage, "getItem"> | null =
    typeof globalThis.localStorage === "undefined" ? null : globalThis.localStorage,
) {
  const params = new URLSearchParams(search);
  const param = params.get(AG_UI_RUNTIME_QUERY_PARAM);
  if (param === "0" || param === "false") {
    return false;
  }
  if (param === "1" || param === "true") {
    return true;
  }

  // No query override: default on unless storage explicitly opts out.
  return storage?.getItem(AG_UI_RUNTIME_STORAGE_KEY) !== "false";
}

export const copilotAgUiRuntimeFlag = {
  queryParam: AG_UI_RUNTIME_QUERY_PARAM,
  storageKey: AG_UI_RUNTIME_STORAGE_KEY,
};
