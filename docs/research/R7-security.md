# R7: Security design review

*Research report for DESIGN.md (draft 2026-09-24). Written 2026-09-24. Scope: invite codes, key epochs, authorship, malicious members, sealed backups, helper trust, metadata, and how invite secrets leak. Research only: nothing installed or built, and no traffic sent to public relays. Crate and version facts were checked against crates.io and docs.rs on 2026-09-24.*

## Summary

- **The 6-word invite codes hold up, but there's less margin than DESIGN.md suggests, and it shrinks over time.** At RFC 9106's second recommended setting (64 MiB, t=3), hashcat 7.0 cracks about **1,703 Argon2id guesses/s on one RTX 4090**. The design's "100k guesses/s" is therefore only about 59 rented GPUs. With 1M invites collected, one random invite falls in about 70,000 years at that rate, or about 400 years with 10,000 GPUs. That holds only while the number of collected invites N stays small. The design implies a **constant salt**, so N grows forever. And each invite contains the **permanent group secret**, so an invite cracked years later still works. Two cheap fixes remove the problem: salt with the invite's date, and put only the current epoch key in the invite payload. Two of the design's own numbers are also wrong: 4 words fall in about 10 hours, not "minutes", and 5 words in about 9 years, not "days to years".
- **Key epochs are underspecified. That's the most serious design gap in this area.** Without a central orderer, concurrent removals fork the epoch. Each branch's new key is then encrypted to the member removed in the other branch. A removed member can also backdate edits by claiming they were concurrent. And **anyone holding any group secret, even a leaked or old one, can add a "ghost" device to the membership CRDT**, which later rotations will then include (Matrix's 2022–23 vulnerability class). MLS doesn't fit because it needs a totally ordered delivery service. The right prior art is DCGKA, Keyhive/BeeKEM, p2panda-encryption and localfirst/auth's "strong-remove" rules.
- **Authorship is forgeable, and the consequences go beyond misattribution.** Loro operation IDs are `(peer, counter)`. A member who reuses another member's peer ID with conflicting counters causes **permanent divergence** between replicas (Kleppmann, PaPoC'22). Loro's own docs warn that duplicate PeerIDs "corrupt the document". This can also happen with no attacker: a backup restored to a second device reuses the PeerID if the engine pins one per device. Signing each update blob and binding peer IDs to device keys costs almost no CPU. The real cost is that snapshots written by one member contain everyone's operations and can't be verified per author without extra signed digests.
- **"Reject malicious data without crashing" conflicts with `panic = "abort"`.** UniFFI can only turn panics into exceptions when panics unwind. Loro had two import/read panics in September 2026 (issues #1088 and #1106). #1088 was described as "corrupted snapshots can be persisted and relayed, causing devices to crash in cold-start loops." A bad blob in the mailbox becomes a **poison pill** that crashes every member's app on every launch.
- **A new malicious-member attack defeats "undo": the history budget shrinks when content is deleted.** History may use "a multiple of the current state size". Mass-deleting a list shrinks that budget, and the next compaction then discards the checkpoints needed to undo the deletion. Bloating a shared list past 25 MB also **silently stops every member's Android Auto Backup** (`onQuotaExceeded`).
- **The §9 claim that Auto Backup is "encrypted with the screen lock" is wrong for users without a screen lock.** Client-side encryption requires Android 9+ **and** a PIN, pattern or password. Otherwise Google can read the backup. The fix is `disableIfNoEncryptionCapabilities="true"` or sealing when no lock is set. Block Store only backs up to the cloud when E2EE is available. **Sealed backups** stop generic scanning, but they are obfuscation: one person reverse-engineers the format once (or reads the source, if it's published) and every copy is exposed. And each backup holds **every group secret**, which are live, long-lived credentials.
- **The claim that "a device's groups can't be linked to each other" (per-device-per-group Nostr keys) is refuted against relays.** Relays link them through the shared IP and WebSocket connection, flush timing, REQ filters that list several topic tags, and NIP-42 (a relay "MUST treat all pubkeys as authenticated" on one connection). In iroh 1.2, address lookup publishes each device's **stable** EndpointId with its relay URL, and optionally its direct IPs. Anyone who ever learned a device's ID, including ex-members, can then track its presence indefinitely. Also, iroh uses QAD on its relays, not public STUN.
- **The helper network sits on a reasonable base (per-group records, no global reputation) but has five open gaps:**
  - Sybil identities can take over the probation slots.
  - "One copy per helper" is meaningless when one machine runs many helper identities; it needs IP/ASN diversity.
  - Anyone who knows a group ID can cheaply eclipse its bootstrap set by grinding node IDs; a keyed bootstrap point fixes this.
  - Colluding helpers defeat the "compare helpers" freshness check.
  - Proof-of-work favours GPU attackers over phones (Laurie & Clayton).
- **The biggest leak path for invites is that links and QR codes carry the permanent group secret. Codes expire after a day; links never do.** A link pasted into Slack, Discord, Gmail, Telegram cloud chats or an Outlook tenant (Safe Links) is stored by that provider, or passed through its link scanner. In-app browsers (Instagram, Facebook, Messenger) inject JavaScript and see the full URL. The "Get the app" clipboard step can't mark the clip as sensitive from a web page, and the default keyboard app can read the clipboard (Android 10+). **Fix: make links and QR codes the same short-lived, revocable, mailbox-hosted invite as codes, just with 256-bit entropy.**

## Claim-by-claim table

| # | Claim (from DESIGN.md) | Verdict | Severity if wrong | One-line reason |
|---|---|---|---|---|
| 1 | 6-word EFF codes (~77 bits) + Argon2id make offline multi-target brute force infeasible | **Partial** (holds today; brittle) | Medium | GPU rate confirmed (1,703 H/s per 4090 at 64 MiB/t=3). The math holds for realistic N. A constant salt makes N unbounded, and the payload is a permanent secret. Two of the design's numbers (4 and 5 words) are wrong. Phone timing is unmeasured. 4 EFF words contain hyphens. |
| 2 | Key epochs (rotate on removal, new key encrypted to each remaining device) | **Partial / underspecified** | **High** | Forks under concurrency, backdating by removed members, mutual removal with no roles, ghost devices added via old secrets, derived IDs not rotated, no post-compromise security (PCS) except on explicit removal. |
| 3 | CRDT ops are not authenticated; a member could forge edits as another member | **Confirmed**, and worse than stated | **High** | Beyond forged attribution, peer-ID/counter equivocation causes permanent divergence. Signing blobs is cheap; verifying snapshots per author is the hard part. |
| 4 | Incoming data from members is untrusted; limits, validation, fuzzing make malicious members harmless | **Partial** | **High** | `panic = "abort"` plus recent Loro panics make poison pills possible. The history budget shrinks on mass deletion. Bloat can overflow the 25 MB backup quota. Pending-dependency buffers and nesting depth need caps. |
| 5 | Sealed backups defeat automated scanning; Auto Backup is E2E | **Partial** | Medium | Auto Backup E2E needs a screen lock. Sealing is obfuscation (once reverse-engineered, it's broken for every copy). Backups hold all live group secrets. Desktop OS keystores are readable by same-user malware. |
| 6 | Helper trust/reputation resists sybil, eclipse, rollback, collusion, junk | **Partial** | Medium (later phase) | Probation capture by sybils, per-machine copy limits, cheap bootstrap eclipse, unaccountable replication, collusion defeating freshness checks, PoW asymmetry, and members' records being able to poison selection. |
| 7 | Metadata: per-device-per-group Nostr keys keep a device's groups unlinkable; exposure limited to §9 table | **Refuted** (vs relays) / partial (vs scrapers) | Medium | Relays link groups by IP, connection, timing, REQ filters and NIP-42. A stable iroh EndpointId is published to DNS/pkarr. Static group-derived tags let ex-members keep watching. |
| 8 | Invite secrets in URL fragments are safe from servers; clipboard/QR are acceptable | **Partial** | **High** (for links/QR) | The fragment never reaches the web server, but links pass through chat, email and scanner servers and in-app browsers, and they carry the permanent secret. The web page can't mark the clipboard as sensitive, and the keyboard app can read it. QR screenshots go to cloud photo backups. |

## Details per claim

### Claim 1: 6-word invite codes + Argon2id

**What DESIGN.md says (§5):** a 6-word code from the 7,776-word EFF list. Argon2id(code) produces a lookup tag and an encryption key. The group secret is posted to Nostr under the tag. Estimates: 4 words fall "every few minutes", 5 words take "days to years", 6 words take "tens of thousands of years", assuming ~1M invites and ~100k guesses/s.

**Evidence**

- **Entropy.** I downloaded and analyzed the EFF large list: 7,776 unique words, 12.925 bits each, so **6 words = 77.55 bits**. EFF itself recommends six words (≈77 bits) with this list. No word is a prefix of another. Word lengths are 3–9 characters (average 7.0).
- **Hyphens.** The list has **4 hyphenated words: `drop-down`, `felt-tip`, `t-shirt`, `yo-yo`.** The design's hyphen-separated format (`maple-orbit-guitar-…`) makes naive splitting ambiguous. It's still parsable greedily, because `drop`, `felt`, `t` and `yo` are not list words, but it's a trap for implementers.
- **Typos.** 2,364 word pairs are one edit apart (e.g. `able`/`cable`, `action`/`auction`), so a typo can silently become another valid word.
- **Defender parameters.**
  - RFC 9106 FIRST RECOMMENDED is Argon2id t=1, p=4, 2 GiB. That's impossible on phones.
  - RFC 9106 SECOND RECOMMENDED is **t=3, p=4, m=64 MiB**, "suggested as a default setting for memory-constrained environments".
  - OWASP's minimum is m=19 MiB, t=2, p=1 (or 46 MiB with t=1). OWASP also says "a unique salt must be added to each password".
- **Attacker rate (measured).**
  - hashcat v7.0.0 (Aug 2025) added GPU Argon2 (mode 34000). Its release notes list, for Argon2id at RFC 9106 settings (m=65536, t=3, p=1): **RTX 4090 1,703 H/s**, RX 7900 XTX 1,367 H/s, i7-14700K 96 H/s, Ryzen 9 9900X 92 H/s.
  - Scaling by memory × passes (bandwidth-bound; an estimate, not measured) gives ≈8,600 H/s at OWASP's 19 MiB/t=2 and ≈640 H/s at 256 MiB/t=2.
  - So the design's 100k H/s is only **~59 RTX 4090s**, about $18/h at an assumed $0.30 per GPU-hour.
- **Defender speed (not measured; estimate).**
  - There's no good published measurement for low-end Android. The closest data point: a 2018 phone ran KeePassDroid's native Argon2 (64 MiB, 40 rounds, 8 threads) in ~5 s, versus 40 s for Keepass2Android's managed implementation (keepass2android #306). By proportion, 64 MiB × 3 passes is ≈0.4 s with full parallelism.
  - The planned crate is RustCrypto **`argon2` 0.6.0** (released 2026-08-27). I read its source: it has an **AVX2 path only (no NEON)**. Lanes run **single-threaded unless the optional `parallel` (rayon) feature is enabled**.
  - Estimate for a low-end Cortex-A53/A55 phone: ~0.3–0.6 s at p=4 with `parallel`, and ~1–3 s single-lane. **Must be measured (E2).**

**Multi-target math** (my calculation; expected guesses to the first success ≈ keyspace/N):

| Words | N collected invites | Rate | Expected time to first crack | Cost at $0.30/GPU-h |
|---|---|---|---|---|
| 4 | 1e6 | 1e5 H/s (59 GPUs) | **~10 hours** (design says "minutes") | ~$180 |
| 5 | 1e6 | 1e5 H/s | **~9 years** (design says "days to years") | ~$1.4M |
| 6 | 1e6 | 1e5 H/s | ~70,000 years | ~$11B |
| 6 | 1e6 | 1.7e7 H/s (10k GPUs) | ~410 years | ~$11B |
| 6 | 1e8 | 1.7e7 H/s | **~4 years** | ~$110M |
| 6 | any N, **date-salted**, 1e4 invites/day | 1.7e7 H/s | ~41,000 years per success | ~$1.1T |
| 6 | targeted single invite | 1.7e7 H/s | ~2e8 years | ~$5e15 |

**Does the lookup tag leak anything?**

- The tag is the offline verification oracle. That's inherent, and exactly what the math above assumes.
- It leaks nothing more **as long as** tag and key both come from the Argon2id output through HKDF with separate labels. If any implementation ever computes the tag with a cheaper function (for example "hash the code to find the event, then Argon2 for the key"), the whole scheme collapses to fast-hash speed. That constraint must be written into the design.
- Other small leaks:
  - Relays see the inviter's and joiners' IPs on the same tag, which reveals a social link.
  - Invite volume and timing are public.
  - If the invite event is signed with the inviter's per-device-per-group Nostr key, it also links the invite to the group's mailbox.
- NIP-40 itself warns that expiration is **not a security feature**. Relays "MAY persist them indefinitely". DESIGN.md already reasons correctly from that.

**Problems found**

1. **No salt is specified, so it's a constant salt.** One Argon2 evaluation tests a guess against every invite ever posted, and N grows forever. The design's own table shows the margin shrinking as N grows.
2. **The payload is the permanent group secret.** "Expiry covers human leaks", but a brute-forced invite from two years ago still gives the group secret. Unless the group has rotated since, that's full author access.
3. The design's 4-word and 5-word numbers are off by ~100×, though the conclusion (6 words, large list) still stands.
4. Phone-side cost is unmeasured, and `argon2` without `parallel` wastes the p=4 advantage.

**Fixes**

- **Date-bucketed salt:** `salt = "kleiders/invite/v1" || YYYY-MM-DD (UTC issue date)`. The joiner tries today, then yesterday (at most 2 evaluations). This cuts multi-target gain from "all invites ever" to "invites issued that day" (the table shows ~$1T per success at 10k invites/day). It costs one extra Argon2 only for joins that cross midnight UTC.
  - Optional, at large scale: a per-invite random salt plus a 16–20-bit public bucket tag, which removes multi-target gain entirely at the cost of the joiner trying a few candidates.
- **Invite payload = current epoch key + group metadata, never a long-lived root.** Combine this with periodic rotation (Claim 2) so a cracked old invite is worth little.
- **Parameters:** Argon2id m=64 MiB, t=3, p=4 with `argon2` `features = ["parallel"]`. Measure on the oldest target device. Fall back to m=46 MiB/t=1 only if 64 MiB is too slow, and document the attacker rate each choice implies.
- **Specification:** derive tag and key with HKDF from the Argon2 output. Tag ≥128 bits. Sign the invite event with a fresh throwaway Nostr key.
- **Word list:** replace the 4 hyphenated words in *our copy* (keep 7,776 entries, keep indices stable forever), or parse without relying on separators. Offer 4-letter-prefix autocomplete for entry.
- Correct the 4-word and 5-word numbers in DESIGN.md.

### Claim 2: Key epochs

**What DESIGN.md says:** a new epoch key is encrypted to each remaining device's public key and distributed through the group. Epochs are used for "Remove device" and for removing people. There are no roles, and anyone can remove anyone.

**Comparison with prior art**

| System | Ordering | On removal | Concurrency handling | PCS |
|---|---|---|---|---|
| MLS (RFC 9420) | Delivery Service must totally order Commits (§14). Two Commits for one epoch fork the group. | Commit with Remove → new epoch | None without the DS. The IETF DMLS draft (2025) needs puncturable PRFs and keeping old tree state, "damaging their FS" | Yes, via Update/Commit |
| Signal / Messenger sender keys | Server fan-out | "Whenever a group member leaves… **all** group participants clear their Sender Key and start over" (Messenger whitepaper 2023) | Each sender rotates its own chain, so no shared-epoch fork | Only on membership change |
| Matrix Megolm | Homeserver | Clients rotate the outbound session on leave; also every 100 msgs / 1 week | Membership is **server-controlled and unauthenticated**, which enabled user/device-addition attacks (Albrecht et al., IEEE S&P 2023) | Weak |
| Keybase teams | Server-ordered sigchain | "The PTK must rotate… new PTK keys are encrypted for all remaining members" and "can happen lazily". Previous seeds are chained so new members can read the past | Central server sequences the chain | On removal/revocation |
| DCGKA (Weidner, Kleppmann et al., CCS'21) | Causal order only | New seed to each member per the member-view | Explicitly **does not** specify membership under concurrent add/remove | Yes |
| Keyhive/BeeKEM (Ink & Switch, 2025) | Causal order only | Remove paths are "blanked after all other concurrent operations are merged" | Conflict keys are kept; handles concurrent updates | Yes |
| p2panda-encryption (2025) | Causal hash graph, signed ops | New secret, O(n) via 2SM/HPKE; epochs identified by hash, not counter | **Strong-remove**: a removed member can't add anyone even concurrently; mutual removal removes both | Yes (message scheme) |
| localfirst/auth | Signed, hash-chained graph | Lockboxes rotated | Resolver rules: `resolveMutualRemovals` by **seniority**, `cantAddBackRemovedMember`, `cantDoAnythingWhenRemoved` | On rotation |

**Pitfalls the design doesn't address**

1. **Forked epochs leak to removed members.** Suppose A removes X while B concurrently removes Y. Epoch E_A is encrypted to Y, and E_B to X. Whichever branch "wins", the other branch's removed member holds a valid key. **Rule needed:** any device that observes concurrent epochs must deterministically create a successor epoch whose recipients are the intersection of both memberships. The rotator should be chosen deterministically (e.g. lowest device ID among the online remaining devices), with a lowest-hash tie-break so duplicate successors converge. Data written during the fork is unavoidably readable by the other branch's removed member. Document that.
2. **Removed members can backdate.** Loro version vectors are self-declared, so a removed member can claim any edit was "concurrent with" its removal. **Rule needed:** the removal op carries the remover's version vector, and ops by the removed author outside that vector are rejected (localfirst/auth's `cantDoAnythingWhenRemoved`). This only works with authenticated authorship (Claim 3).
3. **Mutual or hostile removal with no roles.** Any member can remove everyone else and keep the only new key. Pick a rule (seniority as in localfirst/auth, or remove-both as in p2panda). Add a visible "Sam removed Alex · Undo" path, and consider "a member can't remove someone who joined before them" unless the product accepts the takeover risk.
4. **Ghost members (the Matrix lesson).** Membership lives in a CRDT that anyone with the key can write. Someone holding a leaked link, a cracked invite or a stolen backup can add their own device. From then on, **every rotation encrypts the new key to them**. **Rules needed:**
   - A membership addition is valid only if it's authored under the *current* epoch and signed by a current member device.
   - Every join and every new device is shown and notified to all members ("Alex's new device joined", "Sam joined with an invite code").
5. **Derived identifiers.** If the group ID, Nostr topic tag, DHT discovery key or snapshot-slot key come from the original secret rather than the epoch key, a removed member keeps finding the mailbox, watching activity timing, and (for shared-key slots) vandalizing it (see new risk N2). Derive every network-visible identifier from the epoch key, preferably also rotated daily. Use a stable *internal* group ID that never appears on the wire.
6. **No PCS except on explicit removal.** Passive compromises are healed only by a rotation that nobody triggers: a leaked link, a leaked sealed backup, malware that copied the keystore. Add opportunistic rotation, e.g. a designated online device rotates every ~30 days and on every device add. This also retires leaked links and old invites (Claims 1 and 8).
7. **Stale recovery material.** After "Remove device" rotates the personal group, an older Block Store copy restores a secret that (correctly) no longer works. Update Block Store immediately after rotation. Specify that recovery from a pre-rotation backup requires another of the user's devices (Keybase used "paper keys" for this).
8. **Key hygiene.** One Ed25519 key is used as the iroh TLS identity, for signing, and (converted to X25519) for key wrapping. Use separate purpose keys: an HPKE (RFC 9180) X25519 key for wrapping, and a signing key bound to the iroh key by a signature.
9. **Scale detail.** A group of 100 members × 3 devices produces ~300 wrapped keys, about 24 KB of ciphertext (~32 KB base64). Some relays cap content at 8–16 KB (NIP-11 example), while strfry's default is 64 KB. Chunk the rotation, or deliver it per device.

**Fix (overall):** don't invent this. Adopt either p2panda-encryption's "data encryption" scheme (Rust, audited-in-progress, the same group-shared-key model) or a Keyhive-style design with strong-remove rules. Otherwise, specify the rules above and test them in E8 with an adversarial simulator.

### Claim 3: Authorship

- **Confirmed.** Loro gives no authentication. Its `set_peer_id` docs state: "Never reuse the same PeerID across concurrent writers… **Duplicate PeerIDs can produce conflicting OpIDs and corrupt the document.** Do not assign a fixed PeerID to a user or device without strict single-ownership locking."
- **Worse than misattribution.** Kleppmann (PaPoC'22) shows that a Byzantine node sending "two different updates with the same ID (r,1)" to different peers leaves them with identical version vectors but different content. They never converge, because an ID a replica has already seen is rejected. A malicious member can also **squat future IDs**: publish ops under a victim's peer ID at counters the victim will use later, so the victim's real edits are dropped on replicas that took the forgery.
- **It happens without an attacker too.** If the engine pins a PeerID per device, restoring Auto Backup to a new phone while the old one lives, restoring twice, or cloning an emulator image all reuse the PeerID.
- **What to sign, and the cost:**
  - **Per update blob:** Ed25519 signature by the device signing key over (group, epoch, blob hash). Cost is tens of µs per sign or verify on phones, plus 64 bytes. Negligible.
  - **Binding:** each device registers its (random, per-session) Loro PeerIDs in the group doc with a signature. On import, `LoroDoc::decode_import_blob_meta` returns `partial_start_vv`/`partial_end_vv`, i.e. which peers' ops a blob contains, *before* importing. Reject any blob from device D that contains ops of peers not bound to D. This fits the per-device slot model naturally.
  - **The hard part: snapshots.** A snapshot written by one member contains all authors' ops, so the writer can include forgeries. Options:
    - (a) Every device periodically signs digests of its own change ranges. Loro documents `export_json_in_id_span` as deterministic output "ideal for hash calculations and integrity checks". Readers verify foreign ranges in a snapshot against those digests and quarantine the rest.
    - (b) Treat snapshots as trusted-member input and only verify per-device bundles. That accepts that a malicious member can forge history in snapshots.
    - (c) Longer term, use a hash-DAG CRDT layer (Kleppmann's fix: op ID = hash of the update).
    - Shallow-snapshot compaction discards op history, so a compacted base state is always "consensus as asserted by whoever compacted".
- **Fixes:**
  - Never pin PeerIDs. Use a random PeerID per process/session, bound by signature.
  - Sign every blob.
  - Verify bundles strictly and snapshots via digests (a). This is also a prerequisite for strong-remove (Claim 2, point 2).

### Claim 4: Malicious members (resource exhaustion, bombs, mass delete)

1. **Poison pill plus `panic = "abort"` (§10 size profile).**
   - UniFFI docs: "UniFFI can't always catch panics, for example when the Rust panic handler is set to abort."
   - Loro #1088 (3 Sept 2026, fixed in 1.16.x): a snapshot is "accepted without error by… `LoroDoc.import` but then panics on read". The reporter's impact note: corrupted snapshots "can be persisted and relayed, causing devices to crash in cold-start loops once synced (SIGABRT in loro-react-native)".
   - Loro #1106 (19 Sept 2026): `fork_at` plus historical diff → `unwrap()` on `None`, then a poisoned mutex. The process aborts.
   - Loro 1.16.2 release notes also list "accept tree snapshot state whose sibling nodes are not in fractional index order… instead of panicking". So these panics are still being found now.
   - **Fixes:**
     - Build the engine with `panic = "unwind"`. Keep abort only in debug assertions if desired. The size cost is small next to iroh and Loro, so measure it in E6.
     - Wrap every import in `catch_unwind`.
     - Keep a **quarantine journal**: write "importing blob H" before import and clear it after. On the next launch, skip and quarantine H if the marker is still set.
     - Import into a scratch fork first, check invariants, then merge.
     - Note that stack overflow and allocation failure abort even with unwind. Hence item 3.
2. **History-budget shrink attack.** Compaction budgets history as "a multiple of its current state size". A member deletes everything, the state collapses, the budget collapses, and the next compaction drops the checkpoints needed to undo the deletion.
   - **Fixes:** base the budget on a high-water mark (e.g. the max state size over the last 90 days). Exempt the 30-day trash and any checkpoint from before a large deletion. Flag bulk deletions ("Sam deleted 214 tasks · Undo").
3. **Bombs and exhaustion.**
   - Loro's KV store (`loro-kv-store` 1.16.0) depends on `lz4_flex` and `xxhash`, so decompression is inside the parser. xxhash detects corruption, not tampering.
   - Guards needed: cap blob size before decode; cap decompressed size; cap container and nesting depth (deep nesting causes a stack overflow, which is an unrecoverable abort); cap the ops per change and the text-mark count.
   - Also cap the pending-dependency buffer. Updates that depend on missing ops stay pending (see the `ImportStatus.pending` docs). An attacker can send endless never-satisfiable updates, so cap the buffer and drop entries after a timeout.
4. **Bloat versus backups.** Auto Backup: "If the amount of data is over 25 MB, the system calls `onQuotaExceeded()` and doesn't back up data to the cloud." One member bloating a shared list stops *every* member's Android backup, silently.
   - **Fixes:** per-member write quotas and per-document size caps. Exclude other members' bulky history from the backup set (it can be refetched). Raise a visible notice when backup is failing.
5. **Deterministic validation versus principle 8.** Rejecting ops (size caps, removed authors) changes merge results. If v2 rejects what v1 accepted, replicas diverge.
   - **Fix:** define every rejection rule in the format from day one, versioned per group ("rules version" raised only when every member supports it). Treat this as part of the "crypto flaw" exception list in principle 8.
6. **Fuzzing.** DESIGN.md already requires fuzzing our own parsers. Extend it to Loro's `import`/`import_batch` and `decode_import_blob_meta` with structure-aware corpora (A5/A6).

### Claim 5: Sealed backups and E2E Auto Backup assumptions

- **Auto Backup.** Official docs: client-side encryption "is enabled on devices running Android 9 or higher as long as the user has enabled backup… and has set a screen lock (PIN, pattern, or password)". Without a lock, backups are protected only by Google-held keys.
  - Android 12+ offers `<cloud-backup disableIfNoEncryptionCapabilities="true">`; Android ≤11 uses `requireFlags="clientSideEncryption"`.
  - Device-to-device transfers ignore that flag (they're local).
  - **Fix:** exclude the secrets file from cloud backup when there's no encryption capability, or seal the whole backup in that case (extend the design's "<Android 9 → sealed" fallback to "no screen lock → sealed"). Correct the §9 table.
- **Block Store.** Up to **16 entries × 4 KB**. E2EE needs Android 9+ and a screen lock, checked with `isEndToEndEncryptionAvailable()`. The official sample only calls `setShouldBackupToCloud(true)` when E2EE is available, "otherwise only store bytes for D2D restore". So for users without a lock, recovery of shared lists via Block Store doesn't exist. Handle and message that case.
- **Sealing.** Sealing (key inside the file) does defeat generic hash matching, text extraction, AI indexing and bulk-leak grep. But "untargeted" and "targeted" aren't separate categories:
  - Once one attacker (e.g. an infostealer author) writes an unsealer, it's automated for everyone.
  - If the engine is open-sourced (§14), the format is public on day one.
  - More important: a backup holds **all group secrets**, so one leaked backup gives *ongoing* access to every shared list until they rotate.
  - **Fixes:**
    - Store only the personal-group secret; everything else follows from it.
    - Rotate periodically (Claim 2) so old leaked backups expire.
    - Where available, wrap the seal key with something outside the file: Block Store on Android, the OS keychain on desktop. Keep "key in file" only as the fallback, plus an optional printable recovery kit.
    - Consider Drive's hidden `drive.appdata` folder (a non-sensitive scope, like `drive.file`) to reduce accidental sharing and indexing, if a visible folder isn't essential.
- **Desktop OS keystores.** DPAPI: "only a user with the same logon credential… can decrypt". That means any process running as that user, which is how infostealers dump Credential Manager. Linux Secret Service collections are open to any process in the unlocked session. These stores protect against other users and offline disk theft, not against same-user malware. State that honestly in §9.

### Claim 6: Helper network trust (sybil, eclipse, rollback/withholding, collusion, junk)

What holds: per-group records with no global reputation. That avoids the manipulable global-reputation class (EigenTrust's reliance on pre-trusted peers, and Cheng & Friedman's result that symmetric reputation can't be made sybil-proof). CRDT idempotence makes rollback non-destructive. And Nostr in parallel bounds availability damage.

Gaps:

1. **Sybil capture of probation.**
   - Identities are free. With 10× more sybil helpers than honest newcomers, an attacker takes most "one copy for an unproven helper" slots, behaves perfectly, earns trust, and spreads through overlapping groups.
   - The goal is less dropping data than **metadata collection**: member IPs, activity timing and group sizes at network scale, which a fixed relay list doesn't expose to arbitrary newcomers.
   - **Fixes:** cap the share of any /24 (IPv4), /48 (IPv6) or ASN in a group's helper set, as Tor and BitTorrent do with IP diversity. Make the probation budget per group tiny, and never give probation helpers a group's first-hop uploads.
2. **"One copy per group per helper" is defeated by one machine with many identities.** Enforce diversity by network location, not by identity.
3. **Bootstrap eclipse is cheap.**
   - The bootstrap point, a daily-rotated hash of the group ID, is computable by anyone who knows the group ID, and helpers do know it.
   - Being among the r closest of ~1k helpers needs roughly 10 matching prefix bits: thousands of key generations.
   - S/Kademlia counters this with static and dynamic crypto puzzles, "a static puzzle that impedes that the nodeId can be chosen freely and a dynamic puzzle that ensures that it is complex to generate a huge amount of nodeIds", plus disjoint-path lookups.
   - **Fix:** a keyed bootstrap point, `HMAC(epoch_key, date)`. Helpers can't pre-position for groups they aren't members of, and the joiner has the key from the invite. Optionally add S/Kademlia-style ID puzzles.
4. **Unaccountable replication.** The sender uploads to 2–3 first hops that "replicate to the rest". A malicious first hop silently doesn't, and attribution is impossible: the downstream helper could equally lie.
   - **Fix:** signed store receipts from each set member, returned to the writer through the first hop. The writer spot-checks by reading back from a random set member.
5. **Collusion defeats freshness checks.** "Compare helpers" catches a rollback only if some compared helper is honest.
   - **Fix:** writers publish a signed (slot, version, hash) head to the group, over P2P or Nostr, and readers require a helper's answer to be at least that version.
   - Note the helper network exists for exactly the case where Nostr acceptance fails, so "Nostr covers it" is weakest when it matters most.
6. **Proof-of-work junk defence.** Laurie & Clayton (2004): legitimate senders on slow machines versus attackers with fast ones or botnets. The limits needed to stop abuse are "of the same order of magnitude as global averages for usage", which makes PoW "completely infeasible to deploy" on its own. Phones versus GPUs is the same asymmetry. Per-group quotas are meaningless when group IDs are free.
   - **Fix:** prioritize by *observed reads by distinct clients over time* and by long-lived slot history. Use PoW only as a tie-breaker. Accept that fresh groups under attack fall back to Nostr and members.
7. **Members' records can poison selection.** Records are "shared only within the device's own groups, whose members are trusted", but a malicious member (Claim 4's threat model) can inject fabricated observations that promote sybils or smear honest helpers across *all* its groups ("trust spreads through people").
   - **Fix:** weight records by the number of *distinct* reporting members, cap any single member's influence, and sign records.
8. **Amplification.** An upload carries "the group's helper set (public IDs only)" and helpers replicate to it. A forged set turns helpers into a ~10× fan-out of junk.
   - **Fix:** helpers replicate only to sets signed under the group's current epoch (which helpers can check as a signature by a key registered for the slot), and rate-limit per slot.

### Claim 7: Metadata exposure

- **Nostr relays.**
  - One device publishing several groups' slots from one IP over one WebSocket, flushing all groups at the same moment ("leaving the app flushes immediately"), is trivially linkable.
  - A REQ with several topic tags, or several REQs on one connection, links the groups explicitly.
  - NIP-42: "Clients MAY provide signed events from multiple pubkeys… Relays MUST treat all pubkeys as authenticated". Authenticating several per-group keys on one connection hands the relay the linkage directly.
  - The plaintext "group ID hash" in the message format links every device of a group across time.
  - The unlinkability claim therefore holds at most against *third parties reading public relay data*, and only if timing is decorrelated.
  - **Fixes:** rotating per-epoch/per-day topic tags and slot IDs; no plaintext group hash (use the rotating tag); jittered, per-group-staggered flushes; separate connections per group where cheap; never AUTH more than one group's key per connection. Restate §9 honestly: "relays can see which groups a device belongs to".
- **iroh.**
  - iroh 1.2.0 docs: every endpoint "will automatically publish their own addressing information. Usually this means… RelayUrl… but they could also publish their direct addresses." The `DnsAddressLookup` "will publish the RelayUrl and direct addresses".
  - Number 0 runs the default public PkarrPublisher/DNS, a single operator that sees who resolves whom.
  - The EndpointId is the **stable device identity** shared across all groups. Anyone who ever learned it (members, ex-members, relay operators) can resolve its current relay (and possibly its IPs) indefinitely: a presence and location oracle. The iroh relay operator also sees a device-level contact graph.
  - **Fixes:**
    - Publish with `AddrFilter::relay_only()`.
    - Better: don't publish plaintext by EndpointId at all. Put encrypted `EndpointAddr` records under the group's rotating discovery key and feed them to iroh via `MemoryLookup`.
    - Consider per-group iroh endpoint keys if the cost allows, or at least rotate the endpoint key on "Remove device".
- **STUN.** iroh relays now provide QAD ("QUIC Address Discovery") rather than public STUN. Update §6 and §9.
- **DHT (Mainline/BEP44).** Nodes storing a group's discovery key see every member IP that queries it. Sybil nodes placed near the key see this for any group whose key they learn; real-world Mainline sybil populations have been measured (Wang & Kangasharju 2012).
  - **Fix:** rotate the discovery key daily from the epoch key, and query through iroh relays where possible.

### Claim 8: Invite secrets in URL fragments, clipboard, QR

- **Fragments.** Correct that the fragment isn't sent in HTTP requests. But:
  - (a) Chat and email platforms that aren't end-to-end encrypted store the whole message, secret included, indefinitely.
  - (b) Microsoft Defender Safe Links rewrites every URL in email to `*.safelinks.protection.outlook.com`, scans at time of click, "detonates" unknown URLs, and can "track user clicks". The original URL (fragment included, presumably, since navigation must preserve it; verify in A8) passes through Microsoft's service.
  - (c) In-app browsers: Instagram, Facebook and Messenger inject JavaScript into third-party pages (Krause 2022). App Links often don't fire inside in-app browsers, so our GitHub Pages page runs in the host's WebView, which sees the full URL.
  - (d) Browser history and history sync.
  - (e) Our GitHub Pages JavaScript must read the fragment, so a compromise of the repo or account exfiltrates every invite opened there. The future web client has the same code-delivery trust problem.
- **The core issue.** Codes expire in a day, but **links and QR codes contain the permanent group secret**, so all the exposure above is permanent.
  - **Fix:** a link or QR carries a 256-bit *invite capability* that decrypts a short-lived invite event in the mailbox (same mechanism as codes, no Argon2 needed at 256 bits), containing the current epoch key.
  - This gives expiry, revocation ("reset invite link", as Signal groups allow), and a join notification.
  - Personal-group link QRs already expire in 5 minutes; apply the same idea to group invites.
- **Clipboard.**
  - Android 10+: only "the default input method editor (IME)" or the focused app can read the clipboard, so the keyboard app always can. Keyboards with clipboard history or cloud sync then hold the group secret.
  - `ClipDescription.EXTRA_IS_SENSITIVE` (hides the Android 13+ preview) is available to *apps*, but the GitHub Pages "Get the app" step writes via the web Clipboard API and can't set it.
  - Windows: an app can opt out of history and cloud sync by placing `ExcludeClipboardContentFromMonitorProcessing`, or `CanIncludeInClipboardHistory=0` / `CanUploadToCloudClipboard=0`, on the clipboard. The design should do this for every desktop copy of an invite.
  - **Fixes:** use the short-lived capability (above) so the clipboard copy expires. Set `EXTRA_IS_SENSITIVE` on in-app copies and the Windows exclusion formats on desktop. Clear our own clip after the first-launch read.
  - Invite-code entry fields should use `IME_FLAG_NO_PERSONALIZED_LEARNING` with no suggestions, so keyboards don't learn or sync the words.
- **QR screenshots.** Screenshots go to cloud photo backup automatically.
  - `FLAG_SECURE` on the QR screen prevents screenshots and recordings.
  - Android 14's screenshot-detection callback detects only hardware-button screenshots, not recordings or adb, but is useful to rotate the QR when a screenshot happens.
  - Short-lived capabilities make screenshot leaks expire.

## New risks (not in DESIGN.md)

| ID | Risk | Severity | Fix |
|---|---|---|---|
| N1 | **Poison-pill crash loop.** `panic = "abort"` plus a malformed or unlucky blob persisted in the mailbox or relayed P2P means every member's app aborts on each launch (Loro #1088 shows the pattern). | High | `panic = "unwind"` + `catch_unwind` around import; quarantine journal; scratch-fork import; fuzz Loro import. |
| N2 | **Shared snapshot slot is vandalizable.** If the single "snapshot slot" uses a group-derived Nostr key, any member (or ex-member, if derived from the old secret) can overwrite it with junk, or pin it with a future-dated `created_at`. strfry rejects >900 s ahead and nostr-rs-relay >1,800 s, so an attacker can keep re-pinning 15–30 minutes ahead; NIP-01 keeps "the latest" by `created_at`. | Medium | Per-device snapshot slots; ignore events more than a small skew into the future; bind slot pubkeys to member devices in the group doc; derive slot keys from the epoch key. |
| N3 | **Honest Loro PeerID collisions** after backup restore or device clone (if PeerIDs are pinned per device) corrupt documents. | High | Random PeerID per session, bound to the device key by signature; never persist-and-reuse without a lease (Loro's `peer-lease`). |
| N4 | **History-budget shrink:** mass deletion shrinks the budget, and compaction then destroys the undo path. | High | High-water-mark budget; protect pre-deletion checkpoints and trash; bulk-delete notice with Undo. |
| N5 | **Backup quota DoS:** one member bloats a shared list past 25 MB and every member's Auto Backup silently stops. | Medium | Per-member quotas, doc caps, back up only own-needed data, surface backup failure. |
| N6 | **Removal wars / hostile takeover** in role-less groups (a single member removes everyone else). | Medium | Seniority or strong-remove rules, visible removals with Undo, rate limit on removals. |
| N7 | **Ghost members** via any leaked or old secret (link, cracked invite, backup), included in every future rotation. | High | Additions valid only under the current epoch and signed by a current device; join and new-device notifications; periodic rotation. |
| N8 | **Validation rules versus "forever compatible".** New rejection rules cause old and new clients to diverge. | Medium | Per-group rules version ratchet; ship every limit in v1. |
| N9 | **Stable EndpointId presence oracle** via iroh DNS/pkarr publishing, plus the single-operator n0 DNS. | Medium | `relay_only` filter or group-encrypted address records; rotate the key on device removal. |
| N10 | **Invite decoys:** anyone who learns a code can post a competing event under the same tag (their own group, a fake "shared by Uri" name). | Low | Show inviter avatar/fingerprint on the join screen; prefer the earliest event; members see "joined via code". |
| N11 | **Keyboard learning** of typed codes (personalized dictionaries, cloud sync). | Low | `IME_FLAG_NO_PERSONALIZED_LEARNING`, `textNoSuggestions` / `textVisiblePassword`. |
| N12 | **Key reuse** of one Ed25519 key across iroh TLS, signatures and X25519 wrapping. | Low | Separate purpose keys (HPKE X25519 for wrapping) bound by signatures. |
| N13 | **Desktop infostealers** read DPAPI/Credential Manager and Secret Service secrets and sealed backups. | Medium | Honest threat statement; periodic rotation; optional OS-bound wrapping; don't store every group secret individually. |
| N14 | **Relay-visible social graph** from invite tags (inviter IP posts tag T, joiner IP fetches T). | Low | Accept, or fetch invites through iroh relays / a member's device when available. |
| N15 | **Stale recovery after rotation:** a Block Store or cloud backup from before "Remove device" can't decrypt new data. | Low–Medium | Update Block Store immediately after rotation; document that another own device is needed; optional recovery kit. |
| N16 | **Rotation message size** for large groups exceeds some relays' content caps. | Low | Chunk, or deliver per device. |
| N17 | **Helper fan-out amplification** via forged helper sets. | Medium (later) | Only replicate to epoch-signed sets; per-slot rate limit. |

## Suggested experiments (local only)

1. **E2 Argon2id on real low-end ARM.** A Cortex-A53/A55 phone (e.g. an Android Go device), with `argon2` 0.6.0 both with and without `parallel`. Grid m ∈ {19, 46, 64, 128} MiB × t ∈ {1, 2, 3} × p ∈ {1, 4}. Record wall time, peak RSS, and thermal behaviour over 5 consecutive runs. Compare against libsodium through JNI. Target: ≤1 s at 64 MiB/t=3/p=4.
2. **E2 attacker rate (local GPU).** On this PC's NVIDIA GPU, run `hashcat -m 34000 -b` at the chosen parameters (install locally only if the project's rules allow). Confirm scaling with m×t and with p=4 versus p=1.
3. **A1 multi-target.** Implement tag-set lookup over 1e6 synthetic tags and confirm per-guess cost doesn't depend on N. Implement the date salt and confirm cross-day guesses don't transfer.
4. **E7 word list.** Parser tests covering the 4 hyphenated words, edit-distance-1 typos and autocorrect. Measure how prefix autocomplete affects typing.
5. **A5 Loro hostility suite.**
   - Structure-aware `cargo-fuzz` of `import`, `import_batch` and `decode_import_blob_meta`.
   - An lz4 block bomb.
   - Deep nesting (look for stack overflow).
   - Pending-dependency flood (memory growth).
   - ID squatting and equivocation between 3 replicas (assert divergence, then test the signing fix).
   - Mass-delete-then-compact (history-budget shrink).
   - A >25 MB bloat to confirm the Auto Backup quota behaviour in an emulator.
   - Everything run with `panic=abort` versus `unwind` to measure crash-loop behaviour and the size difference.
6. **N3 PeerID restore collision.** Restore one Auto Backup onto two emulators, edit both, sync, and check for divergence (with pinned and with random PeerIDs).
7. **E8 epoch simulator.** Randomized concurrent add/remove/rotate under partitions and message reordering. Invariants:
   - A removed device never learns any epoch key created after its removal is causally known.
   - Forks re-converge to one epoch.
   - Ghost additions under old epochs are rejected.
   - Mutual removals resolve deterministically.
8. **A7 relay linkability** (local strfry): one client in 5 groups. Show linkage via connection, IP, timing and REQ filters. Test NIP-42 with multiple pubkeys. Test future-dated addressable events pinning a slot under strfry defaults (900 s).
9. **iroh publishing audit.** Two local endpoints against a local pkarr/DNS server. Capture exactly what's published under default presets versus `AddrFilter::relay_only()`.
10. **A8 exposure paths.** Use a throwaway domain and our own page that logs `location.hash` locally (no third-party traffic beyond normal use):
    - Send a test link via Gmail, Outlook (Safe Links tenant if available), Slack, Telegram and the Instagram in-app browser.
    - Check wrapped URLs for the fragment.
    - Check Gboard, Samsung Keyboard and SwiftKey clipboard-history retention.
    - Check Windows Cloud Clipboard with and without the exclusion formats.
    - Check that `FLAG_SECURE` blocks QR screenshots.
11. **Auto Backup encryption flags.** On an emulator with and without a screen lock, check that `disableIfNoEncryptionCapabilities` excludes the secrets file (`bmgr` backup, then inspect the transport).
12. **E5 helper sim additions.** Sybil fraction versus probation capture over time; cost of grinding node IDs next to the bootstrap point (plain hash versus keyed HMAC); collusion versus signed version heads; amplification with forged helper sets.

## Sources

Checked in this session (fetched or read on 2026-09-24):

- RFC 9106 (Argon2) recommended options: https://www.rfc-editor.org/rfc/rfc9106.txt
- OWASP Password Storage Cheat Sheet (Argon2id minimums, unique salt): https://github.com/OWASP/CheatSheetSeries/blob/master/cheatsheets/Password_Storage_Cheat_Sheet.md
- hashcat v7.0.0 release notes (mode 34000, RTX 4090 1,703 H/s at m=65536, t=3, p=1): https://github.com/hashcat/hashcat/blob/master/docs/releases_notes_v7.0.0.md
- hashcat 7.0.0 release date (Aug 2025): https://www.helpnetsecurity.com/2025/08/04/hashcat-open-source-password-recovery-7-0-0-released/
- RustCrypto `argon2` 0.6.0 (features; source read from the published crate: AVX2 only, rayon `parallel`): https://docs.rs/crate/argon2/latest/features, https://static.crates.io/crates/argon2/argon2-0.6.0.crate
- Android Argon2 timing anecdote (KeePassDroid ~5 s vs Keepass2Android 40 s, 64 MiB × 40 rounds, 2018): https://github.com/PhilippC/keepass2android/issues/306
- EFF large word list and properties: https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases, https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt
- Nostr NIP-01 (addressable events, latest by `created_at`), NIP-40 (expiration "not a security feature"), NIP-42 (multi-pubkey AUTH), NIP-11 (limits): https://github.com/nostr-protocol/nips/blob/master/01.md, https://github.com/nostr-protocol/nips/blob/master/40.md, https://github.com/nostr-protocol/nips/blob/master/42.md, https://github.com/nostr-protocol/nips/blob/master/11.md
- strfry default config (`rejectEventsNewerThanSeconds = 900`, `maxEventSize = 65536`): https://github.com/hoytech/strfry/blob/master/strfry.conf
- nostr-rs-relay config (`reject_future_seconds = 1800`): https://github.com/scsibug/nostr-rs-relay/blob/master/config.toml
- RFC 9420 MLS §14 (sequencing of state changes): https://datatracker.ietf.org/doc/html/rfc9420
- Decentralized MLS draft (2025): https://www.ietf.org/archive/id/draft-kohbrok-mls-decentralized-mls-00.html
- DCGKA (Weidner, Kleppmann, Hugenroth, Beresford, CCS 2021): https://eprint.iacr.org/2020/1281
- Keyhive / BeeKEM notebook (Ink & Switch, 2025): https://www.inkandswitch.com/keyhive/notebook/02/, https://www.inkandswitch.com/keyhive/notebook/06/
- p2panda group encryption (2025): https://p2panda.org/2025/02/24/group-encryption.html
- localfirst/auth membership resolver (strong-remove, seniority): https://github.com/local-first-web/auth/blob/main/packages/auth/src/team/membershipResolver.ts
- Keybase teams crypto (PTK rotation, lazy): https://book.keybase.io/docs/teams/crypto
- Messenger E2EE whitepaper (sender keys reset on leave, Dec 2023): https://engineering.fb.com/wp-content/uploads/2023/12/MessengerEnd-to-EndEncryptionOverview_12-6-2023.pdf
- Matrix vulnerabilities (Albrecht, Celi, Dowling, Jones; IEEE S&P 2023): https://nebuchadnezzar-megolm.github.io/, https://eprint.iacr.org/2023/485
- Kleppmann, "Making CRDTs Byzantine Fault Tolerant" (PaPoC 2022): https://martin.kleppmann.com/papers/bft-crdt-papoc22.pdf
- Loro `LoroDoc` API (`set_peer_id` pitfalls, `decode_import_blob_meta`, `export_json_in_id_span`, `import` pending): https://docs.rs/loro/latest/loro/struct.LoroDoc.html, https://docs.rs/loro/latest/loro/struct.ImportBlobMetadata.html
- Loro PeerID management docs: https://loro.dev/docs/concepts/peerid_management
- Loro issues #1088 (Sept 2026, snapshot import then panic, cold-start loops) and #1106 (Sept 2026, abort): https://github.com/loro-dev/loro/issues/1088, https://github.com/loro-dev/loro/issues/1106; releases: https://github.com/loro-dev/loro/releases
- `loro-kv-store` dependencies (`lz4_flex`, `xxhash-rust`): https://crates.io/crates/loro-kv-store
- UniFFI panic handling ("can't always catch panics… when the Rust panic handler is set to abort"): https://mozilla.github.io/uniffi-rs/latest/internals/rust_calls.html
- Android Auto Backup (E2E needs a screen lock, `disableIfNoEncryptionCapabilities`, 25 MB): https://developer.android.com/identity/data/autobackup
- Android Block Store (16 × 4 KB, E2EE conditions): https://developer.android.com/identity/block-store
- Android clipboard (`EXTRA_IS_SENSITIVE`, access toast): https://developer.android.com/develop/ui/views/touch-and-input/copy-paste
- Android 10 clipboard restriction (default IME / focused app): https://developer.android.com/about/versions/10/privacy/changes
- Windows cloud clipboard / history exclusion formats: https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats
- Android 14 screenshot detection limits: https://developer.android.com/about/versions/14/features/screenshot-detection
- Microsoft Defender Safe Links (rewriting, time-of-click, detonation, click tracking): https://learn.microsoft.com/en-us/defender-office-365/safe-links-about
- In-app browser JavaScript injection (Krause, Aug 2022): https://krausefx.com/blog/ios-privacy-instagram-and-facebook-can-track-anything-you-do-on-any-website-in-their-in-app-browser
- iroh 1.2.0 docs (address lookup publishes relay URL / direct addresses, QAD on relays, n0 public PkarrPublisher, `AddrFilter::relay_only`): https://docs.rs/iroh/latest/iroh/, https://docs.rs/iroh/latest/iroh/address_lookup/index.html
- S/Kademlia (Baumgart & Mies 2007; crypto puzzles, disjoint paths): https://telematics.tm.kit.edu/publications/Files/267/SKademlia_2007.pdf
- Laurie & Clayton, "Proof-of-Work Proves Not to Work" (2004): https://www.cl.cam.ac.uk/~rnc1/proofwork.pdf
- DPAPI `CryptProtectData` (same-user decryption): https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata
- Google Drive scopes (`drive.file` and `drive.appdata` non-sensitive): https://developers.google.com/workspace/drive/api/guides/api-specific-auth

Cited from the literature but not re-fetched in this session (the search budget ran out):

- Douceur, "The Sybil Attack" (IPTPS 2002).
- Wang & Kangasharju, "Real-World Sybil Attacks in BitTorrent Mainline DHT" (IEEE GLOBECOM 2012).
- Kamvar, Schlosser, Garcia-Molina, "The EigenTrust Algorithm for Reputation Management in P2P Networks" (WWW 2003).
- Cheng & Friedman, "Sybilproof Reputation Mechanisms" (P2PECON 2005).
- BeeKEM paper, IACR ePrint 2026/1434 (listed in search results only): https://eprint.iacr.org/2026/1434.pdf
- Matrix Megolm rotation defaults (100 messages / 1 week), from a search snippet: https://uhoreg.gitlab.io/matrix-tutorial/megolm.html

Assumptions I made (not sourced): a GPU rental price of $0.30/h per RTX 4090; GPU rates for other Argon2 parameters scaled by m × t; low-end ARM Argon2 timings estimated from the KeePassDroid anecdote and hashcat's CPU figures.

## Independent verification

*Added 2026-09-24 by an independent verifier. I tried to refute the eight claim verdicts above using my own checks: primary source code on GitHub (Loro, RustCrypto `argon2`, AOSP `ClipboardService`, iroh CHANGELOG, localfirst/auth), full-text papers read locally (DCGKA, Kleppmann & Howard's BEC paper, Laurie & Clayton, S/Kademlia), the RFC 9420 text, and the Google API reference. Web search was unavailable (the session's search budget was spent), so every source below was fetched directly. Nothing was installed and no traffic went to Nostr relays.*

### Verdicts

| # | Claim | Original verdict | Upheld? | Notes |
|---|---|---|---|---|
| 1 | 6-word codes + Argon2id | Partial | **Yes** | Every number checks out (see below). One small addition about hyphenated words. |
| 2 | Key epochs | Partial / underspecified, High | **Yes** | The conclusion holds. Two supporting details are overstated or out of date: the RFC 9420 §14 wording, and p2panda's audit status. |
| 3 | Authorship not authenticated | Confirmed, High | **Yes** | The fix is incomplete: signatures stop impersonation but not a member equivocating on its *own* IDs. |
| 4 | Malicious members / untrusted input | Partial, High | **Yes, and stronger** | `catch_unwind` alone does not help: #1106 aborts even with unwinding. The "scratch fork" fix uses the very API that aborted. |
| 5 | Sealed backups / Auto Backup E2E | Partial, Medium | **Yes** | Block Store's per-entry limit is 1,024 bytes in the API reference, not 4 KB. |
| 6 | Helper network trust | Partial, Medium | **Yes** | The quotes and the grinding estimate hold. |
| 7 | Metadata / unlinkable groups | Refuted, Medium | **Yes** | DESIGN.md §9 itself says relays see IPs and endpoint IDs, which contradicts the §7 unlinkability sentence. |
| 8 | Invite secrets in links / clipboard / QR | Partial, High | **Yes** | Add Android's 1-hour clipboard auto-clear, the risk that link scanners trigger a join, and App Links verification. |

### Claim 1: invite codes

- **Math recomputed independently:**
  - log2(7776) = 12.925 bits per word, so 6 words = 77.55 bits.
  - 4 words, N=1e6, 1e5 H/s: 10.2 h.
  - 5 words: 9.0 years.
  - 6 words: 70,050 years at 1e5 H/s, 412 years at 1.7e7 H/s, and 4.1 years at N=1e8 (about $108M).
  - Date-salted at 1e4 invites/day: 41,000 years per success.
  - 59 RTX 4090s give 1e5 H/s.
  - DESIGN.md's "minutes" (4 words) and "days to years" (5 words) are wrong. Its "years to decades" for a 2,048-word list is right (23 years).
- **GPU rate.** I re-read the hashcat v7.0.0 release notes: "GPU, NVIDIA GeForce RTX 4090: 1703 H/s" at m=65536, t=3, p=1. Confirmed.
- **argon2 0.6.0.** From the GitHub source: `Cargo.toml` has `parallel = ["dep:rayon"]` (optional). `lib.rs` has only `cpufeatures::new!(avx2_cpuid, "avx2")` with `compress_avx2`, and there is no NEON path. Confirmed.
- **EFF list.** Re-downloaded: 7,776 unique words, and the list is prefix-free (0 prefix pairs). The hyphenated words are exactly `drop-down`, `felt-tip`, `t-shirt` and `yo-yo`. Note that `down` and `shirt` *are* list words, but `drop`, `felt`, `t` and `yo` are not, so the researcher's "greedy parse works" holds. **Addition:** 1 − (7772/7776)^6 ≈ **0.31% of codes** contain a hyphenated word. A naive split-and-validate parser would reject about 1 in 325 valid codes, so this belongs in the E7 tests.

### Claim 2: key epochs

- **Corrections to the supporting evidence (the verdict doesn't change):**
  - **RFC 9420 §14 doesn't require a Delivery Service to totally order Commits.** The actual text: "Applications MUST have an established way to resolve conflicting Commit messages for the same epoch. They can do this either by preventing conflicting messages from occurring in the first place, or by developing rules for deciding which Commit out of several sent in an epoch will be canonical."
    - In practice this means a server. The DCGKA paper says "MLS typically depends on a semi-trusted server to determine the sequence of commits", and it's "not safe for one subset of users to perform a commit, because a different subset of users may perform a different commit". So "MLS doesn't fit" still holds.
    - The relevant lesson for Kleiders is MLS's canonical-commit rule: the losing branch's removal is discarded and has to be redone. That's the same problem the researcher's "intersection successor epoch" rule solves.
  - **p2panda-encryption is not audited.** The current README says "This crate has not yet received a security audit." The Feb 2025 blog called an audit "pending". Replace "audited-in-progress" in the Claim 2 fix with "unaudited (audit announced 2025)".
- **Confirmed from primary text:**
  - DCGKA (§5.2) takes group membership as a pluggable "Decentralized Group Membership (DGM) function" ("For space reasons we elide a detailed discussion of these algorithms"). It also assumes *authenticated* causal broadcast, so it depends on Claim 3 being fixed first.
  - localfirst/auth `membershipResolver.ts` contains `resolveMutualRemovals` ("mutual and circular removals are resolved by seniority"), `cantAddBackRemovedMember` and `cantDoAnythingWhenRemoved`. Note that localfirst/auth has **admin roles**, and only admins can remove. Kleiders' "anyone can remove anyone" is a stricter case than the prior art covers.
  - p2panda blog: "If two members remove each other at the same time, they will both leave the group."
  - BeeKEM: "the remove paths are blanked after all other concurrent operations are merged".
  - DMLS draft (2025-03-17): "damaging their FS".
  - Keybase: "the PTK must rotate. This can happen lazily."
  - Matrix (Albrecht et al.): "a malicious homeserver can add users under their control…", plus the device-addition attack.

### Claim 3: authorship (also the requested spot-check of this "confirmed" claim)

- **Quotes verified in Loro's source** (`crates/loro/src/lib.rs`, v1.16.2):
  - `set_peer_id`: "Duplicate PeerIDs can produce conflicting OpIDs and corrupt the document" and "Do not assign a fixed PeerID to a user or device without strict single-ownership locking. Prefer the default random PeerID per process/session."
  - `export_json_in_id_span`: "ideal for hash calculations and integrity checks".
  - `ImportBlobMetadata` (in `loro-internal/src/encoding.rs`) has `partial_start_vv` and `partial_end_vv`, which "only contain counters from peers included within the import blob". So the peer-binding check the researcher proposes can be implemented.
- **Divergence confirmed independently** by Kleppmann & Howard, "Byzantine Eventual Consistency" (arXiv 2012.00472, §4.3). A faulty replica makes two messages with the same vector timestamp, so honest replicas "detect that their vector timestamps are equal, and thus incorrectly believe that they are in the same state, even though their sets of messages are different." Loro syncs by version vector, so this applies directly.
- **Gap in the fix.** Binding PeerIDs to device keys and signing blobs stops *impersonation*. It does **not** stop a malicious member **equivocating on its own PeerID**: signing two different blobs for the same (peer, counter) range and sending each to different members. Both signatures are valid, the replicas split permanently, and Loro can't reconcile them.
  - Mitigation: include the content hash of each (peer, counter-range) in the signed blob metadata, and gossip these signed digests. Two valid signatures over conflicting hashes for one range are proof of equivocation, so the author can be excluded.
  - Repair still needs a rule, for example "the lowest hash wins and the other branch is re-applied as new ops by the honest holder", or the hash-DAG option (c).
- **Partly present already.** DESIGN.md already signs at the transport level: Nostr events use per-device keys (§7), and helper "slot data is signed by member keys" (§7a). What's missing is binding the Loro PeerIDs *inside* a blob to that signer, and signatures that survive relaying (member relay layer 6, snapshots).
- **Cost wording.** "Tens of µs" per Ed25519 operation is plausible on modern cores but unmeasured on Cortex-A53-class phones. Either way it's negligible per blob.

### Claim 4: malicious members

- **Confirmed:**
  - UniFFI's manual (GitHub source): "UniFFI can't always catch panics, for example when the Rust panic handler is set to `abort`."
  - `loro-kv-store` 1.16.0 depends on `lz4_flex` 0.11 and `xxhash-rust`.
  - Auto Backup: over 25 MB, "the system calls `onQuotaExceeded()` and doesn't back up data to the cloud." It resumes once the data falls back under the limit, so the DoS lasts only while the list stays bloated.
  - Loro #1088 (opened 3 Sept 2026, closed via PR #1108): snapshots "silently import successfully but crash on first read", giving "a crash loop on every cold start once such a snapshot is persisted".
- **The fix is too weak.**
  - **Loro #1106** (19 Sept 2026, closed via PR #1107) goes `unwrap()` on `None`, then "poisoned LoroMutex", then a **non-unwinding panic in a destructor, then SIGABRT**. A panic during unwinding aborts even with `panic = "unwind"`, so `catch_unwind` can't catch it.
  - The trigger was **`fork_at`** followed by a historical diff. That's exactly the "import into a scratch fork" the fix recommends.
  - **Corrected fix:**
    - Keep `panic = "unwind"` and `catch_unwind`.
    - Also validate untrusted blobs in a **separate OS process**: an Android `android:process=":import"` service, or a child process on desktop. An abort, stack overflow or OOM then kills only that process.
    - Validate by loading into a *fresh* `LoroDoc` from bytes, not with `fork_at`.
    - After any caught panic, throw away the `LoroDoc` instance, because its mutex may be poisoned.
    - Keep the quarantine journal.
- **History-budget shrink (N4) is only partly mitigated.** DESIGN.md's 30-day trash keeps deleted tasks in the state, so a *UI* deletion doesn't shrink it right away. But the trash is an app-level convention: a malicious member can send raw Loro delete ops, or overwrite every field with empty text, and skip the trash entirely. The attack stands against the stated threat model. Medium–High is fairer than High, because desktops keep a larger archive budget.

### Claim 5: backups

- **Confirmed** (Auto Backup docs): client-side encryption is on only on Android 9+ "as long as the user has enabled backup… and has set a screen lock". `disableIfNoEncryptionCapabilities` (Android 12+) and `requireFlags="clientSideEncryption"` (API 28+) both exist. `BlockstoreClient.isEndToEndEncryptionAvailable()`: "available for Pie and above devices with a lockscreen PIN/pattern".
- **Correction:** the `BlockstoreClient` API reference gives `MAX_ENTRY_COUNT` = 16 and **`MAX_SIZE` = 1024** ("Maximum allowed size of byte blobs"). The Block Store guide says "maximum 4kb per entry". Design for **1 KB per entry**. A 32-byte personal-group secret plus a wrapped seal key fit easily, but anything larger (for example several group secrets) may not.

### Claim 6: helper network

- **Confirmed from the PDFs:**
  - Laurie & Clayton: "the limits we would wish to set are of the same order of magnitude as global averages for usage… making the system completely infeasible to deploy."
  - S/Kademlia: "limiting free nodeId generation with crypto puzzles" and "parallel lookups over multiple disjoint paths".
- **Grinding estimate.** Beating the closest honest node among n≈1,000 takes about n key generations, and holding all r slots about r·n. That's microseconds of Ed25519 key generation each, so the "cheap eclipse" point holds. The damage is bounded by DESIGN.md's own design: the bootstrap set is used only for first join, and Nostr runs in parallel. So "Medium (later phase)" is right.

### Claim 7: metadata

- **Confirmed:**
  - NIP-42: "Clients MAY provide signed events from multiple pubkeys in a sequence of AUTH messages. Relays MUST treat all pubkeys as authenticated accordingly."
  - iroh 1.2.0 (released 2026-09-09): endpoints "automatically publish their own addressing information… they could also publish their direct addresses". "Number 0 runs a public instance of a PkarrPublisher". `AddrFilter::relay_only()` and `MemoryLookup` both exist. The EndpointId *is* the endpoint's stable public key.
  - **iroh no longer uses STUN at all.** The CHANGELOG lists "*(iroh)* Remove stun-rs (#3546)", and relays offer a QAD server. So DESIGN.md §6's layer 3 "iroh + public STUN" and §9's "STUN" are out of date.
- **Why "refuted" is right.** DESIGN.md §9 already says Nostr relays see "the uploader's IP" and iroh relays see "IP addresses, endpoint IDs". That contradicts §7's "a device's groups can't be linked to each other" for any relay operator. The claim can hold only against third parties reading public relay data, and even there flushing all groups at the same moment ("leaving the app flushes immediately") creates correlated `created_at` values that such a reader can see.

### Claim 8: invite secrets

- **Confirmed:**
  - Safe Links "rewrites" and "wraps" URLs via `*.safelinks.protection.outlook.com`, checks them at time of click, detonates URLs "that don't have a valid reputation… asynchronously in the background", and offers "Track user clicks". It covers Teams and Office apps (time-of-click, not rewritten), and the Built-in protection preset turns it on for every Defender for Office 365 tenant.
  - Krause (10 Aug 2022): "The Instagram app injects their JavaScript code into every website shown". Any script in the page context can read `location.hash`.
  - AOSP `ClipboardService`: "The default IME is always allowed to access the clipboard."
  - The Windows exclusion formats are exactly as stated.
- **Missed mitigating fact:** AOSP auto-clears the clipboard **1 hour** after a copy (`DEFAULT_CLIPBOARD_TIMEOUT_MILLIS = 3600000`, DeviceConfig `auto_clear_enabled` defaults to true; added in Android 13, and OEMs may change it). That caps how long the system clipboard is exposed. It also creates a **functional cliff**: if install and first launch take more than an hour, the "Get the app" invite is gone. Keyboard clipboard histories (Gboard, SwiftKey cloud clipboard) are separate and keep their own copies.

### Things the researcher missed (for this dimension)

1. **Self-equivocation** survives signing (Claim 3 above). Add signed range digests with equivocation proofs, plus a repair rule.
2. **Aborts that `catch_unwind` can't catch** (double panics, stack overflow, OOM), and `fork_at` itself being the aborting path in #1106. Validate untrusted imports in a separate process.
3. **Link scanners execute pages.** Safe Links detonation and other security sandboxes open the URL and run its JavaScript. If the future web client (or the landing page) ever **joins automatically** from the fragment, a scanner becomes a ghost member or uses up an invite. Always require a user gesture before joining. The same lesson came from email magic-link logins.
4. **App Links verification.** Invite links must be verified https **Android App Links**. Unverified web links and custom schemes go through the disambiguation dialog, where another app can register the same pattern and receive the secret. `assetlinks.json` must be served at the *domain root* `/.well-known/`, which a GitHub Pages *project* site (`user.github.io/repo`) can't do without a user/org site or a custom domain. Jekyll also skips dot-directories unless configured. Verification needs Google services, so the `foss` flavor on de-Googled devices may fall back to the chooser.
5. **GitHub Pages can't set HTTP response headers**, so the recommended CSP can only be a `<meta http-equiv>` tag, which has no `frame-ancestors` support. Keep the landing page script-minimal and self-contained.
6. **Harvest-now-decrypt-later.** Epoch keys wrapped with X25519 and posted to public Nostr relays, which can be archived forever, are the exact target Signal's PQXDH was built against ("designed to prevent 'harvest now, decrypt later' attacks"). The code/HKDF/XChaCha path is symmetric and fine. Because formats are "forever compatible" (principle 8), reserve a hybrid KEM suite ID (X25519 + ML-KEM) in the rotation format now.
7. **No forward secrecy against public archives.** Any later leak of an epoch key decrypts everything archived under that epoch. Periodic rotation (the researcher's PCS fix) is also the only FS bound, so state it that way in §9.
8. **Group-derived pkarr/Mainline records.** If discovery publishes under a key derived from the group (§6), every member *and ex-member* holds the signing key and can overwrite the record. BEP 44 values are capped ("MAY reject… longer than 1000 bytes") and use seq/CAS last-writer-wins, so members clobber each other. The records are readable by anyone who knows the public key. Derive the key per epoch, encrypt the contents, and publish per device rather than per group.
9. **AEAD context binding.** §7's message format puts version and group-ID hash outside the ciphertext. Bind (format version, group, epoch, slot, sender device) as XChaCha20-Poly1305 associated data, so a member can't replay a ciphertext into another slot, epoch or version context.

### Sources used by the verifier

- hashcat v7.0.0 release notes (raw): https://raw.githubusercontent.com/hashcat/hashcat/master/docs/releases_notes_v7.0.0.md
- RustCrypto argon2 source: https://raw.githubusercontent.com/RustCrypto/password-hashes/master/argon2/Cargo.toml, https://raw.githubusercontent.com/RustCrypto/password-hashes/master/argon2/src/lib.rs
- EFF large wordlist (re-analyzed locally): https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt
- RFC 9420 §14 text: https://www.rfc-editor.org/rfc/rfc9420.txt
- DCGKA full text (§2, Table 1, §5.2): https://eprint.iacr.org/2020/1281.pdf
- p2panda-encryption README ("not yet received a security audit"): https://raw.githubusercontent.com/p2panda/p2panda/main/p2panda-encryption/README.md; blog: https://p2panda.org/2025/02/24/group-encryption.html
- localfirst/auth resolver: https://raw.githubusercontent.com/local-first-web/auth/main/packages/auth/src/team/membershipResolver.ts
- Keyhive notebook 02: https://www.inkandswitch.com/keyhive/notebook/02/
- DMLS draft: https://www.ietf.org/archive/id/draft-kohbrok-mls-decentralized-mls-00.html
- Keybase teams crypto: https://book.keybase.io/docs/teams/crypto
- Matrix attacks: https://nebuchadnezzar-megolm.github.io/
- Loro source: https://raw.githubusercontent.com/loro-dev/loro/main/crates/loro/src/lib.rs, https://raw.githubusercontent.com/loro-dev/loro/main/crates/loro-internal/src/encoding.rs, https://raw.githubusercontent.com/loro-dev/loro/main/crates/kv-store/Cargo.toml
- Loro issues: https://github.com/loro-dev/loro/issues/1088, https://github.com/loro-dev/loro/issues/1106
- Kleppmann & Howard, Byzantine Eventual Consistency: https://arxiv.org/pdf/2012.00472
- UniFFI manual source: https://raw.githubusercontent.com/mozilla/uniffi-rs/main/docs/manual/src/internals/rust_calls.md
- Android Auto Backup: https://developer.android.com/guide/topics/data/autobackup
- BlockstoreClient API reference (MAX_SIZE 1024, MAX_ENTRY_COUNT 16): https://developers.google.com/android/reference/com/google/android/gms/auth/blockstore/BlockstoreClient; guide: https://developer.android.com/identity/block-store
- NIP-42: https://raw.githubusercontent.com/nostr-protocol/nips/master/42.md
- iroh docs and CHANGELOG ("Remove stun-rs"): https://docs.rs/iroh/latest/iroh/, https://docs.rs/iroh/latest/iroh/address_lookup/index.html, https://raw.githubusercontent.com/n0-computer/iroh/main/CHANGELOG.md
- BEP 44: https://www.bittorrent.org/beps/bep_0044.html
- S/Kademlia: https://telematics.tm.kit.edu/publications/Files/267/SKademlia_2007.pdf; Laurie & Clayton: https://www.cl.cam.ac.uk/~rnc1/proofwork.pdf
- Safe Links: https://learn.microsoft.com/en-us/defender-office-365/safe-links-about
- Krause 2022: https://krausefx.com/blog/ios-privacy-instagram-and-facebook-can-track-anything-you-do-on-any-website-in-their-in-app-browser
- AOSP ClipboardService (IME access, 1-hour auto-clear): https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/main/services/core/java/com/android/server/clipboard/ClipboardService.java
- Android 13 copy/paste: https://developer.android.com/about/versions/13/features/copy-paste
- Windows clipboard formats: https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-formats
- Android App Links: https://developer.android.com/training/app-links
- Signal PQXDH: https://signal.org/docs/specifications/pqxdh/
