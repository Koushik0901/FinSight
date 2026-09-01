"""
Simple HITL labeling — prints table, asks for approval/edit each batch.

Usage:
  export OPENROUTER_API_KEY=sk-or-...
  python poc/fasttext_categorizer/label_hitl.py --model moonshotai/kimi-k3 --reasoning-effort high --limit 60

Per batch:
  - calls LLM (same prompt + auditable reasoning)
  - prints: # | merchant | → category (conf) | rationale | ⚠ if conf<0.7
  - prompt: [Enter]=approve all, e=edit, s=skip batch, q=quit & save

Edits: type "3 groceries" or "2 __exclude" or "5 dining" then Enter; empty line to finish edits.

Writes incrementally to labeled.csv / labeled.jsonl / labeled_audit.jsonl (flushed per batch, resumable).
"""
import argparse, csv, json, os, pathlib, sys, time
from typing import List, Tuple
import requests
from normalize import normalize_merchant

ROOT = pathlib.Path(__file__).parent
TEMPLATE = ROOT / "data" / "processed" / "labeling_template.csv"
OUT = ROOT / "data" / "processed" / "labeled.csv"

CATEGORIES = ["groceries","dining","transport","housing","utilities","subscriptions","shopping","travel","gifts","health","__exclude"]
SYSTEM_PROMPT_BASE = (
    "Role: You are an expert personal finance transaction categorizer with deep knowledge of Canadian/US merchant descriptors, banking statement formats, and payment processor noise.\n\n"
    "Objective: Classify each merchant_raw into exactly one of 10 spending categories or __exclude for internal transfers, with calibrated confidence and a one-sentence rationale.\n\n"
    "Details:\n"
    "- Allowed category_id values (use id exactly, lowercase): groceries, dining, transport, housing, utilities, subscriptions, shopping, travel, gifts, health, __exclude\n"
    "- __exclude = internal money movement only: \"PAYMENT RECEIVED - THANK YOU\", \"INTERNET TRANSFER 000000...\", \"PREAUTHORIZED DEBIT - CREDIT CARD PAYMENT\", \"E-TRANSFER\" between own accounts. When in doubt, prefer spending category.\n"
    "- If none fit, choose closest — do not invent ids and do not use \"other\".\n"
    "- Ignore location/store numbers/phone — focus on core vendor.\n"
    "Examples:\n"
    "Input: [{\"txn_id\":\"t0\",\"merchant_raw\":\"TIM HORTONS #3356       BURNABY\",\"amount_cents\":-621}]\n"
    "Output: [{\"txn_id\":\"t0\",\"category_id\":\"dining\",\"confidence\":0.95,\"rationale\":\"Tim Hortons is a coffee/dining chain\"}]\n"
    "Input: [{\"txn_id\":\"t1\",\"merchant_raw\":\"PAYMENT RECEIVED - THANK YOU\",\"amount_cents\":-298614}]\n"
    "Output: [{\"txn_id\":\"t1\",\"category_id\":\"__exclude\",\"confidence\":0.99,\"rationale\":\"Credit card payment transfer\"}]\n"
    "Output format: Valid JSON array only — no markdown, no fences. Each element {\"txn_id\":\"...\",\"category_id\":\"...\",\"confidence\":0.0,\"rationale\":\"...\"}."
)

def resolve_key() -> str:
    k = os.environ.get("OPENROUTER_API_KEY","").strip()
    if k: return k
    raise SystemExit("Missing OPENROUTER_API_KEY. export OPENROUTER_API_KEY=sk-or-...")

def call_llm(api_key: str, model: str, merchants: List[Tuple[str,int]], few_shot: List[Tuple[str,str]] = None, reasoning_effort=None, reasoning_budget=None):
    items = [{"txn_id": f"t{i}", "merchant_raw": m, "amount_cents": amt} for i,(m,amt) in enumerate(merchants)]
    user_prompt = f"Classify these {len(items)} transactions:\n{json.dumps(items)}\n\nRespond:\n[{{\"txn_id\":\"...\",\"category_id\":\"...\",\"confidence\":0.0,\"rationale\":\"one sentence\"}}]"
    system_prompt = SYSTEM_PROMPT_BASE
    if few_shot:
        ex_json = json.dumps([{"merchant_raw": m, "category_id": c} for m,c in few_shot[:10]])
        system_prompt += f"\n\nRecent approved examples from this user (use as ground truth for similar merchants):\n{ex_json}"
    body = {"model": model, "messages": [{"role":"system","content":system_prompt},{"role":"user","content":user_prompt}], "temperature":0.0, "max_tokens":4096, "include_reasoning": True}
    if reasoning_effort or reasoning_budget:
        reasoning={}
        if reasoning_effort: reasoning["effort"]=reasoning_effort
        elif reasoning_budget: reasoning["max_tokens"]=reasoning_budget; reasoning["enabled"]=True
        body["reasoning"]=reasoning
        if reasoning_effort and "openai" in model.lower(): body["reasoning_effort"]=reasoning_effort
    headers={"Authorization": f"Bearer {api_key}","HTTP-Referer":"https://finsight.local","X-Title":"FinSight HITL","Content-Type":"application/json"}
    for attempt in range(3):
        try:
            r=requests.post("https://openrouter.ai/api/v1/chat/completions", headers=headers, json=body, timeout=120)
            if r.status_code==429:
                time.sleep(2**attempt*5); continue
            r.raise_for_status()
            data=r.json()
            msg=data["choices"][0]["message"]
            content=msg.get("content") or ""
            reasoning_text=msg.get("reasoning") or msg.get("reasoning_details") or ""
            if isinstance(reasoning_text, list): reasoning_text="\n".join(str(x.get("text") or x) for x in reasoning_text)
            usage=data.get("usage") or {}
            details=usage.get("completion_tokens_details") or {}
            audit={"reasoning":reasoning_text,"content":content,"usage":usage,"reasoning_tokens":details.get("reasoning_tokens") or 0,"response_tokens":usage.get("completion_tokens") or 0,"prompt_tokens":usage.get("prompt_tokens") or 0,"total_tokens":usage.get("total_tokens") or 0,"model":data.get("model") or model,"id":data.get("id")}
            # parse array
            c=content.strip()
            if c.startswith("```"):
                c=c.split("\n",1)[1] if "\n" in c else c
                if c.endswith("```"): c=c[:-3]
                c=c.strip()
            s=c.find("["); e=c.rfind("]")
            if s==-1 or e==-1: raise ValueError(f"No JSON array: {c[:500]}")
            arr=json.loads(c[s:e+1])
            return arr, audit
        except Exception as e:
            if attempt==2: raise
            print(f"  retry {attempt+1}/3: {e}", file=sys.stderr)
            time.sleep(2**attempt*2)
    return [], {}

def load_template(limit=0):
    rows=[]
    with TEMPLATE.open(encoding="utf-8") as f:
        for line in csv.DictReader(f):
            mer=(line.get("merchant_raw") or "").strip()
            if not mer: continue
            try: amt=int(line.get("amount_cents") or 0)
            except: amt=0
            rows.append((mer, amt))
    return rows[:limit] if limit else rows

def get_relevant_examples(chunk: List[Tuple[str,int]], approved: List[Tuple[str,str]], k: int = 8) -> List[Tuple[str,str]]:
    """Pick k approved merchants most relevant to current chunk via BM25 (no external deps)."""
    if not approved or k<=0:
        return []
    import math
    # Tokenize approved docs via same normalization FinSight uses
    docs = [normalize_merchant(mer).split() for mer,_ in approved]
    # Query = all tokens in current chunk
    query_tokens = []
    for mer,_ in chunk:
        query_tokens.extend(normalize_merchant(mer).split())
    if not query_tokens:
        return approved[-k:][::-1]
    N = len(docs)
    avgdl = sum(len(d) for d in docs) / N if N else 0
    # doc frequencies
    df = {}
    for d in docs:
        for tok in set(d):
            df[tok] = df.get(tok, 0) + 1
    # IDF
    idf = {tok: math.log((N - freq + 0.5) / (freq + 0.5) + 1) for tok, freq in df.items()}
    # BM25 params
    k1, b = 1.5, 0.75
    scored = []
    for idx, d in enumerate(docs):
        dl = len(d)
        freq = {}
        for tok in d:
            freq[tok] = freq.get(tok, 0) + 1
        score = 0.0
        for tok in query_tokens:
            if tok not in freq:
                continue
            tf = freq[tok]
            idf_tok = idf.get(tok, 0)
            denom = tf + k1 * (1 - b + b * dl / avgdl) if avgdl else tf + k1
            score += idf_tok * (tf * (k1 + 1) / denom)
        # small recency tie-breaker so stable sort prefers newer on equal score
        score += (idx / N) * 1e-6
        scored.append((score, approved[idx]))
    scored.sort(reverse=True, key=lambda x: x[0])
    # If all scores ~0 (no token overlap), fall back to most recent
    if scored and scored[0][0] == 0:
        return approved[-k:][::-1]
    return [x[1] for x in scored[:k]]

def print_batch(merchants, preds, audits, threshold=0.7):
    # pretty table — no deps
    print("\n" + "="*110)
    print(f"{'#':<3} {'MERCHANT':<45} {'→ CATEGORY':<14} {'CONF':<5} RATIONALE")
    print("-"*110)
    for i, (mer, amt) in enumerate(merchants):
        # find pred by txn_id
        pred = next((p for p in preds if p.get("txn_id")==f"t{i}"), None)
        if not pred: cat,conf,rat="???",0,""
        else: cat,conf,rat=pred.get("category_id","?"), float(pred.get("confidence",0) or 0), pred.get("rationale","")
        flag = " ⚠ LOW" if conf < threshold else ""
        # truncate
        mer_s = (mer[:43] + "…") if len(mer)>43 else mer
        rat_s = (rat[:45] + "…") if len(rat)>45 else rat
        print(f"{i:<3} {mer_s:<45} {cat:<14} {conf:.2f}{flag:<7} {rat_s}")
    # show reasoning snippet if any
    reasoning = audits.get("reasoning","") if isinstance(audits, dict) else ""
    if reasoning:
        print("\n[Model reasoning snippet]")
        print(reasoning[:400] + ("…" if len(reasoning)>400 else ""))
    usage = audits.get("usage",{}) if isinstance(audits, dict) else {}
    if usage:
        print(f"[Tokens] prompt={audits.get('prompt_tokens',0)} reasoning={audits.get('reasoning_tokens',0)} response={audits.get('response_tokens',0)}")

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--model", default="moonshotai/kimi-k3")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--batch", type=int, default=10, help="smaller batch for HITL review (default 10)")
    ap.add_argument("--threshold", type=float, default=0.7, help="flag conf below this")
    ap.add_argument("--auto-threshold", type=float, default=0.85, help="auto-approve if all conf >= this (no prompt)")
    ap.add_argument("--few-shot", type=int, default=8, help="relevant approved examples to send as context (0=disable)")
    ap.add_argument("--reasoning-effort", type=str, default="high", choices=["low","medium","high"])
    ap.add_argument("--reasoning-budget", type=int, default=None)
    ap.add_argument("--resume", action="store_true", help="resume from existing labeled.csv")
    args=ap.parse_args()
    merchants_all=load_template(args.limit)
    print(f"Loaded {len(merchants_all)} merchants — categories: {', '.join(CATEGORIES)}\nOther = threshold <0.6 (not learned), __exclude = transfer")
    # resume: skip already labeled merchants
    done=set()
    if args.resume and OUT.exists():
        with OUT.open(encoding="utf-8") as f:
            for line in csv.DictReader(f):
                done.add(line.get("merchant_raw",""))
        print(f"Resuming — {len(done)} already labeled, skipping")
        merchants_all=[(m,a) for m,a in merchants_all if m not in done]
        print(f"Remaining: {len(merchants_all)}")

    api_key=resolve_key()
    # ensure files exist, append mode
    OUT.parent.mkdir(parents=True, exist_ok=True)
    jsonl_path=OUT.with_suffix(".jsonl")
    audit_path=OUT.with_name("labeled_audit.jsonl")
    # write header if new
    if not OUT.exists():
        with OUT.open("w", newline="", encoding="utf-8") as f:
            csv.writer(f).writerow(["merchant_raw","category_id","amount_cents"])

    # approved examples for few-shot (loaded from existing file + in-memory as we approve)
    approved: List[Tuple[str,str]] = []
    if OUT.exists():
        with OUT.open(encoding="utf-8") as f:
            for line in csv.DictReader(f):
                mer=line.get("merchant_raw","").strip()
                cat=line.get("category_id","").strip()
                if mer and cat:
                    approved.append((mer, cat))
    print(f"Few-shot context: {len(approved)} approved examples available, sending up to {args.few_shot} relevant per batch")
    if args.auto_threshold:
        print(f"Auto-approve: batches where all conf >= {args.auto_threshold} will skip prompt")

    total=len(merchants_all)
    for start in range(0, total, args.batch):
        chunk=merchants_all[start:start+args.batch]
        few_shot = get_relevant_examples(chunk, approved, k=args.few_shot) if args.few_shot else []
        if few_shot:
            print(f"  Few-shot: {len(few_shot)} relevant approved → e.g., {few_shot[0][0][:30]} → {few_shot[0][1]}")
        print(f"\n{'='*110}\nBatch {start//args.batch + 1}/{(total+args.batch-1)//args.batch} — [{start+1}-{start+len(chunk)}/{total}] — calling {args.model} (reasoning={args.reasoning_effort}, few_shot={len(few_shot)}) ...")
        preds, audit = call_llm(api_key, args.model, chunk, few_shot, args.reasoning_effort, args.reasoning_budget)
        print_batch(chunk, preds, audit, args.threshold)

        # auto-approve if all high confidence (use approved batches as ground truth, no need to ask)
        if args.auto_threshold and preds:
            min_conf = min(float(p.get("confidence",0) or 0) for p in preds)
            if min_conf >= args.auto_threshold:
                print(f"✓ Auto-approved — all conf >= {args.auto_threshold} (min {min_conf:.2f}), using few-shot context")
                # skip prompt, go straight to save
                # fall through to save block
            else:
                # low-conf → prompt
                low = [p for p in preds if float(p.get("confidence",1) or 1) < args.threshold]
                if low:
                    print(f"\n⚠ {len(low)} low-confidence predictions (<{args.threshold}) — review these closely!")
                # prompt loop
                while True:
                    try:
                        ans=input("\n[Enter]=approve all | e=edit | s=skip batch | q=quit & save > ").strip().lower()
                    except (EOFError, KeyboardInterrupt):
                        ans="q"
                    if ans in ("", "y", "yes", "a", "approve"):
                        break
                    elif ans=="s":
                        print("Skipped batch — not saved.")
                        preds=[]
                        break
                    elif ans=="q":
                        print("Quitting — progress saved.")
                        return
                    elif ans.startswith("e"):
                        print(f"Edit: type 'INDEX CATEGORY' e.g. '3 groceries' or '0 __exclude' — categories: {', '.join(CATEGORIES)}")
                        print("Empty line to finish edits.")
                        while True:
                            try:
                                line=input("edit> ").strip()
                            except (EOFError, KeyboardInterrupt):
                                line=""
                            if not line: break
                            parts=line.split()
                            if len(parts)<2:
                                print("  usage: 3 groceries")
                                continue
                            try: idx=int(parts[0])
                            except: print("  index must be number"); continue
                            new_cat=parts[1].lower()
                            if new_cat not in CATEGORIES:
                                print(f"  invalid category — choose from {', '.join(CATEGORIES)}")
                                continue
                            tid=f"t{idx}"
                            for p in preds:
                                if p.get("txn_id")==tid:
                                    old=p.get("category_id")
                                    p["category_id"]=new_cat
                                    p["confidence"]=0.99
                                    p["rationale"]=f"human corrected: {old} → {new_cat}"
                                    print(f"  {idx}: {old} → {new_cat} ✓")
                                    break
                            else:
                                print(f"  no txn_id {tid}")
                        print_batch(chunk, preds, audit, args.threshold)
                        continue
                    else:
                        print("  [Enter]/y=approve, e=edit, s=skip, q=quit")
                    continue
        # append approved preds to files (flush per batch — auditable & resumable, also feeds few-shot)
        if not preds:
            continue
        with OUT.open("a", newline="", encoding="utf-8") as f:
            w=csv.writer(f)
            for i, (mer, amt) in enumerate(chunk):
                p=next((x for x in preds if x.get("txn_id")==f"t{i}"), None)
                if not p: continue
                w.writerow([mer, p.get("category_id",""), amt])
                approved.append((mer, p.get("category_id","")))
        with jsonl_path.open("a", encoding="utf-8") as f:
            for i, (mer, amt) in enumerate(chunk):
                p=next((x for x in preds if x.get("txn_id")==f"t{i}"), None)
                if not p: continue
                f.write(json.dumps({"merchant_raw":mer,"category_id":p.get("category_id"),"amount_cents":amt,"confidence":p.get("confidence"),"rationale":p.get("rationale")})+"\n")
        with audit_path.open("a", encoding="utf-8") as f:
            f.write(json.dumps({"batch": start//args.batch, "merchants":[m for m,_ in chunk], "preds":preds, "audit":audit, "few_shot": few_shot})+"\n")
        print(f"✓ Saved {len(preds)} rows → {OUT} (+ audit, few-shot={len(few_shot)})")

    print(f"\nDone — {total} merchants labeled. Next: python poc/fasttext_categorizer/prepare.py && python poc/fasttext_categorizer/train.py")

if __name__=="__main__":
    main()
