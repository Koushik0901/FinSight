import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type CreateMonthlyReviewInput,
  type MonthTotals,
  type MonthlyReview,
  type SavingsRatePoint,
} from "../client";
import { isTauriRuntime } from "../../utils/runtime";

export function useMonthTotals() {
  return useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      const result = await commands.getMonthTotals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: isTauriRuntime(),
  });
}

export function useSavingsRateHistory() {
  return useQuery<SavingsRatePoint[]>({
    queryKey: ["savings-rate-history"],
    queryFn: async () => {
      const result = await commands.getSavingsRateHistory();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    enabled: isTauriRuntime(),
  });
}

export function useMonthlyReviews() {
  return useQuery<MonthlyReview[]>({
    queryKey: ["monthly-reviews"],
    queryFn: async () => {
      const result = await commands.listMonthlyReviews();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useCreateMonthlyReview() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: CreateMonthlyReviewInput) => {
      const result = await commands.createMonthlyReview(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["monthly-reviews"] });
    },
  });
}
