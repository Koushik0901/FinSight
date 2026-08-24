import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type Import } from "../api/openapiClient";
import { unwrap } from "../api/openapiClient";
import { isBackendAvailable } from "../utils/runtime";

export default function UnfinishedImportBanner() {
  const qc = useQueryClient();
  const { data: unfinished = [] } = useQuery<Import[]>({
    queryKey: ["unfinished-imports"],
    queryFn: async () => {
      return unwrap(api.listUnfinishedImports());
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });

  if (unfinished.length === 0) return null;
  const top = unfinished[0]!;

  async function discard() {
    const result = await api.discardUnfinishedImport(top.id);
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
