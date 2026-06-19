import { useQuery } from "@tanstack/react-query";
import { commands, type ActionItem } from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function useActionItems() {
  return useQuery<ActionItem[]>({
    queryKey: ["action-items"],
    queryFn: async () => {
      const result = await commands.getActionItems();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 30_000,
    refetchInterval: 30_000,
    enabled: isTauriRuntime(),
  });
}
