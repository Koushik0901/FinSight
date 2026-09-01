"""
Build fastText datasets from labeled merchants.

Steps:
  1. --make-template : scan samples/ for unique merchant_raw, emit labeling_template.csv
  2. default        : read data/processed/labeled.csv (merchant_raw,category_id,amount_cents)
                      + augment via augment.py → strat split → fasttext.train/.valid/.test

Run: python poc/fasttext_categorizer/prepare.py --make-template
     (then fill labeled.csv)
     python poc/fasttext_categorizer/prepare.py
"""
import argparse
import csv
import pathlib
import random
from collections import Counter
from typing import List, Tuple

from normalize import merchant_for_training
from augment import augment_dataset

ROOT = pathlib.Path(__file__).resolve().parents[2]
SAMPLES_DIR = ROOT / "samples"
OUT_DIR = pathlib.Path(__file__).parent / "data"
PROCESSED = OUT_DIR / "processed"
SYNTHETIC = OUT_DIR / "synthetic"

STARTER_CATEGORIES = [
    "groceries","dining","transport","housing","utilities","subscriptions","shopping","travel","gifts","health"
]

TEMPLATE_COLS = ["merchant_raw","category_id","amount_cents","notes"]


def scan_samples() -> List[Tuple[str,int]]:
    """Collect all unique merchant_raw from samples/*.csv with an example amount."""
    import glob
    merchants: dict[str,int] = {}
    for path in SAMPLES_DIR.glob("*.csv"):
        try:
            text = path.read_text(encoding="utf-8", errors="ignore")
        except Exception:
            continue
        lines = text.splitlines()
        if not lines:
            continue
        header = lines[0].lower()
        # detect format
        for line in lines[1:]:
            line=line.strip()
            if not line:
                continue
            # naive CSV split respecting quotes
            try:
                row = next(csv.reader([line]))
            except Exception:
                continue
            amount = 0
            merchant = ""
            if "description" in header and "amount" in header:
                # amex
                if len(row) >= 4:
                    merchant = row[2].strip()
                    try:
                        amount = int(float(row[3].strip('", ')) * 100) if row[3].strip() else 0
                    except: amount=0
            elif "cibc" in str(path).lower() or "tangerine" in str(path).lower():
                if len(row) >= 2:
                    merchant = row[1].strip()
                    # try to find amount in row
                    for tok in row[2:]:
                        try:
                            if tok.strip():
                                amount = int(float(tok.strip('",'))*100)
                                break
                        except: pass
            else:
                # fallback: take description-like col
                if len(row) >= 2:
                    merchant = row[1].strip() if len(row[1])>3 else row[0].strip()
            if merchant and len(merchant) > 3:
                import re
                alpha = sum(c.isalpha() for c in merchant)
                if alpha < 3:
                    continue
                if re.fullmatch(r"[\d\-/,\s\.]+", merchant):
                    continue
                if len(merchant) < 5:
                    continue
                if merchant not in merchants:
                    merchants[merchant] = amount
    return list(merchants.items())


def make_template():
    pairs = scan_samples()
    pairs.sort(key=lambda x: x[0].lower())
    PROCESSED.mkdir(parents=True, exist_ok=True)
    out = PROCESSED / "labeling_template.csv"
    with out.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=TEMPLATE_COLS)
        w.writeheader()
        for mer, amt in pairs:
            hint = "transfer/exclude if internal movement" if any(k in mer.lower() for k in ["transfer","payment received","preauthorized debit"]) else ""
            w.writerow({"merchant_raw": mer, "category_id": "", "amount_cents": amt, "notes": hint})
    print(f"Wrote {len(pairs)} unique merchants to {out}")
    print(f"Fill category_id with one of: {', '.join(STARTER_CATEGORIES)} or __exclude for transfers")
    print(f"Then save as {PROCESSED / 'labeled.csv'}")


def load_labeled() -> List[Tuple[str,str,int]]:
    p = PROCESSED / "labeled.csv"
    if not p.exists():
        raise SystemExit(f"Missing {p}. Run --make-template and fill it first.")
    rows=[]
    with p.open(encoding="utf-8") as f:
        r = csv.DictReader(f)
        for line in r:
            mer = (line.get("merchant_raw") or "").strip()
            cat = (line.get("category_id") or "").strip().lower()
            if not mer or not cat:
                continue
            # normalize category aliases
            if cat in ("exclude","transfer","__exclude"):
                cat="__exclude"
            try:
                amt = int(line.get("amount_cents") or 0)
            except:
                amt=0
            rows.append((mer, cat, amt))
    # also merge LLM-augmented synthetic if present (thin-class balancing via kimi-k3)
    llm_syn = SYNTHETIC / "llm_augmented.csv"
    if llm_syn.exists():
        with llm_syn.open(encoding="utf-8") as f:
            for line in csv.DictReader(f):
                mer=(line.get("merchant_raw") or "").strip()
                cat=(line.get("category_id") or "").strip().lower()
                if not mer or not cat: continue
                if cat in ("exclude","transfer","__exclude"): cat="__exclude"
                try: amt=int(line.get("amount_cents") or 0)
                except: amt=0
                rows.append((mer, cat, amt))
        print(f"Merged {llm_syn} — total now {len(rows)}")
    if not rows:
        raise SystemExit("No labeled rows found.")
    counts=Counter(c for _,c,_ in rows)
    print(f"Loaded {len(rows)} labeled rows: {dict(counts)}")
    return rows

def write_fasttext(path: pathlib.Path, rows: List[Tuple[str,str,int]]):
    with path.open("w", encoding="utf-8") as f:
        for mer, cat, amt in rows:
            txt = merchant_for_training(mer, amt)
            # fastText label prefix
            label = cat if cat.startswith("__label__") else f"__label__{cat}"
            f.write(f"{label} {txt}\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--make-template", action="store_true", help="scan samples/ and emit labeling_template.csv")
    ap.add_argument("--no-augment", action="store_true", help="skip synthetic augmentation")
    ap.add_argument("--seed", type=int, default=42)
    args = ap.parse_args()

    if args.make_template:
        make_template()
        return

    rows = load_labeled()

    # augment
    if not args.no_augment:
        rows = augment_dataset(rows)
        print(f"After augmentation: {len(rows)} rows (5× per labeled, transfers single)")
        # save synthetic inspection
        SYNTHETIC.mkdir(parents=True, exist_ok=True)
        with (SYNTHETIC / "sample_augmented.csv").open("w", newline="", encoding="utf-8") as f:
            w = csv.writer(f)
            w.writerow(["merchant_raw","category_id","amount_cents"])
            for mer,cat,amt in rows[:200]:
                w.writerow([mer,cat,amt])

    # strat split 80/10/10
    labels = [c for _,c,_ in rows]
    # filter rare labels that would break stratify
    counts=Counter(labels)
    rare = {k for k,v in counts.items() if v < 3}
    if rare:
        print(f"Warning: rare labels <3 examples {rare} — grouping to allow split")
    # if too few per class, fallback to random split
    try:
        from sklearn.model_selection import StratifiedShuffleSplit
        sss1 = StratifiedShuffleSplit(n_splits=1, test_size=0.2, random_state=args.seed)
        train_idx, temp_idx = next(sss1.split(rows, labels))
        train = [rows[i] for i in train_idx]
        temp = [rows[i] for i in temp_idx]
        t_labels = [labels[i] for i in temp_idx]
        sss2 = StratifiedShuffleSplit(n_splits=1, test_size=0.5, random_state=args.seed)
        valid_idx, test_idx = next(sss2.split(temp, t_labels))
        valid = [temp[i] for i in valid_idx]
        test = [temp[i] for i in test_idx]
    except Exception as e:
        print(f"Stratified split unavailable/failed ({e}), falling back to random")
        random.Random(args.seed).shuffle(rows)
        n=len(rows)
        train, valid, test = rows[:int(0.8*n)], rows[int(0.8*n):int(0.9*n)], rows[int(0.9*n):]

    print(f"Split train={len(train)} valid={len(valid)} test={len(test)}")
    for name, split in [("train",train),("valid",valid),("test",test)]:
        c=Counter(cat for _,cat,_ in split)
        print(f"  {name}: {dict(c)}")

    PROCESSED.mkdir(parents=True, exist_ok=True)
    write_fasttext(PROCESSED / "fasttext.train", train)
    write_fasttext(PROCESSED / "fasttext.valid", valid)
    write_fasttext(PROCESSED / "fasttext.test", test)
    # also held-out merchant split: merchants never seen in train
    train_merchants = {merchant_for_training(m, a).split()[0] for m,_,a in train}
    heldout = [r for r in test if merchant_for_training(r[0], r[2]).split()[0] not in train_merchants]
    write_fasttext(PROCESSED / "fasttext.test_heldout_merchant", heldout or test[:20])
    print(f"Wrote {PROCESSED/'fasttext.train'} etc.")
    print(f"Held-out merchant test: {len(heldout)} rows")

if __name__ == "__main__":
    main()
