import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { commands } from "../client";

export function useConversations() {
  return useQuery({
    queryKey: ["conversations"],
    queryFn: async () => {
      const res = await commands.listConversations();
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
  });
}

export function useConversationMessages(conversationId: string | null) {
  return useQuery({
    queryKey: ["conversation-messages", conversationId],
    queryFn: async () => {
      if (!conversationId) return [];
      const res = await commands.getConversationMessages(conversationId);
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
    enabled: !!conversationId,
  });
}

export function useCreateConversation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const res = await commands.createConversation();
      if (res.status === "error") throw new Error(res.error.message);
      return res.data;
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["conversations"] });
    },
  });
}

export function useDeleteConversation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const res = await commands.deleteConversation(id);
      if (res.status === "error") throw new Error(res.error.message);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["conversations"] });
    },
  });
}
