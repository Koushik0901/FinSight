import { describe, expect, it } from "vitest";
import { copilotAgUiRuntimeFlag, isCopilotAgUiRuntimeEnabled } from "./featureFlag";

describe("copilot.agUiRuntime feature flag", () => {
  it("defaults off", () => {
    expect(isCopilotAgUiRuntimeEnabled("", null)).toBe(false);
  });

  it("can be enabled by query string for the isolated spike route", () => {
    expect(isCopilotAgUiRuntimeEnabled("?agui=1", null)).toBe(true);
  });

  it("can be enabled by local storage without changing the default", () => {
    const storage = {
      getItem: (key: string) => (key === copilotAgUiRuntimeFlag.storageKey ? "true" : null),
    };

    expect(isCopilotAgUiRuntimeEnabled("", storage)).toBe(true);
  });
});
