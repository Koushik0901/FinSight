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

async function wrap<T>(p: Promise<any>): Promise<Result<T>> {
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
  listAccounts: () => wrap<components["schemas"]["AccountSummary"][]>(raw.POST("/api/rpc/list_accounts", {})),
  createAccount: (input: components["schemas"]["NewAccount"]) => wrap<components["schemas"]["Account"]>(raw.POST("/api/rpc/create_account", { body: { input } })),
  updateAccount: (id: string, patch: components["schemas"]["AccountPatch"]) => wrap<components["schemas"]["Account"]>(raw.POST("/api/rpc/update_account", { body: { id, patch } })),
  archiveAccount: (id: string) => wrap<null>(raw.POST("/api/rpc/archive_account", { body: { id } })),
  setAccountBalance: (id: string, balanceCents: number) => wrap<null>(raw.POST("/api/rpc/set_account_balance", { body: { id, balanceCents } })),
  updateCategoryColor: (id: string, color: string) => wrap<null>(raw.POST("/api/rpc/update_category_color", { body: { id, color } })),
  createCategory: (label: string, groupId: string | null, color: string) => wrap<components["schemas"]["Category"]>(raw.POST("/api/rpc/create_category", { body: { label, groupId, color } })),
  renameCategory: (id: string, label: string) => wrap<null>(raw.POST("/api/rpc/rename_category", { body: { id, label } })),
  archiveCategory: (id: string) => wrap<null>(raw.POST("/api/rpc/archive_category", { body: { id } })),
  setCategoryGuidance: (id: string, guidance: string | null) => wrap<null>(raw.POST("/api/rpc/set_category_guidance", { body: { id, guidance } })),
  listCategoryGroups: () => wrap<components["schemas"]["CategoryGroup"][]>(raw.POST("/api/rpc/list_category_groups", {})),
  createCategoryGroup: (label: string, hint: string | null) => wrap<components["schemas"]["CategoryGroup"]>(raw.POST("/api/rpc/create_category_group", { body: { label, hint } })),
  setCategoryGroup: (categoryId: string, groupId: string) => wrap<null>(raw.POST("/api/rpc/set_category_group", { body: { categoryId, groupId } })),
  addCategoryExample: (categoryId: string, exampleText: string, sourceTxnId: string | null) => wrap<components["schemas"]["CategoryExample"]>(raw.POST("/api/rpc/add_category_example", { body: { categoryId, exampleText, sourceTxnId } })),
  removeCategoryExample: (id: string) => wrap<null>(raw.POST("/api/rpc/remove_category_example", { body: { id } })),
  listCategoryExamples: (categoryId: string) => wrap<components["schemas"]["CategoryExample"][]>(raw.POST("/api/rpc/list_category_examples", { body: { categoryId } })),
  listTransactions: (filter: components["schemas"]["TxnFilterInput"]) => wrap<components["schemas"]["Transaction"][]>(raw.POST("/api/rpc/list_transactions", { body: { filter } })),
  createTransaction: (input: components["schemas"]["NewTransaction"]) => wrap<components["schemas"]["Transaction"]>(raw.POST("/api/rpc/create_transaction", { body: { input } })),
  updateTransaction: (id: string, patch: components["schemas"]["TxnPatch"]) => wrap<components["schemas"]["UpdateTxnResult"]>(raw.POST("/api/rpc/update_transaction", { body: { id, patch } })),
  deleteTransaction: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_transaction", { body: { id } })),
  createRule: (pattern: string, categoryId: string) => wrap<components["schemas"]["Rule"]>(raw.POST("/api/rpc/create_rule", { body: { pattern, categoryId } })),
  setTransactionOwner: (transactionId: string, memberId: string | null) => wrap<null>(raw.POST("/api/rpc/set_transaction_owner", { body: { transactionId, memberId } })),
  listCategories: () => wrap<components["schemas"]["CategoryDto"][]>(raw.POST("/api/rpc/list_categories", {})),
  setCategorySpendingType: (id: string, spendingType: string | null) => wrap<null>(raw.POST("/api/rpc/set_category_spending_type", { body: { id, spendingType } })),
  getSpendingBreakdown: () => wrap<components["schemas"]["SpendingBreakdown"]>(raw.POST("/api/rpc/get_spending_breakdown", {})),
  getOnboardingState: () => wrap<components["schemas"]["OnboardingState"]>(raw.POST("/api/rpc/get_onboarding_state", {})),
  markOnboardingComplete: () => wrap<null>(raw.POST("/api/rpc/mark_onboarding_complete", {})),
  resetOnboardingCompletion: () => wrap<null>(raw.POST("/api/rpc/reset_onboarding_completion", {})),
  commitStarterCategories: (categories: components["schemas"]["StarterCategory"][]) => wrap<null>(raw.POST("/api/rpc/commit_starter_categories", { body: { categories } })),
  probeOllama: (baseUrl: string) => wrap<components["schemas"]["OllamaProbeResult"]>(raw.POST("/api/rpc/probe_ollama", { body: { baseUrl } })),
  saveLlmProvider: (config: components["schemas"]["LlmProviderConfig"]) => wrap<null>(raw.POST("/api/rpc/save_llm_provider", { body: { config } })),
  appReady: () => wrap<components["schemas"]["AppReady"]>(raw.POST("/api/rpc/app_ready", {})),
  listAccountPositions: (accountId: string) => wrap<components["schemas"]["Position"][]>(raw.POST("/api/rpc/list_account_positions", { body: { accountId } })),
  getInvestmentSummary: (accountId: string) => wrap<components["schemas"]["InvestmentSummary"]>(raw.POST("/api/rpc/get_investment_summary", { body: { accountId } })),
  previewCsvColumns: (path: string, skipHeaderRows: number) => wrap<components["schemas"]["CsvPreview"]>(raw.POST("/api/rpc/preview_csv_columns", { body: { path, skipHeaderRows } })),
  prepareCsvImport: (path: string, accountId: string, mapping: components["schemas"]["CsvImportMapping"]) => wrap<components["schemas"]["PreparedImportPreview"]>(raw.POST("/api/rpc/prepare_csv_import", { body: { path, accountId, mapping } })),
  importCsv: (path: string, accountId: string, mapping: components["schemas"]["CsvImportMapping"]) => wrap<components["schemas"]["ImportResult"]>(raw.POST("/api/rpc/import_csv", { body: { path, accountId, mapping } })),
  getSavedCsvMapping: (accountId: string) => wrap<components["schemas"]["CsvImportMapping"] | null>(raw.POST("/api/rpc/get_saved_csv_mapping", { body: { accountId } })),
  listUnfinishedImports: () => wrap<components["schemas"]["Import"][]>(raw.POST("/api/rpc/list_unfinished_imports", {})),
  discardUnfinishedImport: (importId: string) => wrap<null>(raw.POST("/api/rpc/discard_unfinished_import", { body: { importId } })),
  setCompletionProvider: (config: components["schemas"]["CompletionProviderConfig"]) => wrap<null>(raw.POST("/api/rpc/set_completion_provider", { body: { config } })),
  getCompletionProvider: () => wrap<components["schemas"]["CompletionProviderConfig"]>(raw.POST("/api/rpc/get_completion_provider", {})),
  saveProviderApiKey: (providerId: string, key: string) => wrap<null>(raw.POST("/api/rpc/save_provider_api_key", { body: { providerId, key } })),
  listProviderModels: (config: components["schemas"]["CompletionProviderConfig"]) => wrap<string[]>(raw.POST("/api/rpc/list_provider_models", { body: { config } })),
  testCompletionProvider: (config: components["schemas"]["CompletionProviderConfig"], apiKey: string | null) => wrap<components["schemas"]["ProviderTestResult"]>(raw.POST("/api/rpc/test_completion_provider", { body: { config, apiKey } })),
  getNeedsReviewCount: () => wrap<number>(raw.POST("/api/rpc/get_needs_review_count", {})),
  triggerCategorize: () => wrap<null>(raw.POST("/api/rpc/trigger_categorize", {})),
  recomputeAnomalies: () => wrap<number>(raw.POST("/api/rpc/recompute_anomalies", {})),
  setAnomalyDismissed: (txnId: string, dismissed: boolean) => wrap<null>(raw.POST("/api/rpc/set_anomaly_dismissed", { body: { txnId, dismissed } })),
  triggerRecategorizeLowConfidence: () => wrap<null>(raw.POST("/api/rpc/trigger_recategorize_low_confidence", {})),
  getAgentStatus: () => wrap<components["schemas"]["AgentStatus"]>(raw.POST("/api/rpc/get_agent_status", {})),
  askAgent: (question: string, mode: string | null) => wrap<components["schemas"]["AgentAnswer"]>(raw.POST("/api/rpc/ask_agent", { body: { question, mode } })),
  listCategoriesWithSpending: () => wrap<components["schemas"]["CategoryWithSpending"][]>(raw.POST("/api/rpc/list_categories_with_spending", {})),
  listRulesWithCategories: () => wrap<components["schemas"]["RuleWithCategory"][]>(raw.POST("/api/rpc/list_rules_with_categories", {})),
  toggleRule: (id: string, enabled: boolean) => wrap<null>(raw.POST("/api/rpc/toggle_rule", { body: { id, enabled } })),
  listBudgetEnvelopes: () => wrap<components["schemas"]["BudgetEnvelope"][]>(raw.POST("/api/rpc/list_budget_envelopes", {})),
  listMemberBudgetEnvelopes: (memberId: string) => wrap<components["schemas"]["MemberBudgetEnvelope"][]>(raw.POST("/api/rpc/list_member_budget_envelopes", { body: { memberId } })),
  setBudget: (categoryId: string, amountCents: number) => wrap<null>(raw.POST("/api/rpc/set_budget", { body: { categoryId, amountCents } })),
   getHold: (month: string) => wrap<components["schemas"]["BudgetHold"] | null>(raw.POST("/api/rpc/get_hold", { body: { month } })),
   setHold: (month: string, amountCents: number) => wrap<components["schemas"]["BudgetHold"]>(raw.POST("/api/rpc/set_hold", { body: { month, amountCents } })),
   listFundingTemplates: () => wrap<components["schemas"]["FundingTemplate"][]>(raw.POST("/api/rpc/list_funding_templates", {})),
   createFundingTemplate: (categoryId: string, kind: string, paramsJson: string | null, priority: number | null) => wrap<components["schemas"]["FundingTemplate"]>(raw.POST("/api/rpc/create_funding_template", { body: { categoryId, kind, paramsJson, priority } })),
   updateFundingTemplate: (id: string, categoryId: string | null, kind: string | null, paramsJson: string | null, priority: number | null) => wrap<components["schemas"]["FundingTemplate"] | null>(raw.POST("/api/rpc/update_funding_template", { body: { id, categoryId, kind, paramsJson, priority } })),
   deleteFundingTemplate: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_funding_template", { body: { id } })),
   applyTemplates: (month: string) => wrap<components["schemas"]["BudgetChange"][]>(raw.POST("/api/rpc/apply_templates", { body: { month } })),
   listBudgetTransfers: (month: string) => wrap<components["schemas"]["BudgetTransfer"][]>(raw.POST("/api/rpc/list_budget_transfers", { body: { month } })),
    transferBudget: (fromCategory: string | null, toCategory: string | null, amountCents: number, month: string, note: string | null) => wrap<components["schemas"]["BudgetTransfer"]>(raw.POST("/api/rpc/transfer_budget", { body: { fromCategory, toCategory, amountCents, month, note } })),
    listGoals: () => wrap<components["schemas"]["GoalDto"][]>(raw.POST("/api/rpc/list_goals", {})),
  createGoal: (input: components["schemas"]["NewGoalInput"]) => wrap<components["schemas"]["GoalDto"]>(raw.POST("/api/rpc/create_goal", { body: { input } })),
  updateGoalBalance: (id: string, currentCents: number) => wrap<null>(raw.POST("/api/rpc/update_goal_balance", { body: { id, currentCents } })),
  contributeToGoal: (id: string, amountCents: number, note: string | null, source: string | null) => wrap<components["schemas"]["GoalContributionDto"]>(raw.POST("/api/rpc/contribute_to_goal", { body: { id, amountCents, note, source } })),
  listGoalContributions: (goalId: string) => wrap<components["schemas"]["GoalContributionDto"][]>(raw.POST("/api/rpc/list_goal_contributions", { body: { goalId } })),
  archiveGoal: (id: string) => wrap<null>(raw.POST("/api/rpc/archive_goal", { body: { id } })),
  projectGoalGrowth: (goalId: string, years: number) => wrap<components["schemas"]["ProjectedValue"]>(raw.POST("/api/rpc/project_goal_growth", { body: { goalId, years } })),
  listRecurring: () => wrap<components["schemas"]["RecurringItem"][]>(raw.POST("/api/rpc/list_recurring", {})),
  setSubscriptionVerdict: (merchantKey: string, verdict: string | null) => wrap<null>(raw.POST("/api/rpc/set_subscription_verdict", { body: { merchantKey, verdict } })),
  setSubscriptionTrial: (merchantKey: string, label: string, trialEndsAt: string | null) => wrap<null>(raw.POST("/api/rpc/set_subscription_trial", { body: { merchantKey, label, trialEndsAt } })),
  markSubscriptionCancelled: (merchantKey: string, label: string, cancelledAt: string) => wrap<null>(raw.POST("/api/rpc/mark_subscription_cancelled", { body: { merchantKey, label, cancelledAt } })),
  getReportData: (scope: string, memberId: string | null) => wrap<components["schemas"]["ReportData"]>(raw.POST("/api/rpc/get_report_data", { body: { scope, memberId } })),
  getMonthTotals: () => wrap<components["schemas"]["MonthTotals"]>(raw.POST("/api/rpc/get_month_totals", {})),
  getSavingsRateHistory: () => wrap<components["schemas"]["SavingsRatePoint"][]>(raw.POST("/api/rpc/get_savings_rate_history", {})),
  customReport: (params: components["schemas"]["CustomReportParams"]) => wrap<components["schemas"]["CustomReportResult"]>(raw.POST("/api/rpc/custom_report", { body: { params } })),
  getMonthClose: (year: number, month: number) => wrap<components["schemas"]["MonthCloseView"]>(raw.POST("/api/rpc/get_month_close", { body: { year, month } })),
  saveMonthClose: (input: components["schemas"]["SaveMonthCloseInput"]) => wrap<components["schemas"]["MonthCloseView"]>(raw.POST("/api/rpc/save_month_close", { body: { input } })),
  listMonthCloses: () => wrap<components["schemas"]["MonthCloseListItem"][]>(raw.POST("/api/rpc/list_month_closes", {})),
  getSpendingPathBack: (period: string | null, targetMonthlyCents: number | null) => wrap<components["schemas"]["PathBackView"] | null>(raw.POST("/api/rpc/get_spending_path_back", { body: { period, targetMonthlyCents } })),
  setSpendingAnnotation: (merchantKey: string, verdict: string) => wrap<null>(raw.POST("/api/rpc/set_spending_annotation", { body: { merchantKey, verdict } })),
  getFinancialMetrics: (memberId: string | null) => wrap<components["schemas"]["FinancialMetrics"]>(raw.POST("/api/rpc/get_financial_metrics", { body: { memberId } })),
  explainFinancialMetrics: (memberId: string | null) => wrap<components["schemas"]["MetricExplanation"][]>(raw.POST("/api/rpc/explain_financial_metrics", { body: { memberId } })),
  explainGoals: () => wrap<components["schemas"]["MetricExplanation"][]>(raw.POST("/api/rpc/explain_goals", {})),
  getCashflowForecast: (horizonDays: number | null, bufferCents: number | null, extraExpenseCents: number | null, extraExpenseDate: string | null) => wrap<components["schemas"]["CashflowForecast"]>(raw.POST("/api/rpc/get_cashflow_forecast", { body: { horizonDays, bufferCents, extraExpenseCents, extraExpenseDate } })),
  getNotificationPrefs: () => wrap<components["schemas"]["NotificationPrefsDto"]>(raw.POST("/api/rpc/get_notification_prefs", {})),
  setNotificationPrefs: (prefs: components["schemas"]["NotificationPrefsDto"]) => wrap<null>(raw.POST("/api/rpc/set_notification_prefs", { body: { prefs } })),
  listNotifications: (includeResolved: boolean | null) => wrap<components["schemas"]["Notification"][]>(raw.POST("/api/rpc/list_notifications", { body: { includeResolved } })),
  markNotificationRead: (id: string) => wrap<null>(raw.POST("/api/rpc/mark_notification_read", { body: { id } })),
  markAllNotificationsRead: () => wrap<number>(raw.POST("/api/rpc/mark_all_notifications_read", {})),
  notificationUnreadCount: () => wrap<number>(raw.POST("/api/rpc/notification_unread_count", {})),
  householdNetWorthBreakdown: () => wrap<components["schemas"]["MemberNetWorth"][]>(raw.POST("/api/rpc/household_net_worth_breakdown", {})),
  setFinancialAssumptions: (input: components["schemas"]["FinancialAssumptionsInput"]) => wrap<null>(raw.POST("/api/rpc/set_financial_assumptions", { body: { input } })),
  listRestorationEnvelopes: () => wrap<components["schemas"]["RestorationEnvelope"][]>(raw.POST("/api/rpc/list_restoration_envelopes", {})),
  getRestorationStatus: (id: string) => wrap<components["schemas"]["RestorationStatus"] | null>(raw.POST("/api/rpc/get_restoration_status", { body: { id } })),
  createRestorationEnvelope: (input: components["schemas"]["RestorationEnvelopeInput"]) => wrap<components["schemas"]["RestorationEnvelope"]>(raw.POST("/api/rpc/create_restoration_envelope", { body: { input } })),
  closeRestorationEnvelope: (id: string) => wrap<null>(raw.POST("/api/rpc/close_restoration_envelope", { body: { id } })),
  deleteRestorationEnvelope: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_restoration_envelope", { body: { id } })),
  addRestorationLeg: (envelopeId: string, amountCents: number, notedOn: string, transactionId: string | null) => wrap<components["schemas"]["RestorationLeg"]>(raw.POST("/api/rpc/add_restoration_leg", { body: { envelopeId, amountCents, notedOn, transactionId } })),
  removeRestorationLeg: (legId: string) => wrap<null>(raw.POST("/api/rpc/remove_restoration_leg", { body: { legId } })),
  getFinancialPhilosophy: () => wrap<components["schemas"]["FinancialPhilosophyDto"]>(raw.POST("/api/rpc/get_financial_philosophy", {})),
  setFinancialPhilosophy: (input: components["schemas"]["FinancialPhilosophyDto"]) => wrap<null>(raw.POST("/api/rpc/set_financial_philosophy", { body: { input } })),
  runScenario: (description: string, months: number, params: components["schemas"]["ScenarioParamsInput"] | null) => wrap<components["schemas"]["RanScenario"]>(raw.POST("/api/rpc/run_scenario", { body: { description, months, params } })),
  saveScenario: (description: string, params: components["schemas"]["ScenarioParamsInput"], months: number) => wrap<components["schemas"]["SavedScenarioDetail"]>(raw.POST("/api/rpc/save_scenario", { body: { description, params, months } })),
  listSavedScenarios: () => wrap<components["schemas"]["SavedScenarioDetail"][]>(raw.POST("/api/rpc/list_saved_scenarios", {})),
  duplicateScenario: (id: string) => wrap<components["schemas"]["SavedScenarioDetail"] | null>(raw.POST("/api/rpc/duplicate_scenario", { body: { id } })),
  archiveScenario: (id: string, archived: boolean) => wrap<null>(raw.POST("/api/rpc/archive_scenario", { body: { id, archived } })),
  promoteScenario: (id: string) => wrap<components["schemas"]["ScenarioPlanProposal"]>(raw.POST("/api/rpc/promote_scenario", { body: { id } })),
  applyScenario: (id: string, approvedChangeIds: string[]) => wrap<components["schemas"]["ApplyScenarioResult"]>(raw.POST("/api/rpc/apply_scenario", { body: { id, approvedChangeIds } })),
  reviseScenario: (id: string, params: components["schemas"]["ScenarioParamsInput"]) => wrap<components["schemas"]["SavedScenarioDetail"]>(raw.POST("/api/rpc/revise_scenario", { body: { id, params } })),
  clearScenarioRevision: (id: string) => wrap<components["schemas"]["SavedScenarioDetail"]>(raw.POST("/api/rpc/clear_scenario_revision", { body: { id } })),
  explainScenario: (id: string) => wrap<components["schemas"]["MetricExplanation"]>(raw.POST("/api/rpc/explain_scenario", { body: { id } })),
  deleteScenario: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_scenario", { body: { id } })),
  getTransactionCount: () => wrap<number>(raw.POST("/api/rpc/get_transaction_count", {})),
  listManualAssets: () => wrap<components["schemas"]["ManualAsset"][]>(raw.POST("/api/rpc/list_manual_assets", {})),
  createManualAsset: (input: components["schemas"]["NewManualAsset"]) => wrap<components["schemas"]["ManualAsset"]>(raw.POST("/api/rpc/create_manual_asset", { body: { input } })),
  updateManualAsset: (id: string, patch: components["schemas"]["ManualAssetPatch"]) => wrap<components["schemas"]["ManualAsset"]>(raw.POST("/api/rpc/update_manual_asset", { body: { id, patch } })),
  deleteManualAsset: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_manual_asset", { body: { id } })),
  recordNetWorthSnapshot: () => wrap<null>(raw.POST("/api/rpc/record_net_worth_snapshot", {})),
  listNetWorthHistory: (days: number) => wrap<components["schemas"]["NetWorthPoint"][]>(raw.POST("/api/rpc/list_net_worth_history", { body: { days } })),
  computeDebtPayoff: (extraMonthlyCents: number) => wrap<components["schemas"]["DebtPayoffResult"][]>(raw.POST("/api/rpc/compute_debt_payoff", { body: { extraMonthlyCents } })),
  getUncelebratedMilestones: () => wrap<number[]>(raw.POST("/api/rpc/get_uncelebrated_milestones", {})),
  listHouseholdMembers: () => wrap<components["schemas"]["HouseholdMember"][]>(raw.POST("/api/rpc/list_household_members", {})),
  createHouseholdMember: (name: string, color: string | null) => wrap<components["schemas"]["HouseholdMember"]>(raw.POST("/api/rpc/create_household_member", { body: { name, color } })),
  setSelfMember: (memberId: string) => wrap<null>(raw.POST("/api/rpc/set_self_member", { body: { memberId } })),
  deleteHouseholdMember: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_household_member", { body: { id } })),
  listAccountOwners: () => wrap<components["schemas"]["AccountOwner"][]>(raw.POST("/api/rpc/list_account_owners", {})),
  setAccountOwners: (accountId: string, memberIds: string[]) => wrap<null>(raw.POST("/api/rpc/set_account_owners", { body: { accountId, memberIds } })),
  setAccountOwnerShares: (accountId: string, owners: components["schemas"]["OwnerShare"][]) => wrap<null>(raw.POST("/api/rpc/set_account_owner_shares", { body: { accountId, owners } })),
  listAssetOwners: () => wrap<components["schemas"]["AssetOwner"][]>(raw.POST("/api/rpc/list_asset_owners", {})),
  setAssetOwners: (assetId: string, owners: components["schemas"]["OwnerShare"][]) => wrap<null>(raw.POST("/api/rpc/set_asset_owners", { body: { assetId, owners } })),
  getDataHealth: () => wrap<components["schemas"]["DataHealth"]>(raw.POST("/api/rpc/get_data_health", {})),
  createManualBackup: () => wrap<components["schemas"]["BackupInfo"]>(raw.POST("/api/rpc/create_manual_backup", {})),
  stageRestoreBackup: (path: string) => wrap<null>(raw.POST("/api/rpc/stage_restore_backup", { body: { path } })),
  cancelStagedRestore: () => wrap<null>(raw.POST("/api/rpc/cancel_staged_restore", {})),
  listAgentMemory: () => wrap<components["schemas"]["AgentMemory"][]>(raw.POST("/api/rpc/list_agent_memory", {})),
  forgetAgentMemory: (id: string) => wrap<null>(raw.POST("/api/rpc/forget_agent_memory", { body: { id } })),
  getFinancialHealthScore: () => wrap<components["schemas"]["HealthScore"]>(raw.POST("/api/rpc/get_financial_health_score", {})),
  listRuleProposals: () => wrap<components["schemas"]["RuleProposal"][]>(raw.POST("/api/rpc/list_rule_proposals", {})),
  acceptRuleProposal: (id: string) => wrap<null>(raw.POST("/api/rpc/accept_rule_proposal", { body: { id } })),
  declineRuleProposal: (id: string) => wrap<null>(raw.POST("/api/rpc/decline_rule_proposal", { body: { id } })),
  listCategoryProposals: () => wrap<components["schemas"]["CategoryProposal"][]>(raw.POST("/api/rpc/list_category_proposals", {})),
  acceptCategoryProposal: (id: string) => wrap<components["schemas"]["UpdateTxnResult"]>(raw.POST("/api/rpc/accept_category_proposal", { body: { id } })),
  correctCategoryProposal: (id: string, categoryId: string) => wrap<components["schemas"]["UpdateTxnResult"]>(raw.POST("/api/rpc/correct_category_proposal", { body: { id, categoryId } })),
  rejectCategoryProposal: (id: string) => wrap<null>(raw.POST("/api/rpc/reject_category_proposal", { body: { id } })),
  listAgentSessions: () => wrap<components["schemas"]["AgentSession"][]>(raw.POST("/api/rpc/list_agent_sessions", {})),
  createAgentSession: (title: string, taskType: string) => wrap<components["schemas"]["AgentSession"]>(raw.POST("/api/rpc/create_agent_session", { body: { title, taskType } })),
  closeAgentSession: (id: string) => wrap<null>(raw.POST("/api/rpc/close_agent_session", { body: { id } })),
  listActionBundles: (statusFilter: string | null, sessionId: string | null, limit: number | null) => wrap<components["schemas"]["AgentActionBundle"][]>(raw.POST("/api/rpc/list_action_bundles", { body: { statusFilter, sessionId, limit } })),
  getActionBundle: (id: string) => wrap<components["schemas"]["AgentActionBundle"] | null>(raw.POST("/api/rpc/get_action_bundle", { body: { id } })),
  approveActionItem: (itemId: string) => wrap<null>(raw.POST("/api/rpc/approve_action_item", { body: { itemId } })),
  rejectActionItem: (itemId: string) => wrap<null>(raw.POST("/api/rpc/reject_action_item", { body: { itemId } })),
  listExecutionLog: (bundleId: string) => wrap<components["schemas"]["AgentExecutionEntry"][]>(raw.POST("/api/rpc/list_execution_log", { body: { bundleId } })),
  executeActionBundle: (bundleId: string) => wrap<components["schemas"]["ExecutionSummary"]>(raw.POST("/api/rpc/execute_action_bundle", { body: { bundleId } })),
  listRecipes: (includePaused: boolean) => wrap<components["schemas"]["AgentRecipe"][]>(raw.POST("/api/rpc/list_recipes", { body: { includePaused } })),
  createRecipe: (title: string, description: string, recipeKind: string, promptTemplate: string, cadence: string, dayOfWeek: number | null, dayOfMonth: number | null) => wrap<components["schemas"]["AgentRecipe"]>(raw.POST("/api/rpc/create_recipe", { body: { title, description, recipeKind, promptTemplate, cadence, dayOfWeek, dayOfMonth } })),
  updateRecipe: (id: string, title: string, description: string, promptTemplate: string, cadence: string, dayOfWeek: number | null, dayOfMonth: number | null) => wrap<components["schemas"]["AgentRecipe"]>(raw.POST("/api/rpc/update_recipe", { body: { id, title, description, promptTemplate, cadence, dayOfWeek, dayOfMonth } })),
  pauseRecipe: (id: string) => wrap<null>(raw.POST("/api/rpc/pause_recipe", { body: { id } })),
  resumeRecipe: (id: string) => wrap<null>(raw.POST("/api/rpc/resume_recipe", { body: { id } })),
  deleteRecipe: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_recipe", { body: { id } })),
  triggerRecipe: (id: string) => wrap<string>(raw.POST("/api/rpc/trigger_recipe", { body: { id } })),
  listRecipeRuns: (recipeId: string, limit: number | null) => wrap<components["schemas"]["AgentRecipeRun"][]>(raw.POST("/api/rpc/list_recipe_runs", { body: { recipeId, limit } })),
  setTransactionFlags: (id: string, isReimbursable: boolean, isSplit: boolean) => wrap<components["schemas"]["Transaction"]>(raw.POST("/api/rpc/set_transaction_flags", { body: { id, isReimbursable, isSplit } })),
  setTransactionTransfer: (id: string, isTransfer: boolean) => wrap<components["schemas"]["TransferVerdictResult"]>(raw.POST("/api/rpc/set_transaction_transfer", { body: { id, isTransfer } })),
  applyTransferVerdictToSimilar: (pattern: string, isTransfer: boolean) => wrap<number>(raw.POST("/api/rpc/apply_transfer_verdict_to_similar", { body: { pattern, isTransfer } })),
  setCounterpartyVerdict: (id: string, verdict: components["schemas"]["CounterpartyVerdict"]) => wrap<components["schemas"]["Transaction"]>(raw.POST("/api/rpc/set_counterparty_verdict", { body: { id, verdict } })),
  applyCounterpartyVerdictToSimilar: (pattern: string, verdict: components["schemas"]["CounterpartyVerdict"]) => wrap<number>(raw.POST("/api/rpc/apply_counterparty_verdict_to_similar", { body: { pattern, verdict } })),
  listUnresolvedCounterparties: () => wrap<components["schemas"]["UnresolvedCounterpartyDto"][]>(raw.POST("/api/rpc/list_unresolved_counterparties", {})),
  getTransactionSplits: (transactionId: string) => wrap<components["schemas"]["TransactionSplitDto"][]>(raw.POST("/api/rpc/get_transaction_splits", { body: { transactionId } })),
  setTransactionSplits: (transactionId: string, splits: components["schemas"]["SplitInputDto"][]) => wrap<null>(raw.POST("/api/rpc/set_transaction_splits", { body: { transactionId, splits } })),
  updateGoalMonthly: (id: string, monthlyCents: number) => wrap<null>(raw.POST("/api/rpc/update_goal_monthly", { body: { id, monthlyCents } })),
  updateGoalPriority: (id: string, priority: string, deadlineStrictness: string) => wrap<null>(raw.POST("/api/rpc/update_goal_priority", { body: { id, priority, deadlineStrictness } })),
  updateGoalPurpose: (id: string, purpose: string | null) => wrap<null>(raw.POST("/api/rpc/update_goal_purpose", { body: { id, purpose } })),
  getCurrency: () => wrap<string>(raw.POST("/api/rpc/get_currency", {})),
  setCurrency: (currency: string) => wrap<null>(raw.POST("/api/rpc/set_currency", { body: { currency } })),
  deleteAllData: () => wrap<null>(raw.POST("/api/rpc/delete_all_data", {})),
  exportAllDataJson: () => wrap<string>(raw.POST("/api/rpc/export_all_data_json", {})),
  exportAllDataCsv: () => wrap<string>(raw.POST("/api/rpc/export_all_data_csv", {})),
  getNotificationsEnabled: () => wrap<boolean>(raw.POST("/api/rpc/get_notifications_enabled", {})),
  setNotificationsEnabled: (enabled: boolean) => wrap<null>(raw.POST("/api/rpc/set_notifications_enabled", { body: { enabled } })),
  getAutoCategorizeEnabled: () => wrap<boolean>(raw.POST("/api/rpc/get_auto_categorize_enabled", {})),
  setAutoCategorizeEnabled: (enabled: boolean) => wrap<null>(raw.POST("/api/rpc/set_auto_categorize_enabled", { body: { enabled } })),
  getPlanNextMonthData: () => wrap<components["schemas"]["PlanData"]>(raw.POST("/api/rpc/get_plan_next_month_data", {})),
  applyNextMonthPlan: (assignments: components["schemas"]["PlanAssignment"][]) => wrap<null>(raw.POST("/api/rpc/apply_next_month_plan", { body: { assignments } })),
  listBudgetHistory: (months: number) => wrap<components["schemas"]["CategoryHistory"][]>(raw.POST("/api/rpc/list_budget_history", { body: { months } })),
  listRecentAgentActivity: (limit: number) => wrap<components["schemas"]["AgentActivity"][]>(raw.POST("/api/rpc/list_recent_agent_activity", { body: { limit } })),
  listPlannedTransactions: (filter: components["schemas"]["PlannedTxnFilter"]) => wrap<components["schemas"]["PlannedTransaction"][]>(raw.POST("/api/rpc/list_planned_transactions", { body: { filter } })),
  getPlannedTransaction: (id: string) => wrap<components["schemas"]["PlannedTransaction"] | null>(raw.POST("/api/rpc/get_planned_transaction", { body: { id } })),
  createPlannedTransaction: (input: components["schemas"]["NewPlannedTransaction"]) => wrap<components["schemas"]["PlannedTransaction"]>(raw.POST("/api/rpc/create_planned_transaction", { body: { input } })),
  updatePlannedTransaction: (id: string, patch: components["schemas"]["PlannedTransactionPatch"]) => wrap<components["schemas"]["PlannedTransaction"]>(raw.POST("/api/rpc/update_planned_transaction", { body: { id, patch } })),
  deletePlannedTransaction: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_planned_transaction", { body: { id } })),
  exportTransactionsCsv: (filter: components["schemas"]["TxnFilterInput"]) => wrap<string>(raw.POST("/api/rpc/export_transactions_csv", { body: { filter } })),
  exportSearchTransactionsCsv: (query: components["schemas"]["SearchTxnQueryInput"]) => wrap<string>(raw.POST("/api/rpc/export_search_transactions_csv", { body: { query } })),
  exportAccountCsv: (accountId: string) => wrap<string>(raw.POST("/api/rpc/export_account_csv", { body: { accountId } })),
  listAccountBalanceHistory: (accountId: string, days: number) => wrap<components["schemas"]["AccountBalancePoint"][]>(raw.POST("/api/rpc/list_account_balance_history", { body: { accountId, days } })),
  getAccountBalanceTimeline: (accountId: string, since: string | null) => wrap<components["schemas"]["AccountBalanceTimeline"]>(raw.POST("/api/rpc/get_account_balance_timeline", { body: { accountId, since } })),
  listAccountBalanceSparklines: (days: number) => wrap<components["schemas"]["AccountSparkline"][]>(raw.POST("/api/rpc/list_account_balance_sparklines", { body: { days } })),
  getJourneyStatus: () => wrap<components["schemas"]["JourneyStatus"]>(raw.POST("/api/rpc/get_journey_status", {})),
  getActionItems: () => wrap<components["schemas"]["ActionItem"][]>(raw.POST("/api/rpc/get_action_items", {})),
  getInboxBadgeCount: () => wrap<components["schemas"]["InboxBadgeCount"]>(raw.POST("/api/rpc/get_inbox_badge_count", {})),
  getPushStatus: () => wrap<components["schemas"]["PushStatus"]>(raw.POST("/api/rpc/get_push_status", {})),
  savePushSubscription: (endpoint: string, p256dh: string, auth: string, label: string | null) => wrap<null>(raw.POST("/api/rpc/save_push_subscription", { body: { endpoint, p256dh, auth, label } })),
  deletePushSubscription: (endpoint: string) => wrap<boolean>(raw.POST("/api/rpc/delete_push_subscription", { body: { endpoint } })),
  listPushDevices: () => wrap<components["schemas"]["PushDevice"][]>(raw.POST("/api/rpc/list_push_devices", {})),
  sendTestPush: () => wrap<components["schemas"]["PushDeliveryReport"]>(raw.POST("/api/rpc/send_test_push", {})),
  saveSimplefinSetupToken: (token: string) => wrap<components["schemas"]["SimpleFinConnectionInfo"][]>(raw.POST("/api/rpc/save_simplefin_setup_token", { body: { token } })),
  getSimplefinStatus: () => wrap<components["schemas"]["SimpleFinStatus"]>(raw.POST("/api/rpc/get_simplefin_status", {})),
  listSimplefinConnections: () => wrap<components["schemas"]["SimpleFinConnectionInfo"][]>(raw.POST("/api/rpc/list_simplefin_connections", {})),
  listSimplefinAccounts: () => wrap<components["schemas"]["SimpleFinAccountInfo"][]>(raw.POST("/api/rpc/list_simplefin_accounts", {})),
  importSimplefinAccounts: (accounts: components["schemas"]["SimpleFinAccountImportRequest"][]) => wrap<string[]>(raw.POST("/api/rpc/import_simplefin_accounts", { body: { accounts } })),
  syncSimplefinAccount: (accountId: string) => wrap<components["schemas"]["SyncSummary"]>(raw.POST("/api/rpc/sync_simplefin_account", { body: { accountId } })),
  disconnectSimplefin: () => wrap<null>(raw.POST("/api/rpc/disconnect_simplefin", {})),
  purgeSimplefinData: () => wrap<components["schemas"]["SimpleFinPurgeSummary"]>(raw.POST("/api/rpc/purge_simplefin_data", {})),
  deleteSimplefinConnection: (connectionId: string) => wrap<null>(raw.POST("/api/rpc/delete_simplefin_connection", { body: { connectionId } })),
  syncAllSimplefinAccounts: () => wrap<components["schemas"]["AccountSyncResult"][]>(raw.POST("/api/rpc/sync_all_simplefin_accounts", {})),
  getSimplefinSyncSettings: () => wrap<components["schemas"]["SimpleFinSyncSettings"]>(raw.POST("/api/rpc/get_simplefin_sync_settings", {})),
  setSimplefinSyncSettings: (settings: components["schemas"]["SimpleFinSyncSettings"]) => wrap<null>(raw.POST("/api/rpc/set_simplefin_sync_settings", { body: { settings } })),
  listSimplefinAlerts: () => wrap<components["schemas"]["SimpleFinAlert"][]>(raw.POST("/api/rpc/list_simplefin_alerts", {})),
  acknowledgeSimplefinAlert: (alertId: string) => wrap<null>(raw.POST("/api/rpc/acknowledge_simplefin_alert", { body: { alertId } })),
  listSimplefinTransferSuggestions: () => wrap<components["schemas"]["TransferSuggestionInfo"][]>(raw.POST("/api/rpc/list_simplefin_transfer_suggestions", {})),
  confirmSimplefinTransfer: (transferId: string) => wrap<null>(raw.POST("/api/rpc/confirm_simplefin_transfer", { body: { transferId } })),
  rejectSimplefinTransfer: (transferId: string) => wrap<null>(raw.POST("/api/rpc/reject_simplefin_transfer", { body: { transferId } })),
  listImportReviewCandidates: () => wrap<components["schemas"]["ImportCandidateWithMatches"][]>(raw.POST("/api/rpc/list_import_review_candidates", {})),
  acceptImportCandidateMatch: (candidateId: string, transactionId: string) => wrap<null>(raw.POST("/api/rpc/accept_import_candidate_match", { body: { candidateId, transactionId } })),
  createImportCandidateTransaction: (candidateId: string) => wrap<string>(raw.POST("/api/rpc/create_import_candidate_transaction", { body: { candidateId } })),
  dismissImportCandidate: (candidateId: string) => wrap<null>(raw.POST("/api/rpc/dismiss_import_candidate", { body: { candidateId } })),
  streamCopilotMessage: (conversationId: string, runId: string, text: string, history: components["schemas"]["ChatHistoryEntry"][], sourceMessageId: string | null) => wrap<string>(raw.POST("/api/rpc/stream_copilot_message", { body: { conversationId, runId, text, history, sourceMessageId } })),
  listConversations: () => wrap<components["schemas"]["ConversationSummary"][]>(raw.POST("/api/rpc/list_conversations", {})),
  getConversationMessages: (conversationId: string) => wrap<components["schemas"]["ConversationMessage"][]>(raw.POST("/api/rpc/get_conversation_messages", { body: { conversationId } })),
  deleteConversation: (id: string) => wrap<null>(raw.POST("/api/rpc/delete_conversation", { body: { id } })),
  createConversation: () => wrap<string>(raw.POST("/api/rpc/create_conversation", {})),
  editConversationUserMessage: (input: components["schemas"]["EditConversationMessageInput"]) => wrap<null>(raw.POST("/api/rpc/edit_conversation_user_message", { body: { input } })),
  deleteConversationMessagesAfter: (conversationId: string, messageId: string) => wrap<number>(raw.POST("/api/rpc/delete_conversation_messages_after", { body: { conversationId, messageId } })),
  /** @deprecated Use typed `api` — untyped rpc bypasses arg camelCase asserts */
  rpc: <T>(cmd: string, body: unknown) => {
    if (import.meta.env.DEV) console.warn("[deprecated] rpc", cmd);
    return wrap<T>((raw.POST as any)(`/api/rpc/${cmd}`, { body: body as any }));
  },
};


export const commands = api;

/** @deprecated Use typed `api` — untyped rpc bypasses arg camelCase asserts */
export async function rpc<T>(cmd: string, body: unknown): Promise<Result<T>> {
  if (import.meta.env.DEV) console.warn("[deprecated] rpc", cmd);
  return api.rpc<T>(cmd, body);
}

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
export type BudgetHold = components["schemas"]["BudgetHold"];
export type BudgetTransfer = components["schemas"]["BudgetTransfer"];
export type BudgetChange = components["schemas"]["BudgetChange"];
export type FundingTemplate = components["schemas"]["FundingTemplate"];
export type CustomReportParams = components["schemas"]["CustomReportParams"];
export type CustomReportResult = components["schemas"]["CustomReportResult"];
export type ReportRow = components["schemas"]["ReportRow"];
export type SplitBy = components["schemas"]["SplitBy"];
export type Period = components["schemas"]["Period"];
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
