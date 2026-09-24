# A5: Malicious group member (measured)

Date: 2026-09-24. Loro 1.16.2. Code: `research/experiments/rust/e1_loro/src/bin/a5_member.rs`.

Members are trusted people by design, but a member's device can be compromised, a friendship can go bad, and bugs happen. The engine must stop one member from **silently corrupting** everyone else's data, even if it can't stop them from *visibly* making edits.

## 1. Impersonation via PeerID → permanent silent divergence (critical)
Setup: A and B are honest. M joins legitimately, then sets its Loro PeerID to **A's** and edits.

Result:
- Every import returned OK. **A and B now permanently disagree**:
  - A sees `{t1: "buy milk", t2: "A's real edit"}`;
  - B sees `{t1: "FORGED by M as A"}`. B never gets A's real edit.
- B's "last editor" of t1 is **A**, so M framed A.
- Loro PeerIDs are unauthenticated numbers, and colliding (peer, counter) pairs silently shadow each other (the same mechanism as E1's accidental reuse).

**Options (evaluated):**
- **(a) Bind PeerIDs to device keys and sign every update.**
  - PeerID = 64-bit hash of the device's public key.
  - Each update carries the author's signature. Before import, the engine checks the peer IDs actually inside the blob (Loro's import-blob metadata lists which peers' spans it contains): **a device may only send its own ops**.
  - This fits the slot model: each device's slot holds only its own changes.
  - **Consequence:** the shared "snapshot slot" (one member publishing everyone's ops) can't be verified op by op, so it must go. Its jobs are taken over by document epochs:
    - a member proposes a new epoch built from live state;
    - every member checks the new epoch's state equals its own before switching (a deterministic comparison), then continues on the new document.
  - **Verified** (`a5_meta.rs`): `LoroDoc::decode_import_blob_meta` reports the exact peers in a blob (own updates: `{2}`; all updates / snapshot: `{1, 2}`).
  - **Caveat:** metadata comes from the blob itself, and a crafted blob might claim one peer set and contain another. The robust check is *after* importing into the isolated copy (A6 option 1): the copy's version vector may only have advanced for the sender's own PeerID. The two fixes reinforce each other.
  - Confidence: **medium-high** that per-author signing + own-ops-only (checked on the isolated copy) closes impersonation.
- **(b) Keep shared snapshots and sign per-peer version attestations** (each device signs "my ops up to counter N hash to H"). Requires deterministic hashing of a peer's op range, which Loro doesn't expose. **Not viable without Loro changes.**

**Recommendation:** (a).

## 2. Mass deletion
M deleted all 500 tasks; the deletion synced normally. `fork_at` a version before the deletion still has all 500, so it **is recoverable while history exists**.

**Proposal:**
- "Undo changes by [person] since [time]" (restore from history) as a user feature.
- Never compact history within a minimum window (e.g. 30 days) regardless of size, so a destructive edit can always be reverted for a while.
- Confidence: high.

## 3. Size bombs
- A 50 MB text insert produced a **50 MB update** that another member imported in **117 ms** with no limit anywhere. A 5 MB low-repetition text gave a 5 MB update.
- Relays cap messages (128 KB–1 MB, E4), but **P2P has no cap**, so one member can fill every other member's storage and burn their mobile data.

**Proposal:** engine-level limits enforced before import:
- max update size (e.g. 1 MB);
- max document size (e.g. 20 MB);
- max text length per field (e.g. 100 k chars for a task description);
- per-author rate limits.

Oversized updates are rejected and the author is flagged. Legitimate large content (images) goes through blobs (§4), which have their own limits. Confidence: high. The exact numbers are placeholders to tune.

## 4. Crash via malformed update
See A6. Any member can crash every member's app with an 85-byte update.
