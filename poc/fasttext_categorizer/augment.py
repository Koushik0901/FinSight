"""
Synthetic augmentation for merchant strings.

For each (merchant_raw, category_id, amount_cents) we emit 5 variants:
  1. original
  2. case jitter
  3. location swap / drop
  4. store-number jitter
  5. whitespace / punctuation jitter

Keeps category_id stable.
"""
import random
import re
from typing import Iterable, Tuple

LOCATIONS = ["BURNABY", "VANCOUVER", "TORONTO", "SURREY", "EDMONTON", "COQUITLAM", "HALIFAX", "CALGARY"]
NOISE_NUMS = ["1147", "2221", "3356", "9999", "7046", "3008", "39038"]

random.seed(42)


def _case_jitter(s: str) -> str:
    # Title case with 50% chance
    if random.random() < 0.5:
        return s.title()
    return s.lower()


def _location_jitter(s: str) -> str:
    # replace trailing location token OR drop it
    parts = re.split(r"\s{2,}", s)
    head = parts[0]
    tail = parts[1] if len(parts) > 1 else ""
    if not tail.strip():
        # no double-space tail — try last token
        tokens = s.strip().split()
        if tokens and tokens[-1].isupper():
            tokens[-1] = random.choice(LOCATIONS)
            return " ".join(tokens)
        return s
    # swap tail location
    if random.random() < 0.3:
        return head  # drop location
    return head + "  " + random.choice(LOCATIONS)


def _number_jitter(s: str) -> str:
    # replace # + digits run
    def repl(m):
        return "#" + random.choice(NOISE_NUMS)

    s2, n = re.subn(r"#\s*\d+", repl, s)
    if n == 0:
        # inject a number if has recognizable store pattern
        if re.search(r"\d{4,}", s) is None and random.random() < 0.4:
            # append a fake store id with double-space
            return s + "  " + random.choice(NOISE_NUMS)
    return s2


def _whitespace_jitter(s: str) -> str:
    # collapse or expand spaces, add stray '*'
    s = re.sub(r"\s+", " ", s.strip())
    if random.random() < 0.3:
        s = s.replace(" *", "*").replace("* ", "*")
    if random.random() < 0.2:
        s = s.replace(" ", "  ")
    return s


def augment_row(merchant_raw: str, category_id: str, amount_cents: int = 0) -> Iterable[Tuple[str, str, int]]:
    merchants = [
        merchant_raw,
        _case_jitter(merchant_raw),
        _location_jitter(merchant_raw),
        _number_jitter(merchant_raw),
        _whitespace_jitter(merchant_raw),
    ]
    for m in merchants:
        yield (m, category_id, amount_cents)


def augment_dataset(rows: Iterable[Tuple[str, str, int]]) -> list:
    out = []
    for mer, cat, amt in rows:
        if cat in ("__exclude", "exclude", "transfer"):
            # don't augment transfers
            out.append((mer, cat, amt))
            continue
        out.extend(augment_row(mer, cat, amt))
    return out


if __name__ == "__main__":
    demo = [
        ("TIM HORTONS #3356       BURNABY", "dining", -621),
        ("UBER EATS               TORONTO", "dining", -3194),
        ("OPENAI *CHATGPT SUBSCR  SAN FRANCISCO", "subscriptions", -2800),
    ]
    for mer, cat, amt in demo:
        print(f"ORIG: {mer}")
        for var, c, a in augment_row(mer, cat, amt):
            print("  ->", var)
        print()
