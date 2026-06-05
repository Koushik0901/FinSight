import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AgentMemory } from "../client";

export function useAgentMemory() {
  return useQuery<AgentMemory[]>({
    queryKey: ["agent-memory"],
    queryFn: async () => {
      const result = await commands.listAgentMemory();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useForgetAgentMemory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.forgetAgentMemory(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}
