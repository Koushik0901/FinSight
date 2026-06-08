import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type BudgetEnvelope, type CategoryHistory, type GoalDto, type NewGoalInput, type PlanAssignment } from "../client";

// ── Budget ────────────────────────────────────────────────────────────────

export function useBudgetEnvelopes() {
  return useQuery<BudgetEnvelope[]>({
    queryKey: ["budget-envelopes"],
    queryFn: async () => {
      const result = await commands.listBudgetEnvelopes();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useSetBudget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ categoryId, amountCents }: { categoryId: string; amountCents: number }) => {
      const result = await commands.setBudget(categoryId, amountCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
    },
  });
}

export function useBudgetHistory(months: number) {
  return useQuery<CategoryHistory[]>({
    queryKey: ["budget-history", months],
    queryFn: async () => {
      const result = await commands.listBudgetHistory(months);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });
}

// ── Goals ─────────────────────────────────────────────────────────────────

export function useGoals() {
  return useQuery<GoalDto[]>({
    queryKey: ["goals"],
    queryFn: async () => {
      const result = await commands.listGoals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
  });
}

export function useCreateGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewGoalInput) => {
      const result = await commands.createGoal(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["goals"] });
    },
  });
}

export function useUpdateGoalBalance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, currentCents }: { id: string; currentCents: number }) => {
      const result = await commands.updateGoalBalance(id, currentCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["goals"] });
    },
  });
}

export function useArchiveGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      const result = await commands.archiveGoal(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["goals"] });
    },
  });
}

export function useUpdateGoalMonthly() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, monthlyCents }: { id: string; monthlyCents: number }) => {
      const result = await commands.updateGoalMonthly(id, monthlyCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["goals"] });
    },
  });
}

// ── Plan Next Month ───────────────────────────────────────────────────────

export function usePlanNextMonthData() {
  return useQuery({
    queryKey: ["plan-next-month"],
    queryFn: async () => {
      const result = await commands.getPlanNextMonthData();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });
}

export function useApplyNextMonthPlan() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (assignments: PlanAssignment[]) => {
      const result = await commands.applyNextMonthPlan(assignments);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["plan-next-month"] });
    },
  });
}
