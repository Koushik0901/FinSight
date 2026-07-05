import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type BudgetEnvelope, type CategoryHistory, type GoalDto, type NewGoalInput, type PlanAssignment, type ProjectedValue } from "../client";
import { isTauriRuntime } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

// ── Budget ────────────────────────────────────────────────────────────────

export function useBudgetEnvelopes() {
  return useQuery<BudgetEnvelope[]>({
    queryKey: ["budget-envelopes"],
    queryFn: async () => {
      const result = await commands.listBudgetEnvelopes();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime(),
  });
}

export function useSetBudget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ categoryId, amountCents }: { categoryId: string; amountCents: number }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.setBudget(categoryId, amountCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "budgetEnvelopes");
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
    enabled: isTauriRuntime(),
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
    enabled: isTauriRuntime(),
  });
}

export function useCreateGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewGoalInput) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.createGoal(input);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
    },
  });
}

export function useUpdateGoalBalance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, currentCents }: { id: string; currentCents: number }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateGoalBalance(id, currentCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
    },
  });
}

export function useArchiveGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.archiveGoal(id);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
    },
  });
}

export function useUpdateGoalMonthly() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, monthlyCents }: { id: string; monthlyCents: number }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateGoalMonthly(id, monthlyCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
    },
  });
}

export function useProjectGoalGrowth(goalId: string | undefined, years: number) {
  return useQuery<ProjectedValue>({
    queryKey: ["goal-projection", goalId, years],
    queryFn: async () => {
      if (!goalId) throw new Error("goalId required");
      const result = await commands.projectGoalGrowth(goalId, years);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isTauriRuntime() && !!goalId,
  });
}

export function useUpdateGoalPurpose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, purpose }: { id: string; purpose: string | null }) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateGoalPurpose(id, purpose);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
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
    enabled: isTauriRuntime(),
  });
}

export function useApplyNextMonthPlan() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (assignments: PlanAssignment[]) => {
      if (!isTauriRuntime()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.applyNextMonthPlan(assignments);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      invalidateDomains(qc, "budgetEnvelopes");
    },
  });
}
