import { useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { useJourneyStatus } from "../../api/hooks/journey";
import type { JourneyMilestone } from "../../api/openapiClient";
import { MobilePageHeader } from "../../components/mobile/MobilePageHeader";
import { BottomSheet } from "../../components/mobile/BottomSheet";
import * as I from "../../components/Icons";

const QUOTES = {
  early: "“A journey of a thousand miles begins with a single step.” — Lao Tzu",
  middle: "“Do not save what is left after spending, but spend what is left after saving.” — Warren Buffett",
  growth: "“Compound interest is the eighth wonder of the world.” — attributed to Einstein",
  freedom: "“Financial freedom is available to those who learn about it and work for it.” — Robert Kiyosaki",
} as const;

function quoteForStage(stage: number): string {
  if (stage <= 2) return QUOTES.early;
  if (stage <= 4) return QUOTES.middle;
  if (stage <= 6) return QUOTES.growth;
  return QUOTES.freedom;
}

function statusDotStyle(status: string) {
  if (status === "completed") {
    return {
      background: "var(--positive)",
      borderColor: "var(--positive)",
      color: "var(--bg)",
    } as const;
  }
  if (status === "current") {
    return {
      background: "var(--accent)",
      borderColor: "var(--accent)",
      color: "var(--accent-ink)",
    } as const;
  }
  return {
    background: "transparent",
    borderColor: "var(--line)",
    color: "var(--ink-faint)",
  } as const;
}

function MilestoneCard({
  milestone,
  isLast,
  onOpen,
}: {
  milestone: JourneyMilestone;
  isLast: boolean;
  onOpen: () => void;
}) {
  const completed = milestone.status === "completed";
  const current = milestone.status === "current";
  const locked = !completed && !current;

  const dotStyle = statusDotStyle(milestone.status);
  const pct = Math.max(0, Math.min(100, milestone.progressPct));

  return (
    <div style={{ display: "flex", gap: 12, alignItems: "stretch" }}>
      {/* Timeline rail */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          width: 28,
          flexShrink: 0,
        }}
        aria-hidden="true"
      >
        {/* Dot container sized to 44px touch? dot itself 28, centered */}
        <div
          style={{
            width: 28,
            height: 28,
            borderRadius: "var(--radius-pill)",
            border: `1.5px solid ${dotStyle.borderColor}`,
            background: dotStyle.background,
            color: dotStyle.color,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 12,
            fontWeight: 700,
            lineHeight: 1,
            flexShrink: 0,
            marginTop: 6,
          }}
        >
          {completed ? <I.Check width={13} height={13} /> : milestone.stage}
        </div>
        {!isLast ? (
          <div
            style={{
              width: 2,
              flex: 1,
              marginTop: 8,
              marginBottom: -8,
              background: completed ? "var(--positive)" : "var(--line)",
              opacity: completed ? 0.35 : 1,
              borderRadius: 1,
              minHeight: 20,
            }}
          />
        ) : null}
      </div>

      {/* Card — one insight per card, tappable 44px min */}
      <button
        type="button"
        onClick={onOpen}
        aria-label={`Stage ${milestone.stage} ${milestone.name}, ${milestone.status}, ${pct} percent. Tap for details.`}
        style={{
          flex: 1,
          minWidth: 0,
          textAlign: "left",
          border: "none",
          background: current ? "var(--accent-2)" : "var(--surface-2)",
          borderRadius: "var(--radius-lg)",
          padding: "14px 14px 13px",
          display: "flex",
          flexDirection: "column",
          gap: 10,
          cursor: "pointer",
          minHeight: 44,
          transition: "background 120ms ease, transform 120ms ease",
        }}
      >
        <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 10 }}>
          <div style={{ minWidth: 0, flex: 1 }}>
            <div
              className="eyebrow"
              style={{
                fontSize: 10.5,
                letterSpacing: "0.08em",
                textTransform: "uppercase",
                color: current ? "var(--accent)" : completed ? "var(--positive)" : "var(--ink-faint)",
                fontWeight: 600,
                lineHeight: 1,
                marginBottom: 6,
              }}
            >
              Stage {milestone.stage} · {completed ? "Completed" : current ? "Current focus" : "Locked"}
            </div>
            <div
              style={{
                fontSize: 15,
                fontWeight: current ? 650 : 600,
                color: locked ? "var(--ink-mute)" : "var(--ink)",
                lineHeight: 1.35,
                letterSpacing: "-0.01em",
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {milestone.name}
            </div>
            {/* One insight — the detail line */}
            <div
              style={{
                fontSize: 12.5,
                color: "var(--ink-mute)",
                lineHeight: 1.5,
                marginTop: 4,
                display: "-webkit-box",
                WebkitLineClamp: 2,
                WebkitBoxOrient: "vertical",
                overflow: "hidden",
              }}
              className={milestone.detail.includes("$") ? "money" : undefined}
            >
              {milestone.detail}
            </div>
          </div>
          <span
            style={{
              flexShrink: 0,
              width: 28,
              height: 28,
              borderRadius: "var(--radius-pill)",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              color: "var(--ink-faint)",
              marginTop: 2,
            }}
            aria-hidden="true"
          >
            <I.ArrowRight width={12} height={12} />
          </span>
        </div>

        {/* Progress bar — no tiny legends, just bar + percent */}
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 2 }}>
          <div
            role="progressbar"
            aria-valuenow={pct}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-label={`Stage ${milestone.stage} progress ${pct} percent`}
            style={{
              flex: 1,
              height: 4,
              borderRadius: "var(--radius-pill)",
              background: "var(--line)",
              overflow: "hidden",
            }}
          >
            <div
              className="plot-grow-x"
              style={{
                width: `${pct}%`,
                height: "100%",
                borderRadius: "var(--radius-pill)",
                background: completed ? "var(--positive)" : current ? "var(--accent)" : "var(--ink-faint)",
                opacity: locked && pct === 0 ? 0 : 1,
                transition: "width 320ms ease",
              }}
            />
          </div>
          <span
            className="num"
            style={{
              fontSize: 11.5,
              fontWeight: 600,
              fontVariantNumeric: "tabular-nums",
              color: current ? "var(--accent)" : completed ? "var(--positive)" : "var(--ink-faint)",
              minWidth: 32,
              textAlign: "right",
            }}
          >
            {pct}%
          </span>
        </div>
      </button>
    </div>
  );
}

export default function MobileJourney() {
  const navigate = useNavigate();
  const { data, isLoading, error } = useJourneyStatus();
  const [selected, setSelected] = useState<JourneyMilestone | null>(null);

  const nextMilestone = useMemo(() => {
    if (!data) return null;
    return data.milestones.find((m) => m.status === "current") ?? data.milestones.find((m) => m.status !== "completed") ?? null;
  }, [data]);

  if (isLoading) {
    return (
      <div style={{ padding: 16 }}>
        <div className="stub">
          <span className="spinner" aria-hidden="true" /> Loading journey…
        </div>
      </div>
    );
  }

  if (error || !data) {
    return (
      <div style={{ padding: 16 }}>
        <div className="stub">Unable to load journey. Pull to retry.</div>
      </div>
    );
  }

  const quote = quoteForStage(data.currentStage);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: 24 }}>
      <MobilePageHeader
        title="Journey"
        eyebrow="7 milestones"
        description="From stability to freedom — track progress one milestone at a time."
      />

      {/* Completed count + next action — premium calm, no borders */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 10,
          padding: "14px 16px",
          borderRadius: "var(--radius-lg)",
          background: "var(--elevated)",
        }}
      >
        <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 12 }}>
          <span style={{ fontSize: 11, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>
            Progress
          </span>
          <span
            className="num"
            style={{ fontSize: 13, fontWeight: 650, color: "var(--ink)", fontVariantNumeric: "tabular-nums" }}
          >
            {data.completedCount} of 7 completed
          </span>
        </div>

        {/* Subtle progress track for overall */}
        <div
          role="progressbar"
          aria-valuenow={Math.round((data.completedCount / 7) * 100)}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-label="Overall journey progress"
          style={{ height: 4, borderRadius: "var(--radius-pill)", background: "var(--line)", overflow: "hidden" }}
        >
          <div
            className="plot-grow-x"
            style={{
              width: `${(data.completedCount / 7) * 100}%`,
              height: "100%",
              borderRadius: "var(--radius-pill)",
              background: "var(--accent)",
              transition: "width 360ms ease",
            }}
          />
        </div>

        {nextMilestone ? (
          <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 2 }}>
            <span
              style={{
                width: 28,
                height: 28,
                borderRadius: "var(--radius-pill)",
                background: "var(--accent)",
                color: "var(--accent-ink)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 11,
                fontWeight: 700,
                flexShrink: 0,
              }}
            >
              {nextMilestone.stage}
            </span>
            <div style={{ minWidth: 0, flex: 1 }}>
              <div style={{ fontSize: 11, color: "var(--ink-faint)", lineHeight: 1 }}>Next up</div>
              <div style={{ fontSize: 13.5, fontWeight: 600, color: "var(--ink)", lineHeight: 1.35, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                {nextMilestone.name}
              </div>
            </div>
            <button
              type="button"
              onClick={() => setSelected(nextMilestone)}
              style={{
                flexShrink: 0,
                minHeight: 44,
                minWidth: 44,
                padding: "0 14px",
                borderRadius: "var(--radius-pill)",
                border: "1px solid var(--line)",
                background: "var(--surface-2)",
                color: "var(--ink)",
                fontSize: 12.5,
                fontWeight: 600,
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                gap: 6,
              }}
            >
              View <I.ArrowRight width={11} height={11} />
            </button>
          </div>
        ) : (
          <div style={{ fontSize: 13.5, color: "var(--positive)", fontWeight: 600, lineHeight: 1.4 }}>
            All 7 milestones complete — you’re financially free. Celebrate this.
          </div>
        )}
      </div>

      {/* Vertical timeline */}
      <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
        {data.milestones.map((m, idx) => (
          <MilestoneCard
            key={m.stage}
            milestone={m}
            isLast={idx === data.milestones.length - 1}
            onOpen={() => setSelected(m)}
          />
        ))}
      </div>

      {/* Keep going — calm typographic quote, no card border needed */}
      <div
        style={{
          marginTop: 4,
          padding: "18px 16px",
          borderRadius: "var(--radius-lg)",
          background: "var(--surface-2)",
          display: "flex",
          flexDirection: "column",
          gap: 8,
          alignItems: "center",
          textAlign: "center",
        }}
      >
        <span style={{ fontSize: 10.5, letterSpacing: "0.1em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600 }}>
          Keep going
        </span>
        <p style={{ margin: 0, fontSize: 13.5, lineHeight: 1.6, color: "var(--ink-mute)", maxWidth: 32 + "rem", textWrap: "pretty" as const }}>
          {quote}
        </p>
      </div>

      {/* Detail BottomSheet — not desktop dialogs */}
      <BottomSheet
        open={!!selected}
        onClose={() => setSelected(null)}
        title={selected ? `Stage ${selected.stage} · ${selected.name}` : ""}
      >
        {selected ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 16, paddingBottom: 8 }}>
            {/* Status + progress */}
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 6,
                  fontSize: 11,
                  fontWeight: 700,
                  letterSpacing: "0.06em",
                  textTransform: "uppercase",
                  color:
                    selected.status === "completed"
                      ? "var(--positive)"
                      : selected.status === "current"
                        ? "var(--accent)"
                        : "var(--ink-faint)",
                  background:
                    selected.status === "completed"
                      ? "var(--positive-2, rgba(52,211,153,0.12))"
                      : selected.status === "current"
                        ? "var(--accent-2)"
                        : "var(--surface-2)",
                  padding: "6px 10px",
                  borderRadius: "var(--radius-pill)",
                }}
              >
                <span
                  style={{
                    width: 6,
                    height: 6,
                    borderRadius: "var(--radius-pill)",
                    background: "currentColor",
                    flexShrink: 0,
                  }}
                />
                {selected.status === "completed" ? "Completed" : selected.status === "current" ? "Current focus" : "Locked"}
              </span>
              <span className="num" style={{ marginLeft: "auto", fontSize: 12.5, fontWeight: 650, color: "var(--ink-faint)", fontVariantNumeric: "tabular-nums" }}>
                {selected.progressPct}%
              </span>
            </div>

            <div style={{ height: 4, borderRadius: "var(--radius-pill)", background: "var(--line)", overflow: "hidden" }}>
              <div
                style={{
                  width: `${Math.max(0, Math.min(100, selected.progressPct))}%`,
                  height: "100%",
                  borderRadius: "var(--radius-pill)",
                  background: selected.status === "completed" ? "var(--positive)" : selected.status === "current" ? "var(--accent)" : "var(--ink-faint)",
                }}
              />
            </div>

            <div>
              <h3 style={{ margin: 0, fontSize: 16, fontWeight: 700, color: "var(--ink)", lineHeight: 1.3 }}>{selected.name}</h3>
              <p style={{ margin: "8px 0 0", fontSize: 13.5, lineHeight: 1.6, color: "var(--ink-mute)" }}>{selected.description}</p>
            </div>

            <div
              style={{
                padding: "10px 12px",
                borderRadius: "var(--radius)",
                background: "var(--surface-2)",
              }}
            >
              <div style={{ fontSize: 11, letterSpacing: "0.07em", textTransform: "uppercase", color: "var(--ink-faint)", fontWeight: 600, marginBottom: 4 }}>
                Insight
              </div>
              <div className={selected.detail.includes("$") ? "money" : undefined} style={{ fontSize: 13.5, color: "var(--ink)", lineHeight: 1.5 }}>
                {selected.detail}
              </div>
            </div>

            <blockquote
              style={{
                margin: 0,
                padding: "12px 14px",
                borderRadius: "var(--radius)",
                background: "var(--elevated)",
                borderLeft: `3px solid var(--accent)`,
                fontSize: 13,
                lineHeight: 1.6,
                color: "var(--ink-mute)",
                fontStyle: "italic",
              }}
            >
              {quoteForStage(selected.stage)}
            </blockquote>

            <button
              type="button"
              onClick={() => {
                sessionStorage.setItem("copilot.prefill", selected.actionPrompt);
                setSelected(null);
                navigate("/copilot");
              }}
              style={{
                width: "100%",
                minHeight: 44,
                borderRadius: "var(--radius-pill)",
                border: "none",
                background: "var(--accent)",
                color: "var(--accent-ink)",
                fontSize: 14,
                fontWeight: 650,
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                gap: 8,
                cursor: "pointer",
              }}
            >
              Get guidance <I.ArrowRight width={13} height={13} />
            </button>

            <p style={{ margin: 0, textAlign: "center", fontSize: 11.5, color: "var(--ink-faint)", lineHeight: 1.4 }}>
              Opens Copilot with a tailored prompt for this milestone.
            </p>
          </div>
        ) : null}
      </BottomSheet>
    </div>
  );
}
