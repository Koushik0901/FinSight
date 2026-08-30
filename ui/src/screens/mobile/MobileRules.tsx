import { useMemo, useState } from "react";
import { toast } from "sonner";
import { useRulesWithCategories, useToggleRule } from "../../api/hooks/transactions";
import type { RuleWithCategory } from "../../api/openapiClient";
import { MobilePageHeader } from "../../components/mobile/MobilePageHeader";
import { MobileList } from "../../components/mobile/MobileList";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import { SegmentedControl } from "../../components/mobile/SegmentedControl";
import { MobileEmptyState } from "../../components/mobile/MobileEmptyState";
import * as I from "../../components/Icons";

type Filter = "all" | "active" | "paused";

function cleanPattern(p: string): string {
  return p.replaceAll("%", "").trim() || p;
}

function ConditionChips({ rule }: { rule: RuleWithCategory }) {
  const pattern = cleanPattern(rule.pattern);
  return (
    <span style={{ display: "inline-flex", flexWrap: "wrap", gap: 6, alignItems: "center" }}>
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          padding: "3px 8px",
          borderRadius: 999,
          background: "var(--surface-2)",
          border: "1px solid var(--line)",
          fontSize: 11,
          fontWeight: 600,
          letterSpacing: "0.04em",
          textTransform: "uppercase",
          color: "var(--ink-faint)",
        }}
      >
        when merchant contains
      </span>
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          padding: "3px 8px",
          borderRadius: 999,
          background: "var(--surface)",
          border: "1px solid var(--line-2)",
          fontSize: 12,
          fontWeight: 650,
          color: "var(--ink)",
          maxWidth: "100%",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {pattern}
      </span>
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          padding: "3px 8px",
          borderRadius: 999,
          background: rule.categoryColor ? `${rule.categoryColor}22` : "var(--surface-2)",
          border: `1px solid ${rule.categoryColor || "var(--line)"}`,
          fontSize: 12,
          fontWeight: 600,
          color: rule.categoryColor || "var(--ink-2)",
          gap: 6,
        }}
      >
        <span
          aria-hidden="true"
          style={{
            width: 7,
            height: 7,
            borderRadius: 999,
            background: rule.categoryColor || "var(--ink-faint)",
            flexShrink: 0,
          }}
        />
        {rule.categoryLabel || rule.categoryId}
      </span>
    </span>
  );
}

function ThumbToggle({
  enabled,
  onToggle,
  label,
}: {
  enabled: boolean;
  onToggle: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      aria-label={label}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
      style={{
        minWidth: 44,
        minHeight: 44,
        width: 48,
        height: 44,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 0,
        border: "none",
        background: "transparent",
        cursor: "pointer",
        flexShrink: 0,
        WebkitTapHighlightColor: "transparent",
      }}
    >
      <span
        aria-hidden="true"
        style={{
          width: 48,
          height: 28,
          borderRadius: 999,
          background: enabled ? "var(--accent)" : "var(--line-2)",
          border: `1px solid ${enabled ? "var(--accent)" : "var(--line-2)"}`,
          position: "relative",
          display: "inline-block",
          transition: "background 160ms ease, border-color 160ms ease",
        }}
      >
        <span
          style={{
            position: "absolute",
            top: 2,
            left: enabled ? 22 : 2,
            width: 22,
            height: 22,
            borderRadius: 999,
            background: enabled ? "var(--accent-ink)" : "var(--surface)",
            boxShadow: "0 1px 4px rgba(0,0,0,0.18)",
            transition: "left 160ms cubic-bezier(.2,.7,.2,1), background 160ms ease",
          }}
        />
      </span>
    </button>
  );
}

export default function MobileRules() {
  const { data: rules = [], isLoading, error } = useRulesWithCategories();
  const toggle = useToggleRule();

  const [filter, setFilter] = useState<Filter>("all");
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<RuleWithCategory | null>(null);

  const filtered = useMemo(() => {
    let out = [...rules];
    if (filter === "active") out = out.filter((r) => r.enabled);
    if (filter === "paused") out = out.filter((r) => !r.enabled);
    const q = search.trim().toLowerCase();
    if (q) {
      out = out.filter((r) => {
        const pat = cleanPattern(r.pattern).toLowerCase();
        return (
          pat.includes(q) ||
          r.categoryLabel.toLowerCase().includes(q) ||
          r.categoryId.toLowerCase().includes(q) ||
          r.source.toLowerCase().includes(q)
        );
      });
    }
    out.sort((a, b) => {
      if (a.enabled !== b.enabled) return a.enabled ? -1 : 1;
      return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime();
    });
    return out;
  }, [rules, filter, search]);

  const handleToggle = async (rule: RuleWithCategory) => {
    try {
      await toggle.mutateAsync({ id: rule.id, enabled: !rule.enabled });
      toast.success(rule.enabled ? "Rule paused" : "Rule activated", {
        description: cleanPattern(rule.pattern),
      });
      if (selected && selected.id === rule.id) {
        setSelected({ ...selected, enabled: !selected.enabled });
      }
    } catch {
      toast.error("Failed to update rule");
    }
  };

  if (isLoading) {
    return (
      <div className="stub" aria-live="polite" aria-busy="true" style={{ padding: 16 }}>
        <span className="spinner" aria-hidden="true" /> Loading rules…
      </div>
    );
  }
  if (error) {
    return (
      <div className="stub" style={{ padding: 16 }}>
        Couldn’t load rules. Pull to retry.
      </div>
    );
  }

  const activeCount = rules.filter((r) => r.enabled).length;
  const pausedCount = rules.length - activeCount;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 16,
        padding: 16,
        paddingBottom: "calc(24px + env(safe-area-inset-bottom, 0px))",
        maxWidth: 430,
        margin: "0 auto",
        width: "100%",
        boxSizing: "border-box",
      }}
    >
      <MobilePageHeader
        eyebrow="Rules & automation"
        title="Rules"
        description="Tap a rule to inspect its condition. Toggle stays thumb-friendly — no tiny controls."
      />

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <SegmentedControl
          options={[
            { value: "all", label: `All · ${rules.length}` },
            { value: "active", label: `Active · ${activeCount}` },
            { value: "paused", label: `Paused · ${pausedCount}` },
          ]}
          value={filter}
          onChange={(v) => setFilter(v as Filter)}
          ariaLabel="Filter rules"
          fullWidth
        />

        <label
          htmlFor="mobile-rules-search"
          style={{
            display: "flex",
            alignItems: "center",
            gap: 10,
            minHeight: 44,
            padding: "0 12px",
            borderRadius: 12,
            background: "var(--surface)",
            border: "1px solid var(--line)",
            color: "var(--ink-faint)",
          }}
        >
          <I.Search width={16} height={16} aria-hidden="true" />
          <input
            id="mobile-rules-search"
            type="search"
            inputMode="search"
            placeholder="Search pattern or category"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            style={{
              flex: 1,
              minWidth: 0,
              border: "none",
              outline: "none",
              background: "transparent",
              color: "var(--ink)",
              fontSize: 14,
              minHeight: 44,
            }}
          />
          {search ? (
            <button
              type="button"
              aria-label="Clear search"
              onClick={() => setSearch("")}
              style={{
                minWidth: 44,
                minHeight: 44,
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                border: "none",
                background: "transparent",
                color: "var(--ink-mute)",
                cursor: "pointer",
              }}
            >
              <I.X width={14} height={14} />
            </button>
          ) : null}
        </label>
      </div>

      {rules.length === 0 ? (
        <MobileEmptyState
          icon={<I.Bolt width={28} height={28} />}
          title="No rules yet"
          description="Import transactions and correct a category — FinSight writes a rule so it won’t make the same mistake twice."
        />
      ) : filtered.length === 0 ? (
        <MobileEmptyState
          icon={<I.Search width={28} height={28} />}
          title="No matches"
          description={`No rules match “${search}” in ${filter === "all" ? "any status" : filter}. Try a different filter or search.`}
        />
      ) : (
        <section aria-labelledby="mobile-rules-list-heading">
          <h2 id="mobile-rules-list-heading" className="sr-only">
            Rules
          </h2>
          <MobileList ariaLabel="Rules">
            {filtered.map((rule) => (
              <div
                key={rule.id}
                role="listitem"
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  minHeight: 64,
                  padding: "10px 12px 10px 14px",
                  background: "var(--surface)",
                  borderBottom: "1px solid var(--line)",
                  opacity: rule.enabled ? 1 : 0.62,
                }}
              >
                <button
                  type="button"
                  onClick={() => setSelected(rule)}
                  aria-label={`Open rule: ${cleanPattern(rule.pattern)} → ${rule.categoryLabel}`}
                  style={{
                    flex: 1,
                    minWidth: 0,
                    display: "flex",
                    alignItems: "center",
                    gap: 12,
                    minHeight: 44,
                    border: "none",
                    background: "transparent",
                    padding: 0,
                    textAlign: "left",
                    cursor: "pointer",
                  }}
                >
                  <span
                    aria-hidden="true"
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: 999,
                      background: rule.categoryColor || "var(--line-2)",
                      flexShrink: 0,
                      border: "1px solid var(--line)",
                    }}
                  />
                  <span style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column", gap: 6 }}>
                    <span
                      style={{
                        fontSize: 14,
                        fontWeight: 600,
                        color: "var(--ink)",
                        lineHeight: 1.35,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {cleanPattern(rule.pattern)}
                    </span>
                    <span style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 6 }}>
                      <span
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 6,
                          padding: "2px 8px",
                          borderRadius: 999,
                          background: rule.categoryColor ? `${rule.categoryColor}22` : "var(--surface-2)",
                          border: `1px solid ${rule.categoryColor || "var(--line)"}`,
                          color: rule.categoryColor || "var(--ink-mute)",
                          fontSize: 12,
                          fontWeight: 600,
                          maxWidth: "100%",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                      >
                        <span
                          aria-hidden="true"
                          style={{
                            width: 6,
                            height: 6,
                            borderRadius: 999,
                            background: rule.categoryColor || "var(--ink-faint)",
                            flexShrink: 0,
                          }}
                        />
                        {rule.categoryLabel || rule.categoryId}
                      </span>
                      <span
                        style={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 4,
                          fontSize: 11,
                          color: "var(--ink-faint)",
                        }}
                      >
                        <I.Sparkle
                          width={11}
                          height={11}
                          style={{ color: rule.source === "agent" ? "var(--accent)" : "var(--ink-faint)" }}
                        />
                        {rule.source === "agent" ? "Agent" : "You"}
                      </span>
                    </span>
                  </span>
                  <span
                    aria-hidden="true"
                    style={{ color: "var(--ink-faint)", display: "inline-flex", flexShrink: 0 }}
                  >
                    <I.ArrowRight width={14} height={14} />
                  </span>
                </button>

                <ThumbToggle
                  enabled={rule.enabled}
                  onToggle={() => handleToggle(rule)}
                  label={`${rule.enabled ? "Disable" : "Enable"} rule: ${cleanPattern(rule.pattern)}`}
                />
              </div>
            ))}
          </MobileList>

          <p
            className="muted"
            style={{ fontSize: 12, lineHeight: 1.5, margin: "10px 2px 0", color: "var(--ink-faint)" }}
          >
            Rules match merchant names with patterns. Toggle is 44&nbsp;px — easy to hit with a thumb.
          </p>
        </section>
      )}

      <BottomSheet
        open={!!selected}
        onClose={() => setSelected(null)}
        title={selected ? cleanPattern(selected.pattern) : "Rule"}
        description={selected ? `Rule for ${selected.categoryLabel}` : undefined}
      >
        {selected ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
            <div
              style={{
                padding: 12,
                borderRadius: 12,
                background: "var(--surface)",
                border: "1px solid var(--line)",
                display: "flex",
                flexDirection: "column",
                gap: 10,
              }}
            >
              <div
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  letterSpacing: "0.08em",
                  textTransform: "uppercase",
                  color: "var(--ink-faint)",
                }}
              >
                Condition
              </div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 8, alignItems: "center" }}>
                <ConditionChips rule={selected} />
              </div>
              <div
                style={{
                  display: "flex",
                  flexWrap: "wrap",
                  gap: 8,
                  fontSize: 12,
                  color: "var(--ink-mute)",
                  lineHeight: 1.5,
                }}
              >
                <span className="chip" style={{ fontSize: 11 }}>
                  merchant contains
                </span>
                <span style={{ color: "var(--ink-faint)" }}>matching is case-insensitive</span>
              </div>
            </div>

            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: 12,
                padding: "12px 12px",
                background: "var(--surface)",
                border: "1px solid var(--line)",
                borderRadius: 12,
                minHeight: 44,
              }}
            >
              <span
                style={{
                  fontSize: 11,
                  fontWeight: 700,
                  letterSpacing: "0.08em",
                  textTransform: "uppercase",
                  color: "var(--ink-faint)",
                }}
              >
                Category
              </span>
              <span
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 8,
                  fontWeight: 600,
                  fontSize: 13,
                  color: "var(--ink)",
                }}
              >
                <span
                  aria-hidden="true"
                  style={{
                    width: 10,
                    height: 10,
                    borderRadius: 999,
                    background: selected.categoryColor || "var(--line-2)",
                    border: "1px solid var(--line)",
                    flexShrink: 0,
                  }}
                />
                {selected.categoryLabel || selected.categoryId}
              </span>
            </div>

            <div
              style={{
                display: "flex",
                justifyContent: "space-between",
                gap: 12,
                padding: "10px 12px",
                background: "var(--surface-2)",
                border: "1px solid var(--line)",
                borderRadius: 12,
                fontSize: 12,
                color: "var(--ink-mute)",
              }}
            >
              <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                <I.Sparkle
                  width={12}
                  height={12}
                  style={{ color: selected.source === "agent" ? "var(--accent)" : "var(--ink-faint)" }}
                />
                {selected.source === "agent" ? "Created by Agent" : "Created by you"}
              </span>
              <span>{new Date(selected.createdAt).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}</span>
            </div>

            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: 12,
                padding: "6px 6px 6px 12px",
                background: "var(--surface)",
                border: "1px solid var(--line)",
                borderRadius: 12,
                minHeight: 56,
              }}
            >
              <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                <span style={{ fontSize: 13.5, fontWeight: 600, color: "var(--ink)" }}>
                  {selected.enabled ? "Enabled" : "Paused"}
                </span>
                <span style={{ fontSize: 12, color: "var(--ink-mute)", lineHeight: 1.4 }}>
                  {selected.enabled ? "This rule runs automatically." : "Paused — won’t categorize until resumed."}
                </span>
              </div>
              <ThumbToggle
                enabled={selected.enabled}
                onToggle={() => handleToggle(selected)}
                label={`${selected.enabled ? "Pause" : "Enable"} rule`}
              />
            </div>

            <div
              style={{
                display: "flex",
                gap: 10,
                paddingTop: 4,
                borderTop: "1px solid var(--line)",
                marginTop: 4,
                paddingBottom: "env(safe-area-inset-bottom, 0px)",
              }}
            >
              <button
                type="button"
                className="btn"
                style={{ flex: 1, minHeight: 44, justifyContent: "center" }}
                onClick={() => setSelected(null)}
              >
                Close
              </button>
              <button
                type="button"
                className="btn"
                style={{
                  flex: 1,
                  minHeight: 44,
                  justifyContent: "center",
                  color: "var(--negative)",
                  borderColor: "color-mix(in oklab, var(--negative) 30%, var(--line))",
                  background: "color-mix(in oklab, var(--negative) 8%, var(--surface))",
                }}
                onClick={() => {
                  toast.error("Delete isn’t available yet", {
                    description: "Disable the rule to stop it — deletion will come in a later release.",
                  });
                }}
              >
                <I.Trash width={14} height={14} aria-hidden="true" />
                Delete rule
              </button>
            </div>
          </div>
        ) : null}
      </BottomSheet>
    </div>
  );
}
