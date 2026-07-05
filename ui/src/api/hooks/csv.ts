import { useQuery } from "@tanstack/react-query";
import { commands, type CsvPreview, type CsvImportMapping, type PreparedImportPreview } from "../client";

export function usePreviewCsvColumns(path: string | null, skipHeaderRows: number) {
  return useQuery<CsvPreview>({
    queryKey: ["csv-preview", path, skipHeaderRows],
    queryFn: async () => {
      const result = await commands.previewCsvColumns(path!, skipHeaderRows);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: !!path,
    staleTime: 30_000,
  });
}

/** The CSV mapping last used for this account, so a repeat import pre-fills. */
export function useSavedCsvMapping(accountId: string | null) {
  return useQuery<CsvImportMapping | null>({
    queryKey: ["csv-saved-mapping", accountId],
    queryFn: async () => {
      const result = await commands.getSavedCsvMapping(accountId!);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: !!accountId,
    staleTime: 30_000,
  });
}

/** Speculative import outcome for (account, mapping) — computed read-only on the
 *  backend so the dialog can show "N new · D duplicates · R to review" before the
 *  user commits. Keyed on the mapping so edits supersede in-flight prepares. */
export function usePrepareImport(
  path: string | null,
  accountId: string | null,
  mapping: CsvImportMapping | null,
) {
  return useQuery<PreparedImportPreview>({
    queryKey: ["csv-prepare", path, accountId, mapping],
    queryFn: async () => {
      const r = await commands.prepareCsvImport(path!, accountId!, mapping!);
      if (r.status === "error") throw new Error(r.error.message);
      return r.data;
    },
    enabled: !!path && !!accountId && !!mapping,
    staleTime: 10_000,
  });
}
