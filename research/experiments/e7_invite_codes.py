"""E7: invite-code word list properties: entropy, length, typo tolerance.

Questions:
- How long are 6-word codes to type/read?
- Can a single-letter typo turn one valid word into another valid word
  (undetectable locally, only discovered as "invite not found")?
- Are words prefix-unique (autocomplete after N letters)?
- Does a checksum help?
"""
import math
import statistics
from collections import Counter


def load(path):
    return [line.split()[1] for line in open(path, encoding="utf-8") if line.strip()]


def edits1(word, alphabet="abcdefghijklmnopqrstuvwxyz-"):
    splits = [(word[:i], word[i:]) for i in range(len(word) + 1)]
    deletes = [a + b[1:] for a, b in splits if b]
    transposes = [a + b[1] + b[0] + b[2:] for a, b in splits if len(b) > 1]
    replaces = [a + c + b[1:] for a, b in splits if b for c in alphabet]
    inserts = [a + c + b for a, b in splits for c in alphabet]
    return set(deletes + transposes + replaces + inserts) - {word}


def analyze(name, words):
    s = set(words)
    lens = [len(w) for w in words]
    bits = math.log2(len(words))
    neighbors = sum(1 for w in words if edits1(w) & s)
    # shortest unique prefix length per word
    def uniq_prefix(w):
        for k in range(1, len(w) + 1):
            p = w[:k]
            if sum(1 for x in words if x.startswith(p)) == 1:
                return k
        return len(w) + 1  # word is a prefix of another word
    prefixes = [uniq_prefix(w) for w in words]
    prefix_of_other = sum(1 for p, w in zip(prefixes, words) if p > len(w))
    print(f"\n## {name}: {len(words)} words, {bits:.2f} bits/word")
    print(f"- word length: mean {statistics.mean(lens):.1f}, max {max(lens)}")
    for n in (4, 5, 6, 7, 8):
        print(f"- {n} words: {n*bits:.1f} bits, ~{n*statistics.mean(lens)+n-1:.0f} characters typed")
    print(f"- words with a valid neighbor 1 typo away (typo turns into another valid word): {neighbors} ({neighbors/len(words):.1%})")
    print(f"- unique prefix length: mean {statistics.mean(prefixes):.2f}, distribution {sorted(Counter(min(p,8) for p in prefixes).items())}")
    print(f"- words that are a prefix of another word: {prefix_of_other}")
    six = 1 - (1 - neighbors / len(words)) ** 6
    print(f"- chance a 6-word code contains at least one word that a single typo maps to another valid word: {six:.1%}")


large = load("eff_large_wordlist.txt")
short = load("eff_short_wordlist_2_0.txt")
analyze("EFF large", large)
analyze("EFF short 2.0", short)
