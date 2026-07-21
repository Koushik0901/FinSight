/**
 * DEV-ONLY browser mock backend.
 *
 * FinSight is a Tauri desktop app: every screen reads its data through
 * `commands.*`, which delegate to `window.__TAURI_INTERNALS__.invoke`. In a
 * plain `vite` browser that global is absent, so `isTauriRuntime()` is false and
 * screens render empty. This module installs a fixture-backed
 * `__TAURI_INTERNALS__` so the app renders full, realistic data in an ordinary
 * browser — enabling a fast visual-design iteration loop and letting us exercise
 * every data state (rich / empty / partial / large / multi-account) instantly.
 *
 * SAFETY: this is imported ONLY from main.tsx, ONLY when
 *   import.meta.env.DEV && new URLSearchParams(location.search).has("mock")
 * and ONLY when no real `__TAURI_INTERNALS__` already exists. It is dynamically
 * imported so it is tree-shaken out of production builds, and it never runs
 * under vitest (which drives components directly, not through main.tsx).
 *
 * It is a design harness, not a source of truth. Numbers are plausible, not
 * audited. The real-Tauri build remains the correctness backstop.
 */

type Kind = "rich" | "empty" | "partial" | "large" | "multi";

// ── AccountSummary builder (fills every required binding field) ──────────────
type AnyRec = Record<string, unknown>;

const ACCOUNT_COLORS: Record<string, string> = {
  Checking: "#60A5FA",
  Savings: "#34D399",
  Credit: "#FB923C",
  Investment: "#A78BFA",
  Cash: "#FACC15",
  Loan: "#F8718C",
  Other: "#9CA3AF",
};

let acctSeq = 0;
function acct(o: AnyRec): AnyRec {
  const type = (o.type as string) ?? "Checking";
  return {
    id: (o.id as string) ?? `acc-${++acctSeq}`,
    owner: "You",
    bank: "Bank",
    type,
    name: "Account",
    balance_cents: 0,
    currency: "USD",
    color: ACCOUNT_COLORS[type] ?? "#60A5FA",
    source: "manual",
    liquidity_type: type === "Investment" ? "invested" : "liquid",
    emergency_fund_eligible: type === "Savings" || type === "Checking",
    goal_earmark: null,
    apy_pct: null,
    simplefin_account_id: null,
    last_synced_at: null,
    nickname: null,
    connection_id: null,
    institution_id: null,
    external_account_id: null,
    official_name: null,
    mask: null,
    subtype: null,
    account_group: "default",
    available_balance_cents: null,
    balance_date: null,
    extra_json: null,
    raw_json: null,
    import_pending: false,
    apr_pct: null,
    min_payment_cents: null,
    payoff_date: null,
    limit_cents: null,
    original_balance_cents: null,
    started_at: null,
    balance_known: true,
    balance_source: "manual",
    ...o,
  };
}

// ── Deterministic time-series helpers ───────────────────────────────────────
function isoDaysAgo(days: number): string {
  return new Date(Date.now() - days * 86_400_000).toISOString();
}
function isoInDays(days: number): string {
  return new Date(Date.now() + days * 86_400_000).toISOString().slice(0, 10);
}
function monthKey(monthsAgo: number): string {
  const d = new Date();
  d.setDate(1);
  d.setMonth(d.getMonth() - monthsAgo);
  return d.toISOString().slice(0, 7);
}

/** Rising net-worth series: `months` monthly points ending near `endCents`. */
/** Monthly balance points landing on `endCents`, with a mid-series high so the
 *  peak callout has something non-trivial to find. */
function balanceSeries(months: number, endCents: number): { date: string; balanceCents: number }[] {
  const pts: { date: string; balanceCents: number }[] = [];
  const start = Math.round(endCents * 0.35);
  for (let i = 0; i < months; i++) {
    const t = months <= 1 ? 1 : i / (months - 1);
    // A hump peaking around two-thirds through, then easing back to `end`.
    const hump = Math.sin(t * Math.PI) * Math.abs(endCents) * 0.45;
    const d = new Date();
    d.setDate(1);
    d.setMonth(d.getMonth() - (months - 1 - i));
    pts.push({
      date: d.toISOString().slice(0, 10),
      balanceCents: Math.round(start + (endCents - start) * t + hump),
    });
  }
  return pts;
}

function netWorthSeries(months: number, startCents: number, endCents: number): AnyRec[] {
  const pts: AnyRec[] = [];
  for (let i = 0; i < months; i++) {
    const t = months <= 1 ? 1 : i / (months - 1);
    // gentle ease + deterministic wobble so the line has character
    const wobble = Math.sin(i * 1.7) * (endCents - startCents) * 0.02;
    const total = Math.round(startCents + (endCents - startCents) * t + wobble);
    const d = new Date();
    d.setDate(1);
    d.setMonth(d.getMonth() - (months - 1 - i));
    pts.push({ date: d.toISOString().slice(0, 10), totalCents: total });
  }
  return pts;
}

// ── Dataset construction ─────────────────────────────────────────────────────
interface Dataset {
  accounts: AnyRec[];
  metrics: AnyRec;
  metricsByMember?: Record<string, AnyRec>;
  healthScore: AnyRec | null;
  savingsRateHistory: AnyRec[];
  categories: AnyRec[];
  recurring: AnyRec[];
  goals: AnyRec[];
  manualAssets: AnyRec[];
  members: AnyRec[];
  milestones: number[];
  needsReview: number;
  agentStatus: AnyRec;
  monthTotals: AnyRec;
  onboarding: AnyRec;
  netWorthEnd: number;
  netWorthStart: number;
  budgetEnvelopes: AnyRec[];
  budgetHistory: AnyRec[];
  categoryGroups: AnyRec[];
  planNextMonthData: AnyRec;
}

function cat(id: string, label: string, color: string, thisM: number, lastM: number, type: string | null): AnyRec {
  return {
    id,
    label,
    color,
    groupId: "g1",
    groupLabel: "Spending",
    spendingType: type,
    thisMonthCents: thisM,
    lastMonthCents: lastM,
    txnCount: Math.max(1, Math.round(thisM / 4000)),
    budgetCents: Math.round(lastM * 1.05),
  };
}

function recur(merchant: string, color: string, label: string, kind: string, cadence: string, amt: number, dueInDays: number): AnyRec {
  return {
    merchantRaw: merchant,
    categoryLabel: label,
    categoryColor: color,
    kind,
    cadence,
    confidence: 0.9,
    reasons: ["Regular cadence", "Stable amount"],
    lastAmountCents: amt,
    minAmountCents: amt,
    maxAmountCents: amt,
    avgGapDays: cadence === "monthly" ? 30 : 7,
    occurrences: 6,
    lastSeen: isoDaysAgo(30 - dueInDays),
    nextExpected: isoInDays(dueInDays),
    // Backend-computed in real runs; mirrored here so the harness shows the
    // same cadence-normalised figure the screen sums.
    monthlyEquivalentCents:
      cadence === "monthly"
        ? Math.abs(amt)
        : cadence === "annual"
          ? Math.round(Math.abs(amt) / 12)
          : Math.round(Math.abs(amt) * (30.44 / 7)),
    feedsProjections: amt < 0 && kind !== "income",
  };
}

function goal(id: string, name: string, target: number, current: number, monthly: number, color: string, accountId: string | null, goalType = "savings"): AnyRec {
  return {
    id,
    name,
    goalType,
    targetCents: target,
    currentCents: current,
    monthlyCents: monthly,
    targetDate: isoInDays(365),
    color,
    notes: null,
    purpose: null,
    sortOrder: 0,
    accountId,
    // The neutral defaults — every goal that predates priorities. The
    // `partial` dataset overrides one so the ranked state is designable.
    priority: "normal",
    deadlineStrictness: "target",
  };
}

function envelope(categoryId: string, categoryLabel: string, categoryColor: string, groupLabel: string, budgetCents: number, spentCents: number, carryoverCents: number): AnyRec {
  return { categoryId, categoryLabel, categoryColor, groupLabel, budgetCents, spentCents, carryoverCents, txnCount: Math.max(0, Math.round(spentCents / 4000)) };
}

const monthAbbrev = (mk: string) => new Date(`${mk}-01`).toLocaleDateString("en-US", { month: "short" });

function history(categoryId: string, label: string, color: string, spentByMonth: number[], budgetedCents: number): AnyRec {
  return {
    categoryId,
    label,
    color,
    monthly: spentByMonth.map((spentCents, i) => {
      const month = monthKey(spentByMonth.length - 1 - i);
      return { month, label: monthAbbrev(month), spentCents, budgetedCents };
    }),
  };
}

/** Safe, non-crashing default — every screen.tsx that reads PlanData assumes these arrays exist. */
const EMPTY_PLAN_DATA: AnyRec = { incomeCents: 0, categories: [], goals: [], sinkingFunds: [], recurringExpenseCents: 0, lookBack: [] };

function baseMetrics(o: AnyRec): AnyRec {
  return {
    liquidCents: 0,
    investedCents: 0,
    debtCents: 0,
    emergencyFundCents: 0,
    netWorthCents: 0,
    accountsWithUnknownBalance: 0,
    avgMonthlyIncomeCents: 0,
    avgMonthlyExpenseCents: 0,
    netMonthlyCents: 0,
    rollingSavingsRatePct: 0,
    thisMonthIncomeCents: 0,
    thisMonthExpenseCents: 0,
    thisMonthNetCents: 0,
    thisMonthSavingsRatePct: 0,
    // Null is the WITHHELD state — too little history to state a figure. The
    // `empty` dataset leaves it null so that path is visible in the harness;
    // populated datasets override with real numbers.
    emergencyFundMonths: null,
    runwayDays: null,
    safetyBasisSpanDays: 0,
    targetSavingsRatePct: 20,
    emergencyFundTargetMonths: 6,
    expectedAnnualReturnPct: 7,
    // Single-currency is the default because it is the common case; the
    // `partial` dataset overrides it so the mixed-currency caveat is
    // designable without needing a real cross-border account.
    currency: "USD",
    unconvertedHoldings: [],
    ...o,
  };
}

/** Dev-harness "explain this number" provenance, mirroring the shape and
 *  withholding rules of finsight-core::provenance so the inspector renders
 *  realistically in every mock dataset (incl. the `empty` withheld path and the
 *  `partial` mixed-currency path). Values are read from the SAME mock metrics
 *  the cards show, so the explanation and the figure always agree here too. */
function buildMetricExplanations(m: AnyRec): AnyRec[] {
  const n = (k: string) => (m[k] as number) ?? 0;
  const codes = ((m.unconvertedHoldings as AnyRec[]) ?? []).map((h) => h.code as string);
  const unknown = n("accountsWithUnknownBalance");
  const span = n("safetyBasisSpanDays");
  const nwExcl: string[] = [];
  const nwWarn: AnyRec[] = [];
  if (unknown > 0) {
    nwExcl.push(`${unknown} account(s) with no confirmed balance excluded — counted as unknown, never as zero.`);
    nwWarn.push({ level: "caution", message: `This figure omits ${unknown} account(s) whose balance hasn't been confirmed.` });
  }
  if (codes.length) {
    nwExcl.push(`Money held in ${codes.join(", ")} is not converted and not included.`);
    nwWarn.push({ level: "caution", message: `You also hold money in ${codes.join(", ")}. It isn't converted, so this is a partial view.` });
  }
  const cashflowExcl = [
    "Transfers between your own accounts (they aren't income or spending).",
    "Activity inside investment accounts (buys/sells aren't cashflow).",
    ...(codes.length ? [`Transactions in other currencies (${codes.join(", ")}) — only your primary currency is totalled.`] : []),
  ];
  const thin = span > 0 && span < 30
    ? [{ level: "caution", message: `Only ${span} day(s) of history so far — this monthly average is extrapolated from a partial month.` }]
    : span <= 0
      ? [{ level: "caution", message: "No transaction history in this window yet, so this is $0 by default." }]
      : [];
  const residual = n("netWorthCents") - (n("liquidCents") + n("investedCents") - n("debtCents"));
  const efMonths = m.emergencyFundMonths as number | null;
  const runway = m.runwayDays as number | null;
  const withheld = (extra?: string) => ({ level: "withheld", message: `Withheld until there's about 30 days of history — currently ${span}.${extra ?? ""}` });
  const period90 = "Trailing 90 days";
  return [
    { key: "net_worth", label: "Net worth", value: { kind: "money", cents: n("netWorthCents") },
      definition: "Everything you own minus everything you owe: confirmed account balances (debts negative) plus any manual assets.",
      inputs: [
        { label: "Cash & liquid accounts", amountCents: n("liquidCents"), detail: null },
        { label: "Investments", amountCents: n("investedCents"), detail: null },
        { label: "Debts", amountCents: n("debtCents") === 0 ? 0 : -n("debtCents"), detail: "credit cards & loans, counted as negative" },
        ...(residual !== 0 ? [{ label: "Manual assets & other holdings", amountCents: residual, detail: null }] : []),
      ],
      exclusions: nwExcl, assumptions: [], period: "As of today", warnings: nwWarn },
    { key: "avg_monthly_income", label: "Average monthly income", value: { kind: "money", cents: n("avgMonthlyIncomeCents") },
      definition: "Your typical income per month: money coming in, averaged over the months of history in the window.",
      inputs: [{ label: "Averaged over", amountCents: null, detail: "the months of activity in the last 90 days" }],
      exclusions: [...cashflowExcl, "Reimbursements you were paid back (they net against the original expense)."],
      assumptions: [], period: period90, warnings: thin },
    { key: "avg_monthly_expense", label: "Average monthly spending", value: { kind: "money", cents: n("avgMonthlyExpenseCents") },
      definition: "Your typical spending per month: money going out, averaged over the months of history in the window.",
      inputs: [{ label: "Averaged over", amountCents: null, detail: "the months of activity in the last 90 days" }],
      exclusions: cashflowExcl, assumptions: [], period: period90, warnings: thin },
    { key: "monthly_surplus", label: "Monthly surplus", value: { kind: "money", cents: n("netMonthlyCents") },
      definition: "What's left over in a typical month: average income minus average spending.",
      inputs: [
        { label: "Average monthly income", amountCents: n("avgMonthlyIncomeCents"), detail: null },
        { label: "Average monthly spending", amountCents: -n("avgMonthlyExpenseCents"), detail: null },
      ],
      exclusions: ["Transfers and investment-account activity (see income and spending)."], assumptions: [], period: period90, warnings: thin },
    { key: "savings_rate", label: "Savings rate", value: { kind: "percent", pct: n("rollingSavingsRatePct") },
      definition: "The share of your income you keep: (income − spending) ÷ income, over the window.",
      inputs: [
        { label: "Average monthly income", amountCents: n("avgMonthlyIncomeCents"), detail: null },
        { label: "Average monthly spending", amountCents: n("avgMonthlyExpenseCents"), detail: null },
      ],
      exclusions: ["Transfers and investment-account activity."],
      assumptions: [{ label: "Your target savings rate", value: `${n("targetSavingsRatePct")}%` }], period: period90, warnings: thin },
    { key: "emergency_fund_months", label: "Emergency-fund coverage",
      value: efMonths === null ? { kind: "withheld" } : { kind: "months", months: efMonths },
      definition: "How many months your emergency-fund savings would cover at your typical spending.",
      inputs: [
        { label: "Emergency-fund savings", amountCents: n("emergencyFundCents"), detail: null },
        { label: "Conservative monthly spending", amountCents: n("avgMonthlyExpenseCents"), detail: "the larger of your 12-month and 90-day average, so annual bills are counted" },
      ],
      exclusions: [], assumptions: [{ label: "Your target", value: `${n("emergencyFundTargetMonths")} months of expenses` }],
      period: "As of today, at your conservative monthly spending",
      warnings: efMonths === null ? [withheld(" A confident wrong number here would overstate how safe you are.")] : [{ level: "info", message: "Based on your complete months of spending history." }] },
    { key: "runway_days", label: "Cash runway",
      value: runway === null ? { kind: "withheld" } : { kind: "days", days: runway },
      definition: "How long your liquid cash would last with no new income, at your typical spending.",
      inputs: [
        { label: "Liquid cash", amountCents: n("liquidCents"), detail: null },
        { label: "Conservative monthly spending", amountCents: n("avgMonthlyExpenseCents"), detail: null },
      ],
      exclusions: [], assumptions: [], period: "As of today, at your conservative monthly spending",
      warnings: runway === null ? [withheld()] : [{ level: "info", message: "Based on your complete months of spending history." }] },
  ];
}

/** Dev-harness cash-flow forecast, mirroring finsight-core::cashflow: dated
 *  events on their days + a residual smooth burn, then lowest-point / breach /
 *  safe-to-spend. Reads the mock metrics so it moves with the chosen dataset
 *  (the `empty` set → withheld/rough, populated sets → a real trajectory). */
function buildCashflowForecast(m: AnyRec, horizonDays?: number, bufferCents?: number, extraExpenseCents?: number): AnyRec {
  const horizon = Math.min(90, Math.max(7, Number(horizonDays ?? 30)));
  const buffer = Math.max(0, Number(bufferCents ?? 0));
  const start = Number(m.liquidCents ?? 0);
  const avgExp = Number(m.avgMonthlyExpenseCents ?? 0);
  const avgInc = Number(m.avgMonthlyIncomeCents ?? 0);
  const hasData = avgExp > 0;

  const raw: Array<{ day: number; amountCents: number; kind: string; label: string }> = [];
  if (hasData) {
    raw.push({ day: 3, amountCents: -1_200, kind: "subscription", label: "Streaming Plus" });
    if (10 < horizon) raw.push({ day: 10, amountCents: -Math.round(avgExp * 0.4), kind: "bill", label: "Rent" });
    if (14 < horizon && avgInc > 0) raw.push({ day: 14, amountCents: avgInc, kind: "income", label: "Employer payroll" });
  }
  if (Number(extraExpenseCents ?? 0) > 0) raw.push({ day: 0, amountCents: -Math.abs(Number(extraExpenseCents)), kind: "hypothetical", label: "Hypothetical spend" });

  // Residual everyday burn: expense minus the dated obligations, per day.
  const datedObligationMonthly = hasData ? 1_200 + Math.round(avgExp * 0.4) : 0;
  const dailyBurn = Math.max(0, Math.round((avgExp - datedObligationMonthly) / 30.44));

  const byDay = new Map<number, number>();
  for (const e of raw) byDay.set(e.day, (byDay.get(e.day) ?? 0) + e.amountCents);

  const days: AnyRec[] = [];
  let bal = start;
  let lowest = start;
  let lowestDate = isoInDays(0);
  let firstBreach: string | null = null;
  for (let dd = 0; dd < horizon; dd++) {
    const net = byDay.get(dd) ?? 0;
    bal += net - dailyBurn;
    const date = isoInDays(dd);
    const below = bal < buffer;
    if (bal < lowest) { lowest = bal; lowestDate = date; }
    if (below && !firstBreach) firstBreach = date;
    days.push({ date, projectedBalanceCents: bal, eventNetCents: net, burnCents: -dailyBurn, belowBuffer: below });
  }
  const upcomingEvents = raw
    .filter((e) => e.day < horizon)
    .map((e) => ({
      date: isoInDays(e.day),
      label: e.label,
      amountCents: e.amountCents,
      kind: e.kind,
      confidence: ["income", "bill", "subscription"].includes(e.kind) ? 0.82 : null,
    }));

  const warnings: AnyRec[] = [];
  if (!hasData) {
    warnings.push({ level: "caution", message: "Only a little history so far — this forecast is a rough estimate, not a precise projection." });
  } else {
    warnings.push({ level: "caution", message: `Car insurance (about $680) is due ${isoInDays(horizon + 4)}, just after this window — plan for it.` });
    warnings.push({ level: "info", message: `Everyday spending is projected at about $${Math.round(dailyBurn / 100)}/day from your recent average.` });
  }

  return {
    asOf: isoInDays(0),
    horizonDays: horizon,
    startBalanceCents: start,
    bufferCents: buffer,
    dailyBurnCents: dailyBurn,
    days,
    lowestBalanceCents: lowest,
    lowestDate,
    firstBreachDate: firstBreach,
    safeToSpendCents: Math.max(lowest - buffer, 0),
    upcomingEvents,
    warnings,
    reliable: hasData,
  };
}

function agentStatus(o: AnyRec): AnyRec {
  return {
    uncategorizedCount: 0,
    anomalyCount: 0,
    overBudgetCount: 0,
    upcomingBillsCount: 0,
    lastScanAt: null,
    lastScanCategorized: null,
    ...o,
  };
}

function buildDataset(kind: Kind): Dataset {
  const C = {
    housing: "#A78BFA",
    groceries: "#34D399",
    dining: "#FB923C",
    transport: "#60A5FA",
    utilities: "#FACC15",
    subs: "#F472B6",
    shopping: "#FCA5A5",
    health: "#2DD4BF",
    travel: "#818CF8",
  };

  if (kind === "empty") {
    return {
      accounts: [],
      metrics: baseMetrics({}),
      healthScore: null,
      savingsRateHistory: [],
      categories: [],
      recurring: [],
      goals: [],
      manualAssets: [],
      members: [],
      milestones: [],
      needsReview: 0,
      agentStatus: agentStatus({}),
      monthTotals: { incomeCents: 0, expenseCents: 0, netCents: 0, savingsRatePct: 0, txnCount: 0 },
      onboarding: { account_count: 0, category_count: 0, completion_marked: true },
      netWorthStart: 0,
      netWorthEnd: 0,
      budgetEnvelopes: [],
      budgetHistory: [],
      categoryGroups: [],
      planNextMonthData: EMPTY_PLAN_DATA,
    };
  }

  if (kind === "partial") {
    const accounts = [
      acct({ id: "p-chk", type: "Checking", bank: "Tangerine", name: "Everyday Chequing", balance_cents: 231400 }),
      acct({ id: "p-inv", type: "Investment", bank: "Wealthsimple", name: "TFSA", balance_cents: 0, balance_known: false, balance_source: "seed" }),
    ];
    return {
      accounts,
      metrics: baseMetrics({
        liquidCents: 231400,
        netWorthCents: 231400,
        accountsWithUnknownBalance: 1,
        avgMonthlyIncomeCents: 540000,
        avgMonthlyExpenseCents: 388000,
        netMonthlyCents: 152000,
        rollingSavingsRatePct: 21,
        thisMonthIncomeCents: 540000,
        thisMonthExpenseCents: 174000,
        thisMonthNetCents: 366000,
        thisMonthSavingsRatePct: 24,
        emergencyFundMonths: 0.6,
        runwayDays: 18,
        // A cross-border user: the totals above are USD only, and the CAD
        // holding is real money deliberately left out rather than converted.
        currency: "USD",
        unconvertedHoldings: [
          { code: "CAD", accountCount: 2, balanceCents: 418_000 },
        ],
      }),
      healthScore: {
        total: 52,
        grade: "C",
        breakdown: {
          savingsRatePts: 12, emergencyFundPts: 4, debtRatioPts: 18, goalProgressPts: 6, budgetAdherencePts: 12,
          savingsRatePct: 21, emergencyFundMonths: 0.6, debtToIncomePct: 8, avgGoalPct: 0, budgetAdherencePct: 74,
        },
        tips: ["Build one month of expenses in savings before anything else.", "Set a first goal so progress has a destination."],
      },
      savingsRateHistory: [
        { month: monthKey(1), savingsRatePct: 18, incomeCents: 540000, expenseCents: 442000 },
        { month: monthKey(0), savingsRatePct: 24, incomeCents: 540000, expenseCents: 410000 },
      ],
      categories: [
        cat("c-groc", "Groceries", C.groceries, 62000, 58000, "Need"),
        cat("c-dining", "Dining", C.dining, 41000, 39000, "Want"),
        cat("c-transport", "Transport", C.transport, 24000, 28000, "Need"),
      ],
      recurring: [recur("Rent", C.housing, "Housing", "bill", "monthly", 145000, 6)],
      goals: [],
      manualAssets: [],
      members: [],
      milestones: [],
      needsReview: 0,
      agentStatus: agentStatus({ uncategorizedCount: 0, lastScanAt: isoDaysAgo(0.02) }),
      monthTotals: { incomeCents: 540000, expenseCents: 174000, netCents: 366000, savingsRatePct: 24, txnCount: 22 },
      onboarding: { account_count: 2, category_count: 6, completion_marked: true },
      netWorthStart: 180000,
      netWorthEnd: 231400,
      budgetEnvelopes: [
        envelope("c-groc", "Groceries", C.groceries, "Daily life", 70000, 62000, 0),
        envelope("c-dining", "Dining", C.dining, "Daily life", 40000, 41000, 0),
        envelope("c-transport", "Transport", C.transport, "Daily life", 30000, 24000, 0),
      ],
      budgetHistory: [],
      categoryGroups: [{ id: "g1", label: "Spending", hint: null, sort_order: 0 }],
      planNextMonthData: EMPTY_PLAN_DATA,
    };
  }

  if (kind === "large") {
    const accounts = [
      acct({ id: "l-chk", type: "Checking", bank: "CIBC", name: "Chequing", balance_cents: 612000 }),
      acct({ id: "l-sav", type: "Savings", bank: "CIBC", name: "High-Interest Savings", balance_cents: 2840000 }),
      acct({ id: "l-sav2", type: "Savings", bank: "Tangerine", name: "Emergency Fund", balance_cents: 1560000 }),
      acct({ id: "l-cc1", type: "Credit", bank: "Amex", name: "Cobalt", balance_cents: -184000 }),
      acct({ id: "l-cc2", type: "Credit", bank: "CIBC", name: "Aventura", balance_cents: -92000 }),
      acct({ id: "l-inv1", type: "Investment", bank: "Wealthsimple", name: "TFSA", balance_cents: 6820000 }),
      acct({ id: "l-inv2", type: "Investment", bank: "Wealthsimple", name: "RRSP", balance_cents: 9410000 }),
      acct({ id: "l-loan", type: "Loan", bank: "CIBC", name: "Student Loan", balance_cents: -1240000 }),
    ];
    return {
      accounts,
      metrics: baseMetrics({
        liquidCents: 5012000,
        investedCents: 16230000,
        debtCents: 1516000,
        netWorthCents: 19726000,
        avgMonthlyIncomeCents: 1120000,
        avgMonthlyExpenseCents: 748000,
        netMonthlyCents: 372000,
        rollingSavingsRatePct: 33,
        thisMonthIncomeCents: 1120000,
        thisMonthExpenseCents: 512000,
        thisMonthNetCents: 608000,
        thisMonthSavingsRatePct: 38,
        emergencyFundCents: 4400000,
        emergencyFundMonths: 5.9,
        runwayDays: 201,
      }),
      healthScore: {
        total: 84,
        grade: "A",
        breakdown: {
          savingsRatePts: 22, emergencyFundPts: 20, debtRatioPts: 16, goalProgressPts: 14, budgetAdherencePts: 12,
          savingsRatePct: 33, emergencyFundMonths: 5.9, debtToIncomePct: 14, avgGoalPct: 61, budgetAdherencePct: 88,
        },
        tips: ["You're one month from a full emergency fund — nudge it over the line.", "Consider directing the surplus toward the student loan."],
      },
      savingsRateHistory: Array.from({ length: 6 }, (_, i) => ({
        month: monthKey(5 - i),
        savingsRatePct: [28, 31, 26, 35, 30, 38][i]!,
        incomeCents: 1120000,
        expenseCents: 1120000 * (1 - [28, 31, 26, 35, 30, 38][i]! / 100),
      })),
      categories: [
        cat("c-housing", "Housing", C.housing, 210000, 210000, "Need"),
        cat("c-groc", "Groceries", C.groceries, 78000, 71000, "Need"),
        cat("c-dining", "Dining", C.dining, 52000, 61000, "Want"),
        cat("c-transport", "Transport", C.transport, 34000, 29000, "Need"),
        cat("c-shopping", "Shopping", C.shopping, 47000, 22000, "Want"),
        cat("c-utilities", "Utilities", C.utilities, 18400, 17200, "Need"),
        cat("c-health", "Health", C.health, 12600, 9800, "Need"),
        cat("c-travel", "Travel", C.travel, 68000, 0, "Want"),
        cat("c-subs", "Subscriptions", C.subs, 9400, 9400, "Want"),
      ],
      recurring: [
        recur("Rent", C.housing, "Housing", "bill", "monthly", 210000, 3),
        recur("Hydro One", C.utilities, "Utilities", "bill", "monthly", 8400, 9),
        recur("Netflix", C.subs, "Subscriptions", "subscription", "monthly", 1699, 12),
        recur("Spotify", C.subs, "Subscriptions", "subscription", "monthly", 1199, 5),
        recur("Fitness World", C.health, "Health", "subscription", "monthly", 4500, 14),
        recur("iCloud+", C.subs, "Subscriptions", "subscription", "monthly", 399, 1),
        recur("Employer Payroll", "#6FCA8A", "Income", "income", "biweekly", 560000, 4),
        recur("Internet — Bell", C.utilities, "Utilities", "bill", "monthly", 9500, 8),
      ],
      goals: [
        // A ranked household: the emergency fund outranks everything and the
        // trip has a date that cannot move, so the competing-goals state is
        // visible in the harness rather than only reachable with real data.
        { ...goal("g-ef", "Emergency Fund", 4800000, 4400000, 40000, "#34D399", null, "safety"), priority: "critical" },
        { ...goal("g-vac", "Japan 2027", 900000, 340000, 30000, "#818CF8", null, "travel"), deadlineStrictness: "hard" },
        goal("g-car", "Next Car", 3500000, 1180000, 60000, "#FB923C", null, "purchase"),
        goal("g-house", "House Down Payment", 8000000, 2100000, 120000, "#A78BFA", null, "home"),
      ],
      manualAssets: [{ id: "ma1", name: "2019 Honda Civic", assetType: "vehicle", valueCents: 1650000, currency: "USD", notes: null, createdAt: isoDaysAgo(200), updatedAt: isoDaysAgo(10) }],
      members: [],
      milestones: [],
      needsReview: 12,
      agentStatus: agentStatus({ uncategorizedCount: 12, anomalyCount: 2, overBudgetCount: 1, upcomingBillsCount: 5, lastScanAt: isoDaysAgo(0.01), lastScanCategorized: 34 }),
      monthTotals: { incomeCents: 1120000, expenseCents: 512000, netCents: 608000, savingsRatePct: 38, txnCount: 214 },
      onboarding: { account_count: 8, category_count: 18, completion_marked: true },
      netWorthStart: 12800000,
      netWorthEnd: 19726000,
      budgetEnvelopes: [
        envelope("c-housing", "Housing", C.housing, "Fixed costs", 210000, 210000, 0),
        envelope("c-groc", "Groceries", C.groceries, "Daily life", 82000, 78000, 4000),
        envelope("c-dining", "Dining", C.dining, "Daily life", 55000, 52000, -6000),
        envelope("c-travel", "Travel", C.travel, "Lifestyle", 50000, 68000, 50000),
      ],
      budgetHistory: [
        history("c-groc", "Groceries", C.groceries, [71000, 74000, 69000, 71000, 78000], 82000),
        history("c-dining", "Dining", C.dining, [61000, 58000, 55000, 49000, 52000], 55000),
      ],
      categoryGroups: [
        { id: "g1", label: "Spending", hint: null, sort_order: 0 },
        { id: "fixed", label: "Fixed costs", hint: "predictable, mostly recurring", sort_order: 1 },
        { id: "lifestyle", label: "Lifestyle", hint: "things you choose to spend on", sort_order: 2 },
      ],
      planNextMonthData: {
        incomeCents: 1120000,
        categories: [
          { categoryId: "c-housing", label: "Housing", color: C.housing, groupLabel: "Fixed costs", budgetCents: 210000, m0Cents: 210000, m1Cents: 210000, m2Cents: 210000 },
          { categoryId: "c-dining", label: "Dining", color: C.dining, groupLabel: "Daily life", budgetCents: 55000, m0Cents: 52000, m1Cents: 58000, m2Cents: 61000 },
        ],
        goals: [
          goal("g-ef", "Emergency Fund", 4800000, 4400000, 40000, "#34D399", null, "safety"),
          goal("g-vac", "Japan 2027", 900000, 340000, 30000, "#818CF8", null, "travel"),
        ],
        sinkingFunds: [goal("g-ins", "Car insurance", 48000, 20000, 8000, "#FACC15", null, "sinking-fund")],
        recurringExpenseCents: 42000,
        lookBack: [
          { categoryId: "c-dining", categoryLabel: "Dining", kind: "under", amountCents: 3000, streakMonths: 0 },
          { categoryId: "c-travel", categoryLabel: "Travel", kind: "streak", amountCents: 0, streakMonths: 3 },
        ],
      },
    };
  }

  // ── rich (default) & multi share the core; multi adds household members ─────
  const accountsRich = [
    acct({ id: "r-chk", type: "Checking", bank: "Tangerine", name: "Everyday Chequing", balance_cents: 482000, owner: "You" }),
    acct({ id: "r-sav", type: "Savings", bank: "CIBC", name: "High-Interest Savings", balance_cents: 1840000, owner: "You" }),
    acct({ id: "r-cc", type: "Credit", bank: "Amex", name: "Cobalt Card", balance_cents: -124000, owner: kind === "multi" ? "Sam" : "You" }),
    acct({ id: "r-inv", type: "Investment", bank: "Wealthsimple", name: "TFSA", balance_cents: 5230000, owner: kind === "multi" ? "Sam" : "You" }),
  ];

  const members = kind === "multi"
    ? [
        { id: "m-you", name: "Alex", color: "#C9F950", createdAt: isoDaysAgo(300), is_self: true },
        { id: "m-sam", name: "Sam", color: "#818CF8", createdAt: isoDaysAgo(300), is_self: false },
      ]
    : [];

  const metrics = baseMetrics({
    liquidCents: 2322000,
    investedCents: 5230000,
    debtCents: 124000,
    netWorthCents: 7428000,
    emergencyFundCents: 1840000,
    avgMonthlyIncomeCents: 700000,
    avgMonthlyExpenseCents: 452000,
    netMonthlyCents: 248000,
    rollingSavingsRatePct: 33,
    thisMonthIncomeCents: 700000,
    thisMonthExpenseCents: 364000,
    thisMonthNetCents: 336000,
    thisMonthSavingsRatePct: 48,
    emergencyFundMonths: 4.1,
    runwayDays: 154,
  });

  const metricsByMember: Record<string, AnyRec> = {
    "m-you": baseMetrics({
      liquidCents: 2322000, investedCents: 2615000, netWorthCents: 4813000,
      thisMonthIncomeCents: 420000, thisMonthExpenseCents: 210000, thisMonthNetCents: 210000, thisMonthSavingsRatePct: 50,
      avgMonthlyIncomeCents: 420000, avgMonthlyExpenseCents: 262000, rollingSavingsRatePct: 38, runwayDays: 200,
    }),
    "m-sam": baseMetrics({
      liquidCents: 0, investedCents: 2615000, debtCents: 124000, netWorthCents: 2491000,
      thisMonthIncomeCents: 280000, thisMonthExpenseCents: 154000, thisMonthNetCents: 126000, thisMonthSavingsRatePct: 45,
      avgMonthlyIncomeCents: 280000, avgMonthlyExpenseCents: 190000, rollingSavingsRatePct: 27, runwayDays: 90,
    }),
  };

  return {
    accounts: accountsRich,
    metrics,
    metricsByMember: kind === "multi" ? metricsByMember : undefined,
    healthScore: {
      total: 78,
      grade: "B+",
      breakdown: {
        savingsRatePts: 20, emergencyFundPts: 15, debtRatioPts: 18, goalProgressPts: 13, budgetAdherencePts: 12,
        savingsRatePct: 48, emergencyFundMonths: 4.1, debtToIncomePct: 6, avgGoalPct: 54, budgetAdherencePct: 86,
      },
      tips: ["Two more months of savings hits a full emergency fund.", "Dining is trending down — nice. Bank the difference."],
    },
    savingsRateHistory: [
      { month: monthKey(5), savingsRatePct: 38, incomeCents: 700000, expenseCents: 434000 },
      { month: monthKey(4), savingsRatePct: 42, incomeCents: 700000, expenseCents: 406000 },
      { month: monthKey(3), savingsRatePct: 31, incomeCents: 700000, expenseCents: 483000 },
      { month: monthKey(2), savingsRatePct: 47, incomeCents: 700000, expenseCents: 371000 },
      { month: monthKey(1), savingsRatePct: 44, incomeCents: 700000, expenseCents: 392000 },
      { month: monthKey(0), savingsRatePct: 48, incomeCents: 700000, expenseCents: 364000 },
    ],
    categories: [
      cat("c-housing", "Housing", C.housing, 180000, 180000, "Need"),
      cat("c-groc", "Groceries", C.groceries, 62000, 58000, "Need"),
      cat("c-dining", "Dining", C.dining, 41000, 52000, "Want"),
      cat("c-shopping", "Shopping", C.shopping, 33000, 19000, "Want"),
      cat("c-transport", "Transport", C.transport, 24000, 21000, "Need"),
      cat("c-utilities", "Utilities", C.utilities, 15400, 14200, "Need"),
      cat("c-subs", "Subscriptions", C.subs, 8600, 8600, "Want"),
    ],
    recurring: [
      recur("Rent", C.housing, "Housing", "bill", "monthly", 180000, 4),
      recur("Netflix", C.subs, "Subscriptions", "subscription", "monthly", 1699, 9),
      recur("Spotify", C.subs, "Subscriptions", "subscription", "monthly", 1199, 2),
      recur("Hydro", C.utilities, "Utilities", "bill", "monthly", 8400, 11),
      recur("Fitness World", C.health, "Health", "subscription", "monthly", 4500, 13),
      recur("iCloud+", C.subs, "Subscriptions", "subscription", "monthly", 299, 1),
    ],
    goals: [
      goal("g-ef", "Emergency Fund", 3000000, 1840000, 40000, "#34D399", null, "safety"),
      goal("g-vac", "Vacation Fund", 500000, 220000, 25000, "#818CF8", null, "travel"),
      goal("g-car", "New Car", 2500000, 400000, 50000, "#FB923C", null, "purchase"),
    ],
    manualAssets: [],
    members,
    milestones: [],
    needsReview: 3,
    agentStatus: agentStatus({ uncategorizedCount: 3, anomalyCount: 1, upcomingBillsCount: 2, lastScanAt: isoDaysAgo(0.008), lastScanCategorized: 18 }),
    monthTotals: { incomeCents: 700000, expenseCents: 364000, netCents: 336000, savingsRatePct: 48, txnCount: 84 },
    onboarding: { account_count: 4, category_count: 12, completion_marked: true },
    netWorthStart: 5100000,
    netWorthEnd: 7428000,
    budgetEnvelopes: [
      envelope("c-housing", "Housing", C.housing, "Fixed costs", 180000, 180000, 0),
      envelope("c-groc", "Groceries", C.groceries, "Daily life", 70000, 62000, 3000),
      envelope("c-dining", "Dining", C.dining, "Daily life", 40000, 41000, -2000),
      envelope("c-shopping", "Shopping", C.shopping, "Lifestyle", 35000, 33000, 0),
      envelope("c-transport", "Transport", C.transport, "Daily life", 25000, 24000, 0),
      envelope("c-utilities", "Utilities", C.utilities, "Fixed costs", 16000, 15400, 1200),
      envelope("c-subs", "Subscriptions", C.subs, "Fixed costs", 9000, 8600, 0),
      envelope("c-gifts", "Gifts", "#FDE68A", "Lifestyle", 0, 0, 0),
    ],
    budgetHistory: [
      history("c-groc", "Groceries", C.groceries, [58000, 65000, 61000, 59000, 62000], 70000),
      history("c-dining", "Dining", C.dining, [38000, 44000, 47000, 39000, 41000], 40000),
      history("c-utilities", "Utilities", C.utilities, [16200, 15800, 14900, 15100, 15400], 16000),
    ],
    categoryGroups: [
      { id: "g1", label: "Spending", hint: null, sort_order: 0 },
      { id: "fixed", label: "Fixed costs", hint: "predictable, mostly recurring", sort_order: 1 },
      { id: "lifestyle", label: "Lifestyle", hint: "things you choose to spend on", sort_order: 2 },
    ],
    planNextMonthData: {
      incomeCents: 700000,
      categories: [
        { categoryId: "c-housing", label: "Housing", color: C.housing, groupLabel: "Fixed costs", budgetCents: 180000, m0Cents: 180000, m1Cents: 180000, m2Cents: 180000 },
        { categoryId: "c-utilities", label: "Utilities", color: C.utilities, groupLabel: "Fixed costs", budgetCents: 16000, m0Cents: 15400, m1Cents: 15800, m2Cents: 16200 },
        { categoryId: "c-dining", label: "Dining", color: C.dining, groupLabel: "Daily life", budgetCents: 40000, m0Cents: 41000, m1Cents: 44000, m2Cents: 39000 },
      ],
      goals: [
        goal("g-ef", "Emergency Fund", 3000000, 1840000, 40000, "#34D399", null, "safety"),
        goal("g-vac", "Vacation Fund", 500000, 220000, 25000, "#818CF8", null, "travel"),
      ],
      sinkingFunds: [goal("g-ins", "Car insurance", 48000, 20000, 8000, "#FACC15", null, "sinking-fund")],
      recurringExpenseCents: 38000,
      lookBack: [
        { categoryId: "c-dining", categoryLabel: "Dining", kind: "over", amountCents: 1000, streakMonths: 0 },
        { categoryId: "c-groc", categoryLabel: "Groceries", kind: "under", amountCents: 8000, streakMonths: 0 },
      ],
    },
  };
}

// ── invoke dispatch ─────────────────────────────────────────────────────────
// ── Dev-harness saved scenarios (in-memory; resets each page load) ───────────
function scResult(verdict: boolean, runway: number, impact: number, considerations: string[], goals: string[]): AnyRec {
  return { verdict, runwayChangeDays: runway, monthlyImpactCents: impact, considerations, baselineMonthly: [], scenarioMonthly: [], goalsAffected: goals };
}
function scParams(o: Partial<AnyRec>): AnyRec {
  return { incomeDeltaPct: 0, monthlyExpenseDeltaCents: 0, oneTimeCents: 0, startMonthOffset: 0, label: "Scenario", ...o };
}
let mockScenarioSeq = 0;
let mockScenarios: AnyRec[] = [
  {
    id: "sc-income", description: "Cut income 50%", createdAt: isoDaysAgo(9), months: 24,
    params: scParams({ incomeDeltaPct: -50, label: "Cut income 50%" }),
    originalResult: scResult(false, -180, -300000, ["Runway shortens sharply."], []),
    originalBaseline: { balanceCents: 2314000, avgMonthlyIncomeCents: 600000, avgMonthlyExpenseCents: 388000, goalCount: 1 },
    currentResult: scResult(false, -214, -300000, ["Runway shortens by 214 days."], []),
    isStale: true, recomputable: true,
  },
  {
    id: "sc-save", description: "Add $500/mo to savings", createdAt: isoDaysAgo(2), months: 24,
    params: scParams({ monthlyExpenseDeltaCents: 50000, label: "Add $500/mo to savings" }),
    originalResult: scResult(true, 52, -50000, ["Frees room in the budget."], ["House Fund: +6 mo"]),
    originalBaseline: { balanceCents: 2314000, avgMonthlyIncomeCents: 540000, avgMonthlyExpenseCents: 388000, goalCount: 1 },
    currentResult: scResult(true, 38, -50000, ["Extends runway by 38 days."], ["House Fund: +6 mo"]),
    isStale: false, recomputable: true,
  },
];
function mockDetailFromParams(description: string, params: AnyRec, months: number): AnyRec {
  const cut = Number(params.incomeDeltaPct ?? 0) < 0 || Number(params.oneTimeCents ?? 0) > 0;
  const result = scResult(!cut, cut ? -120 : 24, Number(params.monthlyExpenseDeltaCents ?? 0) * -1, ["Saved just now."], []);
  return {
    id: `sc-${++mockScenarioSeq}`, description, createdAt: isoInDays(0), months,
    params, originalResult: result,
    originalBaseline: { balanceCents: 2314000, avgMonthlyIncomeCents: 540000, avgMonthlyExpenseCents: 388000, goalCount: 1 },
    currentResult: result, isStale: false, recomputable: true,
  };
}

function buildResponders(ds: Dataset): Record<string, (args: AnyRec) => unknown> {
  return {
    list_accounts: () => ds.accounts,
    get_agent_status: () => ds.agentStatus,
    get_needs_review_count: () => ds.needsReview,
    get_financial_metrics: (a) => {
      const memberId = a?.memberId as string | null | undefined;
      if (memberId && ds.metricsByMember?.[memberId]) return ds.metricsByMember[memberId];
      return ds.metrics;
    },
    explain_financial_metrics: (a) => {
      const memberId = a?.memberId as string | null | undefined;
      const m = memberId && ds.metricsByMember?.[memberId] ? ds.metricsByMember[memberId] : ds.metrics;
      return buildMetricExplanations(m);
    },
    get_cashflow_forecast: (a) =>
      buildCashflowForecast(
        ds.metrics,
        a?.horizonDays as number | undefined,
        a?.bufferCents as number | undefined,
        a?.extraExpenseCents as number | undefined,
      ),
    // ── Scenarios (in-memory) ──
    run_scenario: (a) => {
      const params = (a?.params as AnyRec | null) ?? scParams({ label: String(a?.description ?? "Scenario") });
      const months = Number(a?.months ?? 24);
      return mockDetailFromParams(String(a?.description ?? "Scenario"), params, months);
    },
    save_scenario: (a) => {
      const detail = mockDetailFromParams(String(a?.description ?? "Scenario"), (a?.params as AnyRec) ?? scParams({}), Number(a?.months ?? 24));
      mockScenarios = [detail, ...mockScenarios];
      return detail;
    },
    list_saved_scenarios: () => mockScenarios,
    duplicate_scenario: (a) => {
      const src = mockScenarios.find((s) => s.id === a?.id);
      if (!src) return null;
      const copy = { ...src, id: `sc-${++mockScenarioSeq}`, description: `${src.description} (copy)`, createdAt: isoInDays(0) };
      mockScenarios = [copy, ...mockScenarios];
      return copy;
    },
    archive_scenario: (a) => {
      mockScenarios = mockScenarios.filter((s) => s.id !== a?.id);
      return null;
    },
    promote_scenario: (a) => {
      const s = mockScenarios.find((x) => x.id === a?.id);
      const changes: AnyRec[] = [];
      const delta = Number((s?.params as AnyRec)?.monthlyExpenseDeltaCents ?? 0);
      if (delta !== 0) changes.push({ title: delta < 0 ? "Trim monthly spending" : "Commit more each month", detail: `Adjust your monthly commitments by about $${Math.abs(delta) / 100}.`, currentCents: 388000, proposedCents: 388000 + delta });
      const incPct = Number((s?.params as AnyRec)?.incomeDeltaPct ?? 0);
      if (incPct !== 0) changes.push({ title: "Plan around an income change", detail: `This scenario assumes your monthly income changes by ${incPct}%. Update your plan if that becomes real.`, currentCents: 540000, proposedCents: Math.round(540000 * (1 + incPct / 100)) });
      for (const g of ((s?.currentResult as AnyRec)?.goalsAffected ?? []) as string[]) changes.push({ title: "Revisit a goal", detail: `${g} — adjust its contribution or target if you go ahead.`, currentCents: null, proposedCents: null });
      if (changes.length === 0) changes.push({ title: "No changes to your plan", detail: "This scenario doesn't imply any change to your monthly commitments.", currentCents: null, proposedCents: null });
      return { scenarioId: String(a?.id ?? ""), description: String(s?.description ?? "Scenario"), changes, note: "These are suggestions for your review — nothing has been changed. Apply each one yourself if you decide to go ahead." };
    },
    delete_scenario: (a) => {
      mockScenarios = mockScenarios.filter((s) => s.id !== a?.id);
      return null;
    },
    get_financial_health_score: () => ds.healthScore,
    get_savings_rate_history: () => ds.savingsRateHistory,
    list_categories_with_spending: () => ds.categories,
    get_uncelebrated_milestones: () => ds.milestones,
    list_net_worth_history: (a) => {
      const days = Number(a?.days ?? 180);
      if (ds.netWorthEnd === 0 && ds.accounts.length === 0) return [];
      const months = Math.min(24, Math.max(2, Math.round(days / 30) + 1));
      return netWorthSeries(months, ds.netWorthStart, ds.netWorthEnd);
    },
    list_recurring: () => ds.recurring,
    list_manual_assets: () => ds.manualAssets,
    list_goals: () => ds.goals,
    get_month_totals: () => ds.monthTotals,
    list_household_members: () => ds.members,
    get_onboarding_state: () => ds.onboarding,
    list_action_bundles: () => [],
    list_account_balance_sparklines: () => [],
    get_account_balance_timeline: (a) => {
      const accountId = String(a?.accountId ?? "");
      const account = ds.accounts.find((x) => x.id === accountId);
      if (!account) return null;
      // The real backend refuses both of these, so mirror them or the card looks
      // universally available in the harness when it isn't.
      const refusal =
        account.type === "Investment"
          ? "an investment account's value is its market value, not the sum of its cash flows, so it cannot be reconstructed from transactions"
          : account.simplefin_account_id
            ? "this account is linked to a bank feed, so its balances are bank-reported rather than derived — its recorded balance history is the source of truth"
            : null;
      if (refusal) {
        return {
          accountId,
          accountName: account.name,
          points: [],
          peak: null,
          trough: null,
          currentCents: 0,
          anchor: "assumedZero",
          earliestTxnDate: null,
          reconstructable: false,
          skipReason: refusal,
        };
      }
      // `AccountSummary` is one of the snake_case binding types — `balanceCents`
      // here would silently read undefined and flatten every curve to $0.
      const end = Number(account.balance_cents ?? 0);
      const since = a?.since ? String(a.since) : null;
      // The full history is range-INDEPENDENT; `since` only trims what's
      // returned. Deriving it from the window (as the real backend does not)
      // would make "history starts …" drift with the selected chip.
      const points = balanceSeries(24, end);
      const windowed = since ? points.filter((p) => p.date >= since) : points;
      const series = windowed.length >= 2 ? windowed : points.slice(-2);
      let peak = series[0]!;
      let trough = series[0]!;
      for (const p of series) {
        if (p.balanceCents > peak.balanceCents) peak = p;
        if (p.balanceCents < trough.balanceCents) trough = p;
      }
      // An account with no confirmed balance is exactly the `assumedZero` case:
      // history imported behind a zero opening. Mirroring it here keeps the
      // "dates exact, amounts aren't" caveat visible in the design harness.
      const anchored = account.balance_known !== false;
      return {
        accountId,
        accountName: account.name,
        points: series,
        peak,
        trough,
        currentCents: series[series.length - 1]!.balanceCents,
        anchor: anchored ? "anchoredOpening" : "assumedZero",
        earliestTxnDate: points[0]!.date,
        reconstructable: true,
        skipReason: null,
      };
    },
    list_budget_envelopes: () => ds.budgetEnvelopes,
    list_budget_history: () => ds.budgetHistory,
    list_category_groups: () => ds.categoryGroups,
    get_plan_next_month_data: () => ds.planNextMonthData,
    // mutations — echo a plausible success so optimistic flows don't throw
    create_monthly_review: () => ({ id: "mr-1", year: new Date().getFullYear(), month: new Date().getMonth() + 1, monthLabel: "This month", notes: null, snapshot: {}, createdAt: new Date().toISOString() }),
    contribute_to_goal: () => ({ id: "gc-1" }),
    set_budget: () => null,
    create_category_group: (a) => ({ id: String(a.label ?? "group").toLowerCase().replace(/[^a-z0-9]+/g, "-"), label: a.label, hint: a.hint ?? null, sort_order: ds.categoryGroups.length }),
    set_category_group: () => null,
    apply_next_month_plan: () => null,
    update_goal_monthly: () => null,
  };
}

/**
 * Best-effort default for commands not yet fixtured on the active screen.
 * Returns [] rather than null: many hooks `.map()` their result, and an empty
 * array degrades to an empty list everywhere (object-hooks just read undefined
 * fields) — so an unfixtured screen renders sparse instead of hitting the
 * error boundary. Explicit fixtures above always win.
 */
function fallback(cmd: string): unknown {
  void cmd;
  return [];
}

export function installMockBackend(kindRaw: string | null) {
  const kind = (["rich", "empty", "partial", "large", "multi"].includes(kindRaw ?? "")
    ? kindRaw
    : "rich") as Kind;
  const ds = buildDataset(kind);
  const responders = buildResponders(ds);

  let cbSeq = 0;
  const w = window as unknown as AnyRec;

  const invoke = async (cmd: string, args?: AnyRec): Promise<unknown> => {
    // Tauri core/event plugin traffic — resolve harmlessly.
    if (cmd.startsWith("plugin:")) {
      if (cmd === "plugin:event|listen") return ++cbSeq;
      return null;
    }
    const fn = responders[cmd];
    if (fn) return fn(args ?? {});
    if (!w.__finsightMockWarned) w.__finsightMockWarned = new Set<string>();
    const warned = w.__finsightMockWarned as Set<string>;
    if (!warned.has(cmd)) {
      warned.add(cmd);
      // eslint-disable-next-line no-console
      console.info(`[mock] unfixtured command "${cmd}" → default`);
    }
    return fallback(cmd);
  };

  w.__TAURI_INTERNALS__ = {
    invoke,
    transformCallback: (cb: unknown) => {
      const id = ++cbSeq;
      w[`_${id}`] = cb;
      return id;
    },
    unregisterCallback: () => {},
    // The event plugin's unlisten path reads these; stub them so component
    // unmount (AgentActivityFeed / ImportProgress) doesn't throw in the console.
    unregisterListener: () => {},
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { windowLabel: "main", label: "main" },
    },
  };
  // Some @tauri-apps/api/event builds route unlisten through a dedicated
  // plugin-internals global — provide a no-op so cleanup is silent.
  w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  // Marks this as the mock harness (not a real Tauri bridge) so gates like
  // DesktopConnectGate, which otherwise key off isTauriRuntime(), don't treat
  // the mock as a pre-navigation desktop shell — mirrors __FINSIGHT_HTTP__.
  w.__FINSIGHT_MOCK__ = true;

  // eslint-disable-next-line no-console
  console.info(
    `%c FinSight mock backend active `,
    "background:#C9F950;color:#0A0F02;font-weight:700;border-radius:4px;",
    `dataset="${kind}" — switch with ?mock=rich|empty|partial|large|multi`
  );
}
