/* Reports — widget type registry, defaults, content renderers, chart helpers */

/* ────────────────────────────────────────────────────────
   Widget registry — single source of truth for what widgets
   exist, their default sizes, and what config they accept.
   ──────────────────────────────────────────────────────── */
const WIDGET_TYPES = {
  kpi: {
    name: "Big number",
    desc: "A single metric — savings rate, net worth, runway.",
    defaultW: 1, defaultH: 1, group: "Numbers",
    iconKind: "kpi", defaultConfig: { metric: "savingsRate", accent: true, compare: "lastMonth" }
  },
  sparkkpi: {
    name: "Number + sparkline",
    desc: "A metric with its 12-month trend.",
    defaultW: 2, defaultH: 2, group: "Numbers",
    iconKind: "sparkkpi", defaultConfig: { metric: "expense", compare: "lastYear" }
  },
  yoy: {
    name: "This year vs last",
    desc: "Two lines, side by side, month by month.",
    defaultW: 2, defaultH: 2, group: "Time",
    iconKind: "line", defaultConfig: { metric: "expense", range: "12M" }
  },
  cumulative: {
    name: "Cumulative",
    desc: "Running total YTD against prior year.",
    defaultW: 2, defaultH: 2, group: "Time",
    iconKind: "line", defaultConfig: { metric: "expense" }
  },
  bars: {
    name: "Income vs expense",
    desc: "Bars per month showing in vs out.",
    defaultW: 2, defaultH: 2, group: "Time",
    iconKind: "bar", defaultConfig: { range: "6M" }
  },
  networth: {
    name: "Net worth",
    desc: "Assets, liabilities, and net position over time.",
    defaultW: 4, defaultH: 2, group: "Time",
    iconKind: "line", defaultConfig: { range: "12M" }
  },
  donut: {
    name: "Donut · breakdown",
    desc: "Slice of total by category, merchant, or account.",
    defaultW: 2, defaultH: 2, group: "Breakdown",
    iconKind: "donut", defaultConfig: { dimension: "category", range: "1M" }
  },
  tableCat: {
    name: "Category table",
    desc: "Categories ranked by 12-month total, with trend.",
    defaultW: 4, defaultH: 2, group: "Breakdown",
    iconKind: "table", defaultConfig: { sort: "yearTotal", limit: 8 }
  },
  tableMer: {
    name: "Merchant table",
    desc: "Top merchants with transaction counts.",
    defaultW: 4, defaultH: 2, group: "Breakdown",
    iconKind: "table", defaultConfig: { sort: "total", limit: 10 }
  },
  trends: {
    name: "Category trends",
    desc: "Small line chart per category, side by side.",
    defaultW: 4, defaultH: 2, group: "Breakdown",
    iconKind: "grid", defaultConfig: { limit: 8 }
  },
  sankey: {
    name: "Cash flow · Sankey",
    desc: "Where money came from, where it went.",
    defaultW: 4, defaultH: 3, group: "Cash flow",
    iconKind: "flow", defaultConfig: { month: "current" }
  },
  fire: {
    name: "FIRE calculator",
    desc: "Years until your money replaces your job.",
    defaultW: 4, defaultH: 2, group: "Planning",
    iconKind: "kpi", defaultConfig: {}
  },
  goals: {
    name: "Goals progress",
    desc: "Progress bars for active savings goals.",
    defaultW: 2, defaultH: 2, group: "Planning",
    iconKind: "bar", defaultConfig: { limit: 4 }
  },
  note: {
    name: "Text note",
    desc: "Annotation, link, reminder, or section header.",
    defaultW: 2, defaultH: 1, group: "Planning",
    iconKind: "text", defaultConfig: { body: "Click to edit. Write a note, link a goal, leave a thought for your future self." }
  },
};

/* Order in which groups appear in the library */
const WIDGET_GROUP_ORDER = ["Numbers", "Time", "Breakdown", "Cash flow", "Planning"];

/* Default report layouts — seed data for first-time users */
const DEFAULT_REPORTS = [
  {
    id: "overview",
    name: "Monthly overview",
    icon: "◐",
    widgets: [
      { id: "ov1", type: "kpi", w: 1, h: 1, title: "Savings rate", config: { metric: "savingsRate", accent: true } },
      { id: "ov2", type: "kpi", w: 1, h: 1, title: "Net worth", config: { metric: "netWorth" } },
      { id: "ov3", type: "kpi", w: 1, h: 1, title: "Spent this month", config: { metric: "expense" } },
      { id: "ov4", type: "kpi", w: 1, h: 1, title: "Runway", config: { metric: "runway", accent: true } },
      { id: "ov5", type: "sankey", w: 4, h: 3, title: "Where money came from and went", config: { subtitle: "May 2026 · sources left → sinks right" } },
      { id: "ov6", type: "yoy", w: 2, h: 2, title: "Spending · 2026 vs 2025", config: { metric: "expense" } },
      { id: "ov7", type: "donut", w: 2, h: 2, title: "By category · this month", config: { dimension: "category", range: "1M" } },
      { id: "ov8", type: "tableCat", w: 4, h: 2, title: "Top categories · last 12 months" },
    ]
  },
  {
    id: "wealth",
    name: "Wealth & FIRE",
    icon: "▲",
    widgets: [
      { id: "w1", type: "networth", w: 4, h: 2, title: "Net worth · 12 months", config: { subtitle: "Assets stacked above, liabilities below" } },
      { id: "w2", type: "kpi", w: 1, h: 1, title: "Net worth · YTD", config: { metric: "nwYTD", accent: true } },
      { id: "w3", type: "kpi", w: 1, h: 1, title: "Total assets", config: { metric: "assets" } },
      { id: "w4", type: "kpi", w: 1, h: 1, title: "Liabilities", config: { metric: "liabilities" } },
      { id: "w5", type: "kpi", w: 1, h: 1, title: "Invested", config: { metric: "invested", accent: true } },
      { id: "w6", type: "fire", w: 4, h: 2, title: "Years to financial independence" },
      { id: "w7", type: "goals", w: 2, h: 2, title: "Active goals" },
      { id: "w8", type: "note", w: 2, h: 2, title: "What I want this number to mean", config: { body: "Financial independence isn't retirement. It's choice — the ability to walk away from a thing because it isn't right, not because I'm afraid.\n\nReview each January with Adam." } },
    ]
  },
  {
    id: "spending",
    name: "Spending deep dive",
    icon: "◎",
    widgets: [
      { id: "s1", type: "cumulative", w: 3, h: 2, title: "Cumulative spend · YTD vs 2025" },
      { id: "s2", type: "kpi", w: 1, h: 1, title: "vs last year", config: { metric: "vsLY", accent: true } },
      { id: "s3", type: "tableMer", w: 4, h: 2, title: "Top merchants · last 12 months" },
      { id: "s4", type: "bars", w: 2, h: 2, title: "Income vs expense · 6 months" },
      { id: "s5", type: "donut", w: 2, h: 2, title: "Spend by category" },
      { id: "s6", type: "trends", w: 4, h: 2, title: "Category trends · last 12 months" },
    ]
  }
];

/* ────────────────────────────────────────────────────────
   Metric registry — KPIs, sparklines, all the things you
   can pin to a number widget.
   ──────────────────────────────────────────────────────── */
const METRIC_OPTIONS = [
  { id: "savingsRate", label: "Savings rate",          group: "Cash flow" },
  { id: "saved",       label: "Saved this month",      group: "Cash flow" },
  { id: "income",      label: "Income · this month",   group: "Cash flow" },
  { id: "expense",     label: "Expense · this month",  group: "Cash flow" },
  { id: "avgSpend",    label: "Average monthly spend", group: "Cash flow" },
  { id: "vsLY",        label: "Spend vs last year",    group: "Cash flow" },
  { id: "netWorth",    label: "Net worth",             group: "Wealth" },
  { id: "nwYTD",       label: "Net worth · YTD change",group: "Wealth" },
  { id: "assets",      label: "Total assets",          group: "Wealth" },
  { id: "liabilities", label: "Total liabilities",     group: "Wealth" },
  { id: "invested",    label: "Invested",              group: "Wealth" },
  { id: "runway",      label: "Months of runway",      group: "Cash flow" },
  { id: "txnCount",    label: "Transaction count",     group: "Activity" },
];

function metricValue(metric, scope = "month", compare = "lastMonth") {
  const y = FS.yoySpend.thisYear;      // [Jan..Dec] this year (5 filled)
  const ly = FS.yoySpend.lastYear;     // [Jan..Dec] prior year
  const months = FS.yoySpend.months;
  const monthlyInc = 10000;
  const tFilled = y.filter(v => v != null);
  const i = tFilled.length - 1;        // current month idx (4 = May)
  const cm = months[i];
  const sum = (arr, from, to) => arr.slice(from, to + 1).reduce((s, v) => s + (v || 0), 0);
  const last = FS.netWorthLong[FS.netWorthLong.length - 1];
  const jan = FS.netWorthLong[7];      // Jan '26
  const yearAgo = FS.netWorthLong[0];  // Jun '25
  const prevMo = FS.netWorthLong[FS.netWorthLong.length - 2];
  const nw = last.assets + last.liab;
  const nwJan = jan.assets + jan.liab;
  const nwYearAgo = yearAgo.assets + yearAgo.liab;
  const nwPrev = prevMo.assets + prevMo.liab;

  // Scope-aware aggregations
  function curExpense() {
    if (scope === "month")   return y[i];
    if (scope === "quarter") return sum(y, Math.max(0, i - 2), i);
    if (scope === "year")    return sum(y, 0, i);
    if (scope === "all")     return sum(y, 0, i) + sum(ly, 0, 11);
  }
  function priorExpense() {
    const mode = compare;
    if (scope === "month") {
      if (mode === "lastYear")  return ly[i];
      if (mode === "ytd")       return Math.round(sum(y, 0, i - 1) / Math.max(1, i));
      return y[i - 1] ?? null;   // lastMonth default
    }
    if (scope === "quarter") {
      if (mode === "lastYear")  return sum(ly, Math.max(0, i - 2), i);
      return sum(y, Math.max(0, i - 5), i - 3) || null;
    }
    if (scope === "year")  return sum(ly, 0, i);
    if (scope === "all")   return null;
    return null;
  }
  function curIncome() {
    if (scope === "month")   return monthlyInc;
    if (scope === "quarter") return monthlyInc * 3;
    if (scope === "year")    return monthlyInc * (i + 1);
    if (scope === "all")     return monthlyInc * (i + 1 + 12);
  }
  function priorIncome() {
    if (scope === "month")   return monthlyInc;
    if (scope === "quarter") return monthlyInc * 3;
    if (scope === "year")    return monthlyInc * (i + 1);
    return null;
  }

  const fmtMoney = (v) => (v < 0 ? "−" : "") + "$" + Math.abs(v).toLocaleString();
  const signed   = (v) => (v > 0 ? "+$" : v < 0 ? "−$" : "$") + Math.abs(v).toLocaleString();
  const prevLabel = {
    lastMonth: "vs " + (months[i - 1] || "prior"),
    lastYear:  scope === "year" ? "vs 2025 YTD" : ("vs " + cm + " '25"),
    ytd:       "vs YTD avg",
    none:      "",
  }[compare] || "";
  const scopeShort = scope === "month" ? cm
                   : scope === "quarter" ? "Q2"
                   : scope === "year" ? "YTD"
                   : "all-time";

  switch (metric) {
    case "savingsRate": {
      const inc = curIncome(), exp = curExpense();
      const rate = Math.round((inc - exp) / inc * 100);
      const pExp = priorExpense();
      const pInc = priorIncome();
      if (pExp == null || pInc == null) return { display: rate, suffix: "%", delta: scopeShort, tone: null };
      const pRate = Math.round((pInc - pExp) / pInc * 100);
      const d = rate - pRate;
      return { display: rate, suffix: "%", delta: (d >= 0 ? "+" : "") + d + " pts " + prevLabel, tone: d >= 0 ? "pos" : "neg" };
    }
    case "saved": {
      const val = curIncome() - curExpense();
      const pExp = priorExpense();
      if (pExp == null) return { display: fmtMoney(val), delta: scopeShort, tone: "pos" };
      const prev = priorIncome() - pExp;
      const d = val - prev;
      return { display: fmtMoney(val), delta: signed(d) + " " + prevLabel, tone: d >= 0 ? "pos" : "neg" };
    }
    case "income": {
      const val = curIncome();
      return { display: fmtMoney(val), delta: scopeShort + " income", tone: null };
    }
    case "expense": {
      const val = curExpense();
      const prev = priorExpense();
      if (prev == null) return { display: fmtMoney(val), delta: scopeShort, tone: null };
      const d = val - prev;
      return { display: fmtMoney(val), delta: signed(d) + " " + prevLabel, tone: d <= 0 ? "pos" : "neg" };
    }
    case "avgSpend": {
      const filled = y.filter(v => v != null);
      const avg = Math.round(filled.reduce((s, v) => s + v, 0) / filled.length);
      return { display: fmtMoney(avg), delta: "last " + filled.length + " months", tone: null };
    }
    case "vsLY": {
      const tx = sum(y, 0, i), lyTx = sum(ly, 0, i);
      const diff = tx - lyTx;
      return { display: signed(diff), delta: "Cumulative " + scopeShort, tone: diff < 0 ? "pos" : "neg" };
    }
    case "netWorth": {
      let prev = nwPrev, label = "vs " + (months[i - 1] || "prior");
      if (compare === "lastYear") { prev = nwYearAgo; label = "YoY"; }
      else if (compare === "ytd") { prev = nwJan; label = "YTD"; }
      const pct = Math.round((nw - prev) / prev * 100);
      return { display: "$" + Math.round(nw / 1000).toLocaleString() + "k", delta: (pct >= 0 ? "+" : "") + pct + "% " + label, tone: pct >= 0 ? "pos" : "neg" };
    }
    case "nwYTD": {
      const diff = nw - nwJan;
      const pct = Math.round(diff / nwJan * 100);
      return { display: signed(diff), delta: (pct >= 0 ? "+" : "") + pct + "% YTD", tone: diff >= 0 ? "pos" : "neg" };
    }
    case "assets":
      return { display: "$" + Math.round(last.assets / 1000).toLocaleString() + "k", delta: scope === "year" ? "+19% YTD" : "+1.1% MoM", tone: "pos" };
    case "liabilities":
      return { display: "$" + Math.round(Math.abs(last.liab) / 1000).toLocaleString() + "k", delta: scope === "year" ? "−2% YTD" : "−0.3% MoM", tone: "pos" };
    case "invested":
      return { display: "$" + (FS.totals.invested / 1000).toFixed(1) + "k", delta: "+$3.2k last 30d", tone: "pos" };
    case "runway":
      return { display: "22", suffix: " mo", delta: "liquid + invested / spend", tone: null };
    case "txnCount":
      return { display: FS.transactions.length, suffix: " txns", delta: "this " + scopeShort.toLowerCase(), tone: null };
    default:
      return { display: "—" };
  }
}

/* Trend series for sparklines — one per metric */
function metricSeries(metric) {
  switch (metric) {
    case "expense": return FS.yoySpend.thisYear.filter(v => v != null);
    case "income": return FS.incomeExpense.income;
    case "saved": return FS.incomeExpense.income.map((i, k) => i - FS.incomeExpense.expense[k]);
    case "netWorth":
    case "nwYTD":
    case "assets":
    case "liabilities":
    case "invested":
      return FS.netWorthLong.map(d => d.assets + d.liab);
    case "savingsRate":
      return FS.incomeExpense.income.map((i, k) => Math.round((i - FS.incomeExpense.expense[k]) / i * 100));
    default:
      return FS.netWorthHistory.map(d => d.v);
  }
}

/* ────────────────────────────────────────────────────────
   Widget content dispatcher
   ──────────────────────────────────────────────────────── */
function WidgetContent({ widget, scope = "month" }) {
  switch (widget.type) {
    case "kpi":        return <KPIWidget cfg={widget.config || {}} scope={scope} />;
    case "sparkkpi":   return <SparkKPIWidget cfg={widget.config || {}} scope={scope} />;
    case "yoy":        return <YoYWidget cfg={widget.config || {}} scope={scope} />;
    case "cumulative": return <CumulativeWidget cfg={widget.config || {}} scope={scope} />;
    case "bars":       return <BarsWidget cfg={widget.config || {}} scope={scope} />;
    case "donut":      return <DonutWidget cfg={widget.config || {}} scope={scope} />;
    case "tableCat":   return <TableCatWidget cfg={widget.config || {}} scope={scope} />;
    case "tableMer":   return <TableMerWidget cfg={widget.config || {}} scope={scope} />;
    case "sankey":     return <SankeyWidget cfg={widget.config || {}} scope={scope} />;
    case "networth":   return <NetWorthWidget cfg={widget.config || {}} scope={scope} />;
    case "trends":     return <TrendsWidget cfg={widget.config || {}} scope={scope} />;
    case "fire":       return <FireWidget cfg={widget.config || {}} scope={scope} />;
    case "goals":      return <GoalsWidget cfg={widget.config || {}} scope={scope} />;
    case "note":       return <NoteWidget cfg={widget.config || {}} scope={scope} />;
    default:           return <div className="muted" style={{ fontSize: 12 }}>Unknown widget · {widget.type}</div>;
  }
}

/* ────────────────────────────────────────────────────────
   Widget renderers
   ──────────────────────────────────────────────────────── */

function KPIWidget({ cfg, scope }) {
  const m = metricValue(cfg.metric || "savingsRate", scope, cfg.compare || "lastMonth");
  return (
    <div className={`w-kpi ${cfg.accent ? "accent" : ""}`}>
      <div className="value">
        {m.display}{m.suffix && <span className="small">{m.suffix}</span>}
      </div>
      {m.delta && (
        <div className="delta-row">
          <span className={`npill ${m.tone || ""}`}>{m.delta}</span>
        </div>
      )}
    </div>
  );
}

function SparkKPIWidget({ cfg, scope }) {
  const m = metricValue(cfg.metric || "expense", scope, cfg.compare || "lastYear");
  const range = cfg.range || "12M";
  let series = metricSeries(cfg.metric || "expense");
  const limit = { "3M": 3, "6M": 6, "12M": 12, "YTD": 5, "All": series.length }[range] || series.length;
  series = series.slice(-limit);
  return (
    <div className="w-spark-kpi">
      <div>
        <div className="w-kpi accent" style={{ flex: "none" }}>
          <div className="value">
            {m.display}{m.suffix && <span className="small">{m.suffix}</span>}
          </div>
          {m.delta && (
            <div className="delta-row">
              <span className={`npill ${m.tone || ""}`}>{m.delta}</span>
              <span className="muted" style={{ fontSize: 11, fontFamily: "var(--mono)", letterSpacing: "0.04em" }}>{range} TREND</span>
            </div>
          )}
        </div>
      </div>
      <div className="spark-area">
        <Sparkline values={series} color="var(--accent)" height={70} fill={true} />
      </div>
    </div>
  );
}

function YoYWidget({ cfg, scope }) {
  return <div style={{ flex: 1, minHeight: 0 }}><YoYChart data={FS.yoySpend} metric={cfg.metric || "expense"} compare={cfg.compare || "lastYear"} /></div>;
}

function CumulativeWidget({ cfg, scope }) {
  return <div style={{ flex: 1, minHeight: 0 }}><CumulativeChart data={FS.cumulative} metric={cfg.metric || "expense"} compare={cfg.compare || "lastYear"} /></div>;
}

function BarsWidget({ cfg, scope }) {
  return <div style={{ flex: 1, minHeight: 0 }}><IncomeExpenseBars data={FS.incomeExpense} range={cfg.range || "6M"} /></div>;
}

function NetWorthWidget({ cfg, scope }) {
  return <div style={{ flex: 1, minHeight: 0 }}><NetWorthLong data={FS.netWorthLong} range={cfg.range || "12M"} stack={cfg.stack !== false} showNet={cfg.showNet !== false} /></div>;
}

function SankeyWidget({ cfg, scope }) {
  // Period scales totals; "current" = real May data
  const periodMul = cfg.month === "ytd" ? 5 : cfg.month === "12M" ? 12 : 1;
  const periodLabel = cfg.month === "ytd" ? "YTD 2026" : cfg.month === "12M" ? "Last 12 months" : "May 2026";
  return <div style={{ flex: 1, minHeight: 0 }}>
    <SankeyView
      income={FS.sankey.income.map(x => ({ ...x, amount: x.amount * periodMul }))}
      expense={FS.sankey.expense.map(x => ({ ...x, amount: x.amount * periodMul }))}
      save={FS.sankey.save * periodMul}
      highlightSave={cfg.highlightSave !== false}
      periodLabel={periodLabel}
    />
  </div>;
}

function TrendsWidget({ cfg, scope }) {
  const limit = cfg.limit || 8;
  const cats = FS.categories.filter(c => c.yearAvg > 0).sort((a, b) => b.yearAvg - a.yearAvg).slice(0, limit);
  const cols = limit <= 4 ? 2 : limit <= 6 ? 3 : 4;
  return (
    <div style={{ display: "grid", gridTemplateColumns: `repeat(${cols}, 1fr)`, gap: 10, flex: 1 }}>
      {cats.map(c => <CategoryTrend key={c.id} cat={c} />)}
    </div>
  );
}

function DonutWidget({ cfg, scope }) {
  const dim = cfg.dimension || "category";
  const exclude = cfg.exclude || [];
  const range = cfg.range || "1M";
  const rangeMul = { "1M": 1, "3M": 3, "6M": 6, "YTD": 5, "12M": 12 }[range] || 1;
  let items;
  if (dim === "merchant") {
    items = [...FS.topMerchants]
      .filter(m => !exclude.includes(m.cat))
      .sort((a, b) => b.total - a.total)
      .slice(0, 8)
      .map(m => ({
        id: m.name,
        label: m.name,
        amount: range === "12M" ? m.total : Math.round(m.total * (rangeMul / 12)),
        color: FS.categories.find(c => c.id === m.cat)?.color || "#8B8B95"
      }));
  } else if (dim === "account") {
    items = FS.accounts.filter(a => a.balance > 0).map(a => ({
      id: a.id, label: a.name, amount: a.balance, color: a.color
    }));
  } else {
    items = FS.categories
      .filter(c => c.thisMonth > 0 && !exclude.includes(c.id))
      .sort((a, b) => b.thisMonth - a.thisMonth)
      .map(c => ({
        id: c.id,
        label: c.label,
        amount: range === "1M" ? c.thisMonth : Math.round(c.yearAvg * rangeMul),
        color: c.color
      }));
  }
  if (!items.length) {
    return <div className="muted" style={{ fontSize: 13, padding: 24 }}>Everything filtered out. Add a category back in the configurator.</div>;
  }
  const total = items.reduce((s, c) => s + c.amount, 0);
  const R = 64, r = 40;
  let acc = 0;
  const arcs = items.map(c => {
    const start = acc / total * Math.PI * 2;
    acc += c.amount;
    const end = acc / total * Math.PI * 2;
    return { ...c, start, end, pct: c.amount / total };
  });
  const arcPath = (s, e) => {
    if (e - s < 0.0001) return "";
    if (e - s >= Math.PI * 2 - 0.0001) {
      return `M 80,${80 - R} A ${R},${R} 0 1 1 79.99,${80 - R} L 79.99,${80 - r} A ${r},${r} 0 1 0 80,${80 - r} Z`;
    }
    const cx = 80, cy = 80;
    const x1 = cx + R * Math.sin(s), y1 = cy - R * Math.cos(s);
    const x2 = cx + R * Math.sin(e), y2 = cy - R * Math.cos(e);
    const x3 = cx + r * Math.sin(e), y3 = cy - r * Math.cos(e);
    const x4 = cx + r * Math.sin(s), y4 = cy - r * Math.cos(s);
    const large = (e - s) > Math.PI ? 1 : 0;
    return `M${x1},${y1} A${R},${R} 0 ${large} 1 ${x2},${y2} L${x3},${y3} A${r},${r} 0 ${large} 0 ${x4},${y4} Z`;
  };
  return (
    <div className="w-donut">
      <svg viewBox="0 0 160 160" width="160" height="160" style={{ flexShrink: 0 }}>
        {arcs.map(a => (
          <path key={a.id} d={arcPath(a.start, a.end)} fill={a.color} opacity="0.92" />
        ))}
        <text x="80" y="68" textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 10, letterSpacing: "0.08em", fill: "var(--ink-faint)" }}>
          {range}
        </text>
        <text x="80" y="86" textAnchor="middle" style={{ fontFamily: "var(--sans)", fontSize: 17, fontWeight: 600, fill: "var(--ink)" }}>
          ${total.toLocaleString()}
        </text>
        <text x="80" y="100" textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 9.5, letterSpacing: "0.04em", fill: "var(--ink-faint)" }}>
          {arcs.length} {dim}{arcs.length === 1 ? "" : "s"}
        </text>
      </svg>
      <div className="legend">
        {arcs.map(a => (
          <div key={a.id} className="row">
            <span className="cswatch" style={{ background: a.color }}></span>
            <span style={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{a.label}</span>
            <span className="pct">{Math.round(a.pct * 100)}% · ${a.amount.toLocaleString()}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

function TableCatWidget({ cfg, scope }) {
  const limit = cfg.limit || 8;
  const sortField = cfg.sort || "yearTotal";
  let cats = FS.categories.filter(c => c.yearAvg > 0);
  cats.sort((a, b) => {
    if (sortField === "thisMonth") return b.thisMonth - a.thisMonth;
    if (sortField === "delta") return Math.abs(b.thisMonth - b.yearAvg) - Math.abs(a.thisMonth - a.yearAvg);
    return b.yearAvg - a.yearAvg;
  });
  cats = cats.slice(0, limit);
  return (
    <table className="tbl" style={{ marginTop: 8 }}>
      <thead>
        <tr>
          <th>Category</th>
          <th className="right">12-mo total</th>
          <th className="right">Avg / month</th>
          <th>Trend</th>
          <th className="right">vs prior year</th>
        </tr>
      </thead>
      <tbody>
        {cats.map(c => {
          const delta = c.thisMonth - c.yearAvg;
          return (
            <tr key={c.id}>
              <td>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <span className="cswatch" style={{ background: c.color }}></span>
                  {c.label}
                </div>
              </td>
              <td className="right num tabular">${(c.yearAvg * 12).toLocaleString()}</td>
              <td className="right num tabular muted">${c.yearAvg.toLocaleString()}</td>
              <td><MiniTrend color={c.color} seed={c.id} /></td>
              <td className={`right num tabular ${delta < 0 ? "pos" : "neg"}`}>
                {delta < 0 ? "↓" : "↑"} ${Math.abs(delta).toLocaleString()}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function TableMerWidget({ cfg, scope }) {
  const limit = cfg.limit || 10;
  const sortField = cfg.sort || "total";
  let list = [...FS.topMerchants];
  if (sortField === "count") list.sort((a, b) => b.txns - a.txns);
  else list.sort((a, b) => b.total - a.total);
  list = list.slice(0, limit);
  const max = list[0]?.total || 1;
  return (
    <table className="tbl" style={{ marginTop: 8 }}>
      <thead>
        <tr>
          <th>Merchant</th>
          <th>Category</th>
          <th className="right">Txns</th>
          <th>Distribution</th>
          <th className="right">Total spent</th>
        </tr>
      </thead>
      <tbody>
        {list.map((m, i) => {
          const cat = FS.categories.find(c => c.id === m.cat);
          const merLogo = FS.merchants[m.name] || { bg: cat?.color || "#444", short: m.name.slice(0, 2) };
          return (
            <tr key={m.name}>
              <td>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <div style={{ width: 24, height: 24, borderRadius: 6, background: merLogo.bg, color: "#fff", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 600 }}>{merLogo.short}</div>
                  <div>
                    <div style={{ fontSize: 13.5, fontWeight: 500 }}>{m.name}</div>
                    <div className="muted" style={{ fontSize: 11, fontFamily: "var(--mono)" }}>#{i + 1}</div>
                  </div>
                </div>
              </td>
              <td>
                <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
                  <span className="cswatch" style={{ background: cat?.color }}></span>
                  <span style={{ fontSize: 13 }}>{cat?.label}</span>
                </div>
              </td>
              <td className="right num tabular">{m.txns}</td>
              <td>
                <div style={{ height: 5, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", maxWidth: 180 }}>
                  <div style={{ width: `${(m.total / max) * 100}%`, height: "100%", background: cat?.color, borderRadius: 999 }} />
                </div>
              </td>
              <td className="right num tabular figure" style={{ fontSize: 14 }}>${m.total.toLocaleString()}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

function GoalsWidget({ cfg, scope }) {
  const limit = cfg.limit || 4;
  const goals = FS.goals.filter(g => g.type !== "spending-cap").slice(0, limit);
  return (
    <div className="w-goals">
      {goals.map(g => {
        const pct = Math.min(100, g.current / g.target * 100);
        return (
          <div key={g.id} className="goal">
            <div className="head">
              <span className="name">{g.name}</span>
              <span className="amt">${g.current.toLocaleString()} / ${g.target.toLocaleString()}</span>
            </div>
            <div className="goal-bar"><span style={{ width: pct + "%" }}></span></div>
            <div className="foot">{g.pace} · ETA {g.eta}</div>
          </div>
        );
      })}
    </div>
  );
}

function NoteWidget({ cfg, scope }) {
  const body = cfg.body || "Empty note. Open the configurator to write something.";
  return <div className="w-note">{body}</div>;
}

function FireWidget({ cfg, scope }) {
  const defaults = {
    ae: cfg.expenses || 72000,
    as: cfg.save || (FS.sankey.save * 12),
    rr: cfg.returnRate || 7,
    wr: cfg.withdrawalRate || 4,
    nw: cfg.currentNW || FS.totals.invested,
  };
  const [ae, setAE] = React.useState(defaults.ae);
  const [as, setAS] = React.useState(defaults.as);
  const [rr, setRR] = React.useState(defaults.rr);
  const [wr, setWR] = React.useState(defaults.wr);
  const [nwSt, setNW] = React.useState(defaults.nw);

  const target = ae * (100 / wr);
  const r = rr / 100;
  let yrs = 0, n = nwSt;
  while (n < target && yrs < 80) { n = n * (1 + r) + as; yrs++; }

  const reset = () => {
    setAE(defaults.ae); setAS(defaults.as); setRR(defaults.rr); setWR(defaults.wr); setNW(defaults.nw);
    window.toast?.("FIRE inputs reset", { kind: "info" });
  };
  const saveScenario = () => {
    window.toast?.("Scenario saved", {
      sub: `${yrs} years · $${Math.round(target / 1000)}k target`,
      kind: "success",
      action: { label: "View", onClick: () => window.navigate?.("scenarios") },
    });
  };

  return (
    <div style={{ display: "grid", gridTemplateColumns: "1.1fr 1fr", gap: 20, flex: 1, minHeight: 0 }}>
      <div style={{ display: "flex", flexDirection: "column" }}>
        <FireInput label="Annual expenses" value={ae} setValue={setAE} min={20000} max={250000} step={1000} prefix="$" />
        <FireInput label="Annual savings" value={as} setValue={setAS} min={0} max={200000} step={500} prefix="$" />
        <FireInput label="Current invested" value={nwSt} setValue={setNW} min={0} max={1000000} step={1000} prefix="$" />
        <FireInput label="Real return" value={rr} setValue={setRR} min={1} max={12} step={0.5} suffix="%" />
        <FireInput label="Withdrawal rate" value={wr} setValue={setWR} min={2} max={6} step={0.25} suffix="%" />
      </div>
      <div style={{ display: "flex", flexDirection: "column", justifyContent: "center", padding: "16px 20px", background: "linear-gradient(180deg, var(--accent-2) 0%, transparent 60%)", borderRadius: 10, border: "1px solid var(--accent-3)" }}>
        <div className="eyebrow" style={{ marginBottom: 12 }}>Years to FI</div>
        <div className="figure" style={{ fontSize: 72, lineHeight: 1, letterSpacing: "-0.045em" }}>
          {yrs >= 80 ? "—" : yrs}<span style={{ fontSize: 20, color: "var(--ink-mute)", marginLeft: 8, fontWeight: 500 }}>years</span>
        </div>
        <div className="muted" style={{ fontSize: 12.5, marginTop: 12, lineHeight: 1.55 }}>
          Portfolio target <span className="strong">${Math.round(target).toLocaleString()}</span> at {rr}% real, {wr}% withdrawal.
        </div>
        <div style={{ marginTop: 16, display: "flex", gap: 6 }}>
          <button className="btn outline sm" onClick={(e) => { e.stopPropagation(); saveScenario(); }}>
            <I.Sparkle width="12" height="12" /> Save as scenario
          </button>
          <button className="btn ghost sm" onClick={(e) => { e.stopPropagation(); reset(); }}>
            <I.Refresh width="12" height="12" /> Reset
          </button>
        </div>
      </div>
    </div>
  );
}

/* ────────────────────────────────────────────────────────
   Chart helpers — reused across widgets
   ──────────────────────────────────────────────────────── */

function NetWorthLong({ data, range = "12M", stack = true, showNet = true }) {
  // Slice data to range. We have 12 months of real data; 3Y/5Y/All show same span
  // with a label hint, since we don't have deeper history.
  const sliceMap = { "6M": 6, "12M": 12, "3Y": 12, "5Y": 12, "All": 12 };
  const n = Math.min(data.length, sliceMap[range] || 12);
  const sliced = data.slice(-n);

  const W = 1080, H = 280;
  const PAD_L = 50, PAD_R = 16, PAD_T = 28, PAD_B = 32;
  const innerW = W - PAD_L - PAD_R;
  const innerH = H - PAD_T - PAD_B;
  const max = Math.max(...sliced.map(d => d.assets));
  const min = Math.min(...sliced.map(d => d.liab));
  const range_ = max - min || 1;
  const xFor = (i) => PAD_L + (i / (sliced.length - 1)) * innerW;
  const yFor = (v) => PAD_T + (1 - (v - min) / range_) * innerH;
  const zeroY = yFor(0);
  const assetsPath = sliced.map((d, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(d.assets)}`).join(" ");
  const assetsArea = `${assetsPath} L${xFor(sliced.length - 1)},${zeroY} L${PAD_L},${zeroY} Z`;
  const liabPath = sliced.map((d, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(d.liab)}`).join(" ");
  const liabArea = `${liabPath} L${xFor(sliced.length - 1)},${zeroY} L${PAD_L},${zeroY} Z`;
  const netPath = sliced.map((d, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(d.assets + d.liab)}`).join(" ");
  const lastNW = sliced[sliced.length - 1].assets + sliced[sliced.length - 1].liab;
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      <defs>
        <linearGradient id="aGrad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.4" />
          <stop offset="100%" stopColor="var(--accent)" stopOpacity="0.04" />
        </linearGradient>
        <linearGradient id="lGrad" x1="0" y1="1" x2="0" y2="0">
          <stop offset="0%" stopColor="var(--negative)" stopOpacity="0.36" />
          <stop offset="100%" stopColor="var(--negative)" stopOpacity="0.04" />
        </linearGradient>
      </defs>
      <line x1={PAD_L} x2={W - PAD_R} y1={zeroY} y2={zeroY} stroke="rgba(255,255,255,0.1)" strokeDasharray="3 3" />
      {stack && <>
        <path d={assetsArea} fill="url(#aGrad)" />
        <path d={assetsPath} stroke="var(--accent)" strokeWidth="1.5" fill="none" />
        <path d={liabArea} fill="url(#lGrad)" />
        <path d={liabPath} stroke="var(--negative)" strokeWidth="1.5" fill="none" />
      </>}
      {showNet && <>
        <path d={netPath} stroke="var(--ink)" strokeWidth="2.2" fill="none" strokeLinecap="round" strokeLinejoin="round" />
        <circle cx={xFor(sliced.length - 1)} cy={yFor(lastNW)} r="5" fill="var(--ink)" stroke="var(--bg)" strokeWidth="2" />
      </>}
      {sliced.map((d, i) => i % 2 === 0 && (
        <text key={i} x={xFor(i)} y={H - 10} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)" }}>{d.m}</text>
      ))}
      {[0, 0.5, 1].map((t, i) => {
        const v = min + range_ * (1 - t);
        return (
          <text key={i} x={PAD_L - 8} y={PAD_T + innerH * t + 3} textAnchor="end" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)" }}>
            ${Math.round(v / 1000)}k
          </text>
        );
      })}
      <g transform={`translate(${PAD_L} 4)`}>
        <text x="0" y="0" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)", letterSpacing: "0.08em" }}>{range.toUpperCase()} · {sliced.length} months</text>
        {stack && <>
          <rect x="110" y="-6" width="10" height="3" fill="var(--accent)" />
          <text x="124" y="-2" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>assets</text>
          <rect x="170" y="-6" width="10" height="3" fill="var(--negative)" />
          <text x="184" y="-2" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>liabilities</text>
        </>}
        {showNet && <>
          <rect x="248" y="-6" width="10" height="3" fill="var(--ink)" />
          <text x="262" y="-2" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>net</text>
        </>}
      </g>
    </svg>
  );
}

function CumulativeChart({ data, metric = "expense", compare = "lastYear" }) {
  // For income/net: synthesize cumulative income / net flow
  const incomeMonthly = 10000;
  let thisY, lastY;
  if (metric === "income") {
    // Cumulative income: 10k * monthIndex+1, both years equal
    thisY = data.thisYear.map((v, i) => v == null ? null : incomeMonthly * (i + 1));
    lastY = data.lastYear.map((_, i) => incomeMonthly * (i + 1) - 500 * (i + 1));   // ~9500/mo last year
  } else if (metric === "net") {
    // Cumulative net = income - expense
    let t = 0, l = 0;
    thisY = data.thisYear.map((v, i) => v == null ? null : (t += (incomeMonthly - v), t));
    lastY = data.lastYear.map((v, i) => (l += (9500 - v), l));
  } else {
    thisY = data.thisYear;
    lastY = data.lastYear;
  }
  // Compare line override
  if (compare === "average") {
    const last12 = lastY.filter(v => v != null);
    const avg = last12.reduce((s, v) => s + v, 0) / last12.length;
    // Average is a flat line at the running 12-month avg
    lastY = lastY.map(() => avg);
  } else if (compare === "budget") {
    const monthlyBudget = Object.values(FS.budgetGrid).reduce((s, g) => s + g.may.b, 0);
    if (metric === "expense") {
      lastY = lastY.map((_, i) => monthlyBudget * (i + 1));
    }
  }

  const W = 600, H = 220;
  const PAD_L = 44, PAD_R = 14, PAD_T = 16, PAD_B = 30;
  const innerW = W - PAD_L - PAD_R;
  const innerH = H - PAD_T - PAD_B;
  const all = [...thisY.filter(v => v != null), ...lastY];
  const max = Math.max(...all) * 1.06;
  const xFor = (i) => PAD_L + (i / 11) * innerW;
  const yFor = (v) => PAD_T + (1 - v / max) * innerH;
  const lineFor = (arr) => arr.map((v, i) => v == null ? "" : `${i === 0 || arr[i - 1] == null ? "M" : "L"}${xFor(i)},${yFor(v)}`).join(" ");
  const compareLabel = { lastYear: "2025", average: "12-mo avg", budget: "Budget" }[compare] || "prior";
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      {[0.25, 0.5, 0.75].map((t, i) => (
        <line key={i} x1={PAD_L} x2={W - PAD_R} y1={PAD_T + innerH * t} y2={PAD_T + innerH * t} stroke="rgba(255,255,255,0.05)" />
      ))}
      <path d={lineFor(lastY)} stroke="var(--ink-faint)" strokeWidth="1.5" fill="none" strokeDasharray="3 4" />
      <path d={lineFor(thisY)} stroke="var(--accent)" strokeWidth="2" fill="none" />
      {thisY.map((v, i) => v != null && i === thisY.filter(x => x != null).length - 1 && (
        <circle key={i} cx={xFor(i)} cy={yFor(v)} r="4" fill="var(--accent)" stroke="var(--bg)" strokeWidth="2" />
      ))}
      {data.months.map((m, i) => (
        <text key={i} x={xFor(i)} y={H - 10} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)" }}>{m}</text>
      ))}
      {[0.25, 0.5, 0.75].map((t, i) => (
        <text key={i} x={PAD_L - 6} y={PAD_T + innerH * t + 3} textAnchor="end" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)" }}>${Math.round(max * (1 - t) / 1000)}k</text>
      ))}
      <g transform={`translate(${PAD_L} 4)`}>
        <rect x="0" y="-2" width="14" height="3" fill="var(--accent)" rx="1" />
        <text x="20" y="2" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>{metric}</text>
        <rect x="86" y="-2" width="14" height="3" fill="var(--ink-faint)" rx="1" />
        <text x="106" y="2" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>{compareLabel}</text>
      </g>
    </svg>
  );
}

function IncomeExpenseBars({ data, range = "6M" }) {
  // We have 6 months in the data. For 3M show last 3; for 12M/All show what we have.
  const sliceMap = { "3M": 3, "6M": 6, "12M": 6, "YTD": 5, "All": 6 };
  const n = Math.min(data.months.length, sliceMap[range] || 6);
  const months = data.months.slice(-n);
  const income = data.income.slice(-n);
  const expense = data.expense.slice(-n);

  const W = 400, H = 220;
  const PAD_L = 16, PAD_R = 14, PAD_T = 16, PAD_B = 30;
  const innerW = W - PAD_L - PAD_R;
  const innerH = H - PAD_T - PAD_B;
  const max = Math.max(...income) * 1.1;
  const barW = innerW / months.length;
  const yFor = (v) => PAD_T + innerH - (v / max) * innerH;
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      {months.map((m, i) => {
        const x = PAD_L + i * barW;
        const incomeY = yFor(income[i]);
        const expenseY = yFor(expense[i]);
        const incomeH = (income[i] / max) * innerH;
        const expenseH = (expense[i] / max) * innerH;
        return (
          <g key={i}>
            <rect x={x + 6} y={incomeY} width={barW - 12} height={incomeH} fill="var(--ink-faint)" opacity="0.18" rx="2" />
            <rect x={x + 6} y={expenseY} width={barW - 12} height={expenseH} fill="var(--accent)" rx="2" />
            <text x={x + barW / 2} y={H - 10} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: i === months.length - 1 ? "var(--accent)" : "var(--ink-faint)" }}>{m}</text>
          </g>
        );
      })}
      <g transform={`translate(${PAD_L} 0)`}>
        <rect x="0" y="2" width="10" height="3" fill="var(--accent)" />
        <text x="14" y="6" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>spent</text>
        <rect x="56" y="2" width="10" height="3" fill="var(--ink-faint)" opacity="0.4" />
        <text x="70" y="6" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>income</text>
        <text x={W - PAD_R - 8 - 36} y="6" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)", letterSpacing: "0.06em" }}>{range.toUpperCase()}</text>
      </g>
    </svg>
  );
}

function CategoryTrend({ cat }) {
  const W = 220, H = 70;
  const seed = cat.id.charCodeAt(0);
  const pts = Array.from({ length: 12 }, (_, i) => {
    const noise = (Math.sin(i * 0.8 + seed) * 0.15 + Math.cos(i * 1.3) * 0.08);
    return Math.max(0, cat.yearAvg * (1 + noise));
  });
  const max = Math.max(...pts) * 1.1;
  const xFor = (i) => 6 + (i / 11) * (W - 12);
  const yFor = (v) => 8 + (1 - v / max) * (H - 22);
  const path = pts.map((p, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(p)}`).join(" ");
  const area = `${path} L${xFor(11)},${H - 14} L${xFor(0)},${H - 14} Z`;
  return (
    <div className="card tight" style={{ padding: 14, background: "var(--surface-2)", border: "1px solid var(--line)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
          <span className="cswatch" style={{ background: cat.color }}></span>
          <span style={{ fontSize: 13.5 }}>{cat.label}</span>
        </div>
        <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>avg ${cat.yearAvg}</span>
      </div>
      <svg viewBox={`0 0 ${W} ${H}`} width="100%" height={H}>
        <path d={area} fill={cat.color} opacity="0.15" />
        <path d={path} stroke={cat.color} strokeWidth="1.4" fill="none" />
      </svg>
    </div>
  );
}

function FireInput({ label, value, setValue, min, max, step, prefix, suffix }) {
  return (
    <div style={{ padding: "9px 0", borderBottom: "1px solid var(--hairline)" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
        <span style={{ fontSize: 13, color: "var(--ink-2)" }}>{label}</span>
        <span className="figure" style={{ fontSize: 14, color: "var(--accent)" }}>{prefix}{value.toLocaleString()}{suffix}</span>
      </div>
      <input type="range" min={min} max={max} step={step} value={value} onChange={e => setValue(parseFloat(e.target.value))}
        style={{ width: "100%", accentColor: "var(--accent)" }} />
    </div>
  );
}

function MiniTrend({ color, seed }) {
  const base = (seed || "x").charCodeAt(0);
  const pts = Array.from({ length: 12 }, (_, i) => 20 + Math.sin(i * 0.7 + base) * 8 + (base % 7));
  const max = Math.max(...pts), min = Math.min(...pts);
  const d = pts.map((p, i) => `${i === 0 ? "M" : "L"}${i * 8},${30 - ((p - min) / (max - min)) * 22}`).join(" ");
  return (
    <svg width="96" height="32" viewBox="0 0 96 32">
      <path d={d} stroke={color} strokeWidth="1.5" fill="none" />
    </svg>
  );
}

function SankeyView({ income, expense, save, highlightSave = true, periodLabel = "May 2026" }) {
  const W = 1080, H = 360;
  const incomeTotal = income.reduce((s, x) => s + x.amount, 0);
  const saveTotal = save;
  const total = incomeTotal;
  const colX = { in: 0, mid: 420, out: 800 };
  const colW = 180;
  const gap = 4;
  let yIn = 30;
  const inNodes = income.map(i => {
    const h = ((i.amount / total) * (H - 60)) - gap;
    const node = { ...i, x: colX.in, y: yIn, h };
    yIn += h + gap;
    return node;
  });
  const midBlock = { x: colX.mid, y: 30, h: H - 60 };
  let yOut = 30;
  const outNodes = [
    ...expense.map(e => {
      const h = ((e.amount / total) * (H - 60)) - gap;
      const node = { ...e, x: colX.out, y: yOut, h, isSave: false };
      yOut += h + gap;
      return node;
    }),
    (() => {
      const h = ((saveTotal / total) * (H - 60)) - gap;
      const node = { id: "save", label: "Saved", amount: saveTotal, x: colX.out, y: yOut, h, isSave: true };
      yOut += h + gap;
      return node;
    })(),
  ];
  const curve = (x1, y1, x2, y2, thickness) => {
    const cx = (x1 + x2) / 2;
    const top = `M${x1},${y1} C${cx},${y1} ${cx},${y2} ${x2},${y2}`;
    const bot = `L${x2},${y2 + thickness} C${cx},${y2 + thickness} ${cx},${y1 + thickness} ${x1},${y1 + thickness} Z`;
    return top + " " + bot;
  };
  let midY1 = midBlock.y;
  const incomeFlows = inNodes.map(n => {
    const path = curve(n.x + colW, n.y, midBlock.x, midY1, n.h);
    midY1 += n.h + gap;
    return { path };
  });
  let midY2 = midBlock.y;
  const outFlows = outNodes.map(n => {
    const path = curve(midBlock.x + colW, midY2, n.x, n.y, n.h);
    midY2 += n.h + gap;
    const color = FS.categories.find(c => c.id === n.id)?.color || (n.isSave ? "var(--accent)" : "#8B8B95");
    return { path, color };
  });
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      {incomeFlows.map((f, i) => (
        <path key={"in" + i} d={f.path} fill="var(--accent)" opacity="0.32" />
      ))}
      {outFlows.map((f, i) => (
        <path key={"out" + i} d={f.path} fill={f.color} opacity="0.34" />
      ))}
      <rect x={midBlock.x} y={midBlock.y} width={colW} height={midBlock.h} fill="var(--accent)" rx="4" />
      <text x={midBlock.x + colW / 2} y={midBlock.y - 10} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 12, fill: "var(--ink-faint)" }}>TOTAL · {periodLabel.toUpperCase()}</text>
      <text x={midBlock.x + colW / 2} y={midBlock.y + midBlock.h / 2 + 6} textAnchor="middle" style={{ fontFamily: "var(--sans)", fontSize: 18, fontWeight: 600, fill: "var(--accent-ink)" }}>${total.toLocaleString()}</text>
      {inNodes.map(n => (
        <g key={n.id}>
          <rect x={n.x} y={n.y} width={colW} height={n.h} fill="var(--surface-2)" stroke="var(--line-2)" rx="4" />
          <text x={n.x + 12} y={n.y + 18} style={{ fontFamily: "var(--sans)", fontSize: 13, fontWeight: 500, fill: "var(--ink)" }}>{n.label}</text>
          <text x={n.x + colW - 12} y={n.y + 18} textAnchor="end" style={{ fontFamily: "var(--mono)", fontSize: 12, fill: "var(--ink-mute)" }}>${n.amount.toLocaleString()}</text>
        </g>
      ))}
      {outNodes.map(n => {
        const color = FS.categories.find(c => c.id === n.id)?.color || (n.isSave ? "var(--accent)" : "#8B8B95");
        const labelOnly = n.h > 18;
        return (
          <g key={n.id}>
            <rect x={n.x} y={n.y} width={colW} height={n.h} fill={n.isSave ? "var(--accent)" : color} opacity={n.isSave ? 1 : 0.9} rx="4"
              style={n.isSave && highlightSave ? { filter: "drop-shadow(0 0 12px var(--accent))" } : undefined} />
            {labelOnly && (
              <>
                <text x={n.x + colW + 8} y={n.y + 14} style={{ fontFamily: "var(--sans)", fontSize: 13, fontWeight: n.isSave && highlightSave ? 600 : 500, fill: n.isSave && highlightSave ? "var(--accent)" : "var(--ink)" }}>{n.label}</text>
                <text x={n.x + colW + 8} y={n.y + 28} style={{ fontFamily: "var(--mono)", fontSize: 12, fill: "var(--ink-mute)" }}>${n.amount.toLocaleString()}</text>
              </>
            )}
          </g>
        );
      })}
    </svg>
  );
}

function YoYChart({ data, metric = "expense", compare = "lastYear" }) {
  const incomeMonthly = 10000;
  // Build series based on metric
  let thisY, lastY;
  if (metric === "income") {
    thisY = Array(12).fill(null).map((_, i) => i <= 4 ? incomeMonthly : null);
    lastY = Array(12).fill(9500);
  } else if (metric === "net") {
    thisY = data.thisYear.map(v => v == null ? null : incomeMonthly - v);
    lastY = data.lastYear.map(v => 9500 - v);
  } else {
    thisY = data.thisYear;
    lastY = data.lastYear;
  }
  if (compare === "average") {
    const avg = lastY.reduce((s, v) => s + v, 0) / lastY.length;
    lastY = Array(12).fill(avg);
  } else if (compare === "budget") {
    const monthlyBudget = Object.values(FS.budgetGrid).reduce((s, g) => s + g.may.b, 0);
    lastY = Array(12).fill(monthlyBudget);
  }

  const W = 1080, H = 240;
  const PAD_L = 40, PAD_R = 14, PAD_T = 20, PAD_B = 34;
  const innerW = W - PAD_L - PAD_R;
  const innerH = H - PAD_T - PAD_B;
  const all = [...thisY, ...lastY].filter(v => v != null);
  const max = Math.max(...all) * 1.1;
  const xFor = (i) => PAD_L + (i / 11) * innerW;
  const yFor = (v) => PAD_T + (1 - v / max) * innerH;
  const linePath = (vals) => vals.map((v, i) => v == null ? "" : `${i === 0 || vals[i - 1] == null ? "M" : "L"}${xFor(i)},${yFor(v)}`).join(" ");
  const compareLabel = { lastYear: "2025", average: "12-mo avg", budget: "Budget" }[compare] || "prior";
  const metricLabel = { expense: "Spending", income: "Income", net: "Net flow" }[metric] || metric;

  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      {[0.25, 0.5, 0.75].map((t, i) => (
        <line key={i} x1={PAD_L} x2={W - PAD_R} y1={PAD_T + innerH * t} y2={PAD_T + innerH * t} stroke="rgba(255,255,255,0.05)" />
      ))}
      <path d={linePath(lastY)} fill="none" stroke="var(--ink-faint)" strokeWidth="1.5" strokeDasharray="3 4" />
      <path d={linePath(thisY)} fill="none" stroke="var(--accent)" strokeWidth="2" />
      {thisY.map((v, i) => v != null && (
        <circle key={i} cx={xFor(i)} cy={yFor(v)} r="3" fill="var(--accent)" />
      ))}
      {data.months.map((m, i) => (
        <text key={m} x={xFor(i)} y={H - 12} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 11.5, fill: "var(--ink-faint)" }}>{m}</text>
      ))}
      {[0.25, 0.5, 0.75].map((t, i) => (
        <text key={i} x={PAD_L - 8} y={PAD_T + innerH * t + 3} textAnchor="end" style={{ fontFamily: "var(--mono)", fontSize: 11.5, fill: "var(--ink-faint)" }}>${Math.round(max * (1 - t) / 1000)}k</text>
      ))}
      <g transform={`translate(${PAD_L} 4)`}>
        <rect x="0" y="-2" width="14" height="3" fill="var(--accent)" rx="1" />
        <text x="20" y="2" style={{ fontFamily: "var(--mono)", fontSize: 11.5, fill: "var(--ink-mute)" }}>{metricLabel} · 2026</text>
        <rect x="140" y="-2" width="14" height="3" fill="var(--ink-faint)" rx="1" />
        <text x="160" y="2" style={{ fontFamily: "var(--mono)", fontSize: 11.5, fill: "var(--ink-mute)" }}>{compareLabel}</text>
      </g>
    </svg>
  );
}

/* expose for sibling files */
Object.assign(window, {
  WIDGET_TYPES, WIDGET_GROUP_ORDER, DEFAULT_REPORTS, METRIC_OPTIONS,
  WidgetContent,
  KPIWidget, SparkKPIWidget, YoYWidget, CumulativeWidget, BarsWidget, NetWorthWidget,
  SankeyWidget, TrendsWidget, DonutWidget, TableCatWidget, TableMerWidget,
  GoalsWidget, NoteWidget, FireWidget,
  NetWorthLong, CumulativeChart, IncomeExpenseBars, CategoryTrend, FireInput, MiniTrend,
  SankeyView, YoYChart,
});
