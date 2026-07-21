import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type BudgetEnvelope, type CategoryHistory, type GoalContributionDto, type GoalDto, type MemberBudgetEnvelope, type NewGoalInput, type PlanAssignment, type ProjectedValue } from "../client";
import { isBackendAvailable } from "../../utils/runtime";
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
    enabled: isBackendAvailable(),
  });
}

/**
 * Budget-vs-actual scoped to one household member's ownership-weighted share
 * of the spend. The budgets themselves stay household-level — this is a view of
 * progress against the shared target, not a per-person target. `null` member
 * disables the query, so callers can fall back to the household view.
 */
export function useMemberBudgetEnvelopes(memberId: string | null) {
  return useQuery<MemberBudgetEnvelope[]>({
    queryKey: ["member-budget-envelopes", memberId],
    queryFn: async () => {
      const result = await commands.listMemberBudgetEnvelopes(memberId as string);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable() && memberId !== null,
  });
}

export function useSetBudget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ categoryId, amountCents }: { categoryId: string; amountCents: number }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
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
    enabled: isBackendAvailable(),
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
    enabled: isBackendAvailable(),
  });
}

export function useCreateGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewGoalInput) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
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
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateGoalBalance(id, currentCents);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
    },
  });
}

export function useContributeToGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, amountCents, note, source }: { id: string; amountCents: number; note?: string | null; source?: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.contributeToGoal(id, amountCents, note ?? null, source ?? null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: (_data, vars) => {
      invalidateDomains(qc, "goals");
      qc.invalidateQueries({ queryKey: ["goal-contributions", vars.id] });
    },
  });
}

export function useGoalContributions(goalId: string | undefined) {
  return useQuery<GoalContributionDto[]>({
    queryKey: ["goal-contributions", goalId],
    queryFn: async () => {
      if (!goalId) return [];
      const result = await commands.listGoalContributions(goalId);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    enabled: isBackendAvailable() && !!goalId,
  });
}

export function useArchiveGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
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
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
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
    enabled: isBackendAvailable() && !!goalId,
  });
}

export function useUpdateGoalPurpose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, purpose }: { id: string; purpose: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateGoalPurpose(id, purpose);
      if (result.status === "error") throw new Error(result.error.message);
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
    },
  });
}

/**
 * Priority and deadline strictness are saved together because the planner reads
 * them as a pair — a hard deadline on a "someday" goal and a "critical" goal
 * with no date are both coherent, and ordering needs to see both.
 */
export function useUpdateGoalPriority() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({
      id,
      priority,
      deadlineStrictness,
    }: {
      id: string;
      priority: string;
      deadlineStrictness: string;
    }) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.updateGoalPriority(id, priority, deadlineStrictness);
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
    enabled: isBackendAvailable(),
  });
}

export function useApplyNextMonthPlan() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (assignments: PlanAssignment[]) => {
      if (!isBackendAvailable()) throw new Error("This action needs the desktop app runtime.");
      const result = await commands.applyNextMonthPlan(assignments);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    onSuccess: () => {
      invalidateDomains(qc, "budgetEnvelopes");
    },
  });
}
