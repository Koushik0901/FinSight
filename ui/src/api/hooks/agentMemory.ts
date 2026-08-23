import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type AgentMemory } from "../client";
import { unwrap } from "../client";

export function useAgentMemory() {
  return useQuery<AgentMemory[]>({
    queryKey: ["agent-memory"],
    queryFn: async () => {
      return unwrap(commands.listAgentMemory());
    },
  });
}

export function useForgetAgentMemory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      await unwrap(commands.forgetAgentMemory(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}
