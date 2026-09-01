"""
Label merchants via SOTA LLM through OpenRouter → ground truth for fastText POC.

Batches 20 merchants per LLM call (mirrors categorizer.rs:188 LLM_BATCH_SIZE).
Uses OpenAI-compatible chat completions, JSON array response, retries, and maps
to 10 core + __exclude. Low-conf LLM rows (<0.6) are kept but flagged.

Usage:
  export OPENROUTER_API_KEY=sk-or-...
  python poc/fasttext_categorizer/label_via_llm.py --model anthropic/claude-sonnet-4 --limit 0
  # writes poc/fasttext_categorizer/data/processed/labeled.csv

Models (pick one, SOTA 2026):
  anthropic/claude-sonnet-4  (best, ~$3/MTok)
  openai/gpt-4o              (strong, cheap)
  google/gemini-2.5-flash    (fast, cached)
  anthropic/claude-3.5-sonnet (fallback)

Key resolution: env OPENROUTER_API_KEY first, else keychain com.finsight.llm/openrouter (same as finsight-eval).
"""
import argparse, csv, json, os, pathlib, sys, time
from typing import List, Tuple

import requests

ROOT = pathlib.Path(__file__).parent
TEMPLATE = ROOT / "data" / "processed" / "labeling_template.csv"
OUT = ROOT / "data" / "processed" / "labeled.csv"

CATEGORIES = [
    ("groceries", "daily", "Groceries"),
    ("dining", "daily", "Dining"),
    ("transport", "daily", "Transport"),
    ("housing", "fixed", "Housing"),
    ("utilities", "fixed", "Utilities"),
    ("subscriptions", "fixed", "Subscriptions"),
    ("shopping", "lifestyle", "Shopping"),
    ("travel", "lifestyle", "Travel"),
    ("gifts", "lifestyle", "Gifts"),
    ("health", "wellbeing", "Health"),
]

SYSTEM_PROMPT = (
    "Role: You are an expert personal finance transaction categorizer with deep knowledge of Canadian/US merchant descriptors, banking statement formats, and payment processor noise.\n\n"
    "Objective: Classify each merchant_raw into exactly one of 10 spending categories or __exclude for internal transfers, with calibrated confidence and a one-sentence rationale.\n\n"
    "Details:\n"
    "- Allowed category_id values (use id exactly, lowercase): groceries, dining, transport, housing, utilities, subscriptions, shopping, travel, gifts, health, __exclude\n"
    "- __exclude = internal money movement only: \"PAYMENT RECEIVED - THANK YOU\", \"INTERNET TRANSFER 000000...\", \"PREAUTHORIZED DEBIT - CREDIT CARD PAYMENT\", \"E-TRANSFER\" between own accounts, payroll deposits that are transfers. When in doubt between __exclude and a spending category, prefer the spending category.\n"
    "- If none of the 10 spending categories fit, choose the semantically closest — do not invent new ids and do not use \"other\".\n"
    "- Input fields per transaction: txn_id, merchant_raw (raw statement text, may contain store numbers, cities, phone numbers, \"  \" padded locations), amount_cents (negative = spend, positive = income, 0 = unknown).\n"
    "- Ignore location/city padding, store numbers (#3356), phone numbers, and URLs — focus on the core vendor name. \"TIM HORTONS #3356 BURNABY\" and \"TIM HORTONS #6270 SURREY\" are the same vendor.\n"
    "- Use amount only as a weak hint: fixed small amounts (~$8-15) suggest subscriptions (NETFLIX, SPOTIFY), large variable amounts suggest groceries/travel.\n\n"
    "Approach step-by-step:\n"
    "1. Normalize the merchant: strip double-space padded location, remove store/phone noise, lowercase.\n"
    "2. Identify the core vendor and match against known patterns (e.g., SAFEWAY/COSTCO → groceries, UBER/LYFT → transport, OPENAI/NETFLIX → subscriptions).\n"
    "3. Check transfer vocabulary first — if it contains explicit internal-transfer phrasing with high precision, assign __exclude.\n"
    "4. Select the single best category_id from the 10, assign confidence 0.0-1.0 (0.9+ = obvious, 0.6-0.8 = plausible but ambiguous, <0.6 = uncertain), and write a one-sentence rationale referencing the vendor cue.\n"
    "5. Validate that every txn_id in the input appears exactly once in the output and that every category_id is from the allowed list.\n\n"
    "Examples:\n"
    "Input: [{\"txn_id\":\"t0\",\"merchant_raw\":\"TIM HORTONS #3356       BURNABY\",\"amount_cents\":-621}]\n"
    "Output: [{\"txn_id\":\"t0\",\"category_id\":\"dining\",\"confidence\":0.95,\"rationale\":\"Tim Hortons is a coffee/dining chain\"}]\n"
    "Input: [{\"txn_id\":\"t1\",\"merchant_raw\":\"PAYMENT RECEIVED - THANK YOU\",\"amount_cents\":-298614}]\n"
    "Output: [{\"txn_id\":\"t1\",\"category_id\":\"__exclude\",\"confidence\":0.99,\"rationale\":\"Credit card payment transfer, not spending\"}]\n"
    "Input: [{\"txn_id\":\"t2\",\"merchant_raw\":\"SAVE ON FOODS #2221     BURNABY\",\"amount_cents\":-1616}]\n"
    "Output: [{\"txn_id\":\"t2\",\"category_id\":\"groceries\",\"confidence\":0.92,\"rationale\":\"Save On Foods is a grocery chain\"}]\n\n"
    "Sense Check: Ensure no invented categories, no missing txn_ids, no hallucinated merchants, and that transfer detection is conservative (only high-precision phrases become __exclude).\n\n"
    "Output format: Valid JSON array only — no markdown, no fences, no commentary outside the array. Each element must be {\"txn_id\":\"...\",\"category_id\":\"...\",\"confidence\":0.0,\"rationale\":\"...\"}."
)

BATCH = 20

def resolve_key() -> str:
    k = os.environ.get("OPENROUTER_API_KEY","").strip()
    if k: return k
    # try keychain (best-effort, may not be available in WSL)
    try:
        import subprocess
        # linux secret-tool fallback — ignore failures
        out = subprocess.check_output(
            ["secret-tool", "lookup", "service", "com.finsight.llm", "account", "openrouter"],
            text=True, stderr=subprocess.DEVNULL, timeout=5
        ).strip()
        if out: return out
    except Exception:
        pass
    raise SystemExit("Missing OPENROUTER_API_KEY. Export it: export OPENROUTER_API_KEY=sk-or-...")

def call_openrouter(api_key: str, model: str, merchants: List[Tuple[str,int]], reasoning_effort: str | None = None, reasoning_budget: int | None = None, retries=3) -> Tuple[List[dict], dict]:
    """Call OpenRouter chat completions, return (parsed JSON array, audit_meta with reasoning + usage)."""
    items = [{"txn_id": f"t{i}", "merchant_raw": m, "amount_cents": amt} for i,(m,amt) in enumerate(merchants)]
    user_prompt = (
        f"Classify these {len(items)} transactions:\n{json.dumps(items)}\n\n"
        "Respond:\n[{\"txn_id\":\"...\",\"category_id\":\"...\",\"confidence\":0.0,\"rationale\":\"one sentence\"}]"
    )
    body = {
        "model": model,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": user_prompt},
        ],
        "temperature": 0.0,
        "max_tokens": 4096,
        "include_reasoning": True,  # auditable: request reasoning tokens when model supports it
    }
    # Reasoning effort — normalized for OpenRouter
    # - OpenAI o1/o3/gpt-5: reasoning.effort = "low"|"medium"|"high"
    # - Anthropic extended thinking: reasoning.max_tokens
    # - OpenRouter unified: reasoning: {effort, max_tokens} or reasoning: {enabled}
    if reasoning_effort or reasoning_budget:
        reasoning = {}
        # OpenRouter allows only one of reasoning.effort vs reasoning.max_tokens per request
        if reasoning_effort:
            reasoning["effort"] = reasoning_effort  # low/medium/high
            # do not also send max_tokens when effort is set
        elif reasoning_budget:
            reasoning["max_tokens"] = reasoning_budget  # e.g., 2000-10000
            reasoning["enabled"] = True
        body["reasoning"] = reasoning
        # also set provider helper for backward compat
        if reasoning_effort and "openai" in model.lower():
            body["reasoning_effort"] = reasoning_effort
    headers = {
        "Authorization": f"Bearer {api_key}",
        "HTTP-Referer": "https://finsight.local",
        "X-Title": "FinSight FastText POC",
        "Content-Type": "application/json",
    }
    for attempt in range(retries):
        try:
            url = "https://openrouter.ai/api/v1/chat/completions"
            resp = requests.post(url, headers=headers, json=body, timeout=120)
            if resp.status_code == 429:
                time.sleep(2 ** attempt * 5)
                continue
            resp.raise_for_status()
            data = resp.json()
            msg = data["choices"][0]["message"]
            content = msg.get("content") or ""
            reasoning_text = msg.get("reasoning") or msg.get("reasoning_details") or ""
            # reasoning_details is often list of {type, text}
            if isinstance(reasoning_text, list):
                reasoning_text = "\n".join(str(x.get("text") or x.get("content") or x) for x in reasoning_text)
            usage = data.get("usage") or {}
            # OpenRouter/OpenAI: usage.completion_tokens_details.reasoning_tokens, prompt_tokens, completion_tokens
            details = usage.get("completion_tokens_details") or {}
            audit = {
                "reasoning": reasoning_text,
                "content": content,
                "usage": usage,
                "reasoning_tokens": details.get("reasoning_tokens") or usage.get("reasoning_tokens") or 0,
                "response_tokens": usage.get("completion_tokens") or 0,
                "prompt_tokens": usage.get("prompt_tokens") or 0,
                "total_tokens": usage.get("total_tokens") or 0,
                "model": data.get("model") or model,
                "id": data.get("id"),
            }
            # strip markdown fences
            content = content.strip()
            if content.startswith("```"):
                content = content.split("\n",1)[1] if "\n" in content else content
                if content.endswith("```"):
                    content = content[:-3]
                content = content.strip()
            # find JSON array bounds
            start = content.find("[")
            end = content.rfind("]")
            if start == -1 or end == -1:
                # try object → array
                raise ValueError(f"No JSON array in response: {content[:500]}")
            arr = json.loads(content[start:end+1])
            return arr, audit
        except Exception as e:
            if attempt == retries-1:
                raise
            print(f"  retry {attempt+1}/{retries} after error: {e}", file=sys.stderr)
            time.sleep(2 ** attempt * 2)
    return [], {}

def load_template(limit: int = 0) -> List[Tuple[str,int]]:
    if not TEMPLATE.exists():
        raise SystemExit(f"Missing {TEMPLATE}. Run prepare.py --make-template first.")
    rows=[]
    with TEMPLATE.open(encoding="utf-8") as f:
        r=csv.DictReader(f)
        for line in r:
            mer=(line.get("merchant_raw") or "").strip()
            if not mer: continue
            try: amt=int(line.get("amount_cents") or 0)
            except: amt=0
            rows.append((mer, amt))
    if limit and limit < len(rows):
        rows = rows[:limit]
    return rows

def main():
    ap=argparse.ArgumentParser()
    ap.add_argument("--model", default="anthropic/claude-sonnet-4", help="OpenRouter model id")
    ap.add_argument("--limit", type=int, default=0, help="only first N merchants (0=all)")
    ap.add_argument("--out", type=str, default=str(OUT))
    ap.add_argument("--batch", type=int, default=BATCH)
    ap.add_argument("--reasoning-effort", type=str, default=None, choices=["low","medium","high"], help="Reasoning effort (OpenAI o1/o3/gpt-5, or unified OpenRouter reasoning.effort)")
    ap.add_argument("--reasoning-budget", type=int, default=None, help="Reasoning max_tokens budget (Anthropic extended thinking, e.g., 2000-10000)")
    ap.add_argument("--dry-run", action="store_true", help="print prompts without calling API")
    args=ap.parse_args()
    merchants = load_template(args.limit)
    print(f"Loaded {len(merchants)} merchants from {TEMPLATE}")
    print(f"Model: {args.model}  Batch: {args.batch}  Out: {args.out}")
    if args.dry_run:
        print(SYSTEM_PROMPT[:500])
        print(f"Would call {len(merchants)//args.batch + 1} batches")
        return
    api_key = resolve_key()
    valid_ids = {c[0] for c in CATEGORIES} | {"__exclude"}
    if args.reasoning_effort or args.reasoning_budget:
        print(f"Reasoning: effort={args.reasoning_effort} budget={args.reasoning_budget}")
    all_results=[]
    audits=[]
    total = len(merchants)
    for i in range(0, total, args.batch):
        chunk = merchants[i:i+args.batch]
        print(f"[{i+1}/{total}] {len(chunk)} merchants ...", flush=True)
        arr, audit = call_openrouter(api_key, args.model, chunk, reasoning_effort=args.reasoning_effort, reasoning_budget=args.reasoning_budget)
        audits.append({"batch": i//args.batch, "merchants": [m for m,_ in chunk], "audit": audit})
        id_to = {f"t{j}": chunk[j] for j in range(len(chunk))}
        for obj in arr:
            tid = obj.get("txn_id","")
            cat = str(obj.get("category_id","")).strip().lower()
            conf = float(obj.get("confidence", 0.7) or 0.7)
            if cat not in valid_ids:
                print(f"  warn: unknown category_id '{cat}' for {tid}, coercing to shopping", file=sys.stderr)
                cat = "shopping"
            mer, amt = id_to.get(tid, ("",0))
            if not mer:
                continue
            all_results.append((mer, cat, amt, conf, obj.get("rationale","")))
        # also attach reasoning to each row's audit for traceability
        time.sleep(0.5)  # be nice to rate limits

    # write labeled.csv (fastText training input) + labeled_full.jsonl with rationales + audit
    out_path = pathlib.Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", newline="", encoding="utf-8") as f:
        w=csv.writer(f)
        w.writerow(["merchant_raw","category_id","amount_cents"])
        for mer, cat, amt, conf, rat in all_results:
            w.writerow([mer, cat, amt])
    full = out_path.with_suffix(".jsonl")
    with full.open("w", encoding="utf-8") as f:
        for mer, cat, amt, conf, rat in all_results:
            f.write(json.dumps({"merchant_raw": mer, "category_id": cat, "amount_cents": amt, "confidence": conf, "rationale": rat})+"\n")
    # auditable reasoning + token log per batch
    audit_path = out_path.with_name("labeled_audit.jsonl")
    with audit_path.open("w", encoding="utf-8") as f:
        for entry in audits:
            f.write(json.dumps(entry)+"\n")
    print(f"\nWrote {len(all_results)} labeled rows to {out_path}")
    print(f"Full rationales: {full}")
    print(f"Auditable reasoning+tokens per batch: {audit_path} (reasoning, response_tokens, reasoning_tokens, prompt_tokens)")
    tot_reason = sum(a["audit"].get("reasoning_tokens",0) for a in audits)
    tot_resp = sum(a["audit"].get("response_tokens",0) for a in audits)
    tot_prompt = sum(a["audit"].get("prompt_tokens",0) for a in audits)
    print(f"Total tokens — prompt: {tot_prompt}, reasoning: {tot_reason}, response: {tot_resp}, total: {tot_prompt+tot_resp+tot_reason}")
    # summary
    from collections import Counter
    cnt=Counter(c for _,c,_,_,_ in all_results)
    print("Distribution:", dict(cnt))
    low=sum(1 for *_,conf,_ in all_results if conf < 0.6)
    print(f"Low confidence (<0.6): {low}/{len(all_results)} ({low/len(all_results):.1%}) would become 'other' via threshold")
    print(f"\nNext: python poc/fasttext_categorizer/prepare.py && python poc/fasttext_categorizer/train.py && python poc/fasttext_categorizer/evaluate.py")
    main()
