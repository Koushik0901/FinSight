import { useState } from "react";
import { toast } from "sonner";
import {
  useRulesWithCategories,
  useToggleRule,
  useCategoriesWithSpending,
  useCreateRule,
} from "../api/hooks/transactions";
import type { RuleWithCategory, RuleProposal, CategoryWithSpending } from "../api/client";
import * as I from "../components/Icons";
import Button from "../components/Button";
import Card from "../components/Card";
import Input from "../components/Input";
import Select from "../components/Select";
import EmptyState from "../components/EmptyState";
import { useRuleProposals, useAcceptRuleProposal, useDeclineRuleProposal } from "../api/hooks/proposals";
import { useRecentAgentActivity } from "../api/hooks/insights";

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
    <article className="rule" style={{ opacity: rule.enabled ? 1 : 0.55 }}>
      <div className="grow stack stack-xs">
        <div className="cond">
          <span className="tok k">when</span>
          <span className="tok">merchant contains</span>
          <span className="tok k">{rule.pattern.replaceAll("%", "")}</span>
          <span className="tok k">then</span>
          <span className="tok">categorize as</span>
          <span
            className="tok"
            style={{
              color: rule.categoryColor || "var(--ink-2)",
              background: rule.categoryColor ? rule.categoryColor + "22" : "var(--surface-2)",
            }}
          >
            {rule.categoryLabel || rule.categoryId}
          </span>
        </div>
        <div className="muted" style={{ fontSize: 12.5, display: "flex", gap: 12, alignItems: "center" }}>
          <span className="row-xs">
            <I.Sparkle width={11} height={11} style={{ color: rule.source === "agent" ? "var(--accent)" : "var(--ink-faint)" }} />
            Owned by {rule.source === "agent" ? "Agent" : "You"}
          </span>
          <span>·</span>
          <span>{new Date(rule.createdAt).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}</span>
        </div>
      </div>
      <div className="row-sm" style={{ flexShrink: 0 }}>
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
    </article>
  );
}

function ProposalRow({ proposal }: { proposal: RuleProposal }) {
  const accept = useAcceptRuleProposal();
  const decline = useDeclineRuleProposal();
  return (
    <div className="row-md" style={{ alignItems: "center" }}>
      <div className="grow stack stack-xs">
        <div className="eyebrow" style={{ marginBottom: 2 }}>{proposal.whenLabel}</div>
        <div style={{ fontSize: 14 }}>{proposal.description}</div>
      </div>
      <Button
        variant="primary"
        size="sm"
        loading={accept.isPending}
        disabled={accept.isPending}
        aria-label={`Accept: ${proposal.description}`}
        onClick={async () => {
          try { await accept.mutateAsync(proposal.id); toast.success("Rule created"); }
          catch { toast.error("Could not accept proposal"); }
        }}
      >
        Accept
      </Button>
      <Button
        variant="ghost"
        size="sm"
        loading={decline.isPending}
        disabled={decline.isPending}
        aria-label={`Decline: ${proposal.description}`}
        onClick={async () => {
          try { await decline.mutateAsync(proposal.id); toast("Proposal declined"); }
          catch { toast.error("Could not decline proposal"); }
        }}
      >
        Decline
      </Button>
    </div>
  );
}

interface NewRuleFormProps {
  cats: CategoryWithSpending[];
  onCreated: () => void;
  onCancel: () => void;
}

function NewRuleForm({ cats, onCreated, onCancel }: NewRuleFormProps) {
  const [newPattern, setNewPattern] = useState("");
  const [newCategoryId, setNewCategoryId] = useState("");
  const createRule = useCreateRule();

  const resetForm = () => {
    setNewPattern("");
    setNewCategoryId("");
  };

  return (
    <Card className="rule-form stack stack-md" style={{ padding: 20, marginBottom: 16 }}>
      <div style={{ fontSize: 15, fontWeight: 600 }}>New rule</div>
      <div className="form-grid">
        <Input
          id="new-rule-pattern"
          label="Pattern"
          value={newPattern}
          onChange={(e) => setNewPattern(e.target.value)}
          placeholder="%starbucks%"
        />
        <Select
          id="new-rule-category"
          label="Category"
          value={newCategoryId}
          onChange={(e) => setNewCategoryId(e.target.value)}
        >
          <option value="">Select category…</option>
          {cats.map((c) => (
            <option key={c.id} value={c.id}>{c.label}</option>
          ))}
        </Select>
      </div>
      {newPattern && newCategoryId && (
        <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>
          when merchant contains "{newPattern.replace(/%/g, "")}" → {cats.find((c) => c.id === newCategoryId)?.label}
        </div>
      )}
      <div className="row-sm">
        <Button
          variant="primary"
          size="sm"
          loading={createRule.isPending}
          disabled={!newPattern.trim() || !newCategoryId || createRule.isPending}
          onClick={async () => {
            const raw = newPattern.trim();
            const pattern = raw.includes("%") ? raw : `%${raw}%`;
            try {
              await createRule.mutateAsync({ pattern, categoryId: newCategoryId });
              toast.success("Rule created");
              resetForm();
              onCreated();
            } catch {
              toast.error("Failed to create rule");
            }
          }}
        >
          Create rule
        </Button>
        <Button variant="ghost" size="sm" onClick={() => { resetForm(); onCancel(); }}>
          Cancel
        </Button>
      </div>
    </Card>
  );
}

export default function Rules() {
  const { data: rules = [], isLoading, error } = useRulesWithCategories();
  const { data: proposals = [] } = useRuleProposals();
  const { data: activity = [] } = useRecentAgentActivity(20);
  const [showNewRule, setShowNewRule] = useState(false);
  const { data: cats = [] } = useCategoriesWithSpending();
  const sortedCats = [...cats].sort((a, b) => a.label.localeCompare(b.label));

  const active = rules.filter((r) => r.enabled);
  const paused = rules.filter((r) => !r.enabled);

  if (isLoading) return <div className="stub">Loading rules…</div>;
  if (error)     return <div className="stub">Error loading rules.</div>;

  return (
    <div className="screen screen-rules">
      <header className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Workshop · Rules & agents</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Automate the mundane.</h1>
        </div>
        <Button variant="default" onClick={() => setShowNewRule(true)}>
          + New rule
        </Button>
      </header>

      <p className="muted" style={{ maxWidth: 660, marginTop: -12, marginBottom: 28, fontSize: 14, lineHeight: 1.6 }}>
        Rules are how FinSight quietly stays organized. The agent writes them
        when it spots patterns, and you can tune or disable each one. Most users
        never come here — but the door is always open.
      </p>

      <div className="responsive-grid" style={{ gridTemplateColumns: "1.6fr 1fr" }}>
        <div className="stack stack-xl">
          {showNewRule && (
            <NewRuleForm
              cats={sortedCats}
              onCreated={() => setShowNewRule(false)}
              onCancel={() => setShowNewRule(false)}
            />
          )}

          {rules.length === 0 ? (
            <EmptyState
              icon={<I.Bolt style={{ color: "var(--ink-faint)", width: 24, height: 24 }} />}
              title="No rules yet"
              description="Import transactions and let the agent categorize them — rules are created automatically when you correct a categorization."
              compact
            />
          ) : (
            <>
              {active.length > 0 && (
                <section className="stack stack-md" aria-labelledby="rules-active">
                  <div id="rules-active" className="eyebrow">
                    <span className="dot" />
                    Active · {active.length} {active.length === 1 ? "rule" : "rules"}
                  </div>
                  <div className="stack stack-md">
                    {active.map((r) => <RuleCard key={r.id} rule={r} />)}
                  </div>
                </section>
              )}

              {paused.length > 0 && (
                <section className="stack stack-md" aria-labelledby="rules-paused">
                  <div id="rules-paused" className="eyebrow">
                    <span className="dot" />
                    Paused · {paused.length}
                  </div>
                  <div className="stack stack-md">
                    {paused.map((r) => <RuleCard key={r.id} rule={r} />)}
                  </div>
                </section>
              )}
            </>
          )}

          {proposals.length > 0 && (
            <Card tone="accent" className="stack stack-md" style={{ borderStyle: "dashed" }}>
              <div className="eyebrow" style={{ color: "var(--accent)" }}>
                <I.Sparkle width={12} height={12} />
                <span>Agent proposals</span> · {proposals.length}
              </div>
              <div className="stack stack-md">
                {proposals.map((p) => <ProposalRow key={p.id} proposal={p} />)}
              </div>
            </Card>
          )}
        </div>

        <aside className="stack stack-lg">
          <Card className="stack stack-md">
            <div className="eyebrow">
              <span className="dot" />
              How rules work
            </div>
            <ul className="stack stack-md" style={{ margin: 0, padding: 0, listStyle: "none" }}>
              {[
                { title: "Pattern matching", desc: "Rules match merchant names with SQL LIKE patterns" },
                { title: "Agent writes rules", desc: "When you correct a category, the agent proposes a rule to prevent the same mistake" },
                { title: "You stay in control", desc: "Toggle any rule off or on at any time" },
              ].map((item) => (
                <li key={item.title} className="row-sm" style={{ alignItems: "flex-start" }}>
                  <span
                    className="dot"
                    style={{ marginTop: 6, flexShrink: 0 }}
                    aria-hidden="true"
                  />
                  <div className="stack stack-xs">
                    <div style={{ fontSize: 14 }}>{item.title}</div>
                    <div className="muted" style={{ fontSize: 12.5 }}>{item.desc}</div>
                  </div>
                </li>
              ))}
            </ul>
          </Card>

          <Card className="stack stack-md" tight>
            <div className="eyebrow"><span className="dot" />Trust dial</div>
            <p className="muted" style={{ fontSize: 13, lineHeight: 1.55, margin: 0 }}>
              Adjust how much the agent acts without asking. You can change this per category in Settings.
            </p>
            <Card tone="muted" tight className="stack stack-sm">
              <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: 13.5 }}>Auto-categorize</span>
                <span className="chip accent">High autonomy</span>
              </div>
              <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: 13.5 }}>Apply rules automatically</span>
                <span className="chip accent">On</span>
              </div>
            </Card>
          </Card>

          <Card className="stack stack-md" tight>
            <div className="eyebrow">
              <span className="dot" style={{ background: "var(--accent)" }} />
              Agent · last 24h
            </div>
            {activity.length === 0 ? (
              <p className="muted" style={{ fontSize: 13 }}>Nothing yet — import transactions to see activity.</p>
            ) : (
              <ul className="stack stack-sm" style={{ margin: 0, padding: 0, listStyle: "none" }}>
                {activity.map((a, i) => (
                  <li
                    key={i}
                    className="row-md"
                    style={{
                      justifyContent: "space-between",
                      padding: "8px 0",
                      borderBottom: i < activity.length - 1 ? "1px solid var(--hairline)" : "none",
                    }}
                  >
                    <div className="stack stack-xs">
                      <div style={{ fontSize: 13 }}>{a.text}</div>
                      <div className="muted" style={{ fontSize: 12 }}>{a.sub}</div>
                    </div>
                    <span className="num muted" style={{ fontSize: 11.5, whiteSpace: "nowrap" }}>
                      {a.minutesAgo < 60 ? `${a.minutesAgo}m` : `${Math.floor(a.minutesAgo / 60)}h`}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </Card>
        </aside>
      </div>
    </div>
  );
}
