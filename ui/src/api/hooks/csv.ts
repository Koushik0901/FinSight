import { useQuery } from "@tanstack/react-query";
import { commands, type CsvPreview, type CsvImportMapping } from "../client";

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
