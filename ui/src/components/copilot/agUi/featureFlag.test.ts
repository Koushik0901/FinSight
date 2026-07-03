import { describe, expect, it } from "vitest";
import { copilotAgUiRuntimeFlag, isCopilotAgUiRuntimeEnabled } from "./featureFlag";

describe("copilot.agUiRuntime feature flag", () => {
  it("defaults on (Phase 5B: AG-UI is the default runtime)", () => {
    expect(isCopilotAgUiRuntimeEnabled("", null)).toBe(true);
  });

  it("can be disabled by query string (?agui=0 rollback)", () => {
    expect(isCopilotAgUiRuntimeEnabled("?agui=0", null)).toBe(false);
    expect(isCopilotAgUiRuntimeEnabled("?agui=false", null)).toBe(false);
  });

  it("query param wins over storage opt-out", () => {
    const optOut = {
      getItem: (key: string) => (key === copilotAgUiRuntimeFlag.storageKey ? "false" : null),
    };
    expect(isCopilotAgUiRuntimeEnabled("?agui=1", optOut)).toBe(true);
  });

  it("can be disabled by local storage opt-out", () => {
    const storage = {
      getItem: (key: string) => (key === copilotAgUiRuntimeFlag.storageKey ? "false" : null),
    };

    expect(isCopilotAgUiRuntimeEnabled("", storage)).toBe(false);
  });

  it("stays on for any non-opt-out storage value", () => {
    const storage = { getItem: () => null };
    expect(isCopilotAgUiRuntimeEnabled("", storage)).toBe(true);
  });
});
