/**
 * OpenAPI-typed fetch client for the RPC surface.
 * GENERATED  do not hand-edit. Regenerate via `cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen && python gen_api.py` or `node scripts/gen_api.mjs`.
 * Wraps `openapi-fetch` with the `Result<T,AppError>` envelope and 401 -> `FINSIGHT_AUTH_REQUIRED` dispatch
 * that the old `httpBackend.ts` shim provided.
 */
import createClient from "openapi-fetch";
import type { paths, components } from "./openapi";
import { FINSIGHT_AUTH_REQUIRED } from "./eventNames";

export type AppError = { code: string; message: string; details?: unknown | null };
export type Result<T, E = AppError> = { status: "ok"; data: T } | { status: "error"; error: E };

const raw = createClient<paths>({ baseUrl: "" });

async function wrap<T>(p: Promise<{ data?: T; error?: unknown; response: Response }>): Promise<Result<T>> {
  const { data, error, response } = (await p) as unknown as {
    data?: T;
    error?: unknown;
    response: Response;
  };
  if (!response.ok) {
    const body = (error ?? data ?? {}) as { code?: string; message?: string };
    if (
      response.status === 401 &&
      typeof body === "object" &&
      body !== null &&
      (body as { code?: string }).code === "auth.required"
    ) {
      window.dispatchEvent(new CustomEvent(FINSIGHT_AUTH_REQUIRED));
      const w = window as unknown as { __FINSIGHT_ES__?: EventSource | null };
      w.__FINSIGHT_ES__?.close();
      w.__FINSIGHT_ES__ = null;
    }
    return {
      status: "error",
      error: {
        code: (body as { code?: string }).code ?? "rpc.transport",
        message: (body as { message?: string }).message ?? `HTTP ${response.status}`,
      },
    };
  }
  return { status: "ok", data: data as T };
}

export async function unwrap<T>(call: Promise<Result<T>>): Promise<T> {
  const result = await call;
  if (result.status === "error") throw new Error(result.error.message);
  return result.data;
}

export function unwrapResult<T>(result: Result<T>): T {
  if (result.status === "error") throw new Error(result.error.message);
  return result.data;
}

// Copilot stream payloads (emitted by Rust, not via RPC)
export type CopilotTokenPayload = { conversationId: string; runId: string; token: string };
export type CopilotDonePayload = { conversationId: string; runId: string; messageId: string; bundleId: string | null; toolTrace: string[]; followUpQuestions: string[]; missingData: unknown[]; actionLabel: string | null; actionPath: string | null };

// Re-exports for generative-UI blocks (typed via openapi.ts)
export type CopilotResponseBlock = components["schemas"]["AgentResponseBlock"];

export const api = {
  listAccounts: () => wrap<components["schemas"]["AccountSummary"][]>(raw.POST("/api/rpc/list_accounts" as never, {} as never) as never),
  createAccount: (input: components["schemas"]["NewAccount"]) => wrap<components["schemas"]["Account"]>(raw.POST("/api/rpc/create_account" as never, { body: { input } as never } as never) as never),
  updateAccount: (id: string, patch: components["schemas"]["AccountPatch"]) => wrap<components["schemas"]["Account"]>(raw.POST("/api/rpc/update_account" as never, { body: { id, patch } as never } as never) as never),
  archiveAccount: (id: string) => wrap<null>(raw.POST("/api/rpc/archive_account" as never, { body: { id } as never } as never) as never),
  setAccountBalance: (id: string, balanceCents: number) => wrap<null>(raw.POST("/api/rpc/set_account_balance" as never, { body: { id, balanceCents } as never } as never) as never),
  updateCategoryColor: (id: string, color: string) => wrap<null>(raw.POST("/api/rpc/update_category_color" as never, { body: { id, color } as never } as never) as never),
  createCategory: (label: string, groupId: string | null, color: string) => wrap<components["schemas"]["Category"]>(raw.POST("/api/rpc/create_category" as never, { body: { label, groupId, color } as never } as never) as never),
  renameCategory: (id: string, label: string) => wrap<null>(raw.POST("/api/rpc/rename_category" as never, { body: { id, label } as never } as never) as never),
  archiveCategory: (id: string) => wrap<null>(raw.POST("/api/rpc/archive_category" as never, { body: { id } as never } as never) as never),
  setCategoryGuidance: (id: string, guidance: string | null) => wrap<null>(raw.POST("/api/rpc/set_category_guidance" as never, { body: { id, guidance } as never } as never) as never),
  listCategoryGroups: () => wrap<components["schemas"]["CategoryGroup"][]>(raw.POST("/api/rpc/list_category_groups" as never, {} as never) as never),
  createCategoryGroup: (label: string, hint: string | null) => wrap<components["schemas"]["CategoryGroup"]>(raw.POST("/api/rpc/create_category_group" as never, { body: { label, hint } as never } as never) as never),
  setCategoryGroup: (categoryId: string, groupId: string) => wrap<null>(raw.POST("/api/rpc/set_category_group" as never, { body: { categoryId, groupId } as never } as never) as never),
  addCategoryExample: (categoryId: string, exampleText: string, sourceTxnId: string | null) => wrap<components["schemas"]["CategoryExample"]>(raw.POST("/api/rpc/add_category_example" as never, { body: { categoryId, exampleText, sourceTxnId } as never } as never) as never),
  removeCategoryExample: (id: string) => wrap<null>(raw.POST("/api/rpc/remove_category_example" as never, { body: { id } as never } as never) as never),
  listCategoryExamples: (categoryId: string) => wrap<components["schemas"]["CategoryExample"][]>(raw.POST("/api/rpc/list_category_examples" as never, { body: { categoryId } as never } as never) as never),
  listTransactions: (filter: components["schemas"]["TxnFilterInput"]) => wrap<components["schemas"]["Transaction"][]>(raw.POST("/api/rpc/list_transactions" as never, { body: { filter } as never } as never) as never),
  createTransaction: (input: components["schemas"]["NewTransaction"]) => wrap<components["schemas"]["Transaction"]>(raw.POST("/api/rpc/create_transaction" as never, { body: { input } as never } as never) as never),
  updateTransaction: (id: string, patch: components["schemas"]["TxnPatch"]) => wrap<components["schemas"]["UpdateTxnResult"]>(raw.POST("/api/rpc/update_transaction" as never, { body: { id, patch } as never } as never) as never),
  deleteTransaction: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_transaction" as never, { body: { id } as never } as never) as never),
  createRule: (pattern: string, categoryId: string) => wrap<components["schemas"]["Rule"]>(raw.POST("/api/rpc/create_rule" as never, { body: { pattern, categoryId } as never } as never) as never),
  setTransactionOwner: (transactionId: string, memberId: string | null) => wrap<null>(raw.POST("/api/rpc/set_transaction_owner" as never, { body: { transactionId, memberId } as never } as never) as never),
  listCategories: () => wrap<components["schemas"]["CategoryDto"][]>(raw.POST("/api/rpc/list_categories" as never, {} as never) as never),
  setCategorySpendingType: (id: string, spendingType: string | null) => wrap<null>(raw.POST("/api/rpc/set_category_spending_type" as never, { body: { id, spendingType } as never } as never) as never),
  getSpendingBreakdown: () => wrap<components["schemas"]["SpendingBreakdown"]>(raw.POST("/api/rpc/get_spending_breakdown" as never, {} as never) as never),
  getOnboardingState: () => wrap<components["schemas"]["OnboardingState"]>(raw.POST("/api/rpc/get_onboarding_state" as never, {} as never) as never),
  markOnboardingComplete: () => wrap<null>(raw.POST("/api/rpc/mark_onboarding_complete" as never, {} as never) as never),
  resetOnboardingCompletion: () => wrap<null>(raw.POST("/api/rpc/reset_onboarding_completion" as never, {} as never) as never),
  commitStarterCategories: (categories: components["schemas"]["StarterCategory"][]) => wrap<null>(raw.POST("/api/rpc/commit_starter_categories" as never, { body: { categories } as never } as never) as never),
  probeOllama: (baseUrl: string) => wrap<components["schemas"]["OllamaProbeResult"]>(raw.POST("/api/rpc/probe_ollama" as never, { body: { baseUrl } as never } as never) as never),
  saveLlmProvider: (config: components["schemas"]["LlmProviderConfig"]) => wrap<null>(raw.POST("/api/rpc/save_llm_provider" as never, { body: { config } as never } as never) as never),
  appReady: () => wrap<components["schemas"]["AppReady"]>(raw.POST("/api/rpc/app_ready" as never, {} as never) as never),
  listAccountPositions: (accountId: string) => wrap<components["schemas"]["Position"][]>(raw.POST("/api/rpc/list_account_positions" as never, { body: { accountId } as never } as never) as never),
  getInvestmentSummary: (accountId: string) => wrap<components["schemas"]["InvestmentSummary"]>(raw.POST("/api/rpc/get_investment_summary" as never, { body: { accountId } as never } as never) as never),
  previewCsvColumns: (path: string, skipHeaderRows: number) => wrap<components["schemas"]["CsvPreview"]>(raw.POST("/api/rpc/preview_csv_columns" as never, { body: { path, skipHeaderRows } as never } as never) as never),
  prepareCsvImport: (path: string, accountId: string, mapping: components["schemas"]["CsvImportMapping"]) => wrap<components["schemas"]["PreparedImportPreview"]>(raw.POST("/api/rpc/prepare_csv_import" as never, { body: { path, accountId, mapping } as never } as never) as never),
  importCsv: (path: string, accountId: string, mapping: components["schemas"]["CsvImportMapping"]) => wrap<components["schemas"]["ImportResult"]>(raw.POST("/api/rpc/import_csv" as never, { body: { path, accountId, mapping } as never } as never) as never),
  getSavedCsvMapping: (accountId: string) => wrap<components["schemas"]["CsvImportMapping"] | null>(raw.POST("/api/rpc/get_saved_csv_mapping" as never, { body: { accountId } as never } as never) as never),
  listUnfinishedImports: () => wrap<components["schemas"]["Import"][]>(raw.POST("/api/rpc/list_unfinished_imports" as never, {} as never) as never),
  discardUnfinishedImport: (importId: string) => wrap<null>(raw.POST("/api/rpc/discard_unfinished_import" as never, { body: { importId } as never } as never) as never),
  setCompletionProvider: (config: components["schemas"]["CompletionProviderConfig"]) => wrap<null>(raw.POST("/api/rpc/set_completion_provider" as never, { body: { config } as never } as never) as never),
  getCompletionProvider: () => wrap<components["schemas"]["CompletionProviderConfig"]>(raw.POST("/api/rpc/get_completion_provider" as never, {} as never) as never),
  saveProviderApiKey: (providerId: string, key: string) => wrap<null>(raw.POST("/api/rpc/save_provider_api_key" as never, { body: { providerId, key } as never } as never) as never),
  listProviderModels: (config: components["schemas"]["CompletionProviderConfig"]) => wrap<string[]>(raw.POST("/api/rpc/list_provider_models" as never, { body: { config } as never } as never) as never),
  testCompletionProvider: (config: components["schemas"]["CompletionProviderConfig"], apiKey: string | null) => wrap<components["schemas"]["ProviderTestResult"]>(raw.POST("/api/rpc/test_completion_provider" as never, { body: { config, apiKey } as never } as never) as never),
  getNeedsReviewCount: () => wrap<number>(raw.POST("/api/rpc/get_needs_review_count" as never, {} as never) as never),
  triggerCategorize: () => wrap<null>(raw.POST("/api/rpc/trigger_categorize" as never, {} as never) as never),
  recomputeAnomalies: () => wrap<number>(raw.POST("/api/rpc/recompute_anomalies" as never, {} as never) as never),
  setAnomalyDismissed: (txnId: string, dismissed: boolean) => wrap<null>(raw.POST("/api/rpc/set_anomaly_dismissed" as never, { body: { txnId, dismissed } as never } as never) as never),
  triggerRecategorizeLowConfidence: () => wrap<null>(raw.POST("/api/rpc/trigger_recategorize_low_confidence" as never, {} as never) as never),
  getAgentStatus: () => wrap<components["schemas"]["AgentStatus"]>(raw.POST("/api/rpc/get_agent_status" as never, {} as never) as never),
  askAgent: (question: string, mode: string | null) => wrap<components["schemas"]["AgentAnswer"]>(raw.POST("/api/rpc/ask_agent" as never, { body: { question, mode } as never } as never) as never),
  listCategoriesWithSpending: () => wrap<components["schemas"]["CategoryWithSpending"][]>(raw.POST("/api/rpc/list_categories_with_spending" as never, {} as never) as never),
  listRulesWithCategories: () => wrap<components["schemas"]["RuleWithCategory"][]>(raw.POST("/api/rpc/list_rules_with_categories" as never, {} as never) as never),
  toggleRule: (id: string, enabled: boolean) => wrap<null>(raw.POST("/api/rpc/toggle_rule" as never, { body: { id, enabled } as never } as never) as never),
  listBudgetEnvelopes: () => wrap<components["schemas"]["BudgetEnvelope"][]>(raw.POST("/api/rpc/list_budget_envelopes" as never, {} as never) as never),
  listMemberBudgetEnvelopes: (memberId: string) => wrap<components["schemas"]["MemberBudgetEnvelope"][]>(raw.POST("/api/rpc/list_member_budget_envelopes" as never, { body: { memberId } as never } as never) as never),
  setBudget: (categoryId: string, amountCents: number) => wrap<null>(raw.POST("/api/rpc/set_budget" as never, { body: { categoryId, amountCents } as never } as never) as never),
  listGoals: () => wrap<components["schemas"]["GoalDto"][]>(raw.POST("/api/rpc/list_goals" as never, {} as never) as never),
  createGoal: (input: components["schemas"]["NewGoalInput"]) => wrap<components["schemas"]["GoalDto"]>(raw.POST("/api/rpc/create_goal" as never, { body: { input } as never } as never) as never),
  updateGoalBalance: (id: string, currentCents: number) => wrap<null>(raw.POST("/api/rpc/update_goal_balance" as never, { body: { id, currentCents } as never } as never) as never),
  contributeToGoal: (id: string, amountCents: number, note: string | null, source: string | null) => wrap<components["schemas"]["GoalContributionDto"]>(raw.POST("/api/rpc/contribute_to_goal" as never, { body: { id, amountCents, note, source } as never } as never) as never),
  listGoalContributions: (goalId: string) => wrap<components["schemas"]["GoalContributionDto"][]>(raw.POST("/api/rpc/list_goal_contributions" as never, { body: { goalId } as never } as never) as never),
  archiveGoal: (id: string) => wrap<null>(raw.POST("/api/rpc/archive_goal" as never, { body: { id } as never } as never) as never),
  projectGoalGrowth: (goalId: string, years: number) => wrap<components["schemas"]["ProjectedValue"]>(raw.POST("/api/rpc/project_goal_growth" as never, { body: { goalId, years } as never } as never) as never),
  listRecurring: () => wrap<components["schemas"]["RecurringItem"][]>(raw.POST("/api/rpc/list_recurring" as never, {} as never) as never),
  setSubscriptionVerdict: (merchantKey: string, verdict: string | null) => wrap<null>(raw.POST("/api/rpc/set_subscription_verdict" as never, { body: { merchantKey, verdict } as never } as never) as never),
  setSubscriptionTrial: (merchantKey: string, label: string, trialEndsAt: string | null) => wrap<null>(raw.POST("/api/rpc/set_subscription_trial" as never, { body: { merchantKey, label, trialEndsAt } as never } as never) as never),
  markSubscriptionCancelled: (merchantKey: string, label: string, cancelledAt: string) => wrap<null>(raw.POST("/api/rpc/mark_subscription_cancelled" as never, { body: { merchantKey, label, cancelledAt } as never } as never) as never),
  getReportData: (scope: string, memberId: string | null) => wrap<components["schemas"]["ReportData"]>(raw.POST("/api/rpc/get_report_data" as never, { body: { scope, memberId } as never } as never) as never),
  getMonthTotals: () => wrap<components["schemas"]["MonthTotals"]>(raw.POST("/api/rpc/get_month_totals" as never, {} as never) as never),
  getSavingsRateHistory: () => wrap<components["schemas"]["SavingsRatePoint"][]>(raw.POST("/api/rpc/get_savings_rate_history" as never, {} as never) as never),
  getMonthClose: (year: number, month: number) => wrap<components["schemas"]["MonthCloseView"]>(raw.POST("/api/rpc/get_month_close" as never, { body: { year, month } as never } as never) as never),
  saveMonthClose: (input: components["schemas"]["SaveMonthCloseInput"]) => wrap<components["schemas"]["MonthCloseView"]>(raw.POST("/api/rpc/save_month_close" as never, { body: { input } as never } as never) as never),
  listMonthCloses: () => wrap<components["schemas"]["MonthCloseListItem"][]>(raw.POST("/api/rpc/list_month_closes" as never, {} as never) as never),
  getSpendingPathBack: (period: string | null, targetMonthlyCents: number | null) => wrap<components["schemas"]["PathBackView"] | null>(raw.POST("/api/rpc/get_spending_path_back" as never, { body: { period, targetMonthlyCents } as never } as never) as never),
  setSpendingAnnotation: (merchantKey: string, verdict: string) => wrap<null>(raw.POST("/api/rpc/set_spending_annotation" as never, { body: { merchantKey, verdict } as never } as never) as never),
  getFinancialMetrics: (memberId: string | null) => wrap<components["schemas"]["FinancialMetrics"]>(raw.POST("/api/rpc/get_financial_metrics" as never, { body: { memberId } as never } as never) as never),
  explainFinancialMetrics: (memberId: string | null) => wrap<components["schemas"]["MetricExplanation"][]>(raw.POST("/api/rpc/explain_financial_metrics" as never, { body: { memberId } as never } as never) as never),
  explainGoals: () => wrap<components["schemas"]["MetricExplanation"][]>(raw.POST("/api/rpc/explain_goals" as never, {} as never) as never),
  getCashflowForecast: (horizonDays: number | null, bufferCents: number | null, extraExpenseCents: number | null, extraExpenseDate: string | null) => wrap<components["schemas"]["CashflowForecast"]>(raw.POST("/api/rpc/get_cashflow_forecast" as never, { body: { horizonDays, bufferCents, extraExpenseCents, extraExpenseDate } as never } as never) as never),
  getNotificationPrefs: () => wrap<components["schemas"]["NotificationPrefsDto"]>(raw.POST("/api/rpc/get_notification_prefs" as never, {} as never) as never),
  setNotificationPrefs: (prefs: components["schemas"]["NotificationPrefsDto"]) => wrap<null>(raw.POST("/api/rpc/set_notification_prefs" as never, { body: { prefs } as never } as never) as never),
  listNotifications: (includeResolved: boolean | null) => wrap<components["schemas"]["Notification"][]>(raw.POST("/api/rpc/list_notifications" as never, { body: { includeResolved } as never } as never) as never),
  markNotificationRead: (id: string) => wrap<null>(raw.POST("/api/rpc/mark_notification_read" as never, { body: { id } as never } as never) as never),
  markAllNotificationsRead: () => wrap<number>(raw.POST("/api/rpc/mark_all_notifications_read" as never, {} as never) as never),
  notificationUnreadCount: () => wrap<number>(raw.POST("/api/rpc/notification_unread_count" as never, {} as never) as never),
  householdNetWorthBreakdown: () => wrap<components["schemas"]["MemberNetWorth"][]>(raw.POST("/api/rpc/household_net_worth_breakdown" as never, {} as never) as never),
  setFinancialAssumptions: (input: components["schemas"]["FinancialAssumptionsInput"]) => wrap<null>(raw.POST("/api/rpc/set_financial_assumptions" as never, { body: { input } as never } as never) as never),
  listRestorationEnvelopes: () => wrap<components["schemas"]["RestorationEnvelope"][]>(raw.POST("/api/rpc/list_restoration_envelopes" as never, {} as never) as never),
  getRestorationStatus: (id: string) => wrap<components["schemas"]["RestorationStatus"] | null>(raw.POST("/api/rpc/get_restoration_status" as never, { body: { id } as never } as never) as never),
  createRestorationEnvelope: (input: components["schemas"]["RestorationEnvelopeInput"]) => wrap<components["schemas"]["RestorationEnvelope"]>(raw.POST("/api/rpc/create_restoration_envelope" as never, { body: { input } as never } as never) as never),
  closeRestorationEnvelope: (id: string) => wrap<null>(raw.POST("/api/rpc/close_restoration_envelope" as never, { body: { id } as never } as never) as never),
  deleteRestorationEnvelope: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_restoration_envelope" as never, { body: { id } as never } as never) as never),
  addRestorationLeg: (envelopeId: string, amountCents: number, notedOn: string, transactionId: string | null) => wrap<components["schemas"]["RestorationLeg"]>(raw.POST("/api/rpc/add_restoration_leg" as never, { body: { envelopeId, amountCents, notedOn, transactionId } as never } as never) as never),
  removeRestorationLeg: (legId: string) => wrap<null>(raw.POST("/api/rpc/remove_restoration_leg" as never, { body: { legId } as never } as never) as never),
  getFinancialPhilosophy: () => wrap<components["schemas"]["FinancialPhilosophyDto"]>(raw.POST("/api/rpc/get_financial_philosophy" as never, {} as never) as never),
  setFinancialPhilosophy: (input: components["schemas"]["FinancialPhilosophyDto"]) => wrap<null>(raw.POST("/api/rpc/set_financial_philosophy" as never, { body: { input } as never } as never) as never),
  runScenario: (description: string, months: number, params: components["schemas"]["ScenarioParamsInput"] | null) => wrap<components["schemas"]["RanScenario"]>(raw.POST("/api/rpc/run_scenario" as never, { body: { description, months, params } as never } as never) as never),
  saveScenario: (description: string, params: components["schemas"]["ScenarioParamsInput"], months: number) => wrap<components["schemas"]["SavedScenarioDetail"]>(raw.POST("/api/rpc/save_scenario" as never, { body: { description, params, months } as never } as never) as never),
  listSavedScenarios: () => wrap<components["schemas"]["SavedScenarioDetail"][]>(raw.POST("/api/rpc/list_saved_scenarios" as never, {} as never) as never),
  duplicateScenario: (id: string) => wrap<components["schemas"]["SavedScenarioDetail"] | null>(raw.POST("/api/rpc/duplicate_scenario" as never, { body: { id } as never } as never) as never),
  archiveScenario: (id: string, archived: boolean) => wrap<null>(raw.POST("/api/rpc/archive_scenario" as never, { body: { id, archived } as never } as never) as never),
  promoteScenario: (id: string) => wrap<components["schemas"]["ScenarioPlanProposal"]>(raw.POST("/api/rpc/promote_scenario" as never, { body: { id } as never } as never) as never),
  applyScenario: (id: string, approvedChangeIds: string[]) => wrap<components["schemas"]["ApplyScenarioResult"]>(raw.POST("/api/rpc/apply_scenario" as never, { body: { id, approvedChangeIds } as never } as never) as never),
  reviseScenario: (id: string, params: components["schemas"]["ScenarioParamsInput"]) => wrap<components["schemas"]["SavedScenarioDetail"]>(raw.POST("/api/rpc/revise_scenario" as never, { body: { id, params } as never } as never) as never),
  clearScenarioRevision: (id: string) => wrap<components["schemas"]["SavedScenarioDetail"]>(raw.POST("/api/rpc/clear_scenario_revision" as never, { body: { id } as never } as never) as never),
  explainScenario: (id: string) => wrap<components["schemas"]["MetricExplanation"]>(raw.POST("/api/rpc/explain_scenario" as never, { body: { id } as never } as never) as never),
  deleteScenario: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_scenario" as never, { body: { id } as never } as never) as never),
  getTransactionCount: () => wrap<number>(raw.POST("/api/rpc/get_transaction_count" as never, {} as never) as never),
  listManualAssets: () => wrap<components["schemas"]["ManualAsset"][]>(raw.POST("/api/rpc/list_manual_assets" as never, {} as never) as never),
  createManualAsset: (input: components["schemas"]["NewManualAsset"]) => wrap<components["schemas"]["ManualAsset"]>(raw.POST("/api/rpc/create_manual_asset" as never, { body: { input } as never } as never) as never),
  updateManualAsset: (id: string, patch: components["schemas"]["ManualAssetPatch"]) => wrap<components["schemas"]["ManualAsset"]>(raw.POST("/api/rpc/update_manual_asset" as never, { body: { id, patch } as never } as never) as never),
  deleteManualAsset: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_manual_asset" as never, { body: { id } as never } as never) as never),
  recordNetWorthSnapshot: () => wrap<null>(raw.POST("/api/rpc/record_net_worth_snapshot" as never, {} as never) as never),
  listNetWorthHistory: (days: number) => wrap<components["schemas"]["NetWorthPoint"][]>(raw.POST("/api/rpc/list_net_worth_history" as never, { body: { days } as never } as never) as never),
  computeDebtPayoff: (extraMonthlyCents: number) => wrap<components["schemas"]["DebtPayoffResult"][]>(raw.POST("/api/rpc/compute_debt_payoff" as never, { body: { extraMonthlyCents } as never } as never) as never),
  getUncelebratedMilestones: () => wrap<number[]>(raw.POST("/api/rpc/get_uncelebrated_milestones" as never, {} as never) as never),
  listHouseholdMembers: () => wrap<components["schemas"]["HouseholdMember"][]>(raw.POST("/api/rpc/list_household_members" as never, {} as never) as never),
  createHouseholdMember: (name: string, color: string | null) => wrap<components["schemas"]["HouseholdMember"]>(raw.POST("/api/rpc/create_household_member" as never, { body: { name, color } as never } as never) as never),
  setSelfMember: (memberId: string) => wrap<null>(raw.POST("/api/rpc/set_self_member" as never, { body: { memberId } as never } as never) as never),
  deleteHouseholdMember: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_household_member" as never, { body: { id } as never } as never) as never),
  listAccountOwners: () => wrap<components["schemas"]["AccountOwner"][]>(raw.POST("/api/rpc/list_account_owners" as never, {} as never) as never),
  setAccountOwners: (accountId: string, memberIds: string[]) => wrap<null>(raw.POST("/api/rpc/set_account_owners" as never, { body: { accountId, memberIds } as never } as never) as never),
  setAccountOwnerShares: (accountId: string, owners: components["schemas"]["OwnerShare"][]) => wrap<null>(raw.POST("/api/rpc/set_account_owner_shares" as never, { body: { accountId, owners } as never } as never) as never),
  listAssetOwners: () => wrap<components["schemas"]["AssetOwner"][]>(raw.POST("/api/rpc/list_asset_owners" as never, {} as never) as never),
  setAssetOwners: (assetId: string, owners: components["schemas"]["OwnerShare"][]) => wrap<null>(raw.POST("/api/rpc/set_asset_owners" as never, { body: { assetId, owners } as never } as never) as never),
  getDataHealth: () => wrap<components["schemas"]["DataHealth"]>(raw.POST("/api/rpc/get_data_health" as never, {} as never) as never),
  createManualBackup: () => wrap<components["schemas"]["BackupInfo"]>(raw.POST("/api/rpc/create_manual_backup" as never, {} as never) as never),
  stageRestoreBackup: (path: string) => wrap<null>(raw.POST("/api/rpc/stage_restore_backup" as never, { body: { path } as never } as never) as never),
  cancelStagedRestore: () => wrap<null>(raw.POST("/api/rpc/cancel_staged_restore" as never, {} as never) as never),
  listAgentMemory: () => wrap<components["schemas"]["AgentMemory"][]>(raw.POST("/api/rpc/list_agent_memory" as never, {} as never) as never),
  forgetAgentMemory: (id: string) => wrap<null>(raw.POST("/api/rpc/forget_agent_memory" as never, { body: { id } as never } as never) as never),
  getFinancialHealthScore: () => wrap<components["schemas"]["HealthScore"]>(raw.POST("/api/rpc/get_financial_health_score" as never, {} as never) as never),
  listRuleProposals: () => wrap<components["schemas"]["RuleProposal"][]>(raw.POST("/api/rpc/list_rule_proposals" as never, {} as never) as never),
  acceptRuleProposal: (id: string) => wrap<null>(raw.POST("/api/rpc/accept_rule_proposal" as never, { body: { id } as never } as never) as never),
  declineRuleProposal: (id: string) => wrap<null>(raw.POST("/api/rpc/decline_rule_proposal" as never, { body: { id } as never } as never) as never),
  listCategoryProposals: () => wrap<components["schemas"]["CategoryProposal"][]>(raw.POST("/api/rpc/list_category_proposals" as never, {} as never) as never),
  acceptCategoryProposal: (id: string) => wrap<components["schemas"]["UpdateTxnResult"]>(raw.POST("/api/rpc/accept_category_proposal" as never, { body: { id } as never } as never) as never),
  correctCategoryProposal: (id: string, categoryId: string) => wrap<components["schemas"]["UpdateTxnResult"]>(raw.POST("/api/rpc/correct_category_proposal" as never, { body: { id, categoryId } as never } as never) as never),
  rejectCategoryProposal: (id: string) => wrap<null>(raw.POST("/api/rpc/reject_category_proposal" as never, { body: { id } as never } as never) as never),
  listAgentSessions: () => wrap<components["schemas"]["AgentSession"][]>(raw.POST("/api/rpc/list_agent_sessions" as never, {} as never) as never),
  createAgentSession: (title: string, taskType: string) => wrap<components["schemas"]["AgentSession"]>(raw.POST("/api/rpc/create_agent_session" as never, { body: { title, taskType } as never } as never) as never),
  closeAgentSession: (id: string) => wrap<null>(raw.POST("/api/rpc/close_agent_session" as never, { body: { id } as never } as never) as never),
  listActionBundles: (statusFilter: string | null, sessionId: string | null, limit: number | null) => wrap<components["schemas"]["AgentActionBundle"][]>(raw.POST("/api/rpc/list_action_bundles" as never, { body: { statusFilter, sessionId, limit } as never } as never) as never),
  getActionBundle: (id: string) => wrap<components["schemas"]["AgentActionBundle"] | null>(raw.POST("/api/rpc/get_action_bundle" as never, { body: { id } as never } as never) as never),
  approveActionItem: (itemId: string) => wrap<null>(raw.POST("/api/rpc/approve_action_item" as never, { body: { itemId } as never } as never) as never),
  rejectActionItem: (itemId: string) => wrap<null>(raw.POST("/api/rpc/reject_action_item" as never, { body: { itemId } as never } as never) as never),
  listExecutionLog: (bundleId: string) => wrap<components["schemas"]["AgentExecutionEntry"][]>(raw.POST("/api/rpc/list_execution_log" as never, { body: { bundleId } as never } as never) as never),
  executeActionBundle: (bundleId: string) => wrap<components["schemas"]["ExecutionSummary"]>(raw.POST("/api/rpc/execute_action_bundle" as never, { body: { bundleId } as never } as never) as never),
  listRecipes: (includePaused: boolean) => wrap<components["schemas"]["AgentRecipe"][]>(raw.POST("/api/rpc/list_recipes" as never, { body: { includePaused } as never } as never) as never),
  createRecipe: (title: string, description: string, recipeKind: string, promptTemplate: string, cadence: string, dayOfWeek: number | null, dayOfMonth: number | null) => wrap<components["schemas"]["AgentRecipe"]>(raw.POST("/api/rpc/create_recipe" as never, { body: { title, description, recipeKind, promptTemplate, cadence, dayOfWeek, dayOfMonth } as never } as never) as never),
  updateRecipe: (id: string, title: string, description: string, promptTemplate: string, cadence: string, dayOfWeek: number | null, dayOfMonth: number | null) => wrap<components["schemas"]["AgentRecipe"]>(raw.POST("/api/rpc/update_recipe" as never, { body: { id, title, description, promptTemplate, cadence, dayOfWeek, dayOfMonth } as never } as never) as never),
  pauseRecipe: (id: string) => wrap<null>(raw.POST("/api/rpc/pause_recipe" as never, { body: { id } as never } as never) as never),
  resumeRecipe: (id: string) => wrap<null>(raw.POST("/api/rpc/resume_recipe" as never, { body: { id } as never } as never) as never),
  deleteRecipe: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_recipe" as never, { body: { id } as never } as never) as never),
  triggerRecipe: (id: string) => wrap<string>(raw.POST("/api/rpc/trigger_recipe" as never, { body: { id } as never } as never) as never),
  listRecipeRuns: (recipeId: string, limit: number | null) => wrap<components["schemas"]["AgentRecipeRun"][]>(raw.POST("/api/rpc/list_recipe_runs" as never, { body: { recipeId, limit } as never } as never) as never),
  setTransactionFlags: (id: string, isReimbursable: boolean, isSplit: boolean) => wrap<components["schemas"]["Transaction"]>(raw.POST("/api/rpc/set_transaction_flags" as never, { body: { id, isReimbursable, isSplit } as never } as never) as never),
  setTransactionTransfer: (id: string, isTransfer: boolean) => wrap<components["schemas"]["TransferVerdictResult"]>(raw.POST("/api/rpc/set_transaction_transfer" as never, { body: { id, isTransfer } as never } as never) as never),
  applyTransferVerdictToSimilar: (pattern: string, isTransfer: boolean) => wrap<number>(raw.POST("/api/rpc/apply_transfer_verdict_to_similar" as never, { body: { pattern, isTransfer } as never } as never) as never),
  setCounterpartyVerdict: (id: string, verdict: components["schemas"]["CounterpartyVerdict"]) => wrap<components["schemas"]["Transaction"]>(raw.POST("/api/rpc/set_counterparty_verdict" as never, { body: { id, verdict } as never } as never) as never),
  applyCounterpartyVerdictToSimilar: (pattern: string, verdict: components["schemas"]["CounterpartyVerdict"]) => wrap<number>(raw.POST("/api/rpc/apply_counterparty_verdict_to_similar" as never, { body: { pattern, verdict } as never } as never) as never),
  listUnresolvedCounterparties: () => wrap<components["schemas"]["UnresolvedCounterpartyDto"][]>(raw.POST("/api/rpc/list_unresolved_counterparties" as never, {} as never) as never),
  getTransactionSplits: (transactionId: string) => wrap<components["schemas"]["TransactionSplitDto"][]>(raw.POST("/api/rpc/get_transaction_splits" as never, { body: { transactionId } as never } as never) as never),
  setTransactionSplits: (transactionId: string, splits: components["schemas"]["SplitInputDto"][]) => wrap<null>(raw.POST("/api/rpc/set_transaction_splits" as never, { body: { transactionId, splits } as never } as never) as never),
  updateGoalMonthly: (id: string, monthlyCents: number) => wrap<null>(raw.POST("/api/rpc/update_goal_monthly" as never, { body: { id, monthlyCents } as never } as never) as never),
  updateGoalPriority: (id: string, priority: string, deadlineStrictness: string) => wrap<null>(raw.POST("/api/rpc/update_goal_priority" as never, { body: { id, priority, deadlineStrictness } as never } as never) as never),
  updateGoalPurpose: (id: string, purpose: string | null) => wrap<null>(raw.POST("/api/rpc/update_goal_purpose" as never, { body: { id, purpose } as never } as never) as never),
  getCurrency: () => wrap<string>(raw.POST("/api/rpc/get_currency" as never, {} as never) as never),
  setCurrency: (currency: string) => wrap<null>(raw.POST("/api/rpc/set_currency" as never, { body: { currency } as never } as never) as never),
  deleteAllData: () => wrap<null>(raw.POST("/api/rpc/delete_all_data" as never, {} as never) as never),
  exportAllDataJson: () => wrap<string>(raw.POST("/api/rpc/export_all_data_json" as never, {} as never) as never),
  exportAllDataCsv: () => wrap<string>(raw.POST("/api/rpc/export_all_data_csv" as never, {} as never) as never),
  getNotificationsEnabled: () => wrap<boolean>(raw.POST("/api/rpc/get_notifications_enabled" as never, {} as never) as never),
  setNotificationsEnabled: (enabled: boolean) => wrap<null>(raw.POST("/api/rpc/set_notifications_enabled" as never, { body: { enabled } as never } as never) as never),
  getAutoCategorizeEnabled: () => wrap<boolean>(raw.POST("/api/rpc/get_auto_categorize_enabled" as never, {} as never) as never),
  setAutoCategorizeEnabled: (enabled: boolean) => wrap<null>(raw.POST("/api/rpc/set_auto_categorize_enabled" as never, { body: { enabled } as never } as never) as never),
  getPlanNextMonthData: () => wrap<components["schemas"]["PlanData"]>(raw.POST("/api/rpc/get_plan_next_month_data" as never, {} as never) as never),
  applyNextMonthPlan: (assignments: components["schemas"]["PlanAssignment"][]) => wrap<null>(raw.POST("/api/rpc/apply_next_month_plan" as never, { body: { assignments } as never } as never) as never),
  listBudgetHistory: (months: number) => wrap<components["schemas"]["CategoryHistory"][]>(raw.POST("/api/rpc/list_budget_history" as never, { body: { months } as never } as never) as never),
  listRecentAgentActivity: (limit: number) => wrap<components["schemas"]["AgentActivity"][]>(raw.POST("/api/rpc/list_recent_agent_activity" as never, { body: { limit } as never } as never) as never),
  listPlannedTransactions: (filter: components["schemas"]["PlannedTxnFilter"]) => wrap<components["schemas"]["PlannedTransaction"][]>(raw.POST("/api/rpc/list_planned_transactions" as never, { body: { filter } as never } as never) as never),
  getPlannedTransaction: (id: string) => wrap<components["schemas"]["PlannedTransaction"] | null>(raw.POST("/api/rpc/get_planned_transaction" as never, { body: { id } as never } as never) as never),
  createPlannedTransaction: (input: components["schemas"]["NewPlannedTransaction"]) => wrap<components["schemas"]["PlannedTransaction"]>(raw.POST("/api/rpc/create_planned_transaction" as never, { body: { input } as never } as never) as never),
  updatePlannedTransaction: (id: string, patch: components["schemas"]["PlannedTransactionPatch"]) => wrap<components["schemas"]["PlannedTransaction"]>(raw.POST("/api/rpc/update_planned_transaction" as never, { body: { id, patch } as never } as never) as never),
  deletePlannedTransaction: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_planned_transaction" as never, { body: { id } as never } as never) as never),
  exportTransactionsCsv: (filter: components["schemas"]["TxnFilterInput"]) => wrap<string>(raw.POST("/api/rpc/export_transactions_csv" as never, { body: { filter } as never } as never) as never),
  exportSearchTransactionsCsv: (query: components["schemas"]["SearchTxnQueryInput"]) => wrap<string>(raw.POST("/api/rpc/export_search_transactions_csv" as never, { body: { query } as never } as never) as never),
  exportAccountCsv: (accountId: string) => wrap<string>(raw.POST("/api/rpc/export_account_csv" as never, { body: { accountId } as never } as never) as never),
  listAccountBalanceHistory: (accountId: string, days: number) => wrap<components["schemas"]["AccountBalancePoint"][]>(raw.POST("/api/rpc/list_account_balance_history" as never, { body: { accountId, days } as never } as never) as never),
  getAccountBalanceTimeline: (accountId: string, since: string | null) => wrap<components["schemas"]["AccountBalanceTimeline"]>(raw.POST("/api/rpc/get_account_balance_timeline" as never, { body: { accountId, since } as never } as never) as never),
  listAccountBalanceSparklines: (days: number) => wrap<components["schemas"]["AccountSparkline"][]>(raw.POST("/api/rpc/list_account_balance_sparklines" as never, { body: { days } as never } as never) as never),
  getJourneyStatus: () => wrap<components["schemas"]["JourneyStatus"]>(raw.POST("/api/rpc/get_journey_status" as never, {} as never) as never),
  getActionItems: () => wrap<components["schemas"]["ActionItem"][]>(raw.POST("/api/rpc/get_action_items" as never, {} as never) as never),
  getInboxBadgeCount: () => wrap<components["schemas"]["InboxBadgeCount"]>(raw.POST("/api/rpc/get_inbox_badge_count" as never, {} as never) as never),
  getPushStatus: () => wrap<components["schemas"]["PushStatus"]>(raw.POST("/api/rpc/get_push_status" as never, {} as never) as never),
  savePushSubscription: (endpoint: string, p256dh: string, auth: string, label: string | null) => wrap<null>(raw.POST("/api/rpc/save_push_subscription" as never, { body: { endpoint, p256dh, auth, label } as never } as never) as never),
  deletePushSubscription: (endpoint: string) => wrap<boolean>(raw.POST("/api/rpc/delete_push_subscription" as never, { body: { endpoint } as never } as never) as never),
  listPushDevices: () => wrap<components["schemas"]["PushDevice"][]>(raw.POST("/api/rpc/list_push_devices" as never, {} as never) as never),
  sendTestPush: () => wrap<components["schemas"]["PushDeliveryReport"]>(raw.POST("/api/rpc/send_test_push" as never, {} as never) as never),
  saveSimplefinSetupToken: (token: string) => wrap<components["schemas"]["SimpleFinConnectionInfo"][]>(raw.POST("/api/rpc/save_simplefin_setup_token" as never, { body: { token } as never } as never) as never),
  getSimplefinStatus: () => wrap<components["schemas"]["SimpleFinStatus"]>(raw.POST("/api/rpc/get_simplefin_status" as never, {} as never) as never),
  listSimplefinConnections: () => wrap<components["schemas"]["SimpleFinConnectionInfo"][]>(raw.POST("/api/rpc/list_simplefin_connections" as never, {} as never) as never),
  listSimplefinAccounts: () => wrap<components["schemas"]["SimpleFinAccountInfo"][]>(raw.POST("/api/rpc/list_simplefin_accounts" as never, {} as never) as never),
  importSimplefinAccounts: (accounts: components["schemas"]["SimpleFinAccountImportRequest"][]) => wrap<string[]>(raw.POST("/api/rpc/import_simplefin_accounts" as never, { body: { accounts } as never } as never) as never),
  syncSimplefinAccount: (accountId: string) => wrap<components["schemas"]["SyncSummary"]>(raw.POST("/api/rpc/sync_simplefin_account" as never, { body: { accountId } as never } as never) as never),
  disconnectSimplefin: () => wrap<null>(raw.POST("/api/rpc/disconnect_simplefin" as never, {} as never) as never),
  purgeSimplefinData: () => wrap<components["schemas"]["SimpleFinPurgeSummary"]>(raw.POST("/api/rpc/purge_simplefin_data" as never, {} as never) as never),
  deleteSimplefinConnection: (connectionId: string) => wrap<null>(raw.POST("/api/rpc/delete_simplefin_connection" as never, { body: { connectionId } as never } as never) as never),
  syncAllSimplefinAccounts: () => wrap<components["schemas"]["AccountSyncResult"][]>(raw.POST("/api/rpc/sync_all_simplefin_accounts" as never, {} as never) as never),
  getSimplefinSyncSettings: () => wrap<components["schemas"]["SimpleFinSyncSettings"]>(raw.POST("/api/rpc/get_simplefin_sync_settings" as never, {} as never) as never),
  setSimplefinSyncSettings: (settings: components["schemas"]["SimpleFinSyncSettings"]) => wrap<null>(raw.POST("/api/rpc/set_simplefin_sync_settings" as never, { body: { settings } as never } as never) as never),
  listSimplefinAlerts: () => wrap<components["schemas"]["SimpleFinAlert"][]>(raw.POST("/api/rpc/list_simplefin_alerts" as never, {} as never) as never),
  acknowledgeSimplefinAlert: (alertId: string) => wrap<null>(raw.POST("/api/rpc/acknowledge_simplefin_alert" as never, { body: { alertId } as never } as never) as never),
  listSimplefinTransferSuggestions: () => wrap<components["schemas"]["TransferSuggestionInfo"][]>(raw.POST("/api/rpc/list_simplefin_transfer_suggestions" as never, {} as never) as never),
  confirmSimplefinTransfer: (transferId: string) => wrap<null>(raw.POST("/api/rpc/confirm_simplefin_transfer" as never, { body: { transferId } as never } as never) as never),
  rejectSimplefinTransfer: (transferId: string) => wrap<null>(raw.POST("/api/rpc/reject_simplefin_transfer" as never, { body: { transferId } as never } as never) as never),
  listImportReviewCandidates: () => wrap<components["schemas"]["ImportCandidateWithMatches"][]>(raw.POST("/api/rpc/list_import_review_candidates" as never, {} as never) as never),
  acceptImportCandidateMatch: (candidateId: string, transactionId: string) => wrap<null>(raw.POST("/api/rpc/accept_import_candidate_match" as never, { body: { candidateId, transactionId } as never } as never) as never),
  createImportCandidateTransaction: (candidateId: string) => wrap<string>(raw.POST("/api/rpc/create_import_candidate_transaction" as never, { body: { candidateId } as never } as never) as never),
  dismissImportCandidate: (candidateId: string) => wrap<null>(raw.POST("/api/rpc/dismiss_import_candidate" as never, { body: { candidateId } as never } as never) as never),
  streamCopilotMessage: (conversationId: string, runId: string, text: string, history: components["schemas"]["ChatHistoryEntry"][], sourceMessageId: string | null) => wrap<string>(raw.POST("/api/rpc/stream_copilot_message" as never, { body: { conversationId, runId, text, history, sourceMessageId } as never } as never) as never),
  listConversations: () => wrap<components["schemas"]["ConversationSummary"][]>(raw.POST("/api/rpc/list_conversations" as never, {} as never) as never),
  getConversationMessages: (conversationId: string) => wrap<components["schemas"]["ConversationMessage"][]>(raw.POST("/api/rpc/get_conversation_messages" as never, { body: { conversationId } as never } as never) as never),
  deleteConversation: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_conversation" as never, { body: { id } as never } as never) as never),
  createConversation: () => wrap<string>(raw.POST("/api/rpc/create_conversation" as never, {} as never) as never),
  editConversationUserMessage: (input: components["schemas"]["EditConversationMessageInput"]) => wrap<null>(raw.POST("/api/rpc/edit_conversation_user_message" as never, { body: { input } as never } as never) as never),
  deleteConversationMessagesAfter: (conversationId: string, messageId: string) => wrap<number>(raw.POST("/api/rpc/delete_conversation_messages_after" as never, { body: { conversationId, messageId } as never } as never) as never),
  // generic fallback for dynamic commands (used by hooks that haven't migrated yet)
  rpc: <T>(cmd: string, body: unknown) =>
    wrap<T>(raw.POST(`/api/rpc/${cmd}` as never, { body: body as never } as never) as never),
};


export const commands = api;

// Re-export all schemas as top-level types for backward compat with old bindings imports
export type Account = components["schemas"]["Account"];
export type AccountBalancePoint = components["schemas"]["AccountBalancePoint"];
export type AccountBalanceTimeline = components["schemas"]["AccountBalanceTimeline"];
export type AccountOwner = components["schemas"]["AccountOwner"];
export type AccountPatch = components["schemas"]["AccountPatch"];
export type AccountSparkline = components["schemas"]["AccountSparkline"];
export type AccountSummary = components["schemas"]["AccountSummary"];
export type AccountSyncResult = components["schemas"]["AccountSyncResult"];
export type AccountType = components["schemas"]["AccountType"];
export type ActionItem = components["schemas"]["ActionItem"];
export type AgentAccountRow = components["schemas"]["AgentAccountRow"];
export type AgentAccountsOverviewBlock = components["schemas"]["AgentAccountsOverviewBlock"];
export type AgentActionBundle = components["schemas"]["AgentActionBundle"];
export type AgentActionItem = components["schemas"]["AgentActionItem"];
export type AgentActionPlanBlock = components["schemas"]["AgentActionPlanBlock"];
export type AgentActivity = components["schemas"]["AgentActivity"];
export type AgentAffordabilityVerdictBlock = components["schemas"]["AgentAffordabilityVerdictBlock"];
export type AgentAllocationSegment = components["schemas"]["AgentAllocationSegment"];
export type AgentAllocationSplitBlock = components["schemas"]["AgentAllocationSplitBlock"];
export type AgentAnswer = components["schemas"]["AgentAnswer"];
export type AgentCategoryBreakdownBlock = components["schemas"]["AgentCategoryBreakdownBlock"];
export type AgentCategoryReviewQueueBlock = components["schemas"]["AgentCategoryReviewQueueBlock"];
export type AgentCategoryRow = components["schemas"]["AgentCategoryRow"];
export type AgentChange = components["schemas"]["AgentChange"];
export type AgentChartBlock = components["schemas"]["AgentChartBlock"];
export type AgentChartPoint = components["schemas"]["AgentChartPoint"];
export type AgentClarificationBlock = components["schemas"]["AgentClarificationBlock"];
export type AgentClarificationOption = components["schemas"]["AgentClarificationOption"];
export type AgentComparisonBarsBlock = components["schemas"]["AgentComparisonBarsBlock"];
export type AgentDriver = components["schemas"]["AgentDriver"];
export type AgentExecutionEntry = components["schemas"]["AgentExecutionEntry"];
export type AgentFundingSource = components["schemas"]["AgentFundingSource"];
export type AgentMemory = components["schemas"]["AgentMemory"];
export type AgentMetricBlock = components["schemas"]["AgentMetricBlock"];
export type AgentMoneyPoint = components["schemas"]["AgentMoneyPoint"];
export type AgentNavigationTarget = components["schemas"]["AgentNavigationTarget"];
export type AgentRankedOption = components["schemas"]["AgentRankedOption"];
export type AgentRankedOptionsBlock = components["schemas"]["AgentRankedOptionsBlock"];
export type AgentRecatRow = components["schemas"]["AgentRecatRow"];
export type AgentRecategorizationPreviewBlock = components["schemas"]["AgentRecategorizationPreviewBlock"];
export type AgentRecipe = components["schemas"]["AgentRecipe"];
export type AgentRecipeRun = components["schemas"]["AgentRecipeRun"];
export type AgentResponseBlock = components["schemas"]["AgentResponseBlock"];
export type AgentReviewCategory = components["schemas"]["AgentReviewCategory"];
export type AgentReviewMonth = components["schemas"]["AgentReviewMonth"];
export type AgentReviewQueueItem = components["schemas"]["AgentReviewQueueItem"];
export type AgentScenarioAlternative = components["schemas"]["AgentScenarioAlternative"];
export type AgentSession = components["schemas"]["AgentSession"];
export type AgentSpendTimelineBlock = components["schemas"]["AgentSpendTimelineBlock"];
export type AgentSpendingDriversBlock = components["schemas"]["AgentSpendingDriversBlock"];
export type AgentSpendingReviewBlock = components["schemas"]["AgentSpendingReviewBlock"];
export type AgentStatus = components["schemas"]["AgentStatus"];
export type AgentTableBlock = components["schemas"]["AgentTableBlock"];
export type AgentTimelinePoint = components["schemas"]["AgentTimelinePoint"];
export type AgentTransactionTableBlock = components["schemas"]["AgentTransactionTableBlock"];
export type AgentTxRow = components["schemas"]["AgentTxRow"];
export type AgentTxnSearchQuery = components["schemas"]["AgentTxnSearchQuery"];
export type AgentWatchItem = components["schemas"]["AgentWatchItem"];
export type AgentWatchListBlock = components["schemas"]["AgentWatchListBlock"];
export type AmountConvention = components["schemas"]["AmountConvention"];
export type AppReady = components["schemas"]["AppReady"];
export type ApplyScenarioResult = components["schemas"]["ApplyScenarioResult"];
export type AssetOwner = components["schemas"]["AssetOwner"];
export type BackupInfo = components["schemas"]["BackupInfo"];
export type BalanceAnchorQuality = components["schemas"]["BalanceAnchorQuality"];
export type BaselineSummary = components["schemas"]["BaselineSummary"];
export type BudgetEnvelope = components["schemas"]["BudgetEnvelope"];
export type CashflowDay = components["schemas"]["CashflowDay"];
export type CashflowEvent = components["schemas"]["CashflowEvent"];
export type CashflowEventKind = components["schemas"]["CashflowEventKind"];
export type CashflowForecast = components["schemas"]["CashflowForecast"];
export type CashflowWarning = components["schemas"]["CashflowWarning"];
export type Categorization = components["schemas"]["Categorization"];
export type Category = components["schemas"]["Category"];
export type CategoryDto = components["schemas"]["CategoryDto"];
export type CategoryExample = components["schemas"]["CategoryExample"];
export type CategoryGroup = components["schemas"]["CategoryGroup"];
export type CategoryHistory = components["schemas"]["CategoryHistory"];
export type CategoryPlanRow = components["schemas"]["CategoryPlanRow"];
export type CategoryProposal = components["schemas"]["CategoryProposal"];
export type CategoryTotal = components["schemas"]["CategoryTotal"];
export type CategoryWithSpending = components["schemas"]["CategoryWithSpending"];
export type ChatHistoryEntry = components["schemas"]["ChatHistoryEntry"];
export type CloseFlag = components["schemas"]["CloseFlag"];
export type ColumnRole = components["schemas"]["ColumnRole"];
export type CompletionProviderConfig = components["schemas"]["CompletionProviderConfig"];
export type ConversationMessage = components["schemas"]["ConversationMessage"];
export type ConversationSummary = components["schemas"]["ConversationSummary"];
export type CopilotStreamFrame = components["schemas"]["CopilotStreamFrame"];
export type CounterpartyVerdict = components["schemas"]["CounterpartyVerdict"];
export type CsvImportMapping = components["schemas"]["CsvImportMapping"];
export type CsvPreview = components["schemas"]["CsvPreview"];
export type DataHealth = components["schemas"]["DataHealth"];
export type DebtPayoffMonth = components["schemas"]["DebtPayoffMonth"];
export type DebtPayoffResult = components["schemas"]["DebtPayoffResult"];
export type DebtPayoffSummary = components["schemas"]["DebtPayoffSummary"];
export type DigestFrequency = components["schemas"]["DigestFrequency"];
export type Disposition = components["schemas"]["Disposition"];
export type DriftLine = components["schemas"]["DriftLine"];
export type Driver = components["schemas"]["Driver"];
export type EditConversationMessageInput = components["schemas"]["EditConversationMessageInput"];
export type ExecutionItemResult = components["schemas"]["ExecutionItemResult"];
export type ExecutionSummary = components["schemas"]["ExecutionSummary"];
export type FinancialAssumptionsInput = components["schemas"]["FinancialAssumptionsInput"];
export type FinancialMetrics = components["schemas"]["FinancialMetrics"];
export type FinancialPhilosophyDto = components["schemas"]["FinancialPhilosophyDto"];
export type GoalContributionDto = components["schemas"]["GoalContributionDto"];
export type GoalDto = components["schemas"]["GoalDto"];
export type HealthScore = components["schemas"]["HealthScore"];
export type HealthScoreBreakdown = components["schemas"]["HealthScoreBreakdown"];
export type Holding = components["schemas"]["Holding"];
export type HouseholdMember = components["schemas"]["HouseholdMember"];
export type Import = components["schemas"]["Import"];
export type ImportCandidate = components["schemas"]["ImportCandidate"];
export type ImportCandidateMatch = components["schemas"]["ImportCandidateMatch"];
export type ImportCandidateWithMatches = components["schemas"]["ImportCandidateWithMatches"];
export type ImportResult = components["schemas"]["ImportResult"];
export type ImportSource = components["schemas"]["ImportSource"];
export type ImportSummary = components["schemas"]["ImportSummary"];
export type InboxBadgeCount = components["schemas"]["InboxBadgeCount"];
export type Institution = components["schemas"]["Institution"];
export type InvestmentSummary = components["schemas"]["InvestmentSummary"];
export type JourneyMilestone = components["schemas"]["JourneyMilestone"];
export type JourneyStatus = components["schemas"]["JourneyStatus"];
export type LlmProviderConfig = components["schemas"]["LlmProviderConfig"];
export type LookBackFact = components["schemas"]["LookBackFact"];
export type ManualAsset = components["schemas"]["ManualAsset"];
export type ManualAssetPatch = components["schemas"]["ManualAssetPatch"];
export type Mechanism = components["schemas"]["Mechanism"];
export type MemberBudgetEnvelope = components["schemas"]["MemberBudgetEnvelope"];
export type MemberNetWorth = components["schemas"]["MemberNetWorth"];
export type MerchantTotal = components["schemas"]["MerchantTotal"];
export type MetricAssumption = components["schemas"]["MetricAssumption"];
export type MetricExplanation = components["schemas"]["MetricExplanation"];
export type MetricInput = components["schemas"]["MetricInput"];
export type MetricValue = components["schemas"]["MetricValue"];
export type MetricWarning = components["schemas"]["MetricWarning"];
export type MetricWarningLevel = components["schemas"]["MetricWarningLevel"];
export type MissingDataItem = components["schemas"]["MissingDataItem"];
export type MonthCloseListItem = components["schemas"]["MonthCloseListItem"];
export type MonthCloseSnapshot = components["schemas"]["MonthCloseSnapshot"];
export type MonthCloseView = components["schemas"]["MonthCloseView"];
export type MonthSummary = components["schemas"]["MonthSummary"];
export type MonthTotals = components["schemas"]["MonthTotals"];
export type MonthlyActual = components["schemas"]["MonthlyActual"];
export type NetWorthPoint = components["schemas"]["NetWorthPoint"];
export type NewAccount = components["schemas"]["NewAccount"];
export type NewGoalInput = components["schemas"]["NewGoalInput"];
export type NewInstitution = components["schemas"]["NewInstitution"];
export type NewManualAsset = components["schemas"]["NewManualAsset"];
export type NewPlannedTransaction = components["schemas"]["NewPlannedTransaction"];
export type NewSimpleFinConnection = components["schemas"]["NewSimpleFinConnection"];
export type NewTransaction = components["schemas"]["NewTransaction"];
export type Notification = components["schemas"]["Notification"];
export type NotificationCategory = components["schemas"]["NotificationCategory"];
export type NotificationCategoryPref = components["schemas"]["NotificationCategoryPref"];
export type NotificationPrefsDto = components["schemas"]["NotificationPrefsDto"];
export type OllamaProbeResult = components["schemas"]["OllamaProbeResult"];
export type OnboardingState = components["schemas"]["OnboardingState"];
export type OwnerShare = components["schemas"]["OwnerShare"];
export type PathBackView = components["schemas"]["PathBackView"];
export type PeriodAssessment = components["schemas"]["PeriodAssessment"];
export type PeriodClass = components["schemas"]["PeriodClass"];
export type Persistence = components["schemas"]["Persistence"];
export type PlanAssignment = components["schemas"]["PlanAssignment"];
export type PlanChange = components["schemas"]["PlanChange"];
export type PlanData = components["schemas"]["PlanData"];
export type PlannedTransaction = components["schemas"]["PlannedTransaction"];
export type PlannedTransactionPatch = components["schemas"]["PlannedTransactionPatch"];
export type PlannedTxnFilter = components["schemas"]["PlannedTxnFilter"];
export type Position = components["schemas"]["Position"];
export type PreparedImportPreview = components["schemas"]["PreparedImportPreview"];
export type PriceChangeDto = components["schemas"]["PriceChangeDto"];
export type PrivacyLevel = components["schemas"]["PrivacyLevel"];
export type ProgressPayload = components["schemas"]["ProgressPayload"];
export type ProjectedValue = components["schemas"]["ProjectedValue"];
export type ProposedRule = components["schemas"]["ProposedRule"];
export type ProposedRuleDto = components["schemas"]["ProposedRuleDto"];
export type ProviderTestResult = components["schemas"]["ProviderTestResult"];
export type PushDeliveryReport = components["schemas"]["PushDeliveryReport"];
export type PushDevice = components["schemas"]["PushDevice"];
export type PushPayload = components["schemas"]["PushPayload"];
export type PushStatus = components["schemas"]["PushStatus"];
export type QuietHours = components["schemas"]["QuietHours"];
export type RanScenario = components["schemas"]["RanScenario"];
export type RecurringItem = components["schemas"]["RecurringItem"];
export type ReportData = components["schemas"]["ReportData"];
export type RestorationEnvelope = components["schemas"]["RestorationEnvelope"];
export type RestorationEnvelopeInput = components["schemas"]["RestorationEnvelopeInput"];
export type RestorationLeg = components["schemas"]["RestorationLeg"];
export type RestorationStatus = components["schemas"]["RestorationStatus"];
export type RowError = components["schemas"]["RowError"];
export type Rule = components["schemas"]["Rule"];
export type RuleProposal = components["schemas"]["RuleProposal"];
export type RuleWithCategory = components["schemas"]["RuleWithCategory"];
export type SaveMonthCloseInput = components["schemas"]["SaveMonthCloseInput"];
export type SavedScenarioDetail = components["schemas"]["SavedScenarioDetail"];
export type SavingsRatePoint = components["schemas"]["SavingsRatePoint"];
export type ScenarioParamsInput = components["schemas"]["ScenarioParamsInput"];
export type ScenarioPlanProposal = components["schemas"]["ScenarioPlanProposal"];
export type ScenarioResult = components["schemas"]["ScenarioResult"];
export type SearchTxnQueryInput = components["schemas"]["SearchTxnQueryInput"];
export type Security = components["schemas"]["Security"];
export type SimpleFinAccountImportRequest = components["schemas"]["SimpleFinAccountImportRequest"];
export type SimpleFinAccountInfo = components["schemas"]["SimpleFinAccountInfo"];
export type SimpleFinAlert = components["schemas"]["SimpleFinAlert"];
export type SimpleFinConnection = components["schemas"]["SimpleFinConnection"];
export type SimpleFinConnectionInfo = components["schemas"]["SimpleFinConnectionInfo"];
export type SimpleFinConnectionPatch = components["schemas"]["SimpleFinConnectionPatch"];
export type SimpleFinPurgeSummary = components["schemas"]["SimpleFinPurgeSummary"];
export type SimpleFinStatus = components["schemas"]["SimpleFinStatus"];
export type SimpleFinSyncSettings = components["schemas"]["SimpleFinSyncSettings"];
export type SkippedChange = components["schemas"]["SkippedChange"];
export type SpendingBreakdown = components["schemas"]["SpendingBreakdown"];
export type SpendingPlan = components["schemas"]["SpendingPlan"];
export type SplitInputDto = components["schemas"]["SplitInputDto"];
export type StarterCategory = components["schemas"]["StarterCategory"];
export type SyncRun = components["schemas"]["SyncRun"];
export type SyncSummary = components["schemas"]["SyncSummary"];
export type Transaction = components["schemas"]["Transaction"];
export type TransactionSplitDto = components["schemas"]["TransactionSplitDto"];
export type TransactionStatus = components["schemas"]["TransactionStatus"];
export type TransactionTransfer = components["schemas"]["TransactionTransfer"];
export type TransferSuggestionInfo = components["schemas"]["TransferSuggestionInfo"];
export type TransferVerdictResult = components["schemas"]["TransferVerdictResult"];
export type TxnActivity = components["schemas"]["TxnActivity"];
export type TxnFilterInput = components["schemas"]["TxnFilterInput"];
export type TxnPatch = components["schemas"]["TxnPatch"];
export type UnconvertedHolding = components["schemas"]["UnconvertedHolding"];
export type UnresolvedCounterpartyDto = components["schemas"]["UnresolvedCounterpartyDto"];
export type UpdateTxnResult = components["schemas"]["UpdateTxnResult"];
export type Urgency = components["schemas"]["Urgency"];
export type WarningLevel = components["schemas"]["WarningLevel"];

export { raw as openapiClient };
export const client = raw;
