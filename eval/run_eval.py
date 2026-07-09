#!/usr/bin/env python
"""FinSight Copilot evaluation pipeline.

Runs the benchmark end to end:
  1. (optional) invoke the Rust harness to generate Copilot answers for every
     benchmark question against the deterministic seeded household;
  2. score each answer with an LLM judge (gemini) against the benchmark's
     reference facts;
  3. log params / metrics / artifacts to MLflow and write human-readable
     summary + failure reports into the run directory.

Rerun any time to detect Copilot quality regressions:
  python run_eval.py                          # full run (harness + judge)
  python run_eval.py --limit 3                # smoke run, first 3 questions
  python run_eval.py --skip-harness --outputs runs/<ts>/copilot_outputs.jsonl

The OpenRouter key is read from OPENROUTER_API_KEY (env) or eval/.env. This is a
dev/CI tool, separate from the app; the app itself keeps keys in the OS keychain.
"""

from __future__ import annotations

import sys
try:  # Windows consoles default to cp1252 and can't encode the status glyphs.
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
except Exception:
    pass

import argparse
import csv
import datetime as dt
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

import mlflow
from dotenv import load_dotenv
from openai import OpenAI

from judge import CRITERIA, Judge, JudgeResult

EVAL_DIR = Path(__file__).resolve().parent
REPO_ROOT = EVAL_DIR.parent
OPENROUTER_BASE = "https://openrouter.ai/api/v1"
PASS_THRESHOLD = 4  # overall >= 4 counts as a "pass"


def resolve_key() -> str:
    # Look in eval/.env first, then the repo-root .env, then the environment.
    load_dotenv(EVAL_DIR / ".env")
    load_dotenv(REPO_ROOT / ".env")
    key = (os.environ.get("OPENROUTER_API_KEY") or "").strip()
    if not key:
        sys.exit(
            "No OpenRouter key. Set OPENROUTER_API_KEY in your environment, or create "
            f"{EVAL_DIR / '.env'} containing:\n    OPENROUTER_API_KEY=sk-or-...\n"
            "(This is the eval tool, separate from the app's keychain.)"
        )
    return key


def git_sha() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"], cwd=REPO_ROOT, text=True
        ).strip()
    except Exception:
        return "unknown"


def file_hash(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()[:12]


def run_harness(args, out_path: Path, key: str) -> None:
    cmd = [
        "cargo", "run", "-p", "finsight-eval", "--release", "--",
        "--benchmark", str(args.benchmark),
        "--out", str(out_path),
        "--model", args.copilot_model,
        "--timeout-secs", str(args.timeout_secs),
    ]
    if args.limit:
        cmd += ["--limit", str(args.limit)]
    env = {**os.environ, "OPENROUTER_API_KEY": key}
    print(f"▶ harness: {' '.join(cmd)}", flush=True)
    proc = subprocess.run(cmd, cwd=REPO_ROOT, env=env)
    if proc.returncode != 0:
        sys.exit(f"harness failed (exit {proc.returncode})")


def load_jsonl(path: Path) -> list[dict]:
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("//"):
            rows.append(json.loads(line))
    return rows


def mean(xs: list[float]) -> float:
    return sum(xs) / len(xs) if xs else 0.0


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--benchmark", default=str(EVAL_DIR / "benchmark.jsonl"))
    ap.add_argument("--copilot-model", default="z-ai/glm-5.2:exacto")
    ap.add_argument("--judge-model", default="google/gemini-3.1-pro-preview")
    ap.add_argument("--limit", type=int, default=0, help="only the first N questions (0=all)")
    ap.add_argument("--timeout-secs", type=int, default=180)
    ap.add_argument("--judge-samples", type=int, default=1, help="judge calls per answer (averaged)")
    ap.add_argument("--experiment", default="finsight-copilot-eval")
    # MLflow 3 requires a database backend (the file store is deprecated).
    ap.add_argument("--mlflow-uri", default=f"sqlite:///{(EVAL_DIR / 'mlflow.db').as_posix()}")
    ap.add_argument("--skip-harness", action="store_true", help="reuse an existing outputs file")
    ap.add_argument("--outputs", default="", help="existing harness outputs (with --skip-harness)")
    ap.add_argument("--run-note", default="", help="free-text note logged as a param")
    args = ap.parse_args()

    key = resolve_key()
    benchmark = Path(args.benchmark)

    ts = dt.datetime.now().strftime("%Y%m%d-%H%M%S")
    run_dir = EVAL_DIR / "runs" / ts
    run_dir.mkdir(parents=True, exist_ok=True)

    outputs_path = Path(args.outputs) if args.outputs else run_dir / "copilot_outputs.jsonl"
    if not args.skip_harness:
        run_harness(args, outputs_path, key)
    if not outputs_path.exists():
        sys.exit(f"outputs not found: {outputs_path}")

    rows = load_jsonl(outputs_path)
    print(f"▶ judging {len(rows)} answer(s) with {args.judge_model} …", flush=True)

    judge = Judge(
        client=OpenAI(base_url=OPENROUTER_BASE, api_key=key),
        model=args.judge_model,
    )

    judged: list[dict] = []
    for i, row in enumerate(rows, 1):
        try:
            res: JudgeResult = judge.score(row, samples=args.judge_samples)
        except Exception as e:  # a judge failure shouldn't sink the whole run
            print(f"  [{i}/{len(rows)}] {row['id']}: JUDGE ERROR {e}", flush=True)
            res = JudgeResult(
                scores={c: 1 for c in CRITERIA}, overall=1, fabrication=False,
                critical_failure=True, critical_reason=f"judge error: {e}",
                summary="judge failed", raw={"error": str(e)},
            )
        rec = {
            "id": row["id"],
            "category": row.get("category"),
            "difficulty": row.get("difficulty"),
            "question": row["question"],
            "overall": res.overall,
            **{f"score_{c}": res.scores[c] for c in CRITERIA},
            "mean_criteria": round(res.mean_criteria, 3),
            "fabrication": res.fabrication,
            "critical_failure": res.critical_failure,
            "critical_reason": res.critical_reason,
            "is_usable": row.get("is_usable"),
            "tools_called": row.get("tools_called"),
            "latency_ms": row.get("latency_ms"),
            "harness_error": row.get("error"),
            "judge_summary": res.summary,
            "answer": row.get("answer"),
            "judge_raw": res.raw,
        }
        judged.append(rec)
        flag = "  ⚑" if (res.critical_failure or res.fabrication) else ""
        print(
            f"  [{i}/{len(rows)}] {row['id']:10s} overall={res.overall} "
            f"corr={res.scores['correctness']} safe={res.scores['safety']} "
            f"tool={res.scores['tool_use']}{flag}",
            flush=True,
        )

    # ── Aggregate ─────────────────────────────────────────────────────────────
    overalls = [r["overall"] for r in judged]
    metrics = {
        "overall_mean": round(mean(overalls), 3),
        "pass_rate": round(mean([1.0 if r["overall"] >= PASS_THRESHOLD else 0.0 for r in judged]), 3),
        "usable_rate": round(mean([1.0 if r["is_usable"] else 0.0 for r in judged]), 3),
        "fabrication_rate": round(mean([1.0 if r["fabrication"] else 0.0 for r in judged]), 3),
        "critical_failure_rate": round(mean([1.0 if r["critical_failure"] else 0.0 for r in judged]), 3),
        "harness_error_rate": round(mean([1.0 if r["harness_error"] else 0.0 for r in judged]), 3),
        "latency_ms_mean": round(mean([float(r["latency_ms"] or 0) for r in judged]), 1),
    }
    for c in CRITERIA:
        metrics[f"{c}_mean"] = round(mean([r[f"score_{c}"] for r in judged]), 3)
    # Per-difficulty and per-category overall means.
    for key_field, prefix in (("difficulty", "diff"), ("category", "cat")):
        groups: dict[str, list[int]] = {}
        for r in judged:
            groups.setdefault(r[key_field] or "unknown", []).append(r["overall"])
        for name, vals in groups.items():
            metrics[f"{prefix}_{name}_overall_mean"] = round(mean(vals), 3)

    # ── Artifacts ─────────────────────────────────────────────────────────────
    (run_dir / "judged.jsonl").write_text(
        "\n".join(json.dumps(r, ensure_ascii=False) for r in judged), encoding="utf-8"
    )
    with (run_dir / "scores.csv").open("w", newline="", encoding="utf-8") as f:
        cols = ["id", "category", "difficulty", "overall", *[f"score_{c}" for c in CRITERIA],
                "fabrication", "critical_failure", "is_usable", "latency_ms"]
        w = csv.DictWriter(f, fieldnames=cols, extrasaction="ignore")
        w.writeheader()
        for r in judged:
            w.writerow(r)
    write_reports(run_dir, args, metrics, judged)

    # ── MLflow ────────────────────────────────────────────────────────────────
    mlflow.set_tracking_uri(args.mlflow_uri)
    mlflow.set_experiment(args.experiment)
    with mlflow.start_run(run_name=f"{args.copilot_model.split('/')[-1]}-{ts}"):
        mlflow.log_params({
            "copilot_model": args.copilot_model,
            "judge_model": args.judge_model,
            "benchmark_file": benchmark.name,
            "benchmark_sha": file_hash(benchmark),
            "n_questions": len(judged),
            "judge_samples": args.judge_samples,
            "timeout_secs": args.timeout_secs,
            "repo_git_sha": git_sha(),
            "run_note": args.run_note,
        })
        mlflow.log_metrics(metrics)
        for name in ("copilot_outputs.jsonl", "judged.jsonl", "scores.csv",
                     "summary.md", "failures.md"):
            p = run_dir / name
            if p.exists():
                mlflow.log_artifact(str(p))

    print(f"\n✔ {run_dir}")
    print(f"  overall_mean={metrics['overall_mean']}  pass_rate={metrics['pass_rate']}  "
          f"usable={metrics['usable_rate']}  fabrication={metrics['fabrication_rate']}  "
          f"critical={metrics['critical_failure_rate']}")
    print(f"  MLflow: python -m mlflow ui --backend-store-uri {args.mlflow_uri}")


def write_reports(run_dir: Path, args, metrics: dict, judged: list[dict]) -> None:
    lines = [
        f"# FinSight Copilot eval — {run_dir.name}",
        "",
        f"- Copilot model: `{args.copilot_model}`",
        f"- Judge model: `{args.judge_model}`",
        f"- Questions: {len(judged)}",
        "",
        "## Headline metrics",
        f"- **Overall mean**: {metrics['overall_mean']} / 5",
        f"- **Pass rate** (overall ≥ {PASS_THRESHOLD}): {metrics['pass_rate']:.0%}",
        f"- **Usable rate** (would not hit the canned fallback): {metrics['usable_rate']:.0%}",
        f"- **Fabrication rate**: {metrics['fabrication_rate']:.0%}",
        f"- **Critical-failure rate**: {metrics['critical_failure_rate']:.0%}",
        f"- **Harness-error rate**: {metrics['harness_error_rate']:.0%}",
        f"- Mean latency: {metrics['latency_ms_mean']:.0f} ms",
        "",
        "## Per-criterion mean",
    ]
    for c in CRITERIA:
        lines.append(f"- {c}: {metrics[f'{c}_mean']}")
    lines += ["", "## Per-difficulty (overall mean)"]
    for d in ("easy", "medium", "hard"):
        k = f"diff_{d}_overall_mean"
        if k in metrics:
            lines.append(f"- {d}: {metrics[k]}")
    lines += ["", "## Per-category (overall mean)"]
    for k in sorted(k for k in metrics if k.startswith("cat_")):
        lines.append(f"- {k[4:-13]}: {metrics[k]}")
    (run_dir / "summary.md").write_text("\n".join(lines), encoding="utf-8")

    # Failures = anything below pass, or a hard flag. Sorted worst-first.
    fails = [r for r in judged if r["overall"] < PASS_THRESHOLD or r["critical_failure"] or r["fabrication"]]
    fails.sort(key=lambda r: (r["overall"], not r["critical_failure"]))
    flines = [f"# Failure analysis — {run_dir.name}", "",
              f"{len(fails)} of {len(judged)} answers below pass / flagged.", ""]
    for r in fails:
        flags = []
        if r["critical_failure"]:
            flags.append("CRITICAL")
        if r["fabrication"]:
            flags.append("FABRICATION")
        if not r["is_usable"]:
            flags.append("FALLBACK")
        flines += [
            f"## {r['id']} ({r['category']} · {r['difficulty']}) — overall {r['overall']}"
            + (f"  [{', '.join(flags)}]" if flags else ""),
            f"- Q: {r['question']}",
            f"- scores: " + ", ".join(f"{c}={r[f'score_{c}']}" for c in CRITERIA),
            f"- judge: {r['judge_summary']}",
        ]
        if r["critical_reason"]:
            flines.append(f"- critical: {r['critical_reason']}")
        if r["harness_error"]:
            flines.append(f"- harness error: {r['harness_error']}")
        ans = (r.get("answer") or "").strip().replace("\n", " ")
        flines.append(f"- answer: {ans[:300]}{'…' if len(ans) > 300 else ''}")
        flines.append("")
    (run_dir / "failures.md").write_text("\n".join(flines), encoding="utf-8")


if __name__ == "__main__":
    main()
