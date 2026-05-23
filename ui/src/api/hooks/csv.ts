import { useQuery } from "@tanstack/react-query";
import { commands, type CsvPreview } from "../client";

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
