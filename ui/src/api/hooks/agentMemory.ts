import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type AgentMemory } from "../openapiClient";
import { unwrap } from "../openapiClient";

export function useAgentMemory() {
  return useQuery<AgentMemory[]>({
    queryKey: ["agent-memory"],
    queryFn: async () => {
      return unwrap(api.listAgentMemory());
    },
  });
}

export function useForgetAgentMemory() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      await unwrap(api.forgetAgentMemory(id));
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["agent-memory"] });
    },
  });
}
