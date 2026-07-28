//! MCP Apps UI resources — the optional visual half of the MCP surface.
//!
//! An MCP server can return a `ui://` HTML resource that a host (ChatGPT,
//! Claude) renders in a sandboxed iframe next to the tool result. That is how
//! the typed finance cards the in-app Copilot draws can reach an external
//! assistant at all: without this, a connected model gets JSON and renders
//! whatever prose it likes.
//!
//! Two rules shape everything here:
//!
//! 1. **Tools stay useful headless.** The widget is an enhancement, never a
//!    requirement. Every tool returns complete `structuredContent` on its own,
//!    and a host that ignores UI resources loses nothing but pixels.
//! 2. **Self-contained, no network.** Each widget is one HTML string with
//!    inline CSS and JS and no external fetches, so it satisfies the strictest
//!    host CSP without us declaring any `connectDomains`.
//!
//! The bridge is JSON-RPC over `postMessage`: the host sends
//! `ui/notifications/tool-result` with the tool's `structuredContent`, and the
//! widget renders it. We deliberately do NOT call back into `tools/call` —
//! these are read-only views, and a widget that could invoke tools would be a
//! second, unaudited path to the write surface.

use serde_json::{json, Value};

/// MCP Apps mime type. The `profile=` form is what the ext-apps spec and the
/// ChatGPT docs both name; hosts that don't recognise it just skip the resource.
const UI_MIME: &str = "text/html;profile=mcp-app";

/// A widget and the tool whose result it draws. `render_js` is just the
/// `render(data)` function; the shared shell (styles + postMessage bridge) is
/// wrapped around it by [`Widget::html`], so the four widgets cannot drift
/// apart on theming or on how they receive data.
pub(crate) struct Widget {
    pub tool: &'static str,
    pub uri: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub render_js: &'static str,
}

impl Widget {
    pub fn html(&self) -> String {
        format!(
            "<style>{SHELL_CSS}</style>\n<div id=\"root\"></div>\n<script>\n{}\n{BRIDGE_JS}\n</script>",
            self.render_js
        )
    }
}

/// Versioned URIs (`/v1`): a host may cache a resource by URI, so a changed
/// widget needs a changed URI rather than silently serving stale markup.
pub(crate) const WIDGETS: &[Widget] = &[
    Widget {
        tool: "get_net_worth",
        uri: "ui://finsight/net-worth/v1.html",
        name: "net-worth",
        description: "Net worth breakdown with assets, debt, and unconfirmed accounts",
        render_js: NET_WORTH_RENDER,
    },
    Widget {
        tool: "get_spending_breakdown",
        uri: "ui://finsight/spending-breakdown/v1.html",
        name: "spending-breakdown",
        description: "Spending by category and merchant, with a monthly trend",
        render_js: SPENDING_RENDER,
    },
    Widget {
        tool: "search_transactions",
        uri: "ui://finsight/transactions/v1.html",
        name: "transactions",
        description: "Matching transactions with running total",
        render_js: TRANSACTIONS_RENDER,
    },
    Widget {
        tool: "run_purchase_affordability",
        uri: "ui://finsight/affordability/v1.html",
        name: "affordability",
        description: "Purchase affordability verdict and alternatives",
        render_js: AFFORDABILITY_RENDER,
    },
];

pub(crate) fn widget_for(tool: &str) -> Option<&'static Widget> {
    WIDGETS.iter().find(|w| w.tool == tool)
}

pub(crate) fn widget_by_uri(uri: &str) -> Option<&'static Widget> {
    WIDGETS.iter().find(|w| w.uri == uri)
}

/// `resources/list` entries, sorted for the same reason the tool list is: a
/// HashMap-free but still stable order makes diffing a client's view possible.
pub(crate) fn resource_list() -> Vec<Value> {
    let mut out: Vec<Value> = WIDGETS
        .iter()
        .map(|w| {
            json!({
                "uri": w.uri,
                "name": w.name,
                "description": w.description,
                "mimeType": UI_MIME,
            })
        })
        .collect();
    out.sort_by(|a, b| a["uri"].as_str().cmp(&b["uri"].as_str()));
    out
}

pub(crate) fn resource_contents(w: &Widget) -> Value {
    json!({
        "uri": w.uri,
        "mimeType": UI_MIME,
        "text": w.html(),
        "_meta": {
            // No `csp` block: these widgets fetch nothing, so there is no domain
            // to allow. Saying so explicitly is better than a host guessing.
            "ui": { "prefersBorder": true }
        }
    })
}

/// The `_meta` a tool definition carries to point at its widget. Both spellings
/// are emitted on purpose: `ui.resourceUri` is the MCP Apps standard field and
/// `openai/outputTemplate` is the ChatGPT alias, and a server that wants to
/// render in both places has to say it twice.
pub(crate) fn tool_meta(tool: &str) -> Option<Value> {
    widget_for(tool).map(|w| {
        json!({
            "ui": { "resourceUri": w.uri },
            "openai/outputTemplate": w.uri,
        })
    })
}

// --------------------------------------------------------------- widgets ---
//
// Shared conventions across all four:
//   * `structuredContent` arrives as `{ok, data}` — the same envelope the tool
//     returns over the wire — so each widget reads `msg.params.structuredContent`
//     and then `.data`.
//   * Amounts are rendered from the `*_display` strings the server already
//     formatted. The widgets never divide cents, for exactly the reason the
//     model is told not to.
//   * Colours come from `prefers-color-scheme` so the card sits in a light or
//     dark host without being told which.

/// Shared <style> + bridge bootstrap. Kept as one string so every widget looks
/// like the same product and the postMessage wiring exists in exactly one place.
const SHELL_CSS: &str = r#"
:root{color-scheme:light dark}
*{box-sizing:border-box}
body{margin:0;font:14px/1.5 ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif;
  color:#16181d;background:transparent}
@media (prefers-color-scheme:dark){body{color:#e8eaed}}
.wrap{padding:14px 16px}
.eyebrow{font-size:11px;letter-spacing:.08em;text-transform:uppercase;opacity:.55;margin:0 0 6px}
.headline{font-size:28px;font-weight:600;letter-spacing:-.02em;margin:0}
.sub{opacity:.65;margin:2px 0 0;font-size:13px}
.grid{display:grid;gap:10px;grid-template-columns:repeat(auto-fit,minmax(140px,1fr));margin-top:14px}
.cell{padding:10px 12px;border:1px solid rgba(128,128,128,.22);border-radius:10px}
.cell .k{font-size:11px;opacity:.6;margin:0 0 3px}
.cell .v{font-size:16px;font-weight:600;margin:0;font-variant-numeric:tabular-nums}
.rows{margin-top:14px;display:flex;flex-direction:column;gap:7px}
.row{display:grid;grid-template-columns:1fr auto;gap:10px;align-items:baseline}
.row .lbl{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.row .amt{font-variant-numeric:tabular-nums;font-weight:600}
.bar{height:5px;border-radius:3px;background:currentColor;opacity:.8;min-width:2px}
.track{grid-column:1/-1;height:5px;border-radius:3px;background:rgba(128,128,128,.16)}
table{width:100%;border-collapse:collapse;margin-top:12px;font-variant-numeric:tabular-nums}
th{text-align:left;font-size:11px;text-transform:uppercase;letter-spacing:.06em;opacity:.55;
  padding:0 8px 6px 0;font-weight:500}
td{padding:6px 8px 6px 0;border-top:1px solid rgba(128,128,128,.16)}
td.num{text-align:right;font-weight:600}
.neg{color:#c0392b}@media (prefers-color-scheme:dark){.neg{color:#ff7a6b}}
.pos{color:#1d8a4e}@media (prefers-color-scheme:dark){.pos{color:#4ade80}}
.pill{display:inline-block;padding:3px 9px;border-radius:999px;font-size:12px;font-weight:600}
.pill.yes{background:rgba(29,138,78,.14);color:#1d8a4e}
.pill.no{background:rgba(192,57,43,.14);color:#c0392b}
@media (prefers-color-scheme:dark){.pill.yes{color:#4ade80}.pill.no{color:#ff7a6b}}
.note{margin-top:12px;padding:9px 11px;border-radius:8px;background:rgba(128,128,128,.1);
  font-size:12.5px;opacity:.85}
.empty{opacity:.6;padding:18px 0}
.scroll{max-height:340px;overflow:auto}
"#;

/// Listens for the host's tool-result notification and hands `data` to
/// `render`. Defensive by design: a host may deliver the result before or after
/// the frame loads, and an unrecognised payload must degrade to a quiet empty
/// state rather than a stack trace in someone's chat window.
const BRIDGE_JS: &str = r#"
const $=document.getElementById('root');
const money=(o,k)=>o&&o[k+'_display']!=null?o[k+'_display']:null;
const esc=s=>String(s==null?'':s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
function paint(sc){
  try{
    const data=sc&&sc.data!=null?sc.data:sc;
    if(!data||(sc&&sc.ok===false)){$.innerHTML='<div class="wrap empty">No data to show.</div>';return}
    $.innerHTML=render(data);
  }catch(e){$.innerHTML='<div class="wrap empty">Could not render this result.</div>'}
}
window.addEventListener('message',e=>{
  // Only the embedding host may drive this frame. Inert today (every value is
  // escaped and nothing calls back into tools/call), but it costs one line and
  // it means a future sink can't be reached by any other window.
  if(e.source!==window.parent)return;
  const m=e&&e.data;if(!m||typeof m!=='object')return;
  if(m.method==='ui/notifications/tool-result'||m.method==='ui/initialize'){
    const p=m.params||{};
    if(p.structuredContent!=null)paint(p.structuredContent);
  }
});
// Announce readiness both ways: some hosts wait for it, others have already
// sent the result, and re-announcing costs nothing.
try{window.parent.postMessage({jsonrpc:'2.0',id:1,method:'ui/initialize',params:{}},'*')}catch(e){}
"#;

const NET_WORTH_RENDER: &str = r#"
function render(d){
  const unknown=d.accounts_with_unknown_balance||0;
  const names=(d.unknown_balance_accounts||[]).map(esc).join(', ');
  return `<div class="wrap">
    <p class="eyebrow">Net worth</p>
    <p class="headline">${esc(money(d,'net_worth')||'—')}</p>
    <p class="sub">${d.accounts_with_known_balance||0} account(s) with a confirmed balance</p>
    <div class="grid">
      <div class="cell"><p class="k">Accounts</p><p class="v">${esc(money(d,'known_account_balance')||'—')}</p></div>
      <div class="cell"><p class="k">Manual assets</p><p class="v">${esc(money(d,'manual_asset')||'—')}</p></div>
      <div class="cell"><p class="k">Debt owed</p><p class="v neg">${esc(money(d,'liability')||'—')}</p></div>
    </div>
    ${unknown?`<div class="note"><strong>${unknown} account(s) excluded</strong> — balance not confirmed${names?': '+names:''}. The total above omits them; it is not $0 for these.</div>`:''}
    <div class="note">Debt is already subtracted from the figure above — don't subtract it again.</div>
  </div>`;
}
"#;

const SPENDING_RENDER: &str = r#"
function render(d){
  const cats=d.top_categories||[],merch=d.top_merchants||[],months=d.monthly||[];
  const max=a=>Math.max(1,...a.map(x=>Math.abs(x.spent_cents||0)));
  const mc=max(cats),mm=max(months);
  const bar=(v,m)=>`<div class="track"><div class="bar" style="width:${Math.round(Math.abs(v)/m*100)}%"></div></div>`;
  const list=(a,key,m)=>a.length?a.map(x=>`<div class="row"><span class="lbl">${esc(x[key])}</span>
      <span class="amt">${esc(x.spent_display||'')}</span>${bar(x.spent_cents||0,m)}</div>`).join(''):'<p class="empty">Nothing recorded.</p>';
  return `<div class="wrap">
    <p class="eyebrow">Spending · last ${d.window_months||'?'} month(s)</p>
    <p class="headline">${esc(money(d,'total_spent')||'—')}</p>
    ${d.note?`<p class="sub">${esc(d.note)}</p>`:''}
    <div class="rows"><p class="eyebrow" style="margin-top:6px">By category</p>${list(cats,'category',mc)}</div>
    ${months.length?`<div class="rows"><p class="eyebrow" style="margin-top:6px">By month</p>${list(months,'month',mm)}</div>`:''}
    ${merch.length?`<div class="rows"><p class="eyebrow" style="margin-top:6px">Top merchants</p>
      ${merch.slice(0,8).map(x=>`<div class="row"><span class="lbl">${esc(x.merchant)}</span><span class="amt">${esc(x.spent_display||'')}</span></div>`).join('')}</div>`:''}
  </div>`;
}
"#;

const TRANSACTIONS_RENDER: &str = r#"
function render(d){
  const rows=d.transactions||[];
  if(!rows.length)return '<div class="wrap empty">No transactions matched.</div>';
  return `<div class="wrap">
    <p class="eyebrow">${d.count||rows.length} transaction(s)${d.capped?' · more exist':''}</p>
    <p class="headline">${esc(money(d,'total_abs')||money(d,'total')||'—')}</p>
    <div class="scroll"><table>
      <thead><tr><th>Date</th><th>Merchant</th><th>Account</th><th style="text-align:right">Amount</th></tr></thead>
      <tbody>${rows.map(r=>`<tr>
        <td>${esc((r.date||'').slice(0,10))}</td>
        <td>${esc(r.merchant)}${r.category?`<br><span style="opacity:.55;font-size:12px">${esc(r.category)}</span>`:''}</td>
        <td style="opacity:.7">${esc(r.account||'')}</td>
        <td class="num ${(r.amount_cents||0)<0?'neg':'pos'}">${esc(r.amount_display||'')}</td></tr>`).join('')}</tbody>
    </table></div>
    ${d.capped?'<div class="note">Only the first page is shown — narrow the search for the rest.</div>':''}
  </div>`;
}
"#;

const AFFORDABILITY_RENDER: &str = r#"
function render(d){
  const yes=!!d.affordable_now;
  const alts=d.alternatives||[];
  return `<div class="wrap">
    <p class="eyebrow">Can you afford ${esc(money(d,'purchase_amount')||'this')}?</p>
    <p class="headline"><span class="pill ${yes?'yes':'no'}">${yes?'Yes':'Not yet'}</span></p>
    <p class="sub">${esc(d.recommendation||'')}</p>
    <div class="grid">
      <div class="cell"><p class="k">Emergency fund now</p><p class="v">${esc(money(d,'starting_emergency_fund')||'—')}</p></div>
      <div class="cell"><p class="k">After this purchase</p><p class="v">${esc(money(d,'emergency_fund_after_purchase')||'—')}</p></div>
      <div class="cell"><p class="k">Monthly surplus</p><p class="v">${esc(money(d,'monthly_surplus')||'—')}</p></div>
    </div>
    ${alts.length?`<div class="rows"><p class="eyebrow" style="margin-top:6px">Your options</p>
      ${alts.map(a=>`<div class="cell" style="margin-bottom:6px"><p class="v" style="font-size:14px">${esc(a.name)}</p>
        <p class="sub" style="margin-top:3px">${esc(a.action)}</p>
        <p class="sub" style="margin-top:3px;opacity:.5">${esc(a.tradeoff||'')}</p></div>`).join('')}</div>`:''}
    ${(d.missing_data||[]).length?`<div class="note"><strong>Provisional:</strong> ${esc(d.missing_data.join('; '))}</div>`:''}
  </div>`;
}
"#;
