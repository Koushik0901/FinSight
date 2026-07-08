# FinSight Copilot evaluation

A reproducible benchmark + LLM-as-a-judge pipeline that measures how well the
FinSight Copilot answers realistic personal-finance questions, tracks every run
in MLflow, and surfaces failure patterns so quality regressions are caught as
FinSight evolves.

```
┌─────────────────┐   real ReasoningEngine    ┌──────────────────────┐
│ benchmark.jsonl │──────────────────────────▶│ finsight-eval (Rust) │
│ 42 questions +  │   + tool loop over a       │  seeds a fixed        │
│ reference facts │   deterministic household  │  household, runs each  │
└─────────────────┘                            │  Q → copilot_outputs   │
                                               └──────────┬───────────┘
                                                          │ JSONL (answers,
                                                          │  tools, blocks)
                                                          ▼
┌──────────────────────────┐   gemini judge    ┌──────────────────────┐
│ MLflow (sqlite) + reports │◀──────────────────│ run_eval.py (judge)  │
│ metrics · scores · fails  │  rubric + facts   │  scores 6 criteria   │
└──────────────────────────┘                   └──────────────────────┘
```

## What it measures

Each answer is scored 1–5 by an LLM judge on six criteria — **correctness,
reasoning, tool_use, safety, relevance, usefulness** — strictly against the
per-question `reference_facts` and `grading_notes` (not the judge's own finance
opinions). Two hard flags are tracked separately:

- **fabrication** — the answer stated a number/claim unsupported by the facts.
- **critical_failure** — a serious safety/honesty failure (named specific
  securities, invented data, claimed an unapproved action was applied, etc.).

Plus operational signals from the harness: **is_usable** (did it call a tool and
produce prose — a `false` would have hit the canned planner fallback in
production), latency, and tool calls.

## One-time setup

```bash
cd eval
uv venv .venv && uv pip install --python .venv/Scripts/python.exe \
    mlflow openai tenacity pandas python-dotenv keyring
```

**API key.** The pipeline talks to OpenRouter for both the Copilot-under-test and
the judge. Provide a working key via env or a git-ignored `eval/.env`:

```
# eval/.env
OPENROUTER_API_KEY=sk-or-...
```

(The Rust harness will also fall back to the app's OS keychain slot
`com.finsight.llm/openrouter` when run standalone, but the Python judge needs the
env/`.env` key. Using an env var is the portable/CI-friendly path and keeps the
eval independent of the desktop app's keychain.)

## Run it

```bash
cd eval
.venv/Scripts/python.exe run_eval.py                 # full benchmark (harness + judge)
.venv/Scripts/python.exe run_eval.py --limit 3       # smoke: first 3 questions
.venv/Scripts/python.exe run_eval.py \               # different Copilot model
    --copilot-model z-ai/glm-5.2:exacto --judge-model google/gemini-3.1-pro-preview
```

Each run writes to `eval/runs/<timestamp>/`:

| file | contents |
|------|----------|
| `copilot_outputs.jsonl` | raw Copilot answer + tools + cards per question |
| `judged.jsonl` | per-question scores, flags, judge justifications |
| `scores.csv` | flat score table for spreadsheets |
| `summary.md` | headline metrics + per-criterion / per-category / per-difficulty means |
| `failures.md` | every sub-pass / flagged answer, worst first, with the judge's reason |

…and logs params + metrics + those artifacts to MLflow.

```bash
python -m mlflow ui --backend-store-uri sqlite:///eval/mlflow.db   # browse runs, compare regressions
```

## The benchmark

`benchmark.jsonl` — 42 questions (10 easy / 18 medium / 14 hard) across 18
categories: net worth, balances, affordability, emergency fund, spending &
category analysis, transaction search, anomalies, recurring, cashflow, debt
ranking & payoff, goals, recategorization, open-ended planning, savings-vs-debt
tradeoffs, **ambiguous** (must clarify), **unsupported** (must decline),
**safety** (principles-only investing / no fabrication), and **grounding**
(unknown balance handling).

Each row carries `reference_facts` (ground truth) and `grading_notes` (what to
reward/penalize) so scoring is objective and low-variance.

### Ground truth is fixed by the seed

The Copilot is evaluated against a deterministic synthetic household defined in
`crates/finsight-eval/src/seed.rs` (fixed round amounts; the six most-recent
complete months relative to the clock):

- Checking **$2,000**, Emergency Fund savings **$5,000** (4.0% APY),
  Visa **−$1,200** (19.9% APR, $30 min), Auto Loan **−$8,000** (6.5% APR),
  Brokerage = **UNKNOWN** balance.
- Known net worth = **−$2,200**. Income **$4,000/mo**; expenses **~$1,837/mo**;
  surplus **~$2,163/mo**. Biggest category **Housing $1,200/mo**.
- Emergency fund ≈ **2.7 months** (below the 3–6 target). Debts: **Visa first**
  under both avalanche and snowball. One flagged **$2,500 Apple Store** anomaly;
  **4 uncategorized** transactions.

A unit test (`cargo test -p finsight-eval`) asserts these numbers so the
benchmark's reference facts can never silently drift from the seed.

## Fidelity

- The harness runs the **real** `ReasoningEngine` + the **same** tool set the app
  ships (`finsight_agent::reasoning::tools::standard_toolset`, shared with
  `finsight-app`), so the benchmark grades the agent users actually get.
- Each question runs against its own freshly-seeded DB (no cross-question state).
- `is_usable` mirrors the production `is_usable_tool_answer` gate.

## Adding / changing benchmark cases

Append a line to `benchmark.jsonl`:

```json
{"id":"unique-id","category":"...","difficulty":"easy|medium|hard","question":"...","expected_tools":["..."],"reference_facts":"ground truth the judge grades against","grading_notes":"what to reward/penalize"}
```

If a new question relies on data not in the household, extend `seed.rs` **and**
its assertion test so the reference facts stay verifiable.

## Files

| path | role |
|------|------|
| `crates/finsight-eval/` | Rust harness (seed + benchmark runner) |
| `eval/benchmark.jsonl` | the questions + ground truth |
| `eval/judge.py` | rubric + judge call + parsing (bias-mitigated) |
| `eval/run_eval.py` | orchestrator: harness → judge → MLflow → reports |
| `eval/selftest.py` | offline pipeline test (no API/key) |
| `eval/runs/`, `eval/mlflow.db` | git-ignored run outputs + MLflow store |
