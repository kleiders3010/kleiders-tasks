# E5: Helper-network simulation

Date: 2026-09-24. Code: `research/experiments/e5_helper_sim.py` (2,000 helpers, 14 simulated days, hourly steps).

**Model:**
- Desktops are online 50% of the time (8 h sessions); phones on a charger 30% (6 h sessions).
- **Every restart wipes a helper's RAM.**
- The writer uploads to the online helpers in its group's set, and helpers copy to each other when online together.
- "Member refresh" = some member device is awake in a given hour and re-uploads the slot.
- A reader tries once per hour for 24 h after the write.
- **Not modeled** (so real availability is higher): member devices as mailboxes (tier 1), and Nostr in parallel.

## Probability a reader finds the latest version
| Scenario | r=2 | r=4 | r=6 | r=10 | r=15 | r=20 | r=30 |
|---|---|---|---|---|---|---|---|
| 30% desktops, members refresh 50%/h | 53% | 77% | 89% | 97–98% | 99.6% | ~100% | 100% |
| 30% desktops, **no refresh** (all members offline) | 20% | 41% | 63% | 81% | 96% | 99% | 100% |
| 10% desktops, members refresh | 46% | 73% | 88% | 97% | 99% | 99.8% | 100% |
| 10% desktops, no refresh | 15% | 31% | 48% | 80% | 93% | 97% | 100% |
| 30% desktops, reliability-weighted selection, no refresh | 22% | 39% | 64% | 90% | 99% | 99% | 100% |

## Dishonest helpers (accept and silently drop), with member refresh (1,000 groups)
| Scenario | r=6 | r=10 | r=15 | r=20 | r=30 |
|---|---|---|---|---|---|
| none | 89% | 97% | 99.7% | 99.9% | 100% |
| 30% bad, no reputation | 79% | 93% | 98.4% | 99.8% | 100% |
| 30% bad, **with reputation** | 85% | 96% | 99.1% | 99.7% | 100% |
| 50% bad, no reputation | 69% | 85% | 96% | 98% | 99.9% |
| 50% bad, **with reputation** | 79% | 92% | 97% | 99.5% | 99.9% |

(Sampling noise ≈ ±1 point.)

## Verdicts
- **"12–30 copies" is confirmed, but the low end is optimistic.** RAM-only phone-heavy helpers need **r ≈ 15–20** for ~99% when no member is refreshing. Start at **r = 20** and let adaptation lower it where measured reliability allows.
- **Member refresh matters as much as r:** active members keep their group's mailbox alive. The design already has background sync refresh slots. This confirms it's essential, not a nice-to-have.
- **Reliability weighting helps** (r=10: 81% → 90% without refresh).
- **Reputation helps against dishonest helpers:** +6–10 points at r=6–10. At r ≥ 20 the network tolerates even 50% dishonest helpers at ~99.5%.
- Helpers alone never guarantee delivery in the first hour after a write, when few are online. That's why Nostr and member devices run in parallel.

## Reality check from R5 (prior art): participation
This simulation assumed 2,000 helpers. R5 found real volunteer participation is **0.01–0.3%** of users (Syncthing ~100 relays for ~1M devices; Tor ~8k relays for ~2.5M daily users, with active recruitment and funding), not the ~10% §7a implies. At 10k users that's roughly **1–30 helpers**, far too few for r = 20. **The helper network only becomes meaningful at hundreds of thousands of users,** and even then it's the first network of its kind (no precedent serves offline phones with volunteers). R8 adds that phone helpers relaying for strangers conflict with Play's Device & Network Abuse policy (proxy services only allowed when they're the app's core purpose). These points belong in the decision list.

## Follow-ups
- Model tier-1 member mailboxes and Nostr together for end-to-end availability.
- Eclipse is not modeled: helper sets are chosen by reliability, not ID (see A4).
