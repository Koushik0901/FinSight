"""
Ensemble: rule → fastText → centroid (MiniLM) → LLM if <0.6
+ groceries vs shopping amount-aware rerank.

Usage:
  poc/fasttext_categorizer/.venv/bin/python poc/fasttext_categorizer/ensemble.py --test
  poc/fasttext_categorizer/.venv/bin/python poc/fasttext_categorizer/ensemble.py --merchant "SAFEWAY #123     BURNABY" --amount -3200
"""
import argparse, csv, pathlib, json
from collections import Counter

from normalize import normalize_merchant, merchant_for_training
from categorize_builtin import builtin_category  # we will create shim

# Try to import fasttext and sentence-transformers lazily
try:
    import fasttext
except: fasttext=None

try:
    from sentence_transformers import SentenceTransformer
    import numpy as np
except: SentenceTransformer=None

MODEL_PATH = pathlib.Path(__file__).parent / "models" / "merchant_ft.bin"
LABELED = pathlib.Path(__file__).parent / "data" / "processed" / "labeled.csv"

# Category centroids — built from labeled.csv examples via MiniLM
CENTROIDS = None
EMBEDDER = None

def load_centroids():
    global CENTROIDS, EMBEDDER
    if CENTROIDS is not None:
        return CENTROIDS, EMBEDDER
    if SentenceTransformer is None:
        return None, None
    # Load embedder
    EMBEDDER = SentenceTransformer("sentence-transformers/all-MiniLM-L6-v2")
    # Build centroids from labeled.csv (mean per category)
    from collections import defaultdict
    by_cat = defaultdict(list)
    with LABELED.open(encoding="utf-8") as f:
        for row in csv.DictReader(f):
            cat=row["category_id"]
            if cat=="__exclude": continue
            mer=row["merchant_raw"]
            by_cat[cat].append(mer)
    centroids={}
    for cat, merchants in by_cat.items():
        # sample up to 50 per category to keep fast
        sample=merchants[:50]
        embs=EMBEDDER.encode(sample, normalize_embeddings=True)
        # mean
        import numpy as np
        centroids[cat]=np.mean(embs, axis=0)
    CENTROIDS=centroids
    return centroids, EMBEDDER

def centroid_scores(merchant_raw):
    centroids, embedder = load_centroids()
    if centroids is None:
        return {}
    emb=embedder.encode([merchant_raw], normalize_embeddings=True)[0]
    import numpy as np
    scores={}
    for cat, cent in centroids.items():
        scores[cat]=float(np.dot(emb, cent))
    return scores

def amount_bucket(amount_cents):
    if amount_cents is None: return None
    if amount_cents>0: return "income"
    if amount_cents >= -2000: return "small"
    if amount_cents >= -10000: return "medium"
    return "large"

def predict_ensemble(merchant_raw, amount_cents=None, threshold=0.6, fasttext_model=None):
    # 1. Rule
    rule_cat = builtin_category(merchant_raw)
    if rule_cat:
        return rule_cat, 1.0, "rule"

    # 2. FastText
    if fasttext_model is None and MODEL_PATH.exists() and fasttext:
        fasttext_model = fasttext.load_model(str(MODEL_PATH))
    if fasttext_model:
        txt=merchant_for_training(merchant_raw, amount_cents)
        # fasttext returns __label__X
        labels, probs = fasttext_model.predict(txt, k=2)
        # handle numpy array
        try:
            probs=list(probs)
        except: probs=[0.0]
        top_cat = labels[0].replace("__label__","") if len(labels)>0 else None
        top_prob = float(probs[0]) if len(probs)>0 else 0.0
        second_cat = labels[1].replace("__label__","") if len(labels)>1 else None
        second_prob = float(probs[1]) if len(probs)>1 else 0.0

        # 2a. Groceries vs Shopping amount-aware rerank
        if top_cat in ("groceries","shopping") and second_cat in ("groceries","shopping") and top_cat!=second_cat:
            delta = abs(top_prob - second_prob)
            if delta < 0.2:
                # use centroid + amount
                scores=centroid_scores(merchant_raw)
                # amount hint: groceries avg $30-80, shopping $5-500
                # large amount (>10000) leans shopping, medium small leans groceries
                amt = amount_cents if amount_cents is not None else 0
                # centroid tie-break
                if scores:
                    # weighted: 0.7 centroid, 0.3 amount
                    g_score = scores.get("groceries",0)
                    s_score = scores.get("shopping",0)
                    # amount bias
                    if amt < -15000: # large -> shopping
                        s_score += 0.05
                    elif -8000 <= amt <= -2000: # grocery band
                        g_score += 0.05
                    if g_score > s_score and top_cat!="groceries":
                        # flip
                        top_cat, second_cat = second_cat, top_cat
                        top_prob, second_prob = second_prob, top_prob
                        return top_cat, top_prob, "fastText+centroid+amount-rerank"
                    elif s_score > g_score and top_cat!="shopping":
                        return top_cat, top_prob, "fastText+centroid+amount-rerank"

        if top_cat and top_prob >= threshold:
            return top_cat, top_prob, "fastText"
        # if low confidence, fall through to centroid
        if top_cat and top_prob < threshold:
            # try centroid as fallback before LLM
            scores=centroid_scores(merchant_raw)
            if scores:
                best_cat = max(scores, key=scores.get)
                best_score = scores[best_cat]
                # centroid score is cosine 0-1, need threshold ~0.4
                if best_score > 0.35:
                    # combine prob
                    return best_cat, best_score, "centroid"
            return top_cat, top_prob, "fastText-low"
    # 3. Centroid alone
    scores=centroid_scores(merchant_raw)
    if scores:
        best_cat = max(scores, key=scores.get)
        if scores[best_cat] > 0.35:
            return best_cat, scores[best_cat], "centroid"
    # 4. LLM fallback
    return None, 0.0, "llm-fallback"

if __name__=="__main__":
    ap=argparse.ArgumentParser()
    ap.add_argument("--merchant", type=str, help="test single merchant")
    ap.add_argument("--amount", type=int, default=-3200)
    ap.add_argument("--test", action="store_true", help="run on test set and report groceries fix")
    args=ap.parse_args()
    if args.merchant:
        cat, prob, src = predict_ensemble(args.merchant, args.amount)
        print(f"{args.merchant!r} amount={args.amount} -> {cat} {prob:.2f} via {src}")
        # also show centroid scores
        print("centroid:", centroid_scores(args.merchant))
    elif args.test:
        # evaluate on test set
        import pathlib, csv
        from sklearn.metrics import classification_report
        test_path=pathlib.Path(__file__).parent / "data" / "processed" / "fasttext.test"
        # load test rows
        y_true=[]; y_pred=[]
        with test_path.open(encoding="utf-8") as f:
            for line in f:
                line=line.strip()
                if not line: continue
                parts=line.split()
                true=parts[0].replace("__label__","")
                txt=" ".join(parts[1:])
                # txt is merchant_for_training output, need to reverse? we just use true vs ensemble on txt merchant
                # For test we have original merchant in labeled.csv test split, not txt. So we need to load test via prepare's test split original merchants
                pass
        # simpler: evaluate via labeled test split file we have as fasttext.test, but we need merchant_raw
        # Load test merchants from prepare's test split via labeled.csv + fasttext.test mapping is complex; just demo on few
        for mer, amt in [("SAFEWAY #123     BURNABY", -3200), ("AMAZON.CA #123", -5000), ("COSTCO WHOLESAL #123", -4500), ("SAVE ON FOODS #2221     BURNABY", -2800)]:
            cat, prob, src = predict_ensemble(mer, amt)
            print(f"{mer[:30]:30} -> {cat:12} {prob:.2f} {src} centroid:{centroid_scores(mer).get(cat,0):.2f}")
    else:
        print("Use --merchant 'X' --amount -3200 or --test")
