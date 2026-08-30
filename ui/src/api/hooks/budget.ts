import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, raw, type BudgetEnvelope, type CategoryHistory, type GoalContributionDto, type GoalDto, type MemberBudgetEnvelope, type NewGoalInput, type PlanAssignment, type ProjectedValue, type Result } from "../openapiClient";
import { unwrap } from "../openapiClient";
import { isBackendAvailable } from "../../utils/runtime";
import { invalidateDomains } from "../invalidation";

// ── Budget ────────────────────────────────────────────────────────────────

export function useBudgetEnvelopes(month?: string) {
  return useQuery<BudgetEnvelope[]>({
    queryKey: ["budget-envelopes", month ?? "current"],
    queryFn: async () => {
      return unwrap(api.listBudgetEnvelopes(month));
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
      return unwrap(api.listMemberBudgetEnvelopes(memberId as string));
    },
    enabled: isBackendAvailable() && memberId !== null,
  });
}

export function useSetBudget() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ categoryId, amountCents, allowOverAssign, month }: { categoryId: string; amountCents: number; allowOverAssign?: boolean; month?: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.setBudget(categoryId, amountCents, allowOverAssign, month));
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
      return unwrap(api.listBudgetHistory(months));
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
      return unwrap(api.listGoals());
    },
    enabled: isBackendAvailable(),
  });
}

export function useCreateGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (input: NewGoalInput) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.createGoal(input));
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
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.updateGoalBalance(id, currentCents));
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
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.contributeToGoal(id, amountCents, note ?? null, source ?? null));
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
      return unwrap(api.listGoalContributions(goalId));
    },
    enabled: isBackendAvailable() && !!goalId,
  });
}

export function useArchiveGoal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.archiveGoal(id));
    },
    onSuccess: () => {
      invalidateDomains(qc, "goals");
    },
  });
}

export function useUpdateGoalMonthly() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, monthlyCents, period }: { id: string; monthlyCents: number; period?: string }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      // Direct raw call keeps period in the payload even before openapi types are regenerated.
      const call = (raw.POST as unknown as (path: string, opts: unknown) => Promise<Result<null>>)("/api/rpc/update_goal_monthly", {
        body: { id, monthlyCents, period: period ?? null },
      });
      await unwrap(call);
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
      return unwrap(api.projectGoalGrowth(goalId, years));
    },
    enabled: isBackendAvailable() && !!goalId,
  });
}

export function useUpdateGoalPurpose() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, purpose }: { id: string; purpose: string | null }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.updateGoalPurpose(id, purpose));
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
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      await unwrap(api.updateGoalPriority(id, priority, deadlineStrictness));
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
      return unwrap(api.getPlanNextMonthData());
    },
    staleTime: 60_000,
    enabled: isBackendAvailable(),
  });
}

export function useApplyNextMonthPlan() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (assignments: PlanAssignment[]) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.applyNextMonthPlan(assignments));
    },
    onSuccess: () => {
      invalidateDomains(qc, "budgetEnvelopes");
    },
  });
}

// ── Hold for Next Month (buffer → hold persistence) ───────────────────────

/**
 * Persist a hold for `month` ("YYYY-MM"). Mirrors `finsight_core::repos::budgets::set_hold`.
 * A hold parks unassigned money for next month: it deducts from this month's
 * `to_budget` and reappears as income-like in `available_funds` for the following month.
 */
export function useSetHold() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async ({ month, amountCents }: { month: string; amountCents: number }) => {
      if (!isBackendAvailable()) throw new Error("This action needs a connected FinSight server.");
      return unwrap(api.setHold(month, amountCents));
    },
    onSuccess: (data) => {
      const monthKey = typeof data === "object" && data !== null && "month" in data && typeof data.month === "string" ? data.month : undefined;
      if (monthKey) qc.setQueryData(["hold", monthKey], data);
      qc.invalidateQueries({ queryKey: ["hold"] });
      qc.invalidateQueries({ queryKey: ["budget-envelopes"] });
      qc.invalidateQueries({ queryKey: ["month-totals"] });
      invalidateDomains(qc, "budgetEnvelopes");
    },
  });
}
