function Recurring() {
  const today = 21;
  const recs = FS.recurring;
  const subs = recs.filter(r => r.category === "subs");
  const totalOut = recs.filter(r => r.amount < 0 && r.cadence === "monthly").reduce((s, r) => s + r.amount, 0);
  const totalIn  = recs.filter(r => r.amount > 0).reduce((s, r) => s + r.amount, 0);
  const subsMonthly = subs.reduce((s, r) => s + Math.abs(r.amount), 0);
  const annualSubs = subsMonthly * 12;
  const [view, setView] = React.useState("calendar");
  const [cancelTarget, setCancelTarget] = React.useState(null);
  const [calMonth, setCalMonth] = React.useState("May 2026");
  const [auditDismissed, setAuditDismissed] = React.useState(new Set());

  return (
    <div className="screen">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot"></span>Recurring · 15 items · 11 subscriptions</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Predictable money, predictable peace of mind.</h1>
        </div>
        <div className="toolbar">
          <button className={view === "calendar" ? "on" : ""} onClick={() => setView("calendar")}><I.Calendar /> Calendar</button>
          <button className={view === "list"     ? "on" : ""} onClick={() => setView("list")}>List</button>
          <button className={view === "subs"     ? "on" : ""} onClick={() => setView("subs")}>Subscriptions</button>
        </div>
      </div>

      {/* Stats */}
      <div className="stat-row" style={{ marginTop: 14 }}>
        <Stat label="Monthly out" value={<Currency value={Math.abs(totalOut)} />} sub={<span className="muted" style={{ fontSize: 12.5 }}>12 items</span>} />
        <Stat label="Monthly in"  value={<Currency value={totalIn} />} sub={<span className="muted" style={{ fontSize: 12.5 }}>2 paychecks</span>} />
        <Stat label="Subscriptions · annual" value={<span>${annualSubs.toLocaleString()}<span className="small">/yr</span></span>} sub={<span className="npill neg">2 need review</span>} />
        <Stat label="Free trials ending" value={<span>1<span className="small">in 5 days</span></span>} sub={<span className="muted" style={{ fontSize: 12.5 }}>MasterClass · $180/yr</span>} accent />
      </div>

      {/* Calendar */}
      {view === "calendar" && (
        <div className="section">
          <RecurringCalendar
            month={calMonth}
            today={today}
            recs={recs}
            onChangeMonth={(d) => setCalMonth(d === 0 ? "May 2026" : shiftMonthLabel(calMonth, d))}
          />
        </div>
      )}

      {/* List */}
      {view === "list" && (
        <div className="section">
          <div className="card flush">
            <div className="card-head">
              <div className="h3">All recurring</div>
              <button className="btn ghost sm" onClick={() => window.toast?.("Add recurring", { sub: "Name, amount, cadence, next date" })}><I.Plus /> Add manual</button>
            </div>
            <table className="tbl">
              <thead>
                <tr>
                  <th>Item</th>
                  <th>Next</th>
                  <th>Cadence</th>
                  <th>Status</th>
                  <th className="right">Amount</th>
                </tr>
              </thead>
              <tbody>
                {recs.map(r => {
                  const cat = FS.categories.find(c => c.id === r.category);
                  const mer = FS.merchants[r.name.split(" · ")[0]] || { bg: cat?.color || "#3F3F46", short: r.name.slice(0, 2) };
                  return (
                    <tr key={r.id}>
                      <td>
                        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                          <div style={{ width: 24, height: 24, borderRadius: 6, background: mer.bg, color: "#fff", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 600 }}>{mer.short}</div>
                          <div>
                            <div style={{ fontSize: 14 }}>{r.name}</div>
                            {r.note && <div style={{ fontSize: 12, color: r.status === "increased" ? "var(--negative)" : "var(--ink-mute)", marginTop: 1 }}>{r.note}</div>}
                          </div>
                        </div>
                      </td>
                      <td className="muted" style={{ fontFamily: "var(--mono)", fontSize: 12.5 }}>{r.next}</td>
                      <td className="muted" style={{ fontSize: 13 }}>{r.cadence} · since {r.since}</td>
                      <td>
                        <span className={`chip ${r.status === "increased" ? "negative" : r.status === "unused" ? "" : r.status === "trial" ? "warning" : "positive"}`} style={{ padding: "1px 7px", fontSize: 11 }}>
                          <span className="dot"></span>{r.status}
                        </span>
                      </td>
                      <td className="right num tabular" style={{ color: r.amount > 0 ? "var(--positive)" : "var(--ink)" }}>
                        {FS.fmt(r.amount, { decimals: 2, signed: r.amount > 0 })}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Subscriptions audit */}
      {view === "subs" && (
        <div className="section" style={{ display: "grid", gridTemplateColumns: "1.4fr 1fr", gap: 18 }}>
          <div>
            <div className="section-hdr" style={{ marginBottom: 14 }}>
              <div>
                <div className="eyebrow"><span className="dot"></span>Subscriptions · {subs.length} active</div>
                <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>What you're paying for.</h2>
              </div>
              <span className="chip">Could save ~$216/year</span>
            </div>

            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {subs.map(s => {
                const mer = FS.merchants[s.name.split(" · ")[0]] || { bg: "#3F3F46", short: s.name.slice(0, 2) };
                const annual = Math.abs(s.amount) * 12;
                return (
                  <div key={s.id} className="card" style={{ padding: 16, display: "grid", gridTemplateColumns: "auto 1fr auto auto", gap: 14, alignItems: "center" }}>
                    <div style={{ width: 34, height: 34, borderRadius: 8, background: mer.bg, color: "#fff", display: "grid", placeItems: "center", fontSize: 13, fontWeight: 600 }}>{mer.short}</div>
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 500 }}>{s.name}</div>
                      <div className="muted" style={{ fontSize: 12.5, marginTop: 2, display: "flex", gap: 8, alignItems: "center" }}>
                        <span>since {s.since}</span>
                        {s.usage && <><span>·</span><span>{s.usage}</span></>}
                        {s.note && <><span>·</span><span style={{ color: s.status === "increased" ? "var(--negative)" : "var(--ink-mute)" }}>{s.note}</span></>}
                      </div>
                      {s.priceHistory && <PriceHistorySpark history={s.priceHistory} />}
                    </div>
                    <div style={{ textAlign: "right" }}>
                      <div className="figure" style={{ fontSize: 16 }}>${Math.abs(s.amount).toFixed(2)}<span className="muted" style={{ fontSize: 12, fontWeight: 500 }}>/mo</span></div>
                      <div className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>${annual.toFixed(0)}/yr</div>
                    </div>
                    <div style={{ display: "flex", gap: 6 }}>
                      {(s.status === "unused" || s.status === "trial") && (
                        <button className="btn outline sm" onClick={() => setCancelTarget(s)} style={{ color: "var(--negative)", borderColor: "var(--negative-2)" }}>Cancel</button>
                      )}
                      <button className="btn ghost sm" onClick={() => window.toast?.(s.name, { sub: `$${Math.abs(s.amount).toFixed(2)}/mo · next ${s.next} · since ${s.since}`, duration: 4000 })}><I.More /></button>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Audit panel */}
          <div className="card">
            <div className="eyebrow" style={{ marginBottom: 14 }}><span className="dot"></span>Agent audit</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {[
                { id: "a1", title: "Disney+",     reason: "Not opened in 3 months",        action: "Cancel · save $132/yr", urgent: false, item: subs.find(s => s.id === "r14") },
                { id: "a2", title: "MasterClass", reason: "Trial ends in 5 days",          action: "Cancel before $180/yr starts", urgent: true,  item: subs.find(s => s.id === "r15") },
                { id: "a3", title: "Adobe CC",    reason: "Price went up $3.00",           action: "Acknowledge", urgent: false },
                { id: "a4", title: "iCloud+ 2TB", reason: "You're at 18% usage",           action: "Downgrade to 200GB · save $84/yr", urgent: false, item: subs.find(s => s.id === "r10") },
              ].filter(a => !auditDismissed.has(a.id)).map((a) => (
                <div key={a.id} style={{ padding: 14, border: "1px solid var(--line)", borderRadius: 10, background: a.urgent ? "var(--negative-2)" : "var(--surface-2)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
                    <div className="strong" style={{ fontSize: 14 }}>{a.title}</div>
                    <span className="chip" style={{ padding: "1px 7px", fontSize: 11 }}>{a.reason}</span>
                  </div>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8 }}>
                    <span style={{ fontSize: 13, color: a.urgent ? "var(--negative)" : "var(--accent)" }}>{a.action}</span>
                    <div style={{ display: "flex", gap: 6 }}>
                      <button className="btn sm" onClick={() => a.item ? setCancelTarget(a.item) : window.toast?.(`Acknowledged: ${a.title}`, { kind: "success" })}>Do it</button>
                      <button className="btn ghost sm" onClick={() => { setAuditDismissed(s => new Set([...s, a.id])); window.toast?.("Audit item dismissed"); }}>Dismiss</button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Cancel drawer */}
      {cancelTarget && <CancelDrawer item={cancelTarget} onClose={() => setCancelTarget(null)} />}
    </div>
  );
}

function PriceHistorySpark({ history }) {
  const W = 100, H = 18;
  const vals = history.map(h => h.v);
  const min = Math.min(...vals), max = Math.max(...vals);
  const range = max - min || 1;
  const xFor = (i) => (i / (history.length - 1)) * W;
  const yFor = (v) => H - ((v - min) / range) * (H - 4) - 2;
  const path = history.map((h, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(h.v)}`).join(" ");
  const trend = vals[vals.length - 1] > vals[0];
  return (
    <div style={{ marginTop: 6, display: "flex", alignItems: "center", gap: 8 }}>
      <svg viewBox={`0 0 ${W} ${H}`} width={W} height={H}>
        <path d={path} stroke={trend ? "var(--negative)" : "var(--accent)"} strokeWidth="1.3" fill="none" />
        {history.map((h, i) => (
          <circle key={i} cx={xFor(i)} cy={yFor(h.v)} r="1.5" fill={trend ? "var(--negative)" : "var(--accent)"} />
        ))}
      </svg>
      <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>
        ${vals[0]} → ${vals[vals.length - 1]} since {history[0].d}
      </span>
    </div>
  );
}

function CancelDrawer({ item, onClose }) {
  const [step, setStep] = React.useState(0);
  const steps = [
    "Identifying your account",
    "Drafting cancellation request",
    "Opening cancellation page",
  ];
  React.useEffect(() => {
    if (step < steps.length) {
      const t = setTimeout(() => setStep(step + 1), 900);
      return () => clearTimeout(t);
    }
  }, [step]);

  return (
    <div className="cmdk-mask" onClick={onClose} style={{ paddingTop: "10vh" }}>
      <div className="cmdk" onClick={(e) => e.stopPropagation()} style={{ width: "min(540px, 92vw)" }}>
        <div style={{ padding: 28 }}>
          <div className="eyebrow" style={{ marginBottom: 14 }}><span className="dot"></span>Cancellation assistant</div>
          <h2 className="h1" style={{ fontSize: 26, marginBottom: 6 }}>Cancel {item.name.split(" · ")[0]}</h2>
          <p className="muted" style={{ fontSize: 14, lineHeight: 1.55, margin: 0 }}>
            You'll save <span className="accent-text strong">${(Math.abs(item.amount) * 12).toFixed(0)}/year</span>. The agent will navigate to the cancellation page, fill in what it knows, and pause so you confirm — it never impersonates you.
          </p>

          <div style={{ marginTop: 24 }}>
            {steps.map((s, i) => {
              const done = i < step;
              const cur = i === step;
              return (
                <div key={i} style={{ display: "grid", gridTemplateColumns: "20px 1fr auto", gap: 10, alignItems: "center", padding: "10px 0" }}>
                  <span style={{
                    width: 14, height: 14, borderRadius: 999,
                    border: "1.5px solid " + (done ? "var(--accent)" : cur ? "var(--accent)" : "var(--line-2)"),
                    background: done ? "var(--accent)" : "transparent",
                    display: "grid", placeItems: "center",
                  }}>
                    {done && <I.Check width="9" height="9" style={{ color: "var(--accent-ink)" }} />}
                    {cur && <span style={{ width: 4, height: 4, borderRadius: 999, background: "var(--accent)", animation: "pulse 1.4s infinite" }}></span>}
                  </span>
                  <span style={{ fontSize: 14, color: done || cur ? "var(--ink)" : "var(--ink-faint)" }}>{s}</span>
                  {done && <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>ok</span>}
                </div>
              );
            })}
          </div>

          {step >= steps.length && (
            <div style={{ marginTop: 18, padding: 16, background: "var(--accent-2)", border: "1px solid var(--accent-3)", borderRadius: 10 }}>
              <div className="strong" style={{ fontSize: 14 }}>Ready to confirm in your browser.</div>
              <div className="muted" style={{ fontSize: 13.5, marginTop: 6, lineHeight: 1.5 }}>Open the page below to finish. The agent stops here — your hand on the button.</div>
              <div style={{ marginTop: 12, display: "flex", gap: 8 }}>
                <button className="btn primary" onClick={() => window.toast?.("Opening cancellation page", { sub: "Agent stops here — you confirm", kind: "accent" })}>Open cancellation page</button>
                <button className="btn ghost" onClick={onClose}>Maybe later</button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

window.Recurring = Recurring;

/* ────────────────────────────────────────────────────────
   RecurringCalendar — month grid with merchant logo chips,
   load bars, and an animated day-detail panel below.
   ──────────────────────────────────────────────────────── */
const MONTH_NAMES = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
const MONTH_LONG = ["January","February","March","April","May","June","July","August","September","October","November","December"];
const WEEKDAY_LONG = ["Sunday","Monday","Tuesday","Wednesday","Thursday","Friday","Saturday"];

function monthMeta(label) {
  const [m, y] = label.split(" ");
  const mi = MONTH_NAMES.indexOf(m);
  const yi = parseInt(y);
  const first = new Date(yi, mi, 1);
  const dim = new Date(yi, mi + 1, 0).getDate();
  return { mi, yi, startDow: first.getDay(), daysInMonth: dim };
}

function weekdayOf(label, day) {
  const { mi, yi } = monthMeta(label);
  return new Date(yi, mi, day).getDay();
}

function eventColor(r) {
  return FS.categories.find(c => c.id === r.category)?.color || "var(--ink-mute)";
}
function eventLogo(r) {
  const key = r.name.split(" · ")[0];
  return FS.merchants[key] || { bg: eventColor(r), short: r.name.slice(0, 2) };
}

function RecurringCalendar({ month, today, recs, onChangeMonth }) {
  const meta = monthMeta(month);
  const [m] = month.split(" ");
  const isCurrentMonth = month === "May 2026";
  const [selectedDay, setSelectedDay] = React.useState(isCurrentMonth ? today : 1);

  // Reset selection when month changes
  React.useEffect(() => {
    setSelectedDay(month === "May 2026" ? today : 1);
  }, [month]);

  // Build grid
  const cells = [];
  for (let i = 0; i < meta.startDow; i++) cells.push(null);
  for (let d = 1; d <= meta.daysInMonth; d++) cells.push(d);
  while (cells.length % 7 !== 0) cells.push(null);

  const eventsFor = (d) => recs.filter(r => r.cadence === "monthly" && r.day === d);
  const sumFor = (d) => eventsFor(d).reduce((s, r) => s + r.amount, 0);

  const allEvents = recs.filter(r => r.cadence === "monthly");
  const monthOut = allEvents.filter(r => r.amount < 0).reduce((s, r) => s + r.amount, 0);
  const monthIn  = allEvents.filter(r => r.amount > 0).reduce((s, r) => s + r.amount, 0);
  const maxAbs = Math.max(...Array.from({ length: meta.daysInMonth }, (_, i) => Math.abs(sumFor(i + 1))), 1);

  return (
    <div className="rcal">
      {/* Header */}
      <div className="rcal-head">
        <div>
          <div className="eyebrow"><span className="dot"></span>{MONTH_LONG[meta.mi]} {meta.yi}</div>
          <div className="rcal-summary">
            <span><b>{allEvents.length}</b><span className="muted"> movements</span></span>
            <span className="muted">·</span>
            <span className="rcal-in"><b>+${monthIn.toLocaleString()}</b><span className="muted"> in</span></span>
            <span className="muted">·</span>
            <span className="rcal-out"><b>−${Math.abs(monthOut).toLocaleString()}</b><span className="muted"> out</span></span>
            <span className="muted">·</span>
            <span className="rcal-net-summary">
              <b>{monthIn + monthOut >= 0 ? "+" : "−"}${Math.abs(monthIn + monthOut).toLocaleString()}</b>
              <span className="muted"> net</span>
            </span>
          </div>
        </div>
        <div className="rcal-nav">
          <button className="rcal-arrow" title="Previous month" onClick={() => onChangeMonth(-1)}>
            <I.ArrowL width="14" height="14" />
          </button>
          <button className="rcal-today" onClick={() => onChangeMonth(0)} title="Jump to today">
            Today
          </button>
          <button className="rcal-arrow" title="Next month" onClick={() => onChangeMonth(+1)}>
            <I.ArrowR width="14" height="14" />
          </button>
        </div>
      </div>

      {/* Weekday labels */}
      <div className="rcal-weekdays">
        {["Sun","Mon","Tue","Wed","Thu","Fri","Sat"].map((d, i) => (
          <div key={d} className={`rcal-dow ${i === 0 || i === 6 ? "weekend" : ""}`}>{d}</div>
        ))}
      </div>

      {/* Grid */}
      <div className="rcal-grid" key={month}>
        {cells.map((d, i) => {
          if (d === null) return <div key={i} className="rcal-cell empty"></div>;
          const events = eventsFor(d);
          const sum = sumFor(d);
          const isToday = isCurrentMonth && d === today;
          const isSelected = d === selectedDay;
          const isPast = isCurrentMonth && d < today;
          const dow = weekdayOf(month, d);
          const isWeekend = dow === 0 || dow === 6;
          const load = Math.abs(sum) / maxAbs;
          const sign = sum > 0 ? "pos" : sum < 0 ? "neg" : "";
          return (
            <button
              key={i}
              type="button"
              className={`rcal-cell ${isToday ? "today" : ""} ${isSelected ? "selected" : ""} ${isPast ? "past" : ""} ${isWeekend ? "weekend" : ""} ${sign}`}
              onClick={() => setSelectedDay(d)}
              style={load > 0 ? { "--load": (load * 100) + "%" } : undefined}
            >
              <div className="rcal-cell-head">
                <span className="rcal-day">{d}</span>
                {isToday && <span className="rcal-today-pip">TODAY</span>}
                {!isToday && sum !== 0 && (
                  <span className={`rcal-net ${sign}`}>
                    {sum > 0 ? "+" : "−"}${Math.abs(sum) >= 1000 ? (Math.abs(sum) / 1000).toFixed(1) + "k" : Math.abs(sum).toFixed(0)}
                  </span>
                )}
              </div>

              {events.length > 0 && (
                <div className="rcal-dots">
                  {events.slice(0, 4).map(e => {
                    const logo = eventLogo(e);
                    return (
                      <span
                        key={e.id}
                        className={`rcal-dot ${e.amount > 0 ? "income" : ""}`}
                        style={{ background: logo.bg }}
                        title={`${e.name} · ${FS.fmt(e.amount, { signed: e.amount > 0 })}`}
                      >{logo.short[0]}</span>
                    );
                  })}
                  {events.length > 4 && (
                    <span className="rcal-dot rcal-more" title={`${events.length - 4} more`}>
                      +{events.length - 4}
                    </span>
                  )}
                </div>
              )}

              {load > 0 && <div className="rcal-load"></div>}

              {isToday && (
                <div className="rcal-today-glow"></div>
              )}
            </button>
          );
        })}
      </div>

      {/* Day detail */}
      <DayDetail
        month={month}
        day={selectedDay}
        events={eventsFor(selectedDay)}
        weekdayLong={WEEKDAY_LONG[weekdayOf(month, selectedDay)]}
        isToday={isCurrentMonth && selectedDay === today}
      />
    </div>
  );
}

function DayDetail({ month, day, events, weekdayLong, isToday }) {
  const total = events.reduce((s, e) => s + e.amount, 0);
  const [m, y] = month.split(" ");
  const mLong = MONTH_LONG[MONTH_NAMES.indexOf(m)];
  return (
    <div className={`rcal-detail ${isToday ? "today" : ""}`} key={`${month}-${day}`}>
      <div className="rcal-detail-head">
        <div>
          <div className="eyebrow"><span className="dot"></span>{weekdayLong} {isToday && <span className="rcal-detail-today">· today</span>}</div>
          <div className="rcal-detail-title">
            <span className="rcal-detail-num">{day}</span>
            <span className="rcal-detail-month">{mLong} {y}</span>
          </div>
          <div className="rcal-detail-sub">
            {events.length === 0
              ? "Nothing scheduled. A quiet day."
              : `${events.length} movement${events.length === 1 ? "" : "s"} · ${events.filter(e => e.amount > 0).length} in, ${events.filter(e => e.amount < 0).length} out`}
          </div>
        </div>
        {events.length > 0 && (
          <div className="rcal-detail-sum">
            <div className="eyebrow">Net</div>
            <div className={`rcal-detail-net ${total < 0 ? "neg" : total > 0 ? "pos" : ""}`}>
              {total === 0 ? "$0" : (total > 0 ? "+$" : "−$") + Math.abs(total).toLocaleString()}
            </div>
          </div>
        )}
      </div>

      {events.length > 0 && (
        <div className="rcal-detail-list">
          {events.map(e => {
            const cat = FS.categories.find(c => c.id === e.category);
            const logo = eventLogo(e);
            return (
              <div key={e.id} className="rcal-detail-row">
                <div className="rcal-detail-logo" style={{ background: logo.bg }}>{logo.short}</div>
                <div className="rcal-detail-info">
                  <div className="rcal-detail-name">
                    {e.name}
                    {e.status === "increased" && <span className="chip negative" style={{ padding: "1px 6px", fontSize: 10, marginLeft: 6 }}>price up</span>}
                    {e.status === "trial" && <span className="chip warning" style={{ padding: "1px 6px", fontSize: 10, marginLeft: 6 }}>trial</span>}
                    {e.status === "unused" && <span className="chip" style={{ padding: "1px 6px", fontSize: 10, marginLeft: 6 }}>unused</span>}
                  </div>
                  <div className="rcal-detail-meta">
                    <span className="cswatch" style={{ background: cat?.color, width: 7, height: 7 }}></span>
                    {cat?.label || e.category}
                    <span>·</span>
                    <span>since {e.since}</span>
                    {e.cadence !== "monthly" && <><span>·</span><span>{e.cadence}</span></>}
                  </div>
                </div>
                <div className={`rcal-detail-amt ${e.amount > 0 ? "pos" : ""}`}>
                  {e.amount > 0 ? "+$" : "−$"}{Math.abs(e.amount).toFixed(2)}
                </div>
                <button
                  className="rcal-detail-action"
                  title="More"
                  onClick={() => window.toast?.(e.name, {
                    sub: `Next ${e.next} · ${FS.fmt(e.amount, { signed: e.amount > 0 })}/mo · since ${e.since}`,
                    duration: 4000,
                  })}
                ><I.More width="13" height="13" /></button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

// Small helper used by the calendar header arrows
function shiftMonthLabel(label, delta) {
  const list = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  const [m, y] = label.split(" ");
  let mi = list.indexOf(m) + delta;
  let yi = parseInt(y);
  while (mi < 0) { mi += 12; yi--; }
  while (mi >= 12) { mi -= 12; yi++; }
  return `${list[mi]} ${yi}`;
}
