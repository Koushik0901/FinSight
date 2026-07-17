import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { createWrapper } from "../../test-utils";
import { useAutoCategorizeEnabled, useSetAutoCategorizeEnabled, useExportJson, useExportCsv } from "./settings";

vi.mock("../client", () => ({
  commands: {
    getAutoCategorizeEnabled: vi.fn().mockResolvedValue({ status: "ok", data: true }),
    setAutoCategorizeEnabled: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    exportAllDataJson: vi.fn().mockResolvedValue({ status: "ok", data: '{"accounts":[]}' }),
    exportAllDataCsv: vi.fn().mockResolvedValue({ status: "ok", data: "date,amount\n2026-06-28,-84.32\n" }),
  },
}));

vi.mock("../../lib/downloadBlob", () => ({
  downloadBlob: vi.fn(),
}));

import { downloadBlob } from "../../lib/downloadBlob";

describe("useAutoCategorizeEnabled", () => {
  it("returns the enabled value from the backend", async () => {
    const { result } = renderHook(() => useAutoCategorizeEnabled(), { wrapper: createWrapper() });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toBe(true);
  });
});

describe("useSetAutoCategorizeEnabled", () => {
  it("calls setAutoCategorizeEnabled", async () => {
    const { result } = renderHook(() => useSetAutoCategorizeEnabled(), { wrapper: createWrapper() });
    result.current.mutate(false);
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
  });
});

describe("useExportJson", () => {
  it("downloads the exported JSON content — works without the Tauri runtime", async () => {
    const { result } = renderHook(() => useExportJson(), { wrapper: createWrapper() });
    result.current.mutate();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(downloadBlob).toHaveBeenCalledWith('{"accounts":[]}', "application/json", "finsight-export.json");
  });
});

describe("useExportCsv", () => {
  it("downloads the exported CSV content — works without the Tauri runtime", async () => {
    const { result } = renderHook(() => useExportCsv(), { wrapper: createWrapper() });
    result.current.mutate();
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(downloadBlob).toHaveBeenCalledWith("date,amount\n2026-06-28,-84.32\n", "text/csv", "finsight-transactions.csv");
  });
});
