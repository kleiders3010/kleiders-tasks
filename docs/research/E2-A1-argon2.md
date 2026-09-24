# E2 + A1: Argon2id cost for defenders vs attackers (measured)

Date: 2026-09-24. Code: `research/experiments/e2_argon2_timing.py`; attacker measurement with hashcat 7.1.2 (mode 34000), run locally against our own test hashes.

## Defender cost (this PC, 16 threads; single join = one derivation)
| Parameters | Desktop median | Low-end phone est. (×4) |
|---|---|---|
| m=19 MiB t=2 p=1 (OWASP minimum) | 37 ms | ~0.15 s |
| m=64 MiB t=3 p=4 (RFC 9106 option 2) | 87 ms | ~0.35 s |
| m=64 MiB t=3 p=1 | 190 ms | ~0.76 s |
| m=128 MiB t=3 p=1 | 398 ms | ~1.6 s |
| m=256 MiB t=2 p=1 | 346 ms | ~1.4 s |
| m=256 MiB t=4 p=4 | 209 ms | ~0.8 s (only on phones with 4 fast cores) |

The ×4 phone factor is an estimate. It needs a real low-end device (see open items).

## Attacker rate (measured: NVIDIA RTX 3080, OpenCL, hashcat 7.1.2 mask attack, 75 s runs)
| Parameters | Guesses/s per RTX 3080 |
|---|---|
| m=19 MiB t=2 p=1 | 4,018 |
| m=64 MiB t=3 p=1 | 783 |
| m=256 MiB t=2 p=1 | 81 (GPU memory-bound) |

## What it means for 6-word invite codes (7,776⁶ ≈ 2.2×10²³ codes)
Attacker model: every invite ever posted is scraped; each guess is tested against all of them.

| Params | Farm | Invites scraped | Chance of cracking *any* invite per year | 50% time |
|---|---|---|---|---|
| m=64 MiB t=3 | 100 × RTX 3080 (78k/s) | 3 M | ~1 in 30,000 | ~20,000 years |
| m=64 MiB t=3 | 10,000 GPUs (7.8M/s) | 3 M | ~1 in 300 | ~200 years |
| m=256 MiB t=2 | 100 GPUs (8.1k/s) | 3 M | ~1 in 290,000 | ~190,000 years |

**Verdict: the design's estimate held.** It assumed ~1,000 guesses/s per GPU; measured 783 at m=64 MiB t=3. Newer GPUs (RTX 40/50) are perhaps 2–3× faster, which doesn't change the conclusion.

**Decision input:** memory is the lever. 64 MiB is safe on any phone; 256 MiB is 10× more expensive for attackers but risky on 2–3 GB RAM phones (may be killed). Proposed: **m=64 MiB, t=3, p=1 as the floor; raise t if low-end phone timing allows.** Parameters are versioned in the invite format, so they can grow later without breaking old invites.

## Open items
- Measure the real phone factor on a low-end ARM device (emulator timing is not representative).
