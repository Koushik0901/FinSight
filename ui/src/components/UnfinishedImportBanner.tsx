import { useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type Import } from "../api/client";

export default function UnfinishedImportBanner() {
  const qc = useQueryClient();
  const { data: unfinished = [] } = useQuery<Import[]>({
    queryKey: ["unfinished-imports"],
    queryFn: async () => {
      const result = await commands.listUnfinishedImports();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });

  if (unfinished.length === 0) return null;
  // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
  const top = unfinished[0]!;

  async function discard() {
    const result = await commands.discardUnfinishedImport(top.id);
    if (result.status === "error") {
      console.error("Failed to discard import:", result.error.message);
    }
    qc.invalidateQueries({ queryKey: ["unfinished-imports"] });
  }

  return (
    <div role="alert" className="banner banner-warning">
      An import didn't finish last time ({top.filename ?? "manual"}). It was deduped on the next
      run, so re-importing is safe.{" "}
      <button onClick={discard}>Discard</button>
    </div>
  );
}
