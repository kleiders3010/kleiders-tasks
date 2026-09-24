# A2–A4: Attacks on the helper network

Date: 2026-09-24. **Analysis**, grounded in the E5 simulation where noted. The helper network isn't built, so these are design-level checks, not exploit runs.

## A2: Rollback / withholding (serving an old slot version)
- A slot version is signed by its author and carries a monotonically increasing version (Loro version vector + event `created_at`).
- **Readers take the newest version across all sources** (several helpers, Nostr, member devices) and **never go backwards** below a version they've already seen.
- So a stale helper can only **delay** readers that haven't seen the newer version yet. It can't make anyone lose data.
- Measured equivalent: a helper serving stale data is, for that read, the same as a helper that dropped it. E5's dishonest-helper tables therefore apply: with member refresh, r=20 gives ~99.7% even with 30% bad helpers.
- The design's "direct upload → proven withholding" rule stands.
- **Verdict: holds.** Confidence: high for integrity (signatures + monotonic reads), medium for availability numbers (simulation).

## A3: Junk flooding (fake groups filling helper RAM)
Attacker goal: evict real groups' slots from helpers.
- A helper's RAM (64–256 MB) with a per-group quota (256 KB) takes ≥256 junk groups to fill.
- **Eviction priority already favors real groups:**
  - groups with long history on that helper;
  - groups whose slots are actually read.
  - Fresh junk groups are evicted first.
  - To beat this, the attacker must keep thousands of junk groups alive *and read them* for weeks, a sustained cost per helper.
- **Proof of work** on new slots, scaled by helper load, makes mass slot creation cost CPU. PoW mainly buys time; it doesn't stop a determined, well-funded attacker.
- **Gap found:** the design doesn't say how a helper *knows* a read is legitimate. Anyone can read encrypted slots, so an attacker can read its own junk. Only **age and consistency** of use distinguish real groups, and a patient attacker can fake both.
- **Verdict: partially holds.** Flooding degrades helpers, but Nostr and member devices keep delivery working (principle 5), so it's an availability attack on one layer, never data loss.
- **Proposal:** cap per-helper capacity for any group younger than N days, so new groups (real or fake) can't displace established ones quickly. Confidence: medium.

## A4: Eclipse / fake-helper positioning
- **Steady state:** helper sets are chosen by reliability and recorded inside the encrypted group document. An outsider can't learn a group's helper set from the document, and can't get chosen except via probation slots (1 per group, rotating) or by earning reputation.
- **Bootstrap set** ("closest to a daily-rotated hash"): **the hash must be keyed with the group secret**, i.e. H(secret, date). A hash over public data would let an attacker who saw today's lookup key compute tomorrow's and pre-position fake helpers. The current wording, "daily-rotated hash of the group ID", **needs this precision**. The group ID must never be derivable from public observations.
- **Sleeper attack:** many helpers behave well for months, build reputation across many groups, then defect together.
  - E5: with member refresh, even a 50% dishonest share at r ≥ 20 keeps ~99.5%.
  - A coordinated switch still causes a short availability dip in groups where sleepers are over-represented.
  - Mitigations: cap any single *operator's* share of a group's set. Operator identity is unknowable, so use proxies: IP prefix / ASN diversity when choosing helpers, as Tor does for relays.
  - Nostr runs in parallel.
- **Traffic observation:** an observer watching a member's device can see which helpers it talks to, which reveals the set. This doesn't enable eclipse (the attacker still can't *join* the set) but it's metadata (A7).
- **Verdict: holds with two precisions:** a secret-keyed bootstrap hash, and IP/ASN diversity in selection. Confidence: medium (no adversarial simulation of sleepers yet).
