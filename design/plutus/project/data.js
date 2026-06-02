/* FinSight — demo data: Mira & Adam, joint + individual accounts, full feature surface */

window.FS = (function () {
  const fmt = (n, opts = {}) => {
    const { signed = false, decimals = 0 } = opts;
    const abs = Math.abs(n).toLocaleString("en-US", {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    });
    if (n < 0) return "−$" + abs;
    if (signed) return "+$" + abs;
    return "$" + abs;
  };

  // ── Accounts ─────────────────────────────────────────
  const accounts = [
    { id: "joint-checking", owner: "joint", bank: "Mercury",    type: "Checking",  name: "Joint Checking",    last4: "4421", balance: 14820.42,  delta30: +1240, sparkline: [12100, 12500, 13200, 12950, 13400, 14100, 14820], color: "#C9F950" },
    { id: "joint-savings",  owner: "joint", bank: "Wealthfront",type: "Savings",   name: "House Fund",        last4: "9087", balance: 28640.00,  delta30: +600,  sparkline: [27040, 27240, 27520, 27860, 28100, 28340, 28640], color: "#34D399" },
    { id: "mira-checking",  owner: "mira",  bank: "Schwab",     type: "Checking",  name: "Mira · Checking",   last4: "3318", balance:  6240.18,  delta30: +320,  sparkline: [5800, 5920, 6010, 5980, 6140, 6200, 6240],     color: "#60A5FA" },
    { id: "adam-checking",  owner: "adam",  bank: "Chase",      type: "Checking",  name: "Adam · Checking",   last4: "7762", balance:  3812.50,  delta30: -180,  sparkline: [3990, 4020, 4100, 3980, 3850, 3700, 3812],     color: "#A78BFA" },
    { id: "amex",           owner: "joint", bank: "Amex",       type: "Credit",    name: "Amex Gold",         last4: "1006", balance: -2418.00,  delta30: -2418, sparkline: [-980, -1240, -1500, -1820, -2100, -2300, -2418], color: "#FB7185" },
    { id: "401k",           owner: "joint", bank: "Fidelity",   type: "Investment",name: "Retirement",        last4: "0814", balance: 86420.00,  delta30: +1200, sparkline: [83200, 84100, 84800, 85100, 85700, 86000, 86420], color: "#2DD4BF" },
  ];

  // ── Manual assets (real estate, vehicles, crypto, etc.) ─────
  const assets = [
    { id: "home",   kind: "Real estate",  name: "Home · 142 Mosswood Ln", value: 612000, delta90: +14000, updated: "Mar 28", note: "Zestimate updated quarterly",        icon: "🏠" },
    { id: "car-m",  kind: "Vehicle",      name: "Mira · 2022 Subaru Outback", value: 18400, delta90: -1200, updated: "Apr 5",  note: "KBB private party",                  icon: "🚙" },
    { id: "car-a",  kind: "Vehicle",      name: "Adam · 2019 Honda Civic",    value: 11800, delta90: -800,  updated: "Apr 5",  note: "KBB private party",                  icon: "🚗" },
    { id: "crypto", kind: "Crypto",       name: "Coinbase",                   value: 4218,  delta90: +812,  updated: "today",  note: "0.04 BTC · 1.2 ETH",                 icon: "₿"  },
    { id: "art",    kind: "Collectibles", name: "Lithograph · Hockney",       value: 3200,  delta90: 0,     updated: "Jan 12", note: "Appraised value",                    icon: "🖼" },
  ];

  // ── Liabilities ─────────────────────────────────────
  const liabilities = [
    { id: "mortgage", kind: "Mortgage",   name: "First Federal · 30-yr fixed", balance: 388420, rate: 6.125, monthly: 2944, payoff: "Apr 2052", originated: "2022", remaining: 312 },
    { id: "auto",     kind: "Auto loan",  name: "Subaru Finance",              balance: 12480, rate: 4.9,   monthly: 412,  payoff: "Sep 2028", originated: "2022", remaining: 28 },
    { id: "student",  kind: "Student",    name: "Mira · Federal Direct",       balance: 18240, rate: 5.5,   monthly: 245,  payoff: "Mar 2034", originated: "2016", remaining: 94 },
    { id: "card",     kind: "Credit card",name: "Amex Gold",                   balance:  2418, rate: 24.9,  monthly: null, payoff: "—",        originated: "2020", remaining: null, note: "auto-paid each cycle" },
  ];

  // ── Daily cash flow for May (today is day 21) ───────
  const days = Array.from({ length: 31 }, (_, i) => {
    const d = i + 1;
    let v = -42 - Math.round(Math.random() * 18) - (d % 7 === 0 ? 60 : 0);
    if (d === 1) v += 4800;
    if (d === 15) v += 5200;
    if (d === 3) v -= 1850;
    if (d === 5) v -= 220;
    if (d === 8) v -= 88;
    if (d === 10) v -= 320;
    if (d === 14) v -= 412;
    if (d === 18) v -= 78;
    if (d === 20) v -= 142;
    return { day: d, net: v, forecast: d > 21 };
  });
  let running = 39200;
  const cashflow = days.map((d) => { running += d.net; return { ...d, running }; });

  // ── Categories ──────────────────────────────────────
  const categoryGroups = [
    { id: "fixed",     label: "Fixed costs",      hint: "predictable, mostly recurring" },
    { id: "daily",     label: "Daily life",       hint: "groceries, fuel, eating out" },
    { id: "lifestyle", label: "Lifestyle",        hint: "things you choose to spend on" },
    { id: "wellbeing", label: "Wellbeing",        hint: "health, body, mind" },
  ];

  const categories = [
    { id: "housing",   group: "fixed",     label: "Housing",       color: "#A78BFA", thisMonth: 1850, lastMonth: 1850, budget: 1900, txns: 1,  yearAvg: 1850, icon: "🏠" },
    { id: "utilities", group: "fixed",     label: "Utilities",     color: "#FACC15", thisMonth: 308,  lastMonth: 296,  budget: 350,  txns: 4,  yearAvg: 308,  icon: "💡" },
    { id: "subs",      group: "fixed",     label: "Subscriptions", color: "#F472B6", thisMonth: 184,  lastMonth: 161,  budget: 200,  txns: 12, yearAvg: 168,  icon: "📦" },
    { id: "groceries", group: "daily",     label: "Groceries",     color: "#34D399", thisMonth: 632,  lastMonth: 712,  budget: 800,  txns: 9,  yearAvg: 680,  icon: "🛒" },
    { id: "dining",    group: "daily",     label: "Dining",        color: "#FB923C", thisMonth: 412,  lastMonth: 380,  budget: 400,  txns: 14, yearAvg: 365,  icon: "🍽" },
    { id: "transport", group: "daily",     label: "Transport",     color: "#60A5FA", thisMonth: 248,  lastMonth: 312,  budget: 350,  txns: 11, yearAvg: 296,  icon: "🚗" },
    { id: "shopping",  group: "lifestyle", label: "Shopping",      color: "#FCA5A5", thisMonth: 286,  lastMonth: 410,  budget: 400,  txns: 6,  yearAvg: 348,  icon: "🛍" },
    { id: "travel",    group: "lifestyle", label: "Travel",        color: "#818CF8", thisMonth: 0,    lastMonth: 1320, budget: 500,  txns: 0,  yearAvg: 412,  icon: "✈" },
    { id: "gifts",     group: "lifestyle", label: "Gifts",         color: "#FDE68A", thisMonth: 74,   lastMonth: 0,    budget: 100,  txns: 1,  yearAvg: 56,   icon: "🎁" },
    { id: "health",    group: "wellbeing", label: "Health",        color: "#2DD4BF", thisMonth: 92,   lastMonth: 220,  budget: 250,  txns: 2,  yearAvg: 188,  icon: "❤" },
  ];

  // ── Per-month budget envelope grid ──────────────────
  // 5-month view: Mar, Apr, May (now), Jun, Jul. Each cell: { b: budgeted, s: spent, c: carryover from prev }.
  const budgetMonths = [
    { id: "mar", label: "Mar", year: 2026, status: "past" },
    { id: "apr", label: "Apr", year: 2026, status: "past" },
    { id: "may", label: "May", year: 2026, status: "current" },
    { id: "jun", label: "Jun", year: 2026, status: "future" },
    { id: "jul", label: "Jul", year: 2026, status: "future" },
  ];

  const budgetGrid = {
    housing:   { mar: { b: 1850, s: 1850, c: 0 },  apr: { b: 1850, s: 1850, c: 0 },  may: { b: 1900, s: 1850, c: 50 },  jun: { b: 1900, s: 0, c: 0 },  jul: { b: 1900, s: 0, c: 0 } },
    utilities: { mar: { b: 350,  s: 332,  c: 18 }, apr: { b: 350,  s: 296,  c: 54 }, may: { b: 350,  s: 308,  c: 42 },  jun: { b: 350,  s: 0, c: 0 },  jul: { b: 350,  s: 0, c: 0 } },
    subs:      { mar: { b: 200,  s: 174,  c: 26 }, apr: { b: 200,  s: 161,  c: 39 }, may: { b: 200,  s: 184,  c: 16 },  jun: { b: 220,  s: 0, c: 0 },  jul: { b: 220,  s: 0, c: 0 } },
    groceries: { mar: { b: 800,  s: 742,  c: 58 }, apr: { b: 800,  s: 712,  c: 88 }, may: { b: 800,  s: 632,  c: 168 }, jun: { b: 800,  s: 0, c: 0 },  jul: { b: 800,  s: 0, c: 0 } },
    dining:    { mar: { b: 400,  s: 422, c: -22 }, apr: { b: 400,  s: 380,  c: 20 }, may: { b: 400,  s: 412, c: -12 },  jun: { b: 450,  s: 0, c: 0 },  jul: { b: 450,  s: 0, c: 0 } },
    transport: { mar: { b: 350,  s: 318,  c: 32 }, apr: { b: 350,  s: 312,  c: 38 }, may: { b: 350,  s: 248, c: 102 },  jun: { b: 350,  s: 0, c: 0 },  jul: { b: 350,  s: 0, c: 0 } },
    shopping:  { mar: { b: 400,  s: 287,  c: 113}, apr: { b: 400,  s: 410, c: -10 }, may: { b: 400,  s: 286, c: 114 },  jun: { b: 400,  s: 0, c: 0 },  jul: { b: 400,  s: 0, c: 0 } },
    travel:    { mar: { b: 500,  s: 0,    c: 500},apr: { b: 500,  s: 1320,c: -320 }, may: { b: 500,  s: 0,   c: 500 },  jun: { b: 500,  s: 0, c: 0 },  jul: { b: 800,  s: 0, c: 0 } },
    gifts:     { mar: { b: 100,  s: 0,    c: 100},apr: { b: 100,  s: 0,    c: 200}, may: { b: 100,  s: 74,   c: 226 },  jun: { b: 100,  s: 0, c: 0 },  jul: { b: 100,  s: 0, c: 0 } },
    health:    { mar: { b: 250,  s: 168,  c: 82 }, apr: { b: 250,  s: 220,  c: 30 }, may: { b: 250,  s: 92,   c: 188 }, jun: { b: 250,  s: 0, c: 0 },  jul: { b: 250,  s: 0, c: 0 } },
  };

  // To Budget: income in minus total assigned this month
  const incomeThisMonth = 10000;
  const assignedThisMonth = Object.values(budgetGrid).reduce((s, g) => s + g.may.b, 0);
  const toBudget = incomeThisMonth - assignedThisMonth;

  // Merchant logo colors (used for circular logo squares)
  const merchants = {
    "Trader Joe's":            { bg: "#DC2626", short: "TJ" },
    "Whole Foods":             { bg: "#15803D", short: "WF" },
    "Costco":                  { bg: "#E11D48", short: "CO" },
    "Mosswood Wine Bar":       { bg: "#7C2D12", short: "MW" },
    "Sweetgreen":              { bg: "#84CC16", short: "SG" },
    "Blue Bottle":             { bg: "#0EA5E9", short: "BB" },
    "Lyft":                    { bg: "#EC4899", short: "LY" },
    "BP Gas":                  { bg: "#16A34A", short: "BP" },
    "Adobe Creative Cloud":    { bg: "#DC2626", short: "Ad" },
    "Spotify Family":          { bg: "#22C55E", short: "Sp" },
    "Acme Corp · Payroll":     { bg: "#1F2937", short: "AC" },
    "Sunset Co · Payroll":     { bg: "#F59E0B", short: "SC" },
    "PG&E":                    { bg: "#0EA5E9", short: "PG" },
    "Comcast":                 { bg: "#1D4ED8", short: "Cm" },
    "Pharmacy":                { bg: "#14B8A6", short: "Rx" },
    "Internet · Sonic":        { bg: "#F97316", short: "So" },
    "Bay Property Mgmt · Rent":{ bg: "#374151", short: "BP" },
    "Notion":                  { bg: "#0F172A", short: "No" },
    "iCloud+ 2TB":             { bg: "#38BDF8", short: "iC" },
    "Disney+":                 { bg: "#0EA5E9", short: "D+" },
    "NYTimes":                 { bg: "#111827", short: "NY" },
    "Gym · Range":             { bg: "#EAB308", short: "Gy" },
    "Coinbase":                { bg: "#2563EB", short: "Cb" },
    "Amazon":                  { bg: "#F59E0B", short: "Am" },
  };

  // ── Transactions ─────────────────────────────────────
  const transactions = [
    { id: "t1",  date: "May 20", merchant: "Mosswood Wine Bar",      category: "dining",    amount: -142.00, account: "amex",           status: "settled", aiTag: "Categorized via merchant similarity", confidence: 0.92, trip: null, note: "Adam's birthday dinner", attachments: 1 },
    { id: "t2",  date: "May 20", merchant: "Lyft",                   category: "transport", amount: -18.40,  account: "amex",           status: "settled", confidence: 0.99 },
    { id: "t3",  date: "May 19", merchant: "Trader Joe's",           category: "groceries", amount: -78.20,  account: "joint-checking", status: "settled", confidence: 0.99 },
    { id: "t4",  date: "May 18", merchant: "Sweetgreen",             category: "dining",    amount: -22.50,  account: "mira-checking",  status: "settled", confidence: 0.96, reimbursable: { from: "Acme work lunch", state: "pending" } },
    { id: "t5",  date: "May 18", merchant: "Blue Bottle",            category: "dining",    amount: -6.75,   account: "mira-checking",  status: "settled", confidence: 0.97 },
    { id: "t6",  date: "May 17", merchant: "Adobe Creative Cloud",   category: "subs",      amount: -22.99,  account: "amex",           status: "settled", aiTag: "Price changed from $19.99", confidence: 0.99 },
    { id: "t7",  date: "May 16", merchant: "Spotify Family",         category: "subs",      amount: -16.99,  account: "amex",           status: "settled", confidence: 0.99 },
    { id: "t8",  date: "May 15", merchant: "Acme Corp · Payroll",    category: "income",    amount: +5200.00,account: "adam-checking",  status: "settled", confidence: 0.99 },
    { id: "t9",  date: "May 14", merchant: "Costco",                 category: "groceries", amount: -412.00, account: "joint-checking", status: "settled", confidence: 0.98, splits: [{ category: "groceries", amount: -248 }, { category: "shopping", amount: -120 }, { category: "gifts", amount: -44 }] },
    { id: "t10", date: "May 13", merchant: "Whole Foods",            category: "groceries", amount: -64.30,  account: "joint-checking", status: "settled", confidence: 0.99 },
    { id: "t11", date: "May 12", merchant: "BP Gas",                 category: "transport", amount: -52.40,  account: "adam-checking",  status: "settled", confidence: 0.99 },
    { id: "t12", date: "May 10", merchant: "PG&E",                   category: "utilities", amount: -220.00, account: "joint-checking", status: "settled", confidence: 0.99, anomaly: { kind: "high", note: "2.1× your 12-month average" } },
    { id: "t13", date: "May 10", merchant: "Comcast",                category: "utilities", amount: -88.00,  account: "joint-checking", status: "settled", confidence: 0.99 },
    { id: "t14", date: "May 09", merchant: "Pharmacy",               category: "health",    amount: -32.10,  account: "mira-checking",  status: "settled", confidence: 0.95 },
    { id: "t15", date: "May 08", merchant: "Internet · Sonic",       category: "utilities", amount: -88.00,  account: "joint-checking", status: "settled", confidence: 0.99 },
    { id: "t16", date: "May 05", merchant: "Trader Joe's",           category: "groceries", amount: -52.40,  account: "joint-checking", status: "settled", confidence: 0.99 },
    { id: "t17", date: "May 03", merchant: "Bay Property Mgmt · Rent",category:"housing",   amount: -1850.00,account: "joint-checking", status: "settled", confidence: 0.99 },
    { id: "t18", date: "May 01", merchant: "Sunset Co · Payroll",    category: "income",    amount: +4800.00,account: "mira-checking",  status: "settled", confidence: 0.99 },
  ];

  // ── Trips ─────────────────────────────────────────
  const trips = [
    { id: "lisbon", name: "Lisbon 2026", from: "Apr 3", to: "Apr 12", spent: 1320, txns: 18, status: "closed" },
    { id: "tahoe",  name: "Tahoe weekend", from: "Feb 14", to: "Feb 16", spent: 412, txns: 6, status: "closed" },
    { id: "italy",  name: "Italy · planned", from: "Sep 4", to: "Sep 18", spent: 0, txns: 0, status: "planned", target: 4500 },
  ];

  // ── Recurring ─────────────────────────────────────
  const recurring = [
    { id: "r1",  name: "Rent · Bay Property",      amount: -1850, day: 3,  cadence: "monthly", category: "housing",   next: "Jun 3",  status: "stable",    since: "Apr 2023" },
    { id: "r2",  name: "Mira · Sunset Co payroll", amount: +4800, day: 1,  cadence: "monthly", category: "income",    next: "Jun 1",  status: "stable",    since: "Jan 2024" },
    { id: "r3",  name: "Adam · Acme payroll",      amount: +5200, day: 15, cadence: "monthly", category: "income",    next: "Jun 15", status: "stable",    since: "Aug 2023" },
    { id: "r4",  name: "PG&E",                     amount: -220,  day: 10, cadence: "monthly", category: "utilities", next: "Jun 10", status: "variable",  since: "2022" },
    { id: "r5",  name: "Comcast",                  amount: -88,   day: 10, cadence: "monthly", category: "utilities", next: "Jun 10", status: "stable",    since: "2022" },
    { id: "r6",  name: "Sonic Internet",           amount: -88,   day: 8,  cadence: "monthly", category: "utilities", next: "Jun 8",  status: "stable",    since: "2022" },
    { id: "r7",  name: "Spotify Family",           amount: -16.99,day: 7,  cadence: "monthly", category: "subs",      next: "Jun 7",  status: "stable",    since: "2021", priceHistory: [{ d: "2021", v: 14.99 }, { d: "2023", v: 15.99 }, { d: "2024", v: 16.99 }] },
    { id: "r8",  name: "Adobe Creative Cloud",     amount: -22.99,day: 17, cadence: "monthly", category: "subs",      next: "Jun 17", status: "increased", since: "2020", note: "+$3.00 vs last month", priceHistory: [{ d: "2020", v: 14.99 }, { d: "2022", v: 17.99 }, { d: "2024", v: 19.99 }, { d: "2026", v: 22.99 }] },
    { id: "r9",  name: "Notion",                   amount: -10.00,day: 19, cadence: "monthly", category: "subs",      next: "Jun 19", status: "stable",    since: "2023" },
    { id: "r10", name: "iCloud+ 2TB",              amount: -9.99, day: 22, cadence: "monthly", category: "subs",      next: "May 22", status: "stable",    since: "2022", usage: "18% of 2TB used" },
    { id: "r11", name: "NYTimes",                  amount: -4.00, day: 28, cadence: "monthly", category: "subs",      next: "May 28", status: "stable",    since: "2022" },
    { id: "r12", name: "Gym · Range",              amount: -149,  day: 25, cadence: "monthly", category: "health",    next: "May 25", status: "stable",    since: "2024", usage: "12 visits in last 90 days" },
    { id: "r13", name: "Costco membership",        amount: -60,   day: 1,  cadence: "yearly",  category: "shopping",  next: "Apr 2027", status: "stable", since: "2021" },
    { id: "r14", name: "Disney+",                  amount: -10.99,day: 24, cadence: "monthly", category: "subs",      next: "May 24", status: "unused",    since: "2023", note: "Not opened in 3 months", usage: "0 plays in 90 days" },
    { id: "r15", name: "MasterClass · free trial", amount: -0.00, day: 26, cadence: "monthly", category: "subs",      next: "May 26", status: "trial",     since: "Apr 26", note: "Trial ends in 5 days — $180/yr begins" },
  ];

  // ── Goals ─────────────────────────────────────────
  const goals = [
    { id: "g1", name: "House down payment",       type: "save-by-date",  target: 80000, current: 28640, eta: "Mar 2027", monthly: 1600, owner: "Joint", pace: "on track" },
    { id: "g2", name: "Six-month emergency fund", type: "build-balance", target: 24000, current: 18200, eta: "Sep 2026", monthly: 900,  owner: "Joint", pace: "on track" },
    { id: "g3", name: "Italy trip · September",   type: "save-by-date",  target: 4500,  current: 1850,  eta: "Aug 2026", monthly: 600,  owner: "Mira",  pace: "ahead" },
    { id: "g4", name: "Pay off Amex Gold",        type: "debt-payoff",   target: 2418,  current: 0,     eta: "Jul 2026", monthly: 1209, owner: "Joint", pace: "needs attention" },
    { id: "g5", name: "Stay under $400/mo dining",type: "spending-cap",  target: 400,   current: 412,   eta: "—",        monthly: 0,    owner: "Joint", pace: "needs attention" },
  ];

  // Sinking funds (smaller buckets within emergency / house)
  const sinkingFunds = [
    { id: "s1", name: "Car insurance",  due: "Oct 2026", target: 480,  current: 200 },
    { id: "s2", name: "Annual taxes",   due: "Apr 2027", target: 2400, current: 600 },
    { id: "s3", name: "Holiday gifts",  due: "Dec 2026", target: 800,  current: 240 },
    { id: "s4", name: "Vet · annual",   due: "Aug 2026", target: 350,  current: 175 },
  ];

  // ── Rules ─────────────────────────────────────────
  const rules = [
    { id: "u1", when: ["merchant", "contains", "Trader Joe's"], then: ["set category", "Groceries"], lastRun: "12 transactions matched", on: true, owner: "Agent" },
    { id: "u2", when: ["amount", "more than", "$500"], then: ["flag for review", ""], lastRun: "2 this month", on: true, owner: "You" },
    { id: "u3", when: ["category is", "Subscriptions", ""], then: ["track recurring", ""], lastRun: "11 active", on: true, owner: "Agent" },
    { id: "u4", when: ["merchant matches", "/lyft|uber|waymo/i"], then: ["set category", "Transport"], lastRun: "8 this month", on: true, owner: "Agent" },
    { id: "u5", when: ["account", "is", "Amex Gold"], then: ["pay balance from", "Joint Checking on the 22nd"], lastRun: "Last paid May 22", on: true, owner: "You" },
    { id: "u6", when: ["after", "rent posts"], then: ["move", "$1,600 → House Fund"], lastRun: "Triggered May 3", on: true, owner: "You" },
    { id: "u7", when: ["transaction is", "anomaly"], then: ["webhook", "POST /finsight/notify"], lastRun: "Last fired May 10", on: false, owner: "You" },
  ];

  // ── Agent activity / anomalies ────────────────────
  const agentActivity = [
    { kind: "ok",   when: "10 min ago", title: "Categorized 14 new transactions.", sub: "All confident; 1 needed your prior correction." },
    { kind: "acc",  when: "2 h ago",    title: "Adobe Creative Cloud went up by $3/mo.", sub: "From $19.99 to $22.99 starting this billing cycle." },
    { kind: "warn", when: "3 h ago",    title: "Anomaly · PG&E bill is 2.1× your average.", sub: "$220 vs typical $105 — worth a glance." },
    { kind: "ok",   when: "yesterday",  title: "Detected new recurring: Disney+ ($10.99).", sub: "Active since Mar 2023. Not opened in 3 months." },
    { kind: "warn", when: "yesterday",  title: "Amex statement closes in 3 days.", sub: "Current balance $2,418. Auto-pay rule will move funds on May 22." },
    { kind: "ok",   when: "2 d ago",    title: "Moved $1,600 to House Fund after rent posted.", sub: "Per your rule set in Mar 2025." },
    { kind: "acc",  when: "2 d ago",    title: "MasterClass trial expires in 5 days.", sub: "$180/yr will start charging unless cancelled by May 26." },
  ];

  // ── 6-month net worth history ─────────────────────
  const netWorthHistory = [
    { m: "Dec",  v: 108200 },
    { m: "Jan",  v: 112400 },
    { m: "Feb",  v: 115800 },
    { m: "Mar",  v: 119200 },
    { m: "Apr",  v: 124100 },
    { m: "May",  v: 137515 },
  ];

  // 12-month net worth + assets vs liabilities breakdown
  const netWorthLong = [
    { m: "Jun '25", assets: 470000, liab: -430000 },
    { m: "Jul",     assets: 478000, liab: -428500 },
    { m: "Aug",     assets: 482000, liab: -426800 },
    { m: "Sep",     assets: 488000, liab: -425100 },
    { m: "Oct",     assets: 494000, liab: -423300 },
    { m: "Nov",     assets: 502000, liab: -421500 },
    { m: "Dec",     assets: 530000, liab: -421800 },
    { m: "Jan '26", assets: 538000, liab: -425600 },
    { m: "Feb",     assets: 544000, liab: -428200 },
    { m: "Mar",     assets: 548000, liab: -428800 },
    { m: "Apr",     assets: 553000, liab: -428900 },
    { m: "May",     assets: 559078, liab: -421563 },
  ];

  // Top merchants this year
  const topMerchants = [
    { name: "Bay Property Mgmt",  total: 16650, txns: 9,   cat: "housing"   },
    { name: "Costco",             total: 2840,  txns: 7,   cat: "groceries" },
    { name: "Trader Joe's",       total: 2120,  txns: 28,  cat: "groceries" },
    { name: "PG&E",               total: 1320,  txns: 5,   cat: "utilities" },
    { name: "Whole Foods",        total: 1180,  txns: 21,  cat: "groceries" },
    { name: "Amazon",             total: 968,   txns: 14,  cat: "shopping"  },
    { name: "Sweetgreen",         total: 482,   txns: 22,  cat: "dining"    },
    { name: "Mosswood Wine Bar",  total: 412,   txns: 4,   cat: "dining"    },
    { name: "Lyft",               total: 384,   txns: 18,  cat: "transport" },
    { name: "BP Gas",             total: 312,   txns: 6,   cat: "transport" },
  ];

  // Cumulative spending YTD vs prior year (5 months in for current)
  const cumulative = {
    months: ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"],
    thisYear: [3120, 6100, 9550, 12870, 16956, null, null, null, null, null, null, null],
    lastYear: [2880, 5830, 8940, 12150, 16762, 20142, 23432, 26972, 30662, 33782, 37002, 40852],
  };

  // Income vs expense by month
  const incomeExpense = {
    months: ["Dec","Jan","Feb","Mar","Apr","May"],
    income:  [10000, 10000, 10000, 10000, 10000, 10000],
    expense: [3850, 3120, 2980, 3450, 3320, 4086],
  };

  // ── Agent insights ──────────────────────────────────
  // Strict rule: every insight here must be derivable from transaction history,
  // account balances, recurring patterns, or your own category rules.
  // No external data (tax rates, market rates, weather, streaming usage).
  const insights = [
    {
      id: "i1",
      kind: "pattern",
      severity: 2,
      age: "today",
      headline: "Your dining spend tracks with travel months.",
      summary: "Across the last 5 trips, dining doubled or more vs your typical pace.",
      reasoning: [
        "Apr 2026 (Lisbon): dining $1,840 vs typical $380 = 4.8×.",
        "Feb 2026 (Tahoe): dining $612 vs $380 = 1.6×.",
        "Sep 2025 (Italy planning): dining $920 vs $360 = 2.6×.",
        "Pattern holds for 4 of 5 trips since FinSight started watching.",
        "Derived from transactions tagged as Travel and Dining, no external data.",
      ],
      data: { kind: "bars", values: [380, 1840, 380, 612, 380, 920], labels: ["Mar","Apr·trip","Jan","Feb·trip","Aug","Sep·trip"] },
      actions: [
        { label: "Add Dining to Travel budget", primary: true },
        { label: "Set up auto-rule", primary: false },
        { label: "Dismiss", ghost: true },
      ],
    },
    {
      id: "i2",
      kind: "anomaly",
      severity: 3,
      age: "3 hours ago",
      headline: "PG&E bill is 2.1× your 12-month average.",
      summary: "$220 vs the $105 you typically pay. Past spikes followed a seasonal pattern.",
      reasoning: [
        "May 10 PG&E charge: $220.",
        "12-month average for PG&E: $105.",
        "Highest single PG&E charge in your 24-month transaction history.",
        "Past 2×+ spikes happened in Jan 2026 and Sep 2025 — both followed by 3 months of normal billing.",
        "Agent can't see your meter or weather — only the charge. Pattern suggests seasonal, not a billing error, but worth checking.",
      ],
      actions: [
        { label: "Acknowledge", primary: true },
        { label: "Mark as expected", primary: false },
        { label: "Raise threshold", ghost: true },
      ],
    },
    {
      id: "i3",
      kind: "subscription",
      severity: 2,
      age: "yesterday",
      headline: "Disney+ has been charged 38 times since Mar 2023.",
      summary: "$418 total. Agent can't tell from transactions whether you use it — worth reviewing.",
      reasoning: [
        "First Disney+ charge: Mar 24, 2023, at $7.99.",
        "Current rate: $10.99/mo since Sep 2024.",
        "38 charges total = $418.62 paid.",
        "Agent doesn't have streaming usage data — only the charge confirms you're paying.",
        "Of 11 active subscriptions, this is one of 2 with no related activity in your transactions or category corrections.",
      ],
      actions: [
        { label: "Open cancellation assistant", primary: true },
        { label: "Mark as still-using", primary: false },
      ],
    },
    {
      id: "i4",
      kind: "pattern",
      severity: 1,
      age: "yesterday",
      headline: "You're a Saturday spender.",
      summary: "38% of your discretionary spend lands on Saturdays. Mondays are quietest.",
      reasoning: [
        "12 weeks of transactions analyzed.",
        "Saturday total: $4,820 (avg $402/Sat).",
        "Sunday total: $2,140 (avg $178/Sun).",
        "Monday is lowest: avg $28/day.",
        "Highest single-day spend in 2026: Saturday Apr 18 ($412).",
      ],
      actions: [
        { label: "Switch Dining to weekly budget", primary: true },
        { label: "Show weekday breakdown", primary: false },
      ],
    },
    {
      id: "i5",
      kind: "goal",
      severity: 1,
      age: "2 days ago",
      headline: "House Fund could land 3 months earlier with one shift.",
      summary: "Travel sits at $500/mo and has been unused for 4 of last 5 months.",
      reasoning: [
        "Current House Fund pace: +$1,600/mo, ETA Mar 2027.",
        "Travel envelope: $500/mo budgeted, $0 spent in Mar, May, and 2 prior months.",
        "Only used Apr (Lisbon) at $1,840 — overspent the envelope that month.",
        "Pulling $200/mo from Travel → +$2,400 over 12 months → ETA roughly Dec 2026.",
        "Italy goal stays funded separately so the trip isn't affected.",
      ],
      actions: [
        { label: "Apply the shift", primary: true },
        { label: "Run other scenarios", primary: false },
      ],
    },
    {
      id: "i6",
      kind: "forecast",
      severity: 0,
      age: "3 days ago",
      headline: "You'll cross $150k net worth around July 8.",
      summary: "Linear extrapolation from your balance history. No market growth assumed.",
      reasoning: [
        "Current net worth: $137,515 (sum of all account balances + assets − liabilities).",
        "Net monthly contribution averaged over last 3 months: $2,840.",
        "Linear projection: ($150,000 − $137,515) / $2,840 ≈ 4.4 months → mid-September.",
        "Faster scenario based on Apr+May actuals: early July.",
        "No market-return assumption included — agent doesn't have access to current market data, only your transaction stream.",
      ],
      actions: [
        { label: "Track this milestone", primary: true },
        { label: "Run other forecasts", primary: false },
      ],
    },
    {
      id: "i7",
      kind: "comparison",
      severity: 1,
      age: "4 days ago",
      headline: "Dining is +13% over your 12-month average this month.",
      summary: "$412 vs the $365/mo average. Three of last four months landed within $50.",
      reasoning: [
        "May dining total: $412 across 14 transactions.",
        "12-month rolling average: $365/mo.",
        "Standard deviation: $48 — May is just outside the typical range.",
        "Of the last 4 months, 3 were within $50 of the average; April was the outlier ($380 because of Lisbon being budgeted as Travel).",
        "Largest single dining charge this month: Mosswood Wine Bar at $142 (May 20).",
      ],
      actions: [
        { label: "Open dining category", primary: true },
        { label: "Set tighter dining cap", primary: false },
      ],
    },
    {
      id: "i8",
      kind: "pattern",
      severity: 1,
      age: "5 days ago",
      headline: "Costco runs replace 2–3 small grocery trips each time.",
      summary: "Months with a Costco run have ~4 other grocery trips. Without: ~7.",
      reasoning: [
        "13 months analyzed where you had at least one Costco charge.",
        "Average other-grocery transactions in Costco months: 4.2 (sum: $186).",
        "Average other-grocery transactions in non-Costco months: 7.1 (sum: $324).",
        "Net grocery spend in Costco months: ~$50 lower despite the larger Costco hit.",
        "Not actionable per se — just a pattern worth knowing.",
      ],
      actions: [
        { label: "Dismiss", primary: true },
      ],
    },
    {
      id: "i9",
      kind: "anomaly",
      severity: 1,
      age: "1 week ago",
      headline: "The $142 Mosswood charge is your largest single dining transaction in 18 months.",
      summary: "Not flagged as suspicious — just the highest. Next largest: $148 in Feb (Tahoe).",
      reasoning: [
        "May 20 at Mosswood Wine Bar: $142.00.",
        "Next largest dining transactions: $148 (Feb 14, Tahoe), $112 (Dec 31), $98 (Aug 12).",
        "Median dining transaction: $26.",
        "You noted 'Adam's birthday dinner' on this transaction — context preserved.",
      ],
      actions: [
        { label: "Looks fine", primary: true },
      ],
    },
    {
      id: "i10",
      kind: "pattern",
      severity: 0,
      age: "1 week ago",
      headline: "Travel envelope is sized for occasional trips, not monthly accrual.",
      summary: "$500/mo budgeted; spent in 1 of 5 recent months — and overspent that month by $1,340.",
      reasoning: [
        "Travel budget: $500/mo, total $2,500 over 5 months.",
        "Actual spend across those 5 months: $1,840 (only in April).",
        "Pattern: 4 months of $0, then one large trip.",
        "Either move to a 'trip-based' allocation (lump sum before each trip) or accept that the envelope is mostly carry-over.",
      ],
      actions: [
        { label: "Convert to trip-based", primary: true },
        { label: "Keep as carry-over", primary: false },
      ],
    },
    {
      id: "i11",
      kind: "tags",
      severity: 2,
      age: "yesterday",
      headline: "9 transactions you tagged 'business' have no receipt attached.",
      summary: "Based on tags you've added — useful if Mira's freelance income will hit Schedule C this year.",
      reasoning: [
        "23 transactions tagged with 'business' by you in the last 12 months.",
        "11 of them exceed $75 (the typical receipt-keeping threshold for self-employment).",
        "9 of those 11 have no attachment uploaded.",
        "Total exposure: $1,840 of business-tagged spend that lacks documentation.",
        "Agent only sees your tags and attachments — it doesn't know your actual tax filing status or whether you'll need these.",
      ],
      actions: [
        { label: "Open tagged transactions", primary: true },
        { label: "Draft email requests for receipts", primary: false },
      ],
    },
    {
      id: "i12",
      kind: "comparison",
      severity: 1,
      age: "4 days ago",
      headline: "Your Health budget is consistently under-used.",
      summary: "$144/mo avg against the $250 budget you set. Either trim the budget or use it.",
      reasoning: [
        "Health budget you set: $250/mo.",
        "Actual 6-month average: $144/mo.",
        "Lowest month (May): $92.",
        "$636 carried over the last 6 months unused.",
        "Agent doesn't have an opinion on whether you should spend more on health — only that the budget you set doesn't match your behavior.",
      ],
      actions: [
        { label: "Lower budget to $175", primary: true },
        { label: "Open Health category", primary: false },
        { label: "Leave as-is", ghost: true },
      ],
    },
  ];

  // ── Scenarios (what-if presets) ─────────────────────
  const scenarios = [
    { id: "sabbatical", title: "Take a 6-month sabbatical starting October", impact: { runway: -180, goalsSlip: ["House Fund: +9 mo", "Italy: cancelled"], coverable: true } },
    { id: "raise",      title: "Both of us each get a 6% raise next quarter", impact: { runway: +90,  goalsSlip: ["House Fund: -2 mo"], coverable: true } },
    { id: "newcar",     title: "Buy a $32,000 used car with 20% down", impact: { runway: -60,  goalsSlip: ["House Fund: +4 mo"], coverable: true } },
    { id: "downpay",    title: "Pull $20k from House Fund early for a remodel", impact: { runway: -10,  goalsSlip: ["House Fund: +5 mo"], coverable: true } },
    { id: "rentup",     title: "Rent goes up by $300/mo on next renewal", impact: { runway: -20, goalsSlip: ["Emergency: +2 mo", "House Fund: +3 mo"], coverable: true } },
    { id: "babyworld",  title: "Add a baby to the household next year", impact: { runway: -120, goalsSlip: ["House Fund: +8 mo", "Italy: cancelled"], coverable: false } },
  ];

  // ── Agent memory items (what it's learned) ──────────
  const agentMemory = [
    { id: "m1", learned: "Trader Joe's → Groceries", weight: "high", source: "12 corrections from you", since: "Aug 2024" },
    { id: "m2", learned: "Amazon Marketplace → Shopping (unless under $20 = Groceries)", weight: "medium", source: "8 corrections", since: "Feb 2025" },
    { id: "m3", learned: "Mira's gym = Health, not Subs", weight: "high", source: "your edit", since: "Mar 2025" },
    { id: "m4", learned: "Income on the 1st & 15th = recurring paychecks", weight: "high", source: "12 months of pattern", since: "Jan 2024" },
    { id: "m5", learned: "You prefer 'Plenty left' over 'Under budget' in notifications", weight: "low", source: "you marked it once", since: "May 2026" },
    { id: "m6", learned: "Saturday spending is intentional — don't flag it as anomalous", weight: "medium", source: "12 dismissed flags", since: "Apr 2025" },
  ];

  // Year-over-year monthly spend (for reports)
  const yoySpend = {
    thisYear:  [3120, 2980, 3450, 3320, 4086, null, null, null, null, null, null, null],
    lastYear:  [2880, 2950, 3110, 3210, 4612, 3380, 3290, 3540, 3690, 3120, 3220, 3850],
    months:    ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"],
  };

  // Income/expense Sankey data
  const sankey = {
    income: [
      { id: "mira",  label: "Mira · Sunset Co", amount: 4800 },
      { id: "adam",  label: "Adam · Acme",      amount: 5200 },
    ],
    expense: [
      { id: "housing",   label: "Housing",       amount: 1850 },
      { id: "groceries", label: "Groceries",     amount: 632 },
      { id: "dining",    label: "Dining",        amount: 412 },
      { id: "utilities", label: "Utilities",     amount: 308 },
      { id: "transport", label: "Transport",     amount: 248 },
      { id: "shopping",  label: "Shopping",      amount: 286 },
      { id: "subs",      label: "Subscriptions", amount: 184 },
      { id: "health",    label: "Health",        amount: 92 },
      { id: "gifts",     label: "Gifts",         amount: 74 },
    ],
    save: 5915,
  };

  // Totals
  const liquid = accounts.filter(a => ["Checking","Savings"].includes(a.type)).reduce((s,a) => s + a.balance, 0);
  const credit = accounts.filter(a => a.type === "Credit").reduce((s,a) => s + a.balance, 0);
  const invested = accounts.filter(a => a.type === "Investment").reduce((s,a) => s + a.balance, 0);
  const assetTotal = assets.reduce((s, a) => s + a.value, 0);
  const liabilityTotal = liabilities.reduce((s, l) => s + l.balance, 0);
  const netWorth = liquid + credit + invested + assetTotal - liabilityTotal;

  return {
    fmt,
    accounts, assets, liabilities, cashflow, categories, categoryGroups, transactions, recurring,
    goals, sinkingFunds, rules, agentActivity, netWorthHistory, netWorthLong,
    yoySpend, sankey, trips, merchants,
    budgetMonths, budgetGrid, toBudget, incomeThisMonth, assignedThisMonth,
    topMerchants, cumulative, incomeExpense,
    insights, scenarios, agentMemory,
    totals: { liquid, credit, invested, netWorth, assetTotal, liabilityTotal },
    today: { d: 21, m: "May", y: 2026, dow: "Tuesday" },
  };
})();
