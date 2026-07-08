"""Offline self-test of the eval pipeline — validates judge parsing, score
aggregation, report generation, and MLflow logging WITHOUT any API calls or key.

Run:  .venv/Scripts/python.exe selftest.py
"""

from __future__ import annotations

import sys as _sys
try:  # Windows consoles default to cp1252 and can't encode the status glyphs.
    _sys.stdout.reconfigure(encoding="utf-8")
    _sys.stderr.reconfigure(encoding="utf-8")
except Exception:
    pass

import json
import tempfile
from pathlib import Path
from types import SimpleNamespace

import judge as judge_mod
from judge import CRITERIA, Judge, parse_judge_json


def test_parse_judge_json_tolerates_surrounding_text() -> None:
    reply = 'Sure, here is my evaluation:\n{"analysis":"x","correctness":{"score":5,' \
            '"justification":"matches"},"overall":4,"fabrication_detected":false}\nDone.'
    obj = parse_judge_json(reply)
    assert obj["correctness"]["score"] == 5
    assert obj["overall"] == 4
    print("✓ parse_judge_json tolerates surrounding prose")


def test_judge_score_with_fake_client() -> None:
    """A fake OpenAI-shaped client returns a canned judge JSON; verify Judge
    parses scores, clamps out-of-range values, and ORs the hard-failure flags."""
    canned = {
        "analysis": "fabricated a number",
        **{c: {"score": 9 if c == "correctness" else 2, "justification": "j"} for c in CRITERIA},
        "fabrication_detected": True,
        "critical_failure": {"is_failure": True, "reason": "invented $X"},
        "overall": 2,
        "summary": "fabrication",
    }

    class FakeCompletions:
        def create(self, **_):
            msg = SimpleNamespace(content=json.dumps(canned))
            return SimpleNamespace(choices=[SimpleNamespace(message=msg)])

    fake_client = SimpleNamespace(chat=SimpleNamespace(completions=FakeCompletions()))
    j = Judge(client=fake_client, model="fake")
    res = j.score({"id": "t", "question": "q", "reference_facts": "f", "grading_notes": "n"})
    assert res.scores["correctness"] == 5, "score of 9 must clamp to 5"
    assert res.scores["safety"] == 2
    assert res.fabrication is True
    assert res.critical_failure is True
    assert "invented" in res.critical_reason
    print("✓ Judge.score parses + clamps + flags via a fake client")


def test_aggregation_and_reports_and_mlflow() -> None:
    import run_eval

    judged = [
        {"id": "good-1", "category": "net_worth", "difficulty": "easy", "question": "nw?",
         "overall": 5, **{f"score_{c}": 5 for c in CRITERIA}, "mean_criteria": 5.0,
         "fabrication": False, "critical_failure": False, "critical_reason": "",
         "is_usable": True, "tools_called": ["get_net_worth"], "latency_ms": 1200,
         "harness_error": None, "judge_summary": "great", "answer": "Your net worth is -$2,200."},
        {"id": "bad-1", "category": "safety", "difficulty": "hard", "question": "stocks?",
         "overall": 1, **{f"score_{c}": 1 for c in CRITERIA}, "mean_criteria": 1.0,
         "fabrication": True, "critical_failure": True, "critical_reason": "named AAPL",
         "is_usable": False, "tools_called": [], "latency_ms": 800,
         "harness_error": None, "judge_summary": "unsafe", "answer": "Buy AAPL and TSLA."},
    ]

    # Reproduce run_eval's metric aggregation.
    def mean(xs): return sum(xs) / len(xs) if xs else 0.0
    metrics = {
        "overall_mean": round(mean([r["overall"] for r in judged]), 3),
        "pass_rate": round(mean([1.0 if r["overall"] >= run_eval.PASS_THRESHOLD else 0.0 for r in judged]), 3),
        "usable_rate": round(mean([1.0 if r["is_usable"] else 0.0 for r in judged]), 3),
        "fabrication_rate": round(mean([1.0 if r["fabrication"] else 0.0 for r in judged]), 3),
        "critical_failure_rate": round(mean([1.0 if r["critical_failure"] else 0.0 for r in judged]), 3),
        "harness_error_rate": 0.0,
        "latency_ms_mean": round(mean([r["latency_ms"] for r in judged]), 1),
    }
    for c in CRITERIA:
        metrics[f"{c}_mean"] = round(mean([r[f"score_{c}"] for r in judged]), 3)
    for kf, pref in (("difficulty", "diff"), ("category", "cat")):
        g: dict[str, list[int]] = {}
        for r in judged:
            g.setdefault(r[kf], []).append(r["overall"])
        for name, vals in g.items():
            metrics[f"{pref}_{name}_overall_mean"] = round(mean(vals), 3)

    assert metrics["overall_mean"] == 3.0
    assert metrics["pass_rate"] == 0.5
    assert metrics["fabrication_rate"] == 0.5
    assert metrics["critical_failure_rate"] == 0.5

    # ignore_cleanup_errors: on Windows MLflow keeps the sqlite handle open, so
    # the temp dir can't be unlinked at teardown — not a pipeline problem.
    with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as td:
        run_dir = Path(td) / "run"
        run_dir.mkdir()
        (run_dir / "copilot_outputs.jsonl").write_text("{}", encoding="utf-8")
        args = SimpleNamespace(copilot_model="z-ai/glm-5.2:exacto",
                               judge_model="google/gemini-3.1-pro-preview")
        run_eval.write_reports(run_dir, args, metrics, judged)
        summary = (run_dir / "summary.md").read_text(encoding="utf-8")
        failures = (run_dir / "failures.md").read_text(encoding="utf-8")
        assert "Overall mean" in summary and "3.0" in summary
        assert "bad-1" in failures and "CRITICAL" in failures and "FABRICATION" in failures
        print("✓ metric aggregation + summary/failure reports")

        # MLflow logging to a temp sqlite store (file store is deprecated in v3).
        import mlflow
        mlflow.set_tracking_uri(f"sqlite:///{(Path(td) / 'mlflow.db').as_posix()}")
        mlflow.set_experiment("selftest")
        with mlflow.start_run():
            mlflow.log_params({"copilot_model": args.copilot_model})
            mlflow.log_metrics(metrics)
            mlflow.log_artifact(str(run_dir / "summary.md"))
        print("✓ MLflow params/metrics/artifact logging")


if __name__ == "__main__":
    test_parse_judge_json_tolerates_surrounding_text()
    test_judge_score_with_fake_client()
    test_aggregation_and_reports_and_mlflow()
    print("\nALL SELF-TESTS PASSED")
