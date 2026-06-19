import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import type { AgentRecipe, AgentRecipeRun } from "../api/client";
import {
  useCreateRecipe,
  useDeleteRecipe,
  usePauseRecipe,
  useRecipeRuns,
  useRecipes,
  useResumeRecipe,
  useTriggerRecipe,
  useUpdateRecipe,
} from "../api/hooks/recipes";
import { CopilotNudge } from "../components/CopilotNudge";
import Drawer from "../components/Drawer";
import * as I from "../components/Icons";
import Button from "../components/Button";
import Card from "../components/Card";
import Input from "../components/Input";
import Select from "../components/Select";
import TextArea from "../components/TextArea";
import Badge from "../components/Badge";
import EmptyState from "../components/EmptyState";

type Cadence = "daily" | "weekly" | "monthly";
type AppLikeError = Error & { code?: string };

interface RecipeTemplate {
  title: string;
  description: string;
  recipeKind: string;
  promptTemplate: string;
  cadence: Cadence;
  dayOfWeek: number | null;
  dayOfMonth: number | null;
}

interface RecipeDraft {
  id: string | null;
  title: string;
  description: string;
  recipeKind: string;
  promptTemplate: string;
  cadence: Cadence;
  dayOfWeek: number | null;
  dayOfMonth: number | null;
}

const WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

const TEMPLATES: RecipeTemplate[] = [
  {
    title: "Monthly Budget Draft",
    description: "Build a new budget draft on the 1st using last month’s spending patterns.",
    recipeKind: "monthly_budget_draft",
    promptTemplate:
      "Review my spending from last month, draft a budget for next month that increases savings by at least 5%, and flag any categories that are consistently over budget.",
    cadence: "monthly",
    dayOfWeek: null,
    dayOfMonth: 1,
  },
  {
    title: "Weekly Cleanup",
    description: "Sweep uncategorized and flagged transactions every Monday.",
    recipeKind: "weekly_cleanup",
    promptTemplate:
      "Find all uncategorized or flagged transactions from the past 7 days, suggest appropriate categories based on the merchant names, and propose rules for recurring merchants.",
    cadence: "weekly",
    dayOfWeek: 0,
    dayOfMonth: null,
  },
  {
    title: "Goal Progress Check",
    description: "Review goal pacing mid-month and adjust contributions if anything is slipping.",
    recipeKind: "goal_contribution_check",
    promptTemplate:
      "Review all my savings goals, assess if I'm on track for each one, and suggest adjustments to monthly contributions if any goal is behind schedule.",
    cadence: "monthly",
    dayOfWeek: null,
    dayOfMonth: 15,
  },
  {
    title: "Subscription Review",
    description: "Audit recurring bills and subscriptions at the start of every month.",
    recipeKind: "subscription_review",
    promptTemplate:
      "Identify all recurring subscriptions and bills from my transactions, flag any that have increased in price or haven't been used recently, and suggest which ones to review or cancel.",
    cadence: "monthly",
    dayOfWeek: null,
    dayOfMonth: 1,
  },
  {
    title: "Savings Rate Check",
    description: "Measure the last 3 months of savings drag and propose concrete fixes.",
    recipeKind: "savings_rate_check",
    promptTemplate:
      "Analyze my savings rate over the past 3 months, identify the top spending categories causing the most drag on my savings rate, and suggest 3 specific changes to improve it.",
    cadence: "monthly",
    dayOfWeek: null,
    dayOfMonth: 28,
  },
];

const EMPTY_DRAFT: RecipeDraft = {
  id: null,
  title: "",
  description: "",
  recipeKind: "custom",
  promptTemplate: "",
  cadence: "monthly",
  dayOfWeek: 0,
  dayOfMonth: 1,
};

function fromTemplate(template: RecipeTemplate): RecipeDraft {
  return { id: null, ...template };
}

function fromRecipe(recipe: AgentRecipe): RecipeDraft {
  return {
    id: recipe.id,
    title: recipe.title,
    description: recipe.description,
    recipeKind: recipe.recipeKind,
    promptTemplate: recipe.promptTemplate,
    cadence: recipe.cadence as Cadence,
    dayOfWeek: recipe.dayOfWeek,
    dayOfMonth: recipe.dayOfMonth,
  };
}

function fmtDateTime(value: string | null) {
  if (!value) return "Not scheduled";
  return new Date(value).toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function cadenceLabel(cadence: string, dayOfWeek: number | null, dayOfMonth: number | null) {
  if (cadence === "daily") return "Daily · 09:00 UTC";
  if (cadence === "weekly") {
    const day = WEEKDAYS[Math.max(0, Math.min(6, dayOfWeek ?? 0))] ?? WEEKDAYS[0];
    return `Weekly · ${day}`;
  }
  if (cadence === "monthly") return `Monthly · day ${Math.max(1, Math.min(28, dayOfMonth ?? 1))}`;
  return cadence;
}

function runStatus(run: AgentRecipeRun | undefined, recipe: AgentRecipe) {
  if (!run && !recipe.lastRunAt) return "Never run";
  if (run?.status === "failed") return "Last run failed";
  if (run?.status === "running") return "Run in progress";
  return recipe.lastRunAt ? `Last run ${fmtDateTime(recipe.lastRunAt)}` : "Never run";
}

function statusBadge(status: string) {
  if (status === "paused") return <Badge>Paused</Badge>;
  if (status === "failed") return <Badge tone="negative">Failed</Badge>;
  if (status === "running") return <Badge>Running</Badge>;
  if (status === "completed") return <Badge tone="positive">Completed</Badge>;
  return <Badge tone="positive">Active</Badge>;
}

function TemplateGrid({
  compact = false,
  onUse,
}: {
  compact?: boolean;
  onUse: (template: RecipeTemplate) => void;
}) {
  return (
    <div
      className="responsive-grid"
      style={{
        gridTemplateColumns: compact ? "repeat(auto-fit, minmax(220px, 1fr))" : "repeat(auto-fit, minmax(240px, 1fr))",
      }}
    >
      {TEMPLATES.map((template) => (
        <Card
          key={template.recipeKind}
          tone="muted"
          className="stack stack-md"
          style={{ padding: compact ? 14 : 18 }}
        >
          <div className="row-sm" style={{ alignItems: "center" }}>
            <I.Recipe style={{ color: "var(--accent)" }} />
            <div style={{ fontWeight: 600 }}>{template.title}</div>
          </div>
          <p className="muted" style={{ fontSize: 13, lineHeight: 1.6, margin: 0 }}>
            {template.description}
          </p>
          <div className="row-sm wrap">
            <Badge>{template.recipeKind}</Badge>
            <Badge>{cadenceLabel(template.cadence, template.dayOfWeek, template.dayOfMonth)}</Badge>
          </div>
          <Button variant="default" size="sm" onClick={() => onUse(template)}>
            <I.Plus /> Use template
          </Button>
        </Card>
      ))}
    </div>
  );
}

function RecipeCard({
  recipe,
  onEdit,
  onTrigger,
  onPause,
  onResume,
  onDelete,
  busy,
}: {
  recipe: AgentRecipe;
  onEdit: (recipe: AgentRecipe) => void;
  onTrigger: (recipe: AgentRecipe) => void;
  onPause: (recipe: AgentRecipe) => void;
  onResume: (recipe: AgentRecipe) => void;
  onDelete: (recipe: AgentRecipe) => void;
  busy: boolean;
}) {
  const [showHistory, setShowHistory] = useState(false);
  const { data: runs = [], isLoading: loadingRuns } = useRecipeRuns(recipe.id);
  const latestRun = runs[0];

  return (
    <Card className="stack stack-md" style={{ padding: 18 }}>
      <div className="row-md" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
        <div className="grow stack stack-xs">
          <div className="row-sm wrap" style={{ alignItems: "center" }}>
            <div style={{ fontSize: 18, fontWeight: 600 }}>{recipe.title}</div>
            <Badge>{recipe.recipeKind}</Badge>
            <Badge>{cadenceLabel(recipe.cadence, recipe.dayOfWeek, recipe.dayOfMonth)}</Badge>
            {statusBadge(recipe.status)}
          </div>
          <p className="muted" style={{ fontSize: 14, lineHeight: 1.6, maxWidth: 720, margin: 0 }}>
            {recipe.description}
          </p>
        </div>
        <div className="row-sm wrap" style={{ justifyContent: "flex-end" }}>
          <Button variant="default" size="sm" disabled={busy} onClick={() => onTrigger(recipe)}>
            <I.Repeat /> Run now
          </Button>
          <Button variant="default" size="sm" disabled={busy} onClick={() => onEdit(recipe)}>
            <I.Pencil /> Edit
          </Button>
          {recipe.status === "paused" ? (
            <Button variant="default" size="sm" disabled={busy} onClick={() => onResume(recipe)}>
              <I.Check /> Resume
            </Button>
          ) : (
            <Button variant="default" size="sm" disabled={busy} onClick={() => onPause(recipe)}>
              <I.Down /> Pause
            </Button>
          )}
          <Button variant="ghost" size="sm" disabled={busy} onClick={() => onDelete(recipe)}>
            <I.Trash /> Delete
          </Button>
        </div>
      </div>

      <div
        className="responsive-grid"
        style={{ gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))" }}
      >
        <Card tone="muted" tight className="stat stack stack-xs" style={{ gap: 8 }}>
          <div className="eyebrow">Last run</div>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{runStatus(latestRun, recipe)}</div>
          {latestRun?.error && (
            <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
              {latestRun.error}
            </div>
          )}
        </Card>
        <Card tone="muted" tight className="stat stack stack-xs" style={{ gap: 8 }}>
          <div className="eyebrow">Next scheduled run</div>
          <div style={{ fontSize: 14, fontWeight: 600 }}>{fmtDateTime(recipe.nextRunAt)}</div>
        </Card>
        <Card tone="muted" tight className="stat stack stack-xs" style={{ gap: 8 }}>
          <div className="eyebrow">Run count</div>
          <div className="figure" style={{ fontSize: 22, fontWeight: 700 }}>{recipe.runCount}</div>
        </Card>
      </div>

      <div>
        <Button variant="ghost" size="sm" onClick={() => setShowHistory((open) => !open)}>
          {showHistory ? <I.Up /> : <I.Down />}
          {showHistory ? "Hide run history" : "Show run history"}
        </Button>
      </div>

      {showHistory && (
        <div className="stack stack-md">
          {loadingRuns ? (
            <div className="stub" style={{ height: "auto", minHeight: 80 }}>Loading run history…</div>
          ) : runs.length === 0 ? (
            <EmptyState compact title="No runs yet" />
          ) : (
            runs.map((run) => (
              <Card key={run.id} tone="muted" tight className="stack stack-sm">
                <div className="row-md" style={{ justifyContent: "space-between", alignItems: "center" }}>
                  <div style={{ fontWeight: 600 }}>{fmtDateTime(run.triggeredAt)}</div>
                  {statusBadge(run.status)}
                </div>
                {run.bundleId && (
                  <div className="muted" style={{ fontSize: 12 }}>
                    Draft bundle: {run.bundleId}
                  </div>
                )}
                {run.error && (
                  <div style={{ color: "var(--negative)", fontSize: 12.5 }}>
                    {run.error}
                  </div>
                )}
              </Card>
            ))
          )}
        </div>
      )}
    </Card>
  );
}

function RecipeDrawer({
  open,
  draft,
  saving,
  onClose,
  onChange,
  onSave,
  onUseTemplate,
}: {
  open: boolean;
  draft: RecipeDraft;
  saving: boolean;
  onClose: () => void;
  onChange: (next: RecipeDraft) => void;
  onSave: () => void;
  onUseTemplate: (template: RecipeTemplate) => void;
}) {
  const isEditing = draft.id !== null;

  return (
    <Drawer open={open} onClose={onClose} title={isEditing ? "Edit recipe" : "Create trusted recipe"} width={560}>
      <div className="drawer-form">
        <div>
          <div className="eyebrow" style={{ marginBottom: 8 }}>
            Recipe kind
          </div>
          <Badge>{draft.recipeKind}</Badge>
        </div>

        <Input
          label="Title"
          value={draft.title}
          onChange={(e) => onChange({ ...draft, title: e.target.value })}
          placeholder="Monthly budget draft"
        />

        <TextArea
          label="Description"
          rows={3}
          value={draft.description}
          onChange={(e) => onChange({ ...draft, description: e.target.value })}
          placeholder="What this recipe automates and why it matters"
        />

        <TextArea
          label="Planner prompt"
          rows={7}
          value={draft.promptTemplate}
          onChange={(e) => onChange({ ...draft, promptTemplate: e.target.value })}
          placeholder="Tell Copilot what draft-only work to prepare."
        />

        <div className="form-grid">
          <Select
            label="Cadence"
            value={draft.cadence}
            onChange={(e) =>
              onChange({
                ...draft,
                cadence: e.target.value as Cadence,
                dayOfWeek: e.target.value === "weekly" ? draft.dayOfWeek ?? 0 : null,
                dayOfMonth: e.target.value === "monthly" ? draft.dayOfMonth ?? 1 : null,
              })
            }
          >
            <option value="daily">Daily</option>
            <option value="weekly">Weekly</option>
            <option value="monthly">Monthly</option>
          </Select>

          {draft.cadence === "weekly" ? (
            <Select
              label="Day of week"
              value={draft.dayOfWeek ?? 0}
              onChange={(e) => onChange({ ...draft, dayOfWeek: Number(e.target.value), dayOfMonth: null })}
            >
              {WEEKDAYS.map((day, index) => (
                <option key={day} value={index}>
                  {day}
                </option>
              ))}
            </Select>
          ) : draft.cadence === "monthly" ? (
            <Input
              type="number"
              label="Day of month"
              min={1}
              max={28}
              value={draft.dayOfMonth ?? 1}
              onChange={(e) =>
                onChange({
                  ...draft,
                  dayOfMonth: Math.max(1, Math.min(28, Number(e.target.value || 1))),
                  dayOfWeek: null,
                })
              }
            />
          ) : (
            <Card tone="muted" tight>
              <div className="eyebrow" style={{ marginBottom: 8 }}>
                Schedule
              </div>
              <p className="muted" style={{ fontSize: 13, margin: 0 }}>
                Daily recipes always queue their draft for 09:00 UTC tomorrow.
              </p>
            </Card>
          )}
        </div>

        {!isEditing && (
          <div className="stack stack-md">
            <div className="eyebrow">Quick templates</div>
            <TemplateGrid compact onUse={onUseTemplate} />
          </div>
        )}

        <div className="form-actions">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" loading={saving} disabled={saving} onClick={() => void onSave()}>
            <I.Check /> {saving ? "Saving…" : isEditing ? "Save changes" : "Create recipe"}
          </Button>
        </div>
      </div>
    </Drawer>
  );
}

export default function Recipes() {
  const navigate = useNavigate();
  const { data: recipes = [], isLoading, error } = useRecipes(true);
  const createRecipe = useCreateRecipe();
  const updateRecipe = useUpdateRecipe();
  const pauseRecipe = usePauseRecipe();
  const resumeRecipe = useResumeRecipe();
  const deleteRecipe = useDeleteRecipe();
  const triggerRecipe = useTriggerRecipe();

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [draft, setDraft] = useState<RecipeDraft>(EMPTY_DRAFT);

  const sortedRecipes = useMemo(
    () => [...recipes].sort((a, b) => b.createdAt.localeCompare(a.createdAt)),
    [recipes],
  );

  const saving = createRecipe.isPending || updateRecipe.isPending;
  const mutating =
    saving ||
    pauseRecipe.isPending ||
    resumeRecipe.isPending ||
    deleteRecipe.isPending ||
    triggerRecipe.isPending;

  const openCustom = () => {
    setDraft(EMPTY_DRAFT);
    setDrawerOpen(true);
  };

  const openFromTemplate = (template: RecipeTemplate) => {
    setDraft(fromTemplate(template));
    setDrawerOpen(true);
  };

  const openEdit = (recipe: AgentRecipe) => {
    setDraft(fromRecipe(recipe));
    setDrawerOpen(true);
  };

  const handleSave = async () => {
    if (!draft.title.trim() || !draft.description.trim() || !draft.promptTemplate.trim()) {
      toast.error("Fill in the title, description, and prompt first");
      return;
    }

    try {
      if (draft.id) {
        await updateRecipe.mutateAsync({
          id: draft.id,
          title: draft.title.trim(),
          description: draft.description.trim(),
          promptTemplate: draft.promptTemplate.trim(),
          cadence: draft.cadence,
          dayOfWeek: draft.cadence === "weekly" ? draft.dayOfWeek ?? 0 : null,
          dayOfMonth: draft.cadence === "monthly" ? draft.dayOfMonth ?? 1 : null,
        });
        toast.success("Recipe updated");
      } else {
        await createRecipe.mutateAsync({
          title: draft.title.trim(),
          description: draft.description.trim(),
          recipeKind: draft.recipeKind,
          promptTemplate: draft.promptTemplate.trim(),
          cadence: draft.cadence,
          dayOfWeek: draft.cadence === "weekly" ? draft.dayOfWeek ?? 0 : null,
          dayOfMonth: draft.cadence === "monthly" ? draft.dayOfMonth ?? 1 : null,
        });
        toast.success("Recipe created");
      }
      setDrawerOpen(false);
      setDraft(EMPTY_DRAFT);
    } catch (err) {
      toast.error("Failed to save recipe", {
        description: err instanceof Error ? err.message : "Unknown error",
      });
    }
  };

  const handleRunNow = async (recipe: AgentRecipe) => {
    try {
      await triggerRecipe.mutateAsync(recipe.id);
      toast.success("Draft bundle sent to Copilot", {
        description: "Review the generated bundle before applying anything.",
      });
      navigate("/copilot");
    } catch (err) {
      const appErr = err as AppLikeError;
      if (appErr.code === "no_provider") {
        toast.error("Set up an AI provider first", {
          description: "Open Settings → Agent and configure a provider before running recipes.",
        });
        return;
      }
      toast.error(`Failed to run ${recipe.title}`, {
        description: appErr.message,
      });
    }
  };

  const handlePause = async (recipe: AgentRecipe) => {
    try {
      await pauseRecipe.mutateAsync(recipe.id);
      toast.success("Recipe paused");
    } catch (err) {
      toast.error("Failed to pause recipe", {
        description: err instanceof Error ? err.message : "Unknown error",
      });
    }
  };

  const handleResume = async (recipe: AgentRecipe) => {
    try {
      await resumeRecipe.mutateAsync(recipe.id);
      toast.success("Recipe resumed");
    } catch (err) {
      toast.error("Failed to resume recipe", {
        description: err instanceof Error ? err.message : "Unknown error",
      });
    }
  };

  const handleDelete = async (recipe: AgentRecipe) => {
    if (!confirm(`Delete "${recipe.title}"? This keeps run history but removes it from the active list.`)) {
      return;
    }
    try {
      await deleteRecipe.mutateAsync(recipe.id);
      toast.success("Recipe deleted");
    } catch (err) {
      toast.error("Failed to delete recipe", {
        description: err instanceof Error ? err.message : "Unknown error",
      });
    }
  };

  if (isLoading) return <div className="stub">Loading recipes…</div>;
  if (error) return <div className="stub">Error loading recipes.</div>;

  return (
    <div className="screen">
      <header className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Trusted recipes · draft-only automation
          </div>
          <h1>Recipes</h1>
          <p className="muted" style={{ marginTop: 8, maxWidth: 720, lineHeight: 1.6 }}>
            Automate recurring financial reviews. Every recipe builds fresh context, asks the planner for draft actions, and sends the bundle to Copilot for approval before anything changes.
          </p>
        </div>
        <div className="row-md wrap" style={{ justifyContent: "flex-end" }}>
          <CopilotNudge
            prompt="Look at my current spending patterns and suggest one trusted recipe I should automate next. Keep it draft-only."
            label="Ask Copilot for an idea"
            description="Generate a recipe concept from your current finances"
            variant="accent"
          />
          <Button variant="primary" onClick={openCustom}>
            <I.Plus /> New recipe
          </Button>
        </div>
      </header>

      {sortedRecipes.length === 0 ? (
        <Card className="stack stack-md">
          <div className="row-sm" style={{ alignItems: "center" }}>
            <I.Recipe style={{ color: "var(--accent)", width: 20, height: 20 }} />
            <div style={{ fontSize: 18, fontWeight: 600 }}>Automate recurring financial reviews</div>
          </div>
          <p className="muted" style={{ margin: 0, lineHeight: 1.7 }}>
            Start from a trusted template, then review each generated bundle in Copilot before approving any actions.
          </p>
          <TemplateGrid onUse={openFromTemplate} />
          <div>
            <Button variant="default" onClick={openCustom}>
              <I.Plus /> Create a custom recipe
            </Button>
          </div>
        </Card>
      ) : (
        <>
          <Card className="stack stack-md" style={{ marginBottom: 18 }}>
            <div className="row-md" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
              <div className="stack stack-xs">
                <div className="eyebrow">Start from a template</div>
                <p className="muted" style={{ fontSize: 13, margin: 0 }}>
                  Add another trusted workflow without writing the prompt from scratch.
                </p>
              </div>
            </div>
            <TemplateGrid compact onUse={openFromTemplate} />
          </Card>

          <div className="stack stack-lg">
            {sortedRecipes.map((recipe) => (
              <RecipeCard
                key={recipe.id}
                recipe={recipe}
                onEdit={openEdit}
                onTrigger={handleRunNow}
                onPause={handlePause}
                onResume={handleResume}
                onDelete={handleDelete}
                busy={mutating}
              />
            ))}
          </div>
        </>
      )}

      <RecipeDrawer
        open={drawerOpen}
        draft={draft}
        saving={saving}
        onClose={() => setDrawerOpen(false)}
        onChange={setDraft}
        onSave={() => void handleSave()}
        onUseTemplate={openFromTemplate}
      />
    </div>
  );
}
