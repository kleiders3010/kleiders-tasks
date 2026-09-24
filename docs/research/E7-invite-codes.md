# E7: Invite-code word lists (measured)

Date: 2026-09-24. Code: `research/experiments/e7_invite_codes.py` (EFF lists fetched from eff.org).

## Results
| | EFF large (current design) | EFF short 2.0 |
|---|---|---|
| Words | 7,776 | 1,296 |
| Bits per word | 12.92 | 10.34 |
| Mean word length | 7.0 | 7.3 |
| Words where **one typo produces another valid word** | **2,647 (34%)** | **0 (0%)** |
| Unique prefix | 2–8 letters (mean 5.3) | **3 letters for every word** |
| 6-word code with at least one typo-ambiguous word | **91.8%** | 0% |

Code length options:
| Code | Bits | Characters to type in full | Keystrokes with 3-letter autocomplete |
|---|---|---|---|
| 6 × large (current) | 77.5 | ~47 | n/a (prefixes not unique) |
| 7 × short | 72.4 | ~57 | 21 |
| 8 × short | 82.7 | ~66 | 24 |

## Finding: the current design makes typos silent
With the large list, a single typo in 92% of codes can turn a word into **another valid word**. The app can't tell locally that anything is wrong: it derives a different key, finds no invite, and can only say "not found". The user doesn't know which word is wrong. Correcting by trying typo variants costs one Argon2 derivation per candidate (0.2–0.8 s each on a phone), which is impractically slow.

The EFF short list 2.0 was designed against exactly this: every word is identified by its first 3 letters, and no single typo produces another valid word. So:
- the app **autocompletes after 3 letters** and shows the full word. The user sees mistakes immediately;
- an unknown prefix is rejected on the spot;
- reading aloud is also easier (words were chosen to be distinct).

## Proposal
**8 words from the EFF short list 2.0, with 3-letter autocomplete.**
- **Security:** 82.7 bits, *stronger* than the current 77.5 bits: ~30× more work for an attacker.
- **Typing:** 24 keystrokes instead of ~47.
- **Typos:** detected on each word as it's typed, never silently.
- **Cost:** 8 words to read aloud instead of 6. Mitigated by the words' distinctness and the display in groups (e.g. 2 lines of 4).

With A1's measured rates (m=64 MiB t=3, 100 × RTX 3080, 3 M scraped invites), 82.7 bits → chance of cracking *any* invite in a year ≈ 1 in 1,000,000.

7 short words (72.4 bits) would be ~34× *weaker* than the current design (≈1 in 900 per year under the same attacker model). Doubling Argon2 memory only buys ~2×, so 7 words is not recommended.
