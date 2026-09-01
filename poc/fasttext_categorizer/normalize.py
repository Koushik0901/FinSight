"""
Python port of Rust merchant normalization + LLM redaction.

Must stay in sync with:
  - crates/finsight-core/src/merchant.rs::normalize_merchant
  - crates/finsight-core/src/categorize.rs::redact_for_llm
  - crates/finsight-core/src/merchant.rs::split_on_double_space

Tests at bottom verify parity on samples.
"""
import re
from typing import List

PAYMENT_PREFIXES = [
    "paypal *", "paypal*", "sq *", "sq*", "tst-", "tst*", "bam*", "pp*", "pos ",
]

NOISE_RE = re.compile(r"^(https?://|www\.|\d{3}-\d{3}-\d{4}|\d{6,})$")

# Keep structural tokens for transfer redaction (Rust: TRANSFER_STRUCTURAL_TOKENS)
TRANSFER_STRUCTURAL = {
    "e-transfer", "e", "transfer", "interac", "email", "money",
    "fulfill", "request", "internet", "banking", "bank", "payment",
    "preauthorized", "debit", "credit", "deposit", "withdrawal", "eft",
    "electronic", "funds",
    "#",  # masked digit runs
}

NAMED_TRANSFER_HINTS = ["e-transfer", "e transfer", "interac", "email money transfer", "fulfill request"]


def split_on_double_space(raw: str) -> str:
    """Rust split_on_double_space: text before first run of 2+ spaces."""
    m = re.search(r"\s{2,}", raw)
    if m:
        return raw[: m.start()]
    return raw


def is_noise_token(tok: str) -> bool:
    t = tok.lower()
    if NOISE_RE.match(t):
        return True
    # long digit runs
    if re.fullmatch(r"\d{4,}", t):
        return True
    # urls
    if t.startswith("http") or t.startswith("www."):
        return True
    return False


def normalize_merchant(raw: str) -> str:
    """Port of Rust normalize_merchant. Returns lowercase 1-3 token key."""
    head = split_on_double_space(raw)
    s = head.lower()

    for prefix in PAYMENT_PREFIXES:
        if s.startswith(prefix):
            stripped = s[len(prefix):].lstrip("*").strip()
            s = stripped

    # tokenize on whitespace, '/', ',', '*'
    parts = re.split(r"[\s/,*]+", s)
    tokens: List[str] = []
    for tok in parts:
        tok = tok.strip(" -_.,;:!()[]\"'")
        # strip non-alphanumeric edges
        tok = re.sub(r"^[^a-z0-9]+|[^a-z0-9]+$", "", tok)
        if not tok:
            continue
        if is_noise_token(tok):
            continue
        tokens.append(tok)

    tokens = tokens[:3]
    joined = " ".join(tokens)
    if not joined:
        # fallback cleaned whole descriptor
        return " ".join(head.lower().split())
    return joined


def redact_for_llm(merchant_raw: str) -> str:
    """Port of Rust redact_for_llm: mask digit runs >=4 to '#', drop names from e-transfer."""
    # 1) mask long digit runs
    masked_chars = []
    digits = ""
    for ch in merchant_raw:
        if ch.isdigit():
            digits += ch
        else:
            if digits:
                masked_chars.append("#" if len(digits) >= 4 else digits)
                digits = ""
            masked_chars.append(ch)
    if digits:
        masked_chars.append("#" if len(digits) >= 4 else digits)
    masked = "".join(masked_chars)

    lower = masked.lower()
    is_named = any(h in lower for h in NAMED_TRANSFER_HINTS)
    if not is_named:
        return masked

    out = []
    for tok in masked.split():
        core = "".join(c for c in tok if c.isalpha())
        if not core:
            out.append(tok)
            continue
        if core.lower() in TRANSFER_STRUCTURAL:
            out.append(tok)
        else:
            # drop name token
            continue
    joined = " ".join(out)
    return joined.strip() or "E-TRANSFER"


def merchant_for_training(merchant_raw: str, amount_cents: int | None = None) -> str:
    """Single text field fed to fastText: normalized merchant + amount bucket token."""
    base = normalize_merchant(redact_for_llm(merchant_raw))
    if amount_cents is None:
        return base
    if amount_cents > 0:
        bucket = "income"
    elif amount_cents >= -2000:
        bucket = "small"
    elif amount_cents >= -10000:
        bucket = "medium"
    else:
        bucket = "large"
    return f"{base} __amount_{bucket}"


if __name__ == "__main__":
    # quick parity checks
    cases = [
        ("UBER EATS               TORONTO", "uber eats"),
        ("PAYPAL *STARBUCKSCO     8002352883", "starbucksco"),
        ("TIM HORTONS #3356       BURNABY", "tim hortons"),
        ("OPENAI *CHATGPT SUBSCR  SAN FRANCISCO", "openai chatgpt subscr"),
        ("Internet Banking E-TRANSFER 106001023942 Swathi", "E-TRANSFER"),  # redacted
        ("Lyft   *STANDARD 07-22  VANCOUVER", "lyft standard"),
    ]
    for raw, expect in cases:
        got = normalize_merchant(raw) if "E-TRANSFER" not in expect else redact_for_llm(raw)
        # for transfer case check redact
        if "E-TRANSFER" in expect:
            got = redact_for_llm(raw)
        else:
            got = normalize_merchant(redact_for_llm(raw))
        status = "OK" if got == expect or expect in got else f"GOT:{got}"
        print(f"{raw!r:50} -> {got!r:30} {status}")
