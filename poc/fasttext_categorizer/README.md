# FastText POC — Auto-Categorization (replaces LLM A1) — VERIFIED 2026-09-01

**Status:** Trained & wired. `models/merchant_ft.bin` 39 MB, 1212 merchants, 11 labels, `cargo check --features fasttext-local` green, `Rule → fastText ≥0.6 → LLM` pipeline in `crates/finsight-agent/src/categorizer.rs`. `poc/fasttext_categorizer/.venv → ~/.cache/finsight-venvs/fasttext` durable. See `docs/llm-replacement-audit.md` A1.
**Metrics (evaluate.py 2026-09-01):** test 96.1% acc / 0.965 macro-F1 (6%→other), valid 95.0%, heldout 95% gap -0.3%. Latency ~0.0 ms/pred in-process.
## Layout
```
poc/fasttext_categorizer/
  data/
    raw/                  — copies of samples/*.csv (git-ignored large)
    processed/
      labeling_template.csv  — auto-generated for manual labeling
      labeled.csv            — YOU fill this (merchant_raw → category_id)
    synthetic/             — augmented variants (generated)
    fasttext.train         — fastText supervised input (__label__X text)
    fasttext.valid / .test
  normalize.py            — Python port of Rust normalize_merchant + redact_for_llm
  augment.py              — synthetic variant generation (5× per row)
  prepare.py              — CSV → train/valid/test + class balancing
  train.py                — fastText.train_supervised + save .bin
  evaluate.py             — accuracy, macro-F1, per-category, held-out merchant, latency
  notebooks/eval.ipynb    — side-by-side confusion vs LLM (when available)
```

## Quickstart (once labeling done)
```bash
pip install -r poc/fasttext_categorizer/requirements.txt
python poc/fasttext_categorizer/prepare.py   # labeled.csv → fasttext.train/.valid/.test + synthetic
python poc/fasttext_categorizer/train.py     # trains poc/fasttext_categorizer/models/merchant_ft.bin
python poc/fasttext_categorizer/evaluate.py  # prints report + writes data/processed/metrics.json
jupyter notebook poc/fasttext_categorizer/notebooks/eval.ipynb
```

## Workflow you will do
1. `python poc/fasttext_categorizer/prepare.py --make-template` → creates `data/processed/labeling_template.csv` with all unique merchants from `samples/`.
2. Open in Sheets/Excel, fill `category_id` per merchant (use 10 starter IDs: groceries/dining/transport/housing/utilities/subscriptions/shopping/travel/gifts/health, or `__exclude` for transfers). Save as `data/processed/labeled.csv`.
3. Run pipeline above. Synthetic augmentation is automatic (5 variants / row) — check `data/synthetic/` for inspection.
4. Commit `labeled.csv` — the training gold.

## Design notes
- **Normalization parity:** `normalize.py` is a line-for-line port of `crates/finsight-core/src/merchant.rs:normalize_merchant` + `categorize.rs:redact_for_llm`. Must stay in sync; unit tests inside.
- **Amount bucket:** `amount_cents` is concatenated as `__amount_small|medium|large|income` token — cheap but helps fixed bills.
- **Threshold-as-other:** Train 10 core only; `prob < 0.6 → "other"` deterministic fallback (not learned). Keeps `other` from becoming a garbage class; matches `LOW_CONFIDENCE_THRESHOLD`. `__exclude` is separate (transfer).
- **Generalization checks:** held-out merchant split + temporal split + unseen-location split. See `evaluate.py`.
- **Categories:** 10 core (`groceries/dining/transport/housing/utilities/subscriptions/shopping/travel/gifts/health`) + deterministic `other` via threshold. Unknown custom categories stay out of scope for POC.
