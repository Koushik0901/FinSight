import { toast } from "sonner";
import { useRulesWithCategories, useToggleRule } from "../api/hooks/transactions";
import type { RuleWithCategory, RuleProposal } from "../api/client";
import * as I from "../components/Icons";
import { useRuleProposals, useAcceptRuleProposal, useDeclineRuleProposal } from "../api/hooks/proposals";

function RuleCard({ rule }: { rule: RuleWithCategory }) {
  const toggle = useToggleRule();

  const handleToggle = async () => {
    try {
      await toggle.mutateAsync({ id: rule.id, enabled: !rule.enabled });
      toast.success(rule.enabled ? "Rule paused" : "Rule activated", {
        description: rule.pattern,
      });
    } catch {
      toast.error("Failed to update rule");
    }
  };

  return (
    <div className="rule" style={{ opacity: rule.enabled ? 1 : 0.55 }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div className="cond">
          <span className="tok k">when</span>
          <span className="tok">merchant contains</span>
          <span className="tok k">{rule.pattern.replaceAll("%", "")}</span>
          <span className="tok k">then</span>
          <span className="tok">categorize as</span>
          <span className="tok" style={{ color: rule.categoryColor || "var(--ink-2)", background: rule.categoryColor ? rule.categoryColor + "22" : "var(--surface-2)" }}>
            {rule.categoryLabel || rule.categoryId}
          </span>
        </div>
        <div className="muted" style={{ fontSize: 12.5, marginTop: 8, display: "flex", gap: 12, alignItems: "center" }}>
          <span style={{ display: "flex", alignItems: "center", gap: 4 }}>
            <I.Sparkle width="11" height="11" style={{ color: rule.source === "agent" ? "var(--accent)" : "var(--ink-faint)" }} />
            Owned by {rule.source === "agent" ? "Agent" : "You"}
          </span>
          <span>·</span>
          <span>{new Date(rule.createdAt).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}</span>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 10, flexShrink: 0 }}>
        <span
          className={`tog${rule.enabled ? " on" : ""}`}
          onClick={handleToggle}
          role="switch"
          aria-checked={rule.enabled}
          aria-label={`${rule.enabled ? "Disable" : "Enable"} rule: ${rule.pattern}`}
          tabIndex={0}
          onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); void handleToggle(); } }}
        />
      </div>
    </div>
  );
}

function ProposalRow({ proposal }: { proposal: RuleProposal }) {
  const accept = useAcceptRuleProposal();
  const decline = useDeclineRuleProposal();
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div className="eyebrow" style={{ marginBottom: 2 }}>{proposal.whenLabel}</div>
        <div style={{ fontSize: 14 }}>{proposal.description}</div>
      </div>
      <button
        className="btn primary"
        disabled={accept.isPending}
        aria-label={`Accept: ${proposal.description}`}
        onClick={async () => {
          try { await accept.mutateAsync(proposal.id); toast.success("Rule created"); }
          catch { toast.error("Could not accept proposal"); }
        }}
      >
        Accept
      </button>
      <button
        className="btn ghost sm"
        disabled={decline.isPending}
        aria-label={`Decline: ${proposal.description}`}
        onClick={async () => {
          try { await decline.mutateAsync(proposal.id); toast("Proposal declined"); }
          catch { toast.error("Could not decline proposal"); }
        }}
      >
        Decline
      </button>
    </div>
  );
}

export default function Rules() {
  const { data: rules = [], isLoading, error } = useRulesWithCategories();
  const { data: proposals = [] } = useRuleProposals();

  const active = rules.filter((r) => r.enabled);
  const paused = rules.filter((r) => !r.enabled);

  if (isLoading) return <div className="stub">Loading rules…</div>;
  if (error)     return <div className="stub">Error loading rules.</div>;

  return (
    <div className="screen">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">Rules &amp; agents</div>
          <h1>The mechanics underneath.</h1>
        </div>
      </div>

      <p className="muted" style={{ maxWidth: 660, marginTop: -12, marginBottom: 28, fontSize: 14, lineHeight: 1.6 }}>
        Rules are how FinSight quietly stays organized. The agent writes them
        when it spots patterns, and you can tune or disable each one. Most users
        never come here — but the door is always open.
      </p>

      <div style={{ display: "grid", gridTemplateColumns: "1.6fr 1fr", gap: 28 }}>
        {/* Rules list */}
        <div>
          {rules.length === 0 ? (
            <div className="card" style={{ textAlign: "center", padding: "48px 32px" }}>
              <I.Bolt style={{ color: "var(--ink-faint)", width: 24, height: 24, margin: "0 auto 12px" }} />
              <div style={{ fontSize: 14, color: "var(--ink-mute)" }}>
                No rules yet. Import transactions and let the agent categorize them — rules are created automatically when you correct a categorization.
              </div>
            </div>
          ) : (
            <>
              {active.length > 0 && (
                <>
                  <div className="eyebrow" style={{ marginBottom: 12 }}>
                    <span className="dot" />
                    Active · {active.length} {active.length === 1 ? "rule" : "rules"}
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 10, marginBottom: 28 }}>
                    {active.map((r) => <RuleCard key={r.id} rule={r} />)}
                  </div>
                </>
              )}

              {paused.length > 0 && (
                <>
                  <div className="eyebrow" style={{ marginBottom: 12 }}>
                    Paused · {paused.length}
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
                    {paused.map((r) => <RuleCard key={r.id} rule={r} />)}
                  </div>
                </>
              )}
            </>
          )}

          {proposals.length > 0 && (
            <div className="card" style={{ marginTop: 28, border: "1px dashed var(--accent)" }}>
              <div className="eyebrow" style={{ marginBottom: 12, color: "var(--accent)" }}>
                <I.Sparkle width="12" height="12" style={{ marginRight: 6 }} />
                <span>Agent proposals</span> · {proposals.length}
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
                {proposals.map((p) => <ProposalRow key={p.id} proposal={p} />)}
              </div>
            </div>
          )}
        </div>

        {/* Agent sidebar */}
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <div className="card">
            <div className="eyebrow" style={{ marginBottom: 12 }}>
              <span className="dot" />
              How rules work
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <div style={{ display: "grid", gridTemplateColumns: "20px 1fr", gap: 10, alignItems: "start" }}>
                <span style={{ width: 8, height: 8, borderRadius: 999, background: "var(--accent)", marginTop: 6, display: "inline-block" }} />
                <div>
                  <div style={{ fontSize: 14 }}>Pattern matching</div>
                  <div className="muted" style={{ fontSize: 12.5, marginTop: 1 }}>Rules match merchant names with SQL LIKE patterns</div>
                </div>
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "20px 1fr", gap: 10, alignItems: "start" }}>
                <span style={{ width: 8, height: 8, borderRadius: 999, background: "var(--accent)", marginTop: 6, display: "inline-block" }} />
                <div>
                  <div style={{ fontSize: 14 }}>Agent writes rules</div>
                  <div className="muted" style={{ fontSize: 12.5, marginTop: 1 }}>When you correct a category, the agent proposes a rule to prevent the same mistake</div>
                </div>
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "20px 1fr", gap: 10, alignItems: "start" }}>
                <span style={{ width: 8, height: 8, borderRadius: 999, background: "var(--accent)", marginTop: 6, display: "inline-block" }} />
                <div>
                  <div style={{ fontSize: 14 }}>You stay in control</div>
                  <div className="muted" style={{ fontSize: 12.5, marginTop: 1 }}>Toggle any rule off or on at any time</div>
                </div>
              </div>
            </div>
          </div>

          <div className="card tight">
            <div className="eyebrow" style={{ marginBottom: 10 }}>Trust dial</div>
            <p className="muted" style={{ fontSize: 13, lineHeight: 1.55, margin: "0 0 14px" }}>
              Adjust how much the agent acts without asking. You can change this per category in Settings.
            </p>
            <div style={{ padding: 12, background: "var(--surface-2)", borderRadius: 10 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
                <span style={{ fontSize: 13.5 }}>Auto-categorize</span>
                <span className="chip accent">High autonomy</span>
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: 13.5 }}>Apply rules automatically</span>
                <span className="chip accent">On</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
