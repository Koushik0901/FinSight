const AG_UI_RUNTIME_QUERY_PARAM = "agui";
const AG_UI_RUNTIME_STORAGE_KEY = "finsight.copilot.agUiRuntime";

export function isCopilotAgUiRuntimeEnabled(
  search = globalThis.location?.search ?? "",
  storage: Pick<Storage, "getItem"> | null =
    typeof globalThis.localStorage === "undefined" ? null : globalThis.localStorage,
) {
  const params = new URLSearchParams(search);
  if (params.get(AG_UI_RUNTIME_QUERY_PARAM) === "1") {
    return true;
  }

  return storage?.getItem(AG_UI_RUNTIME_STORAGE_KEY) === "true";
}

export const copilotAgUiRuntimeFlag = {
  queryParam: AG_UI_RUNTIME_QUERY_PARAM,
  storageKey: AG_UI_RUNTIME_STORAGE_KEY,
};
