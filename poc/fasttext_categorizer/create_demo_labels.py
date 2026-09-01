"""Create demo labeled.csv from template using heuristic keyword mapping (so POC runs immediately)."""
import csv, pathlib, re

ROOT = pathlib.Path(__file__).parent / "data" / "processed"
template = ROOT / "labeling_template.csv"
out = ROOT / "labeled.csv"

# heuristic keyword → category (mirrors categorize.rs KEYWORD_MAP subset)
MAP = [
    (r"tim hortons|starbucks|chipotle|mcdonald|a & w|subway|pizza|restaurant|cafe|bam\*|tst-|uber eats|doordash|skip", "dining"),
    (r"safeway|whole foods|save on foods|costco|walmart|no frills|real cdn|superstore|grocery|market", "groceries"),
    (r"uber|lyft|evo car|compass vending|shell|chevron|petro|transit|parking", "transport"),
    (r"netflix|spotify|openai|chatgpt|anthropic|claude|openrouter|prime member|membership fee", "subscriptions"),
    (r"amazon|amzn|shop|sport chek|adidas|american eagle|best buy|home depot", "shopping"),
    (r"air canada|air india|flight|hotel|airbnb|expedia", "travel"),
    (r"pharma|drug mart|doctor|health|dental|wellness", "health"),
    (r"hydro|pge|comcast|internet|lightspeed|freedom mobile|bell|rogers", "utilities"),
    (r"rent|landlord|housing|mortgage", "housing"),
    (r"donation|gift|charity", "gifts"),
    (r"transfer|payment received|preauthorized debit|eft|deposit|payroll|salary|e-transfer|interac", "__exclude"),
]

def classify(merchant: str) -> str:
    m = merchant.lower()
    for pat, cat in MAP:
        if re.search(pat, m):
            return cat
    # fallback: most common
    return "shopping"

rows=[]
with template.open(encoding="utf-8") as f:
    r=csv.DictReader(f)
    for line in r:
        mer=line["merchant_raw"].strip()
        if not mer: continue
        cat=classify(mer)
        amt=line.get("amount_cents","0")
        rows.append((mer,cat,amt))

# take first 300 and ensure at least 10 per category
from collections import Counter
cnt=Counter(c for _,c,_ in rows)
print("Heuristic distribution", dict(cnt))
# limit to 400 random but stratified
import random; random.seed(42)
# sample up to 40 per category
by_cat={}
for mer,cat,amt in rows:
    by_cat.setdefault(cat, []).append((mer,cat,amt))
sampled=[]
for cat, lst in by_cat.items():
    k=min(len(lst), 50)
    sampled.extend(random.sample(lst, k))
random.shuffle(sampled)
sampled=sampled[:350]
print("Sampled", Counter(c for _,c,_ in sampled))

with out.open("w", newline="", encoding="utf-8") as f:
    w=csv.writer(f)
    w.writerow(["merchant_raw","category_id","amount_cents"])
    for mer,cat,amt in sampled:
        w.writerow([mer,cat,amt])
print(f"Wrote {len(sampled)} demo labeled rows to {out}")
