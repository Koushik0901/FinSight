import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type NotificationPrefsDto, type Notification } from "../client";
import { isBackendAvailable } from "../../utils/runtime";

const PREFS_KEY = ["notification-prefs"];
const LIST_KEY = ["notifications"];
const COUNT_KEY = ["notification-unread-count"];

/** The unified notification preferences (master, per-category, quiet hours, privacy). */
export function useNotificationPrefs() {
  return useQuery<NotificationPrefsDto>({
    queryKey: PREFS_KEY,
    queryFn: async () => {
      const result = await commands.getNotificationPrefs();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable(),
  });
}

export function useSetNotificationPrefs() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (prefs: NotificationPrefsDto) => {
      // Stamp the client's current UTC offset so the server evaluates the
      // quiet-hours window in the user's LOCAL time. `getTimezoneOffset()` is
      // minutes *behind* UTC (e.g. +480 for UTC−8), so negate it to express
      // `local = UTC + offset`. The server has no other way to know the user's
      // clock. Re-stamped on every save, so it tracks travel/DST at save time.
      const stamped: NotificationPrefsDto = { ...prefs, utcOffsetMinutes: -new Date().getTimezoneOffset() };
      const result = await commands.setNotificationPrefs(stamped);
      if (result.status === "error") throw new Error(result.error.message);
    },
    // Optimistic: reflect the toggle immediately, roll back on failure.
    onMutate: async (prefs) => {
      await qc.cancelQueries({ queryKey: PREFS_KEY });
      const prev = qc.getQueryData<NotificationPrefsDto>(PREFS_KEY);
      qc.setQueryData(PREFS_KEY, prefs);
      return { prev };
    },
    onError: (_e, _v, ctx) => {
      if (ctx?.prev) qc.setQueryData(PREFS_KEY, ctx.prev);
    },
    onSettled: () => qc.invalidateQueries({ queryKey: PREFS_KEY }),
  });
}

/** The notification history — active items by default; held (quiet-hours) items appear here too. */
export function useNotifications(includeResolved = false) {
  return useQuery<Notification[]>({
    queryKey: [...LIST_KEY, includeResolved],
    queryFn: async () => {
      const result = await commands.listNotifications(includeResolved);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable(),
  });
}

export function useMarkNotificationRead() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.markNotificationRead(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LIST_KEY });
      qc.invalidateQueries({ queryKey: COUNT_KEY });
    },
  });
}

export function useMarkAllNotificationsRead() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async () => {
      const result = await commands.markAllNotificationsRead();
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: LIST_KEY });
      qc.invalidateQueries({ queryKey: COUNT_KEY });
    },
  });
}

/** Unread, unresolved notification count — folded into the installed-app badge. */
export function useNotificationUnreadCount() {
  return useQuery<number>({
    queryKey: COUNT_KEY,
    queryFn: async () => {
      const result = await commands.notificationUnreadCount();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable(),
    refetchInterval: 60_000,
  });
}
