import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type PushDeliveryReport, type PushStatus } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

/** VAPID public key + how many devices this user has registered. */
export function usePushStatus() {
  return useQuery<PushStatus>({
    queryKey: ["push-status"],
    queryFn: async () => {
      const result = await commands.getPushStatus();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    // The key is generated once and never rotates; only device_count moves, and
    // only in response to actions taken on this screen.
    staleTime: 5 * 60_000,
    enabled: isBackendAvailable(),
  });
}

export function useSavePushSubscription() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (sub: { endpoint: string; p256dh: string; auth: string; label?: string }) => {
      const result = await commands.savePushSubscription(
        sub.endpoint,
        sub.p256dh,
        sub.auth,
        sub.label ?? null
      );
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["push-status"] }),
  });
}

export function useDeletePushSubscription() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (endpoint: string) => {
      const result = await commands.deletePushSubscription(endpoint);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => void qc.invalidateQueries({ queryKey: ["push-status"] }),
  });
}

/** Send a notification to this user's devices so they can confirm it works. */
export function useSendTestPush() {
  return useMutation<PushDeliveryReport>({
    mutationFn: async () => {
      const result = await commands.sendTestPush();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}
