# Plans & Actions

Plans are the Copilot’s proposal before it does anything. Actions are the steps it takes once you approve.

## Plans

A plan (`planner::PlanResult`) contains:

- **Title** — one line, ≤80 chars, what the plan achieves.
- **Steps** — ordered, each with a kind from `ACTION_KINDS` and arguments.
- **Confidence** — clamped 0–1.
- **Provider + model** — for transparency.

Plans are persisted immediately — the UI can render them even if streaming is interrupted.

## Agent memory and clarifications

- **Memory** (`repos/agent_memory`) — small, reviewable items the Copilot stores (“User wants groceries at $600”). Surfaced at **Insights → Memory** and fed into context only when relevant. Delete any you disagree with.
- **Clarifications** — when a question is ambiguous (which account? which month?), the Copilot asks via the `clarifications` state and pauses planning until answered.

## Actions and bundles

Post-planning, approved actions execute via `executor` through `ApiState`. Each step reports running / complete / error. The Copilot then narrates outcomes with figure citations.

**Action bundles** — pending multi-step automations that need your review — appear in the sidebar badge, the Copilot inbox, and **Insights**. They survive restarts and are listed via `actionBundleKeys` queries.

## Guardrails

- Steps are allow-listed; the dispatcher rejects unknown kinds.
- Navigation links the model emits are validated against `APP_ROUTES` in both frontend and Rust (`routes.rs`) — the model cannot link to a screen that does not exist.
- Tool calls are typed and envelope-wrapped (`Result<T, AppError>`); failures surface as error parts, not silent skips.

See also: [Agent Memory](/copilot/memory), [Scenarios](/copilot/scenarios).
