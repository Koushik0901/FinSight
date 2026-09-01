import argparse, csv, json, os, pathlib, sys, time
from collections import Counter
import requests

ROOT = pathlib.Path(__file__).parent
LABELED = ROOT / "data" / "processed" / "labeled.csv"
OUT_SYN = ROOT / "data" / "synthetic" / "llm_augmented.csv"
CATEGORIES = ["groceries","dining","transport","housing","utilities","subscriptions","shopping","travel","gifts","health"]
AUGMENT_SYSTEM_PROMPT = (
    "Role: You are a senior Canadian banking data synthesizer specializing in statement-accurate merchant descriptors.\n\n"
    "Objective: Generate DISTINCT merchant_raw strings for a single spending category that are indistinguishable from real bank/credit statements.\n\n"
    "Details:\n"
    "- Category: {{category}} (one of groceries, dining, transport, housing, utilities, subscriptions, shopping, travel, gifts, health)\n"
    "- Style: exact statement format — vendor name plus optional store number/phone plus double-space city padding (e.g., \"SAVE ON FOODS #2221     BURNABY\", \"TIM HORTONS #3356       BURNABY\", \"BC-HYDRO-BILL-PMNT      800-224-9376\")\n"
    "- Constraints: Canadian vendors only, varied store numbers (#, -, *), varied cities (BURNABY, VANCOUVER, TORONTO, CALGARY, etc.), no duplicates, no vendors from other categories\n"
    "- Housing = rent/property management (CAPREIT, BROADSTREET, CITY OF TORONTO TAX) — NOT hydro/gas\n"
    "- Utilities = hydro/gas/internet/phone (BC HYDRO, FORTISBC, LIGHTSPEED, FREEDOM MOBILE) — NOT rent\n"
    "- Keep each string 8-45 chars, preserve raw casing and spacing artifacts\n\n"
    "Approach step-by-step:\n"
    "1. Review the 3-5 approved few-shot examples for this category and extract vendor tokens, number patterns, and city padding\n"
    "2. Brainstorm 2× the needed count of candidate vendors for this category (avoid cross-category leakage)\n"
    "3. Apply statement formatting: add plausible store numbers (#2221, #3356), double-space city, phone suffixes where realistic\n"
    "4. Deduplicate against examples and among candidates, filter any vendor that belongs to another category\n"
    "5. Validate count and format before emitting\n\n"
    "Examples:\n"
    "Input: Category housing, Examples [{\"merchant_raw\": \"CAPREIT LP               TORONTO\"}, {\"merchant_raw\": \"BROADSTREET PROPERTIES   SASKATOON\"}]\n"
    "Output: [{\"merchant_raw\": \"CAPREIT LP #1045          VANCOUVER\"}, {\"merchant_raw\": \"BOARDWALK RENTALS         CALGARY\"}]\n"
    "Input: Category utilities, Examples [{\"merchant_raw\": \"BC-HYDRO-BILL-PMNT      800-224-9376\"}, {\"merchant_raw\": \"FREEDOM MOBILE          877-946-3184\"}]\n"
    "Output: [{\"merchant_raw\": \"FORTISBC INC             KELOWNA\"}, {\"merchant_raw\": \"SHAW CABLE             VANCOUVER\"}]\n\n"
    "Sense Check: Every output must be statement-realistic, category-pure (housing ≠ utilities), distinct, and JSON-valid. If any candidate violates category purity, discard and regenerate.\n\n"
    "Output format: Valid JSON array only — no markdown, no fences, no commentary — each element {\"merchant_raw\": \"...\"}."
)

def resolve_key():
    k=os.environ.get("OPENROUTER_API_KEY","").strip()
    if k: return k
    raise SystemExit("Missing OPENROUTER_API_KEY")

def call_generate(api_key, model, category, examples, need, reasoning_effort="high", correction=None):
    few = json.dumps([{"merchant_raw": m} for m,_ in examples])
    user_content = f"Generate {need} DISTINCT merchant_raw strings for category '{category}' using these approved examples:\n{few}"
    if correction:
        user_content += f"\n\nHuman correction — regenerate with this feedback: {correction}"
    body={"model":model,"messages":[{"role":"system","content":AUGMENT_SYSTEM_PROMPT},{"role":"user","content":user_content}],"temperature":0.8,"max_tokens": 2048,"include_reasoning": True}
    if reasoning_effort:
        body["reasoning"]={"effort": reasoning_effort}
    headers={"Authorization": f"Bearer {api_key}","HTTP-Referer":"https://finsight.local","X-Title":"FinSight Augment","Content-Type":"application/json"}
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
            if isinstance(reasoning_text, list):
                reasoning_text="\n".join(str(x.get("text") or x) for x in reasoning_text)
            usage=data.get("usage") or {}
            details=usage.get("completion_tokens_details") or {}
            audit={"reasoning": reasoning_text, "content": content, "usage": usage, "reasoning_tokens": details.get("reasoning_tokens") or 0, "response_tokens": usage.get("completion_tokens") or 0, "prompt_tokens": usage.get("prompt_tokens") or 0, "total_tokens": usage.get("total_tokens") or 0, "model": data.get("model") or model, "id": data.get("id")}
            c=content.strip()
            if c.startswith("```"):
                c=c.split("\n",1)[1] if "\n" in c else c
                if c.endswith("```"): c=c[:-3]
                c=c.strip()
            s=c.find("["); e=c.rfind("]")
            if s==-1 or e==-1: raise ValueError(f"No JSON array: {c[:500]}")
            arr=json.loads(c[s:e+1])
            seen=set(); out=[]
            for obj in arr:
                mer=(obj.get("merchant_raw") or "").strip()
                if not mer or mer in seen: continue
                seen.add(mer)
                out.append(mer)
                if len(out)>=need: break
            return out, audit
        except Exception as e:
            if attempt==2: raise
            print(f"  retry {attempt+1}/3: {e}", file=sys.stderr)
            time.sleep(2**attempt*2)
    return [], {}

def print_generated(category, generated):
    print("\n" + "-"*90)
    print(f"Generated for '{category}' — {len(generated)} merchants:")
    print("-"*90)
    for i, mer in enumerate(generated):
        print(f"{i:<3} {mer}")
    print("-"*90)

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--model", default="moonshotai/kimi-k3")
    ap.add_argument("--reasoning-effort", default="high", choices=["low","medium","high"])
    ap.add_argument("--target", type=int, default=50, help="min per category after augment")
    ap.add_argument("--limit", type=int, default=0, help="only this many categories (0=all thin)")
    ap.add_argument("--hitl", action="store_true", help="HITL review per category — approve/edit/regenerate with corrections")
    args=ap.parse_args()
    if not LABELED.exists():
        raise SystemExit(f"Missing {LABELED}")
    rows=list(csv.DictReader(LABELED.open(encoding="utf-8")))
    cnt=Counter(r["category_id"] for r in rows if r["category_id"]!="__exclude")
    print("Current:", dict(cnt))
    by_cat={}
    for r in rows:
        cat=r["category_id"]
        if cat=="__exclude": continue
        by_cat.setdefault(cat, []).append((r["merchant_raw"], cat))
    thin=[(cat, cnt.get(cat,0)) for cat in CATEGORIES if cnt.get(cat,0) < args.target]
    thin=[x for x in thin if x[0]!="__exclude"]
    if args.limit: thin=thin[:args.limit]
    print(f"Thin categories (<{args.target}): {thin}")
    if not thin:
        print("All categories already >= target, nothing to generate")
        return
    api_key=resolve_key()
    OUT_SYN.parent.mkdir(parents=True, exist_ok=True)
    existing=set()
    if OUT_SYN.exists():
        with OUT_SYN.open(encoding="utf-8") as f:
            for line in csv.DictReader(f):
                existing.add(line["merchant_raw"])
    total_new=0
    # open once, keep header
    write_header = not OUT_SYN.exists() or OUT_SYN.stat().st_size==0
    with OUT_SYN.open("a", newline="", encoding="utf-8") as out_f:
        w=csv.writer(out_f)
        if write_header:
            w.writerow(["merchant_raw","category_id","amount_cents","source"])
        for cat, cur in thin:
            need=args.target - cur
            if need<=0: continue
            examples=by_cat.get(cat, [])
            if not examples:
                examples=[(f"Sample {cat} merchant", cat)]
            print(f"\nGenerating {need} for '{cat}' (have {cur}) — examples: {len(examples)} few-shot (e.g., {[m[:30] for m,_ in examples[:2]]}) ...")
            generated, audit = call_generate(api_key, args.model, cat, examples, need, args.reasoning_effort)
            print(f"  got {len(generated)} — reasoning_tokens={audit.get('reasoning_tokens',0)} response={audit.get('response_tokens',0)}")
            # HITL loop
            if args.hitl:
                while True:
                    print_generated(cat, generated)
                    # show audit reasoning snippet
                    if audit.get("reasoning"):
                        print(f"\n[Reasoning] {audit['reasoning'][:300]}...")
                    try:
                        ans=input("\n[Enter]=approve | e=edit | r=regenerate with correction | s=skip | q=quit > ").strip().lower()
                    except (EOFError, KeyboardInterrupt):
                        ans="q"
                    if ans in ("", "y", "yes", "a"):
                        break
                    elif ans=="s":
                        print("Skipped category.")
                        generated=[]
                        break
                    elif ans=="q":
                        print("Quitting — progress saved.")
                        # write what we have and exit
                        for mer in generated:
                            if mer in existing: continue
                            amt=-1500 if cat in ("dining","groceries") else -3000
                            if cat in ("housing","utilities"): amt=-80000
                            if cat=="transport": amt=-2500
                            w.writerow([mer, cat, amt, "llm"])
                            existing.add(mer)
                            total_new+=1
                        # also write audit
                        audit_path = OUT_SYN.with_name("llm_augmented_audit.jsonl")
                        with audit_path.open("a", encoding="utf-8") as af:
                            af.write(json.dumps({"category": cat, "need": need, "examples": examples[:3], "generated": generated[:3], "audit": audit}) + "\n")
                        print(f"\nWrote {total_new} new LLM-augmented rows to {OUT_SYN}")
                        return
                    elif ans.startswith("e"):
                        print("Edit: type 'INDEX NEW_MERCHANT' e.g. '2 STARBUCKS #123 VANCOUVER' — empty line to finish")
                        while True:
                            try:
                                line=input("edit> ").strip()
                            except: line=""
                            if not line: break
                            parts=line.split(maxsplit=1)
                            if len(parts)<2:
                                print("  usage: 2 NEW MERCHANT STRING")
                                continue
                            try: idx=int(parts[0])
                            except: print("  index must be number"); continue
                            if 0 <= idx < len(generated):
                                old=generated[idx]
                                generated[idx]=parts[1]
                                print(f"  {idx}: {old} → {parts[1]} ✓")
                            else:
                                print(f"  index out of range 0-{len(generated)-1}")
                        continue
                    elif ans.startswith("r"):
                        try:
                            corr=input("correction for LLM (e.g., 'avoid hydro for housing, more diverse cities'): ").strip()
                        except: corr=""
                        if not corr: corr="Ensure diversity and category purity, avoid previous mistakes."
                        print(f"  Regenerating '{cat}' with correction: {corr}")
                        generated, audit = call_generate(api_key, args.model, cat, examples, need, args.reasoning_effort, correction=corr)
                        print(f"  got {len(generated)} — reasoning_tokens={audit.get('reasoning_tokens',0)}")
                        continue
                    else:
                        print("  [Enter]/y=approve, e=edit, r=regenerate, s=skip, q=quit")
            # auditable: store reasoning per category
            audit_path = OUT_SYN.with_name("llm_augmented_audit.jsonl")
            with audit_path.open("a", encoding="utf-8") as af:
                af.write(json.dumps({"category": cat, "need": need, "examples": examples[:3], "generated": generated[:3], "audit": audit}) + "\n")
            for mer in generated:
                if mer in existing: continue
                amt=-1500 if cat in ("dining","groceries") else -3000
                if cat in ("housing","utilities"): amt=-80000
                if cat=="transport": amt=-2500
                w.writerow([mer, cat, amt, "llm"])
                existing.add(mer)
                total_new+=1
            time.sleep(0.5)
    print(f"\nWrote {total_new} new LLM-augmented rows to {OUT_SYN}")
    new_cnt=dict(cnt)
    if OUT_SYN.exists():
        with OUT_SYN.open(encoding="utf-8") as f:
            for line in csv.DictReader(f):
                new_cnt[line["category_id"]]=new_cnt.get(line["category_id"],0)+1
    print("After LLM augment would be:", {k: new_cnt.get(k,0) for k in CATEGORIES})

if __name__=="__main__":
    main()
