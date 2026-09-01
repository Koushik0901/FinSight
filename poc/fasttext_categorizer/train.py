"""
Train fastText supervised classifier.

Input: data/processed/fasttext.train (produced by prepare.py)
Output: models/merchant_ft.bin + .vec + metrics

Hyperparams tuned for small merchant vocab:
  dim=50, wordNgrams=2, bucket=200k, minCount=1, epoch=50, lr=0.5, loss=ova
  (ova for imbalanced categories — hs collapses on rare labels)

Run after: python poc/fasttext_categorizer/prepare.py
"""
import pathlib
import json
import time

try:
    import fasttext
except ImportError:
    raise SystemExit("Missing fasttext. pip install -r poc/fasttext_categorizer/requirements.txt (needs build-essential)")

ROOT = pathlib.Path(__file__).parent
DATA = ROOT / "data" / "processed"
MODELS = ROOT / "models"
MODELS.mkdir(parents=True, exist_ok=True)

TRAIN = DATA / "fasttext.train"
VALID = DATA / "fasttext.valid"

OUT_BIN = MODELS / "merchant_ft.bin"


def train():
    if not TRAIN.exists():
        raise SystemExit(f"Missing {TRAIN}. Run prepare.py first.")
    print(f"Training on {TRAIN} ({TRAIN.stat().st_size} bytes)")

    # fastText hyperparams — small vocab friendly
    model = fasttext.train_supervised(
        input=str(TRAIN),
        dim=50,
        wordNgrams=2,
        bucket=200000,
        minCount=1,
        epoch=50,
        lr=0.5,
        loss="ova",  # one-vs-all for multi-class imbalance
        thread=4,
        verbose=2,
    )
    model.save_model(str(OUT_BIN))
    print(f"Saved {OUT_BIN} ({OUT_BIN.stat().st_size/1024:.1f} KB)")

    # valid accuracy
    if VALID.exists():
        res = model.test(str(VALID))
        # res = (n, precision, recall) where precision==accuracy for single-label
        print(f"Valid: n={res[0]} P@1={res[1]:.3f} R@1={res[2]:.3f}")
        metrics = {"valid_n": res[0], "valid_p1": res[1], "valid_r1": res[2], "model": str(OUT_BIN)}
        with open(DATA / "train_metrics.json", "w") as f:
            json.dump(metrics, f, indent=2)

    # also export labels for inspection
    print("Labels:", model.labels)

    # quick demo
    from normalize import merchant_for_training
    demos = [
        "TIM HORTONS #3356       BURNABY",
        "UBER EATS               TORONTO",
        "NETFLIX.COM             VANCOVER",
        "SAFEWAY #123            VANCOUVER",
    ]
    for d in demos:
        txt = merchant_for_training(d, -1200)
        pred, prob = model.predict(txt, k=2)
        print(f"  {d!r:35} -> {pred} {prob}")

if __name__ == "__main__":
    t0=time.time()
    train()
    print(f"Done in {time.time()-t0:.1f}s")
