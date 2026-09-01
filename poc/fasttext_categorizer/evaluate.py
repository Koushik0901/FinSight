"""
Evaluate fastText vs LLM with threshold-as-other logic.

Core 10 categories trained. Inference: max_prob < 0.6 → "other" (deterministic fallback, not learned).
__exclude = transfer (never spend). Runs: test/valid/heldout + low-conf + latency.
Usage: python poc/fasttext_categorizer/evaluate.py [--threshold 0.6]
"""
import pathlib
import json
import csv
import time
from collections import Counter, defaultdict

from sklearn.metrics import accuracy_score, f1_score, classification_report, confusion_matrix

ROOT = pathlib.Path(__file__).parent
DATA = ROOT / "data" / "processed"
MODELS = ROOT / "models"

try:
    import fasttext
except ImportError:
    raise SystemExit("Missing fasttext")

from normalize import merchant_for_training


def load_fasttext_file(path: pathlib.Path):
    rows=[]
    with path.open(encoding="utf-8") as f:
        for line in f:
            line=line.strip()
            if not line: continue
            # __label__X text...
            parts=line.split()
            label = parts[0].replace("__label__","")
            text = " ".join(parts[1:])
            rows.append((text,label))
    return rows

def parse_text_to_merchant_amount(text: str):
    # text is merchant_for_training output — contains __amount_* token
    return text


def evaluate(model, test_path: pathlib.Path, name: str, threshold: float = 0.6):
    rows = load_fasttext_file(test_path)
    y_true=[]
    y_pred_raw=[]
    y_pred_thresh=[]  # threshold-as-other
    y_prob=[]
    for text, true in rows:
        pred, prob = model.predict(text, k=1)
        p = pred[0].replace("__label__","") if pred else "__exclude"
        pr = float(prob[0]) if len(prob) else 0.0
        # threshold-as-other: prob < threshold → "other" (not learned, deterministic fallback)
        p_thresh = "other" if (p != "__exclude" and pr < threshold) else p
        y_true.append(true)
        y_pred_raw.append(p)
        y_pred_thresh.append(p_thresh)
        y_prob.append(pr)

    # raw metrics (10-way)
    acc = accuracy_score(y_true, y_pred_raw)
    mf1 = f1_score(y_true, y_pred_raw, average="macro", zero_division=0)
    print(f"\n=== {name} ({test_path.name}) n={len(y_true)} threshold={threshold} ===")
    print(f"Raw 10-way — Accuracy: {acc:.3f}  Macro-F1: {mf1:.3f}")
    print(classification_report(y_true, y_pred_raw, zero_division=0, digits=3))
    # threshold-as-other stats
    low = sum(1 for pr in y_prob if pr < threshold)
    print(f"Threshold <{threshold} → 'other': {low}/{len(y_prob)} ({low/len(y_prob):.1%}) mapped to 'other'")
    # per-category avg prob
    by_true = defaultdict(list)
    for t,pr in zip(y_true, y_prob):
        by_true[t].append(pr)
    for cat in sorted(by_true):
        avg = sum(by_true[cat])/len(by_true[cat])
        print(f"  {cat:15} avg_prob={avg:.2f} n={len(by_true[cat])}")

    # confusion (raw 10-way)
    cm_labels = sorted(set(y_true) | set(y_pred_raw))
    cm = confusion_matrix(y_true, y_pred_raw, labels=cm_labels)
    out = DATA / f"metrics_{name}.json"
    metrics = {
        "name": name, "n": len(y_true), "accuracy": acc, "macro_f1": mf1,
        "low_conf_rate": low/len(y_prob) if y_prob else 0,
        "threshold": threshold,
        "labels": cm_labels, "confusion": cm.tolist(),
        "report": classification_report(y_true, y_pred_raw, zero_division=0, output_dict=True),
    }
    with open(out, "w") as f:
        json.dump(metrics, f, indent=2)
    print(f"Saved {out}")
    return metrics


def latency_bench(model, n=1000):
    from normalize import merchant_for_training
    txt = merchant_for_training("TIM HORTONS #3356       BURNABY", -621)
    for _ in range(100): model.predict(txt)  # warmup
    t0=time.time()
    for _ in range(n):
        model.predict(txt)
    dt = (time.time()-t0)/n*1000
    print(f"\nLatency: {dt:.2f} ms / predict (n={n})")
    return dt


def main():
    bin_path = MODELS / "merchant_ft.bin"
    if not bin_path.exists():
        raise SystemExit(f"Missing {bin_path}. Run train.py first.")
    model = fasttext.load_model(str(bin_path))
    print(f"Loaded {bin_path} labels={model.labels}")

    results={}
    for fname, name in [
        ("fasttext.test", "test"),
        ("fasttext.valid", "valid"),
        ("fasttext.test_heldout_merchant", "heldout_merchant"),
    ]:
        p = DATA / fname
        if p.exists():
            results[name]=evaluate(model, p, name)
        else:
            print(f"Skip missing {p}")

    results["latency_ms"] = latency_bench(model)

    # summary
    with open(DATA / "metrics_summary.json","w") as f:
        json.dump(results, f, indent=2)
    print(f"\nAll metrics in {DATA/'metrics_summary.json'}")
    # generalization verdict
    test_mf1 = results.get("test",{}).get("macro_f1",0)
    held_mf1 = results.get("heldout_merchant",{}).get("macro_f1",0)
    if held_mf1 and test_mf1:
        gap = test_mf1 - held_mf1
        print(f"\nGeneralization gap (test - heldout): {gap:.3f}")
        if gap > 0.15:
            print("⚠️  Gap >0.15 suggests overfitting to seen merchants — add more merchant diversity + augmentation.")
        elif gap < 0.05:
            print("✅ Gap small — model generalizes to unseen merchants.")
        else:
            print("— Moderate gap — acceptable, but consider more held-out merchant data.")

if __name__ == "__main__":
    main()
