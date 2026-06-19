import { useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useJourneyStatus } from "../api/hooks/journey";
import Button from "../components/Button";
import Card from "../components/Card";
import Badge from "../components/Badge";
import ProgressBar from "../components/ProgressBar";
import * as I from "../components/Icons";

const QUOTES = {
  early: "“A journey of a thousand miles begins with a single step.” — Lao Tzu",
  middle: "“Do not save what is left after spending, but spend what is left after saving.” — Warren Buffett",
  growth: "“Compound interest is the eighth wonder of the world.” — attributed to Einstein",
  freedom: "“Financial freedom is available to those who learn about it and work for it.” — Robert Kiyosaki",
} as const;

function quoteForStage(stage: number) {
  if (stage <= 2) return QUOTES.early;
  if (stage <= 4) return QUOTES.middle;
  if (stage <= 6) return QUOTES.growth;
  return QUOTES.freedom;
}

export default function Journey() {
  const navigate = useNavigate();
  const { data, isLoading, error } = useJourneyStatus();
  const prevCompletedCount = useRef<number | null>(null);

  useEffect(() => {
    if (!data) return;
    const prev = prevCompletedCount.current;
    if (prev !== null && data.completedCount > prev) {
      const justCompleted = data.milestones.filter((m) => m.status === "completed");
      const newest = justCompleted[justCompleted.length - 1];
      if (newest) {
        toast.success(`Stage ${newest.stage} complete — ${newest.name}!`, {
          description: "Keep going — each milestone builds on the last.",
        });
      }
    }
    prevCompletedCount.current = data.completedCount;
  }, [data]);

  if (isLoading) return <div className="stub">Loading journey…</div>;
  if (error || !data) return <div className="stub">Error loading journey.</div>;

  const stageOne = data.milestones[0];

  return (
    <div className="screen">
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Journey · {data.completedCount} of 7 milestones completed
          </div>
          <h1>Your Financial Journey</h1>
          <p className="muted" style={{ margin: "10px 0 0", fontSize: 14 }}>
            Track your progress from financial stability to freedom.
          </p>
        </div>
      </div>

      {stageOne?.status === "current" && stageOne.progressPct < 50 && (
        <Card tone="accent" className="stack stack-md" style={{ marginBottom: 20 }}>
          <div className="h3">Start by linking your first account</div>
          <p className="muted" style={{ fontSize: 13.5 }}>
            Stage 1 begins once you connect at least one account and import enough transactions to see your real cash flow.
          </p>
          <Button variant="primary" onClick={() => navigate("/accounts")}>
            Add your first account →
          </Button>
        </Card>
      )}

      <div className="stack stack-lg">
        {data.milestones.map((milestone, index) => {
          const completed = milestone.status === "completed";
          const current = milestone.status === "current";

          return (
            <article
              key={milestone.stage}
              className="row-md"
              style={{ alignItems: "stretch" }}
              aria-label={`Stage ${milestone.stage} ${milestone.name}`}
            >
              <div className="stack stack-xs" style={{ alignItems: "center", width: 40, flexShrink: 0 }}>
                <div
                  className="row"
                  style={{
                    width: 32,
                    height: 32,
                    borderRadius: "var(--radius-pill)",
                    border: `1px solid ${completed ? "var(--positive)" : current ? "var(--accent)" : "var(--line)"}`,
                    background: completed ? "var(--positive)" : current ? "var(--accent)" : "transparent",
                    color: completed || current ? "var(--bg)" : "var(--ink-mute)",
                    justifyContent: "center",
                    fontSize: 13,
                    fontWeight: 700,
                  }}
                  aria-hidden="true"
                >
                  {completed ? <I.Check width={14} height={14} /> : milestone.stage}
                </div>
                {index < data.milestones.length - 1 && (
                  <div
                    style={{
                      width: 2,
                      flex: 1,
                      marginTop: 6,
                      background: completed ? "var(--positive-2)" : "var(--line)",
                    }}
                    aria-hidden="true"
                  />
                )}
              </div>

              <Card
                tone={current ? "accent" : completed ? "muted" : "default"}
                className="grow stack stack-md"
              >
                <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", gap: 12 }}>
                  <div className="stack stack-xs">
                    <div className="eyebrow">Stage {milestone.stage}</div>
                    <div className="h3" style={{ marginTop: 6, fontSize: 16 }}>{milestone.name}</div>
                    <p className="muted" style={{ fontSize: 13.5, marginTop: 8, lineHeight: 1.55 }}>
                      {milestone.description}
                    </p>
                  </div>
                  {completed && <Badge tone="positive">Completed</Badge>}
                  {current && <Badge tone="accent">Current focus</Badge>}
                </div>

                <div className="stack stack-sm">
                  <ProgressBar
                    value={milestone.progressPct}
                    max={100}
                    tone={completed ? "default" : current ? "default" : "default"}
                    aria-label={`Stage ${milestone.stage} progress`}
                  />
                  <div className="row" style={{ justifyContent: "space-between", gap: 12, fontSize: 12.5 }}>
                    <div className={milestone.detail.includes("$") ? "muted money" : "muted"}>{milestone.detail}</div>
                    <div className="num" style={{ color: current ? "var(--accent)" : "var(--ink-faint)" }}>
                      {milestone.progressPct}%
                    </div>
                  </div>
                </div>

                <div>
                  <Button
                    variant="text"
                    size="sm"
                    onClick={() => {
                      sessionStorage.setItem("copilot.prefill", milestone.actionPrompt);
                      navigate("/copilot");
                    }}
                    style={{ paddingLeft: 0, color: current ? "var(--accent)" : "var(--ink)" }}
                  >
                    Get guidance <I.ArrowRight width={12} height={12} />
                  </Button>
                </div>
              </Card>
            </article>
          );
        })}
      </div>

      <Card tone="muted" className="stack stack-sm" style={{ marginTop: 24, textAlign: "center" }}>
        <div className="eyebrow" style={{ justifyContent: "center" }}>Keep going</div>
        <p style={{ fontSize: 16, lineHeight: 1.6, margin: 0 }}>{quoteForStage(data.currentStage)}</p>
      </Card>
    </div>
  );
}
