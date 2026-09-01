"""
Shim for Rust categorize::builtin_category — minimal keyword map for POC.
For full map see crates/finsight-core/src/categorize.rs KEYWORD_MAP.
"""
import re

KEYWORD_MAP = [
    ("uber eats", "dining"),
    ("doordash", "dining"),
    ("tim hortons", "dining"),
    ("starbucks", "dining"),
    ("mcdonald", "dining"),
    ("chipotle", "dining"),
    ("subway", "dining"),
    ("safeway", "groceries"),
    ("save on foods", "groceries"),
    ("whole foods", "groceries"),
    ("costco", "groceries"),
    ("no frills", "groceries"),
    ("shell", "transport"),
    ("chevron", "transport"),
    ("petro", "transport"),
    ("uber", "transport"),
    ("lyft", "transport"),
    ("evo car", "transport"),
    ("compass", "transport"),
    ("netflix", "subscriptions"),
    ("spotify", "subscriptions"),
    ("openai", "subscriptions"),
    ("prime member", "subscriptions"),
    ("amazon", "shopping"),
    ("walmart", "shopping"),
    ("sport chek", "shopping"),
    ("air canada", "travel"),
    ("westjet", "travel"),
    ("hotel", "travel"),
    ("bc hydro", "utilities"),
    ("fortisbc", "utilities"),
    ("lightspeed", "utilities"),
    ("freedom mobile", "utilities"),
    ("shaw", "utilities"),
    ("coinamatic", "housing"),
    ("capreit", "housing"),
    ("broadstreet", "housing"),
]

def builtin_category(merchant_raw: str):
    m = merchant_raw.lower()
    for kw, cat in KEYWORD_MAP:
        if kw in m:
            return cat
    return None
