# E8: Key epochs without a central orderer (simulation)

Date: 2026-09-24. Code: `research/experiments/e8_epoch_sim.py`. Builds on R7's analysis of key epochs (forks, backdating, ghost members; prior art: DCGKA, Keyhive/BeeKEM, p2panda-encryption, localfirst/auth).

## What was simulated
- 3–8 devices; bursts of concurrent removals and additions; random delivery delays (1–30 steps) and reordering; then quiescence.
- 3,000 randomized runs.
- Epochs form a graph (a new epoch has one or more parent epochs). Each epoch carries its recipient set and the removals known to its creator.

**Rules under test:**
1. **Strong remove:** a removal yields a new epoch excluding the target. A removed device can never be re-added.
2. **Fork merge:** a device that sees several concurrent head epochs, and is the deterministic merger (lowest device ID in the intersection), creates one successor. Its recipients are the **intersection** of the heads' recipients minus every removal it knows. Identical merges get identical IDs, so duplicates collapse.
3. **Valid additions only:** an addition must be signed by a recipient of the *current head* epoch. Holders of an old or leaked key can't add a ghost.
4. **Every new epoch (removal, merge or addition) excludes all removals its creator knows about.**

**Invariants checked:**
- I1: no epoch created by a device that knew of a removal lists the removed device as a recipient.
- I2: after quiescence, all remaining devices share one head epoch.
- I3: no ghost additions via stale keys.

## Results
| Version | Result |
|---|---|
| First run (additions copied the old recipient list) | **441 / 3000 runs violated I1**: an addition made concurrently with a removal re-included the removed device |
| With rule 4 | **3000 / 3000 OK** (I1, I2, I3 hold) |
| Groups emptied by mutual removal | 38 / 3000 (random removals; e.g. A removes B while B removes A, in a 2-member group) |

## What this shows
- The failure mode R7 predicted is real and easy to hit: concurrent add + remove resurrected removed devices in 15% of runs. Rule 4 (every new epoch subtracts all known removals) is **necessary**.
- With rules 1–4, forks re-converge and removed devices never receive later keys in this model.
- **Mutual removal empties small groups.** With no roles, two people removing each other both end up outside. That's consistent with "it's personal", but the UI must make it clear.

## Limits of this model (not verified)
- Ghost prevention (rule 3) is enforced at the *sender* in the model. In the real system every *receiver* must validate signatures and head-epoch references. The simulation doesn't test a malicious sender bypassing checks.
- It doesn't model **data** written during a fork, which R7 notes is unavoidably readable by the other branch's removed member until the merge, or **backdated edits** by removed members. That rule (reject the removed author's ops outside the remover's version vector) needs authenticated authorship (A5) and an engine prototype to test.
- No long partitions, device loss mid-rotation, or large groups (rotation message size; R7 notes ~24 KB for 300 devices).

**Confidence:** medium. The rules are consistent with the prior art R7 identifies, and hold under randomized concurrency in a simplified model. A real implementation needs this as a property test in the engine's CI.
