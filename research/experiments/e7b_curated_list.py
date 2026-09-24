"""E7b: can the EFF large list be curated so no single typo yields another valid word?"""
import math, statistics
from e7_invite_codes import load, edits1

words = [w for w in load("eff_large_wordlist.txt") if "-" not in w]
HOMOPHONES = {"cash","capitol","carrot","canon","shown","flyer","cache","capital","carat","cannon","shone","flier"}
words = [w for w in words if w not in HOMOPHONES]
s = set(words)
# Greedy: keep a word only if none of its edit-1 neighbors is already kept (prefer shorter words first).
kept = []
kept_set = set()
for w in sorted(words, key=lambda x: (len(x), x)):
    if not (edits1(w) & kept_set):
        kept.append(w); kept_set.add(w)
bits = math.log2(len(kept))
def uniq_prefix(w, pool):
    for k in range(1, len(w) + 1):
        if sum(1 for x in pool if x.startswith(w[:k])) == 1:
            return k
    return len(w) + 1
pref = [uniq_prefix(w, kept) for w in kept[:3000]]  # sample for speed
print(f"curated large list: {len(kept)} words, {bits:.2f} bits/word, mean length {statistics.mean(len(w) for w in kept):.1f}")
for n in (6, 7):
    print(f"  {n} words: {n*bits:.1f} bits")
print(f"  unique prefix (sample of 3000): mean {statistics.mean(pref):.2f}, max {max(pref)}")
