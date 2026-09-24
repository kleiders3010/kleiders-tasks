# Decision list

*2026-09-24. Everything found in the research phase that affects the design, each with evidence, a proposal and **only alternatives that were actually evaluated**. Nothing here has been applied to DESIGN.md. Mark each item **Keep / Change / Drop**.*

Evidence files live in this folder: `R*` = research reports (each with an independent verification section at the end), `E*` = experiments, `A*` = attacks. Confidence says how sure I am. "Not verified" says what I didn't check.

---

## Critical: engine correctness and security

### D1. Device identity must never be cloned (restore, backup, second instance): ✅ KEEP (2026-09-24)
- **Problem:** if two devices ever share a Loro PeerID, their documents **silently and permanently diverge**: every import reports success, and a real edit is lost for one side. The most likely cause is Android Auto Backup or device-to-device restore copying the app's files to a new phone, including PeerID, iroh key and Nostr slot keys. Running two desktop instances does the same.
- **Evidence:** E1 §6 (reproduced), R3 (Loro docs: "Duplicate PeerIDs… corrupt the document"), R4 (Auto Backup / D2D restore clones identity files).
- **Proposal:**
  - Device keys and PeerID live in storage excluded from backup (Android `noBackupFilesDir`, backup rules).
  - A restored install generates a fresh identity and re-joins the user's personal group as a *new* device, using the personal secret restored from Block Store.
  - The desktop app is single-instance.
- **Alternatives:** none serious. This is a hard Loro constraint.
- **Confidence:** high. **Not verified:** exact Auto Backup exclusion behavior on OEM backup services.

### D2. Edits can be forged: sign updates and bind PeerIDs to device keys: ✅ KEEP (2026-09-24; one person = personal group, one key per device underneath)
- **Problem:** a member who uses another member's PeerID makes edits appear to come from the victim and **permanently splits the group's data** (B lost A's real edit). All imports report success.
- **Evidence:** A5 §1 (reproduced), R7 claim 3, R3.
- **Proposal:**
  - PeerID = 64-bit hash of the device public key.
  - Every update blob is signed by the device.
  - A device may only publish its **own** ops, checked after importing into an isolated copy (D3): only the sender's PeerID may advance. Verified: Loro reports exactly which peers a blob advanced (A5, A6 prototype).
  - **Consequence:** the design's shared "snapshot slot" (one member publishing everyone's ops) can't be verified per author and is also vandalizable (R7 N2). It's replaced by per-device slots plus **document epochs** (D24): a member proposes a fresh document built from live state, and every member switches only if the new state equals its own.
- **Alternatives evaluated:** per-author signed attestations inside shared snapshots. **Not viable:** it needs deterministic hashing of a peer's op range, which Loro doesn't expose (A5 §1b).
- **Confidence:** medium-high. **Not verified:** the epoch state-equality check on large docs (performance) and in the engine.

### D3. One malformed update crashes every member's app, in a loop
- **Problem:** fuzzing Loro 1.16.2 import (with the checksum recomputed, as any member can) **aborts the process** within seconds. 4 distinct crash sites; smallest input 85 bytes. A double panic (poisoned lock) means `catch_unwind` can't catch it. In a mailbox slot, that's a crash on every sync and launch for every member. Loro has had similar bugs reported this month (#1088, #1106).
- **Evidence:** A6 (fuzzer, 40/40 sessions abort, crash inputs saved), R7, R3.
- **Proposal:**
  1. **Import untrusted data into a copy of the live doc in a disposable process first** (Android: service in its own `android:process`; desktop: child process). Only then import into the live doc. Prototype: the crash stays in the child and the parent survives. Overhead ~18 ms per validation on desktop. The same check enforces D2.
  2. **Crash-loop breaker:** persist "importing X from Y" before importing; after a crash, quarantine X and flag Y.
  3. Release builds use `panic = "unwind"` (so non-aborting panics become errors through UniFFI), not `abort` as §10 says.
  4. Report the crash inputs to Loro upstream. **This posts publicly on GitHub, so it needs your OK.**
- **Alternatives evaluated:** in-process `catch_unwind` only. **Tested, fails** (the double panic aborts).
- **Confidence:** high that (1) contains crashes. **Not verified:** the cost of a separate process on Android, especially inside background jobs.

### D4. No size limits: one member can push 50 MB to everyone
- **Problem:** a 50 MB text insert became a 50 MB update, which every member imported in 0.1 s. P2P has no cap. A list over 25 MB also **silently disables Android Auto Backup** for every member (R7).
- **Evidence:** A5 §3, R7.
- **Proposal:** engine limits before import:
  - max update size (e.g. 1 MB);
  - max document size (e.g. 20 MB, under the Auto Backup quota);
  - max text per field;
  - per-author rate limits.
  Oversized updates are rejected and the author is flagged. The numbers are placeholders.
- **Alternatives:** none serious.
- **Confidence:** high.

### D5. Key-rotation rules (removing devices and people)
- **Problem:**
  - Without a central orderer, concurrent removals fork the key history, and each branch's key goes to the other branch's removed member.
  - Removed members can backdate edits.
  - **Anyone holding any old or leaked secret can add a "ghost" device** that then receives every future key (the same vulnerability class Matrix had).
  - The design specifies none of this.
- **Evidence:** R7 claim 2 (prior art: DCGKA, Keyhive/BeeKEM, p2panda-encryption, localfirst/auth; MLS doesn't fit). **E8 simulation:** 441/3,000 randomized runs leaked keys to removed devices until rule 4 below was added; then 3,000/3,000 passed.
- **Proposal (rules):**
  1. Strong remove: a removed device can never be re-added.
  2. Concurrent key histories merge into one successor whose recipients are the *intersection*, created by a deterministic merger.
  3. Additions are valid only when signed by a current-head recipient (no ghosts).
  4. **Every new key epoch excludes every removal its creator knows about.**
  5. A removal carries the remover's version vector; the removed author's ops outside it are rejected (needs D2).
  6. All network-visible identifiers (Nostr tags, DHT keys, slot keys) derive from the *current epoch key* and rotate daily.
  7. Periodic rotation (~30 days, and on every device add) heals silent leaks.
  8. Block Store is updated right after the personal group rotates.
- **Also from the synthesis (FINDINGS #5):**
  - bind version, group, epoch, slot and sender as AEAD associated data;
  - reserve a hybrid post-quantum key-exchange suite ID now (cheap now, hard to add later);
  - notify all members of every join;
  - p2panda-encryption / Keyhive are candidate rule sets to adopt (p2panda is unaudited).
- **Side effect to accept:** with no roles, mutual removal empties small groups (A removes B while B removes A). 38/3,000 in the simulation.
- **Alternatives evaluated:** MLS (needs a totally ordered server, so no). Keeping the current underspecified design isn't safe.
- **Confidence:** medium (simplified model; see E8 limits).

### D6. Device linking can be phished
- **Problem:** in 2025, state actors hijacked Signal accounts with linked-device QR codes disguised as group invites (GTIG). Here it's worse: with the "linking merges your lists" rule, a victim who scans an attacker's *link* QR thinking it's a list invite **merges their whole personal group into the attacker's**. The attacker approves on their own phone and receives every current and future list.
- **Evidence:** R6 claim 9 (upheld by its verifier as critical).
- **Proposal:**
  - Link QR codes are accepted only from "Your devices → Link this device", **never** from the general scanner or the join path.
  - A full-screen warning on the *scanning* device ("This gives ALL your lists to the device showing this code").
  - A "New device linked" notification on all existing devices, with one-tap removal (uses D5).
- **Alternative evaluated:** requiring proximity (same network / Bluetooth). Stronger against remote phishing, but it blocks legitimate linking across networks and adds permissions (local network / Nearby).
- **Disagreement to flag:** my lean was to hold proximity in reserve. The synthesis (FINDINGS #3) recommends **local-only linking by default**, with internet linking behind an extra warning, and **never silently merging a non-empty device** (which touches your earlier "linking merges your lists" decision). Both positions rest on the same evidence; it's a convenience vs safety call.
- **Confidence:** medium-high.

### D7. Links and QR codes carry the permanent group secret: every leak is forever
- **Problem:** the invite link contains the group secret. It's kept by keyboard clipboard history (and some keyboards' cloud sync), Windows cloud clipboard, browser history and sync, chat providers and link scanners, in-app browsers that inject JavaScript, and photo backups of QR codes. A leak years later still works. If the link's host (GitHub Pages / domain) is ever taken over, the new owner can serve JavaScript that reads the secret from any old link opened in a browser (R8).
- **Evidence:** A8, R7 claim 8, R8, R4.
- **Proposal:** links and QR codes carry a **short-lived invite**, the same mechanism as codes but with a 256-bit key (no Argon2): `invite-id + key`, which unlocks an invite record in the mailbox containing only the **current epoch key**. Leaked links die when the invite expires. Expiry lengths are a product call (codes 1 day; links maybe 7 days).
- **Alternative evaluated:** keep the secret in the link (current design). Pros: works with no mailbox reachable, never expires, simpler. Cons: every leak is permanent, including leaks through the app's own clipboard trick.
- **Confidence:** high on the security gain.

### D8. Invite codes: typos fail silently with the current word list
- **Problem:** in the EFF large list, **34% of words turn into another valid word with one typo**, so 92% of 6-word codes contain at least one silently mistypable word. The app can only say "not found". The list also has 4 hyphenated words (they clash with the separator), 13+ homophone pairs, rare words and US-only spellings.
- **Evidence:** E7, E7b (`research/experiments/e7b_curated_list.py`), R6 claim 5.
- **Options (all evaluated):**

  | | (a) 8 words, EFF short 2.0 | (b) 6 words, curated EFF large | (c) current: 6 words, EFF large |
  |---|---|---|---|
  | Strength | 82.7 bits | 75.9 bits | 77.5 bits |
  | Typo turns into another valid word | never | never (curated out) | 34% of words |
  | Autocomplete after | 3 letters, every word | ~5 letters on average (up to 7) | not unique |
  | Keystrokes | ~24 | ~28–30 | ~47 |
  | Read aloud | 8 words (~20 syllables) | 6 words (fewer syllables) | 6 words |

  The curated list (b) = remove hyphenated words, known homophones, and any word within one typo of another (6,405 words remain).
- **Also (R7), for all options:**
  - salt the code derivation with the **invite's date**, so an attacker can only test guesses against invites from the same day, not every invite ever collected;
  - put only the current epoch key in the invite.

  Both are big security gains at no UX cost.
- **Security with date-salting:** all three options become far stronger than the current design's estimate, since an attacker targets one day's invites instead of millions. **Not recomputed** per option yet.
- **My lean:** (b). It fixes typos with the shortest spoken code and needs no word-count change. But (a) is stronger and faster to type, so this is a genuine trade-off: **spoken length vs typed length**.
- **Not verified:** real-user typing and reading error rates (R6 suggests a 20+ person code-entry test including non-native speakers).

### D9. Argon2 parameters for code derivation
- **Evidence:** E2 / A1 (RTX 3080: 783 guesses/s at m=64 MiB t=3; 81/s at 256 MiB), R7 (RTX 4090: 1,703/s at 64 MiB). Desktop cost ~190 ms; low-end phone estimated ~0.8 s.
- **Proposal:** m=64 MiB, t=3, p=1 as the floor. Parameters are versioned in the invite format so they can rise later.
- **Alternatives evaluated:** 256 MiB (10× harder for attackers, but risky on 2–3 GB phones).
- **Not verified:** a real low-end phone's timing.

---

## High: sync and transport

### D10. Nostr event kind: not NIP-78 (my own earlier recommendation, withdrawn)
- **Problem:** since **2026-09-03**, NIP-78 says relays SHOULD serve kind 30078 **only to its author** (AUTH-gated). A mailbox read by other members breaks on relays that follow it. Some already do (Ditto).
- **Evidence:** R2 claim 3. E4 probe passed only because major relays haven't adopted it yet.
- **Proposal:** a dedicated addressable kind. The custom kind 31234 behaved identically on all 8 relays probed.
- **Alternatives:** none serious.
- **Confidence:** high.

### D11. Nostr rate limits: "rewrite the slot every few seconds" gets our users banned
- **Problem:**
  - Damus's filter allows about **8 posts/min per IP** and bans an IP for **1 h after 10 rejections**, which blocks reads too.
  - khatru's defaults allow 2 events per 3 min.
  - Phones behind carrier-grade NAT share one IP with strangers.
  - The design's write-every-few-seconds, flush-all-groups-on-exit and chunked snapshots can each exceed this. One app was already observed getting banned for exactly this pattern (Sept 2026).
- **Evidence:** R2 claim 2.
- **Proposal:**
  - A per-relay write budget: e.g. ≤ 1 event/min sustained, small bursts, a priority queue. Live collaboration goes over P2P anyway.
  - **Each group uses only a subset of relays** (e.g. 3 of 8, chosen from its epoch key), which cuts per-relay load.
  - Always back off on rejection.
  - Synthesis specifics (FINDINGS #7): ~6 events/min per relay **shared across all groups**, burst 5; foreground slot rewrites every 30–60 s with jitter; stop at the first `rate-limited:` and back off exponentially; a strictly increasing `created_at` per slot.
  - **"OK" doesn't mean stored:** judge relay health by reading events back.
  - Update the relay list at runtime from a **signed** list.
- **Alternative evaluated:** bundle several groups into one event. Fewer events, but it links groups openly and breaks per-group addressable replacement. Worse.
- **Confidence:** medium (the exact limits vary by relay and change over time).

### D12. Event size: ≤ 64 KiB, not 128 KB
- **Evidence:** R2 claim 5 (strfry's default `maxEventSize` is 65,536; NIP-11 advertises the websocket frame size), E4 (60 KB accepted).
- **Proposal:** design for ≤ 64 KiB of event JSON (~47 KiB of payload), with chunking: chunks of ≤ 40 KiB carrying generation/index/count/hash, switched atomically between two buffers (FINDINGS #7). Document epochs (D24) keep most lists in 1 chunk.

### D13. Relay operators dislike "relays as storage", and most relays run the same software
- **Evidence:**
  - R2 claim 10: Damus's filter blocks file kinds ("files on nostr is dumb"); strfry began AUTH-gating some kinds by default in Aug 2026; our kind would be blockable with one config line.
  - R5: Delta Chat left free third-party infrastructure for the same reasons.
  - E4: 6 of 8 probed relays run strfry.
- **Implication:** Nostr works today and could stop working at scale with little warning. This supports D11 (be a quiet citizen) and makes member-device mailboxes (tier 1) and your own always-on machine the most dependable offline paths.
- **Decision needed:** whether you accept this risk as-is for launch. **I don't have an in-constraints alternative that replaces Nostr for phone↔phone offline delivery.** The helper network (D20) isn't one at launch scale.

### D14. iroh's internet path depends on one company's free "development and hobby" service
- **Problem:**
  - n0's public relays are documented as "for **development and hobby use only**", with no SLA, rate limits that are tightening, and a right to block.
  - iroh no longer uses STUN; address discovery and hole-punch coordination **also** run through those relays.
  - Default discovery uses n0's DNS server.
  - No independent public iroh 1.x relay exists. Principle 2 (≥ 2 providers) fails for relaying, hole-punch coordination and discovery.
- **Evidence:** R1 claims 1–3 (verifier upheld: "Refuted, breaks a §1/§2 hard constraint"), E3 (iroh 1.2 source: the N0 preset = n0 relays + n0 DNS), R5 P12/P13 (IPFS circuit relays can't be a second provider: 128 KiB / 2 min limits).
- **Options evaluated:**
  - (a) Use n0's relays best-effort, and do discovery ourselves through Nostr (an encrypted "where am I" slot) plus the Mainline DHT via pkarr. Accept that internet P2P may be throttled or blocked, with Nostr carrying delivery when it is. **This arguably uses n0's service outside its stated terms.** Asking n0 is outward contact and your call.
  - (b) n0's paid plans: violates C2.
  - (c) Self-hosted relays: violates "no hosting".
  - (d) Users' home desktops as relays: an iroh relay needs a public address and TLS, which home PCs behind NAT don't have. Not feasible.
- **Honestly, (a) is the only option inside the constraints, and it's weaker than the design assumed.** LAN and direct connections work without n0.
- **Added by the synthesis:**
  - shipping on relays against their stated terms is also a Play "Device & Network Abuse" exposure, which strengthens the case for asking n0 in writing (your call: outward contact);
  - make the relay list configurable and health-tracked;
  - use a rotating transport key certified by the device key (also helps D23);
  - make member relaying and desktop members the primary live fallback.
- **Confidence:** high on the facts.

### D15. Realistic hole-punching success is lower than the design says
- **Evidence:** R1 claim 6. "~90%" is n0's own number. An independent study of a comparable technique measured 70% ± 7%. Phone↔phone on carrier-grade NAT typically falls back to the relay, which is D14's weak point.
- **Implication:** no decision needed; it lowers expectations for internet P2P between phones. Affects wording and D17.

### D16. Android background behavior is stricter than §8 assumes
- **Evidence:** R4 (upheld by its verifier with AOSP source):
  - Android 15+ cuts background network ~5 s after leaving the app.
  - Android 14+ freezes the process 10 s after it becomes cached and kills its sockets.
  - Deep Doze runs jobs only in maintenance windows 1 h → 2 h → 4 h → 6 h apart. **A widget doesn't exempt the app from Doze**; since Android 16, even the ACTIVE bucket has a job quota.
  - The `shortService` notification deferral is rate-limited (it shows immediately if used again within ~2 min).
- **Proposal:**
  - An **expedited WorkManager job** as the primary "flush on leave" mechanism: no notification, no FGS declaration, bypasses Doze network limits. Keep `shortService` only as a fallback.
  - Correct the user-facing wording: "usually within 15 minutes" isn't true overnight or on idle phones. Something like "can take a few hours when your phone is idle."
- **Alternatives:** none serious (platform facts).
- **Confidence:** high.

### D17. Nobody learns that a shared list changed while their app is closed
- **Problem:** because of D16, "Alex added eggs" can arrive hours late on an idle phone. Competitors sell exactly this moment ("your partner sees it immediately, even at the store"). R6's strongest UX finding: the most common complaint even about server-based apps is "shared list doesn't sync".
- **Evidence:** R6 claim 3, R4.
- **Options evaluated:**
  - (a) **No infrastructure:**
    - honest per-member freshness ("Alex's phone: last synced 2 h ago");
    - a one-tap **"Tell Alex"** that opens your usual messenger with "I updated Groceries". The other person's messenger push wakes them, and opening the app syncs.
  - (b) **Revisit the doorbell** (FCM through a free stateless worker). **You declined this earlier.** The new evidence is only that the delay is longer than we assumed (hours, not 15 min) and that this is the #1 complaint category.

  I'm listing (b) because of the new evidence, not to relitigate. Your call.
- **Note:** the synthesis (FINDINGS #9) goes further than I did: it recommends *designing* the content-free doorbell now (UnifiedPush / FCM) even if it's built later. It also recommends that notifications come only from merged state (batched, showing their age, dropping stale ones).
- **Confidence:** medium on how much (a) satisfies users. **Not verified:** a real diary study (R6 suggests one).

### D18. Don't say "up to date"
- **Evidence:** R6 claim 4. Anytype users saw "Synced" on two devices showing different data. In P2P, "up to date" can't be known.
- **Proposal:** WhatsApp-style states derived from version vectors: **Saved → Sent → On Alex's phone**. This changes the UX principles' "up to date" wording.
- **Alternatives:** none serious.

### D19. Drop phone helpers for strangers
- **Evidence:**
  - R8: Play's Device & Network Abuse policy allows "proxy services to third parties" only when that's the app's core purpose.
  - R8 verifier: the FGS fallback **can't even start** under background-start restrictions.
  - R4: "charging" in JobScheduler means ≥ 90% or rising; the process is frozen between jobs.
  - R5: participation.
- **Proposal:** phones help only their **own** groups (tier 1, already in the design). Helping strangers is desktop-only.
- **Alternatives:** none viable on Play.
- **Confidence:** high.

### D20. The helper network: keep as research, or drop?
- **Evidence against:**
  - R5: no precedent of volunteers serving offline phones. Opt-in operator participation is 0.01–0.3% (Syncthing ~100 relays / 1M devices).
  - Churn is correlated by time zone, and "trust through people" makes it worse.
  - Patient Sybils collect metadata and get *rewarded*.
  - E5: r ≈ 20 needed.
- **Evidence for:** R5's own verifier notes that **default-on, in-app** participation (like BitTorrent clients running DHT nodes) reaches far higher rates than opt-in operators. E5: at r ≥ 20 it tolerates 50% dishonest helpers.
- **Options evaluated:**
  - (a) Keep as a later-phase research track: desktop-only, default decided later, redesigned for correlated churn and ASN / time-zone diversity.
  - (b) Drop it; rely on Nostr + member devices + your own always-on machine.
- **My lean:** (a), but it's not the answer to Nostr risk at launch scale (D13).
- **Confidence:** low that it ever reaches the needed scale.

### D21. Android 17 local-network permission is broader than the design assumes
- **Evidence:** R4 (verifier upheld). It covers incoming and outgoing, multicast, mDNS, CGNAT ranges, and **same-LAN IPv6**. Requesting `NEARBY_WIFI_DEVICES` on SDK 36 is unnecessary.
- **Proposal:** fix the permission table; request `ACCESS_LOCAL_NETWORK` only at targetSdk 37, contextually as planned.

### D22. Images in tasks: postpone
- **Evidence:**
  - R1: iroh-blobs "not yet production quality", with breaking releases every 3–6 weeks.
  - R2: free Blossom servers are media-only or delete after 60 days, with a reference retention of 1 week.
- **Proposal:** keep the design (ThumbHash previews, silent failure) but postpone until iroh-blobs is stable.
- **Alternatives:** none evaluated that fit the constraints.

### D23. Metadata: groups are linkable, so the privacy wording must change
- **Evidence:** A7, R2 claim 8, R7, R1 claim 10.
  - Relays link a device's groups through IP, connection, timing, query filters and NIP-42.
  - The single iroh endpoint ID is published to n0's DNS with the device's relay (and optionally IPs), so **anyone who ever learned it, including ex-members, can track the device's presence**.
  - Static group-derived tags let ex-members watch activity.
- **Proposal:**
  - Rotate every network-visible tag from the epoch key, daily (D5 rule 6).
  - Correct §9 to "relays can see which groups a device belongs to".
- **Option evaluated for the endpoint ID:**
  - (a) a separate iroh endpoint per group: unlinkable, but N endpoints, more sockets and battery (**unmeasured**);
  - (b) accept and document.

  This is a values call.

---

## Storage and history

### D24. Deleted tasks never leave a Loro document: use document epochs
- **Evidence:** E1 §2. With 20 live items, 2 years of churn left 114–171 KB of state in every Loro model, versus **1.1 KB** for a fresh document with the same live content. Shallow snapshots don't remove it.
- **Proposal:** "document epochs": a member proposes a new document from live state, every member verifies equality and switches (combined with D2). Members that were offline use the rebase path, confirmed working in E1 §3. This also makes the "trash purged after 30 days" promise real, and keeps Nostr slots in one chunk.
- **Alternatives:** none (a Loro constraint).
- **Confidence:** high on the mechanism. **Not verified:** the switching protocol in a real engine.

### D25. "History thinning into checkpoints" can't be built inside one Loro document
- **Evidence:** R3 (verifier upheld; `fork_at` on shallow docs returns `NotImplemented`), E1 §4.
- **Options evaluated:**
  - (a) full history within a window, then a document epoch (older history gone);
  - (b) (a) **plus** periodic live-state checkpoints stored as separate small files (after D24, a live state is KBs, so monthly checkpoints are cheap).
- **My lean:** (b).

### D26. Compact only at versions every active device has acknowledged
- **Evidence:** R3: shallow snapshots permanently exclude concurrent in-flight edits, and dependent changes can sit "pending forever, silently".
- **Proposal:** document epochs / compaction only at a version acknowledged by all devices active in the last N days; others use the rebase path. The engine must detect "pending forever".

### D27. Mass deletion must not shrink the history budget
- **Evidence:** R7. A mass delete shrinks a content-based budget, and the next compaction destroys the undo. A5 §2: undo works while history exists.
- **Proposal:** a minimum 30-day window regardless of size, plus "undo changes by [person] since [time]".

### D28. "Backward compatible forever" is our job, not Loro's
- **Evidence:** R3. Loro dropped 0.x decoding in 1.9.0, readers have become stricter, and an old reader rejects a whole blob that contains an unknown nested value type.
- **Proposal:**
  - pin Loro versions;
  - keep a golden corpus of every format ever written, tested in CI;
  - a per-group "feature floor": don't use new container types until every device in the group supports them.
- **Why it matters beyond display (FINDINGS, R3-3):** an old client that publishes a full snapshot of a document containing unknown container types **loses that data and desyncs newer clients**. D2 (own-ops-only, no shared snapshots) removes the main path. The feature floor covers the rest.

### D29. Loro pulls in MPL-2.0 and unmaintained crates
- **Evidence:** R3. `im`, `sized-chunks` and `bitmaps` are MPL-2.0+ and unmaintained, with RUSTSEC advisories including an unpatched HIGH one. This conflicts with principle 9 ("MIT, Apache or BSD only").
- **Options evaluated:**
  - (a) amend principle 9 to allow MPL-2.0 (file-level copyleft: fine for closed source when those files are unmodified), and assess whether the advisory's code path is reachable.
  - (b) replacing Loro: **not evaluated** (Automerge would be the candidate; I haven't measured it).
- **`cargo audit` (loro 1.16.2 tree), run 2026-09-24:**
  - `im` 15.1.0: unmaintained (RUSTSEC-2026-0248) + *aliasing violation in `OrdSet` insertion* (RUSTSEC-2023-0126).
  - `sized-chunks` 0.6.5: unmaintained (RUSTSEC-2026-0251) + **"panic-safety unsoundness… use-after-free / double-free"** (RUSTSEC-2026-0255).
  - `bitmaps` 2.1.0: unmaintained (RUSTSEC-2026-0247).
  - `atomic-polyfill` 1.0.3: unmaintained.

  RUSTSEC-2026-0255 triggers only **when a panic happens inside those collections**, and A6 showed crafted updates *do* cause panics inside Loro import. Memory corruption during a panic is exactly the case an in-process `catch_unwind` would then keep running on. **This strengthens D3 option 1:** untrusted imports belong in a disposable process.
- **Not verified:** whether Loro's panic sites actually run inside `sized-chunks` code paths.

---

## Backup and recovery

### D30. Auto Backup isn't end-to-end encrypted without a screen lock
- **Evidence:** R7. E2E needs Android 9+ **and** a PIN/pattern/password. Otherwise Google can read it.
- **Options evaluated:**
  - (a) `disableIfNoEncryptionCapabilities="true"`: no Auto Backup for those users;
  - (b) seal the backup when there's no lock, consistent with your earlier sealing decision.

### D31. Block Store only restores during new-device setup
- **Evidence:** R4 (verifier upheld). Users who skip restore during phone setup get nothing, so the main shared-list recovery path fails silently.
- **Contradiction:** R4 says Block Store entries are 4 KB; the synthesis cites the API reference at **1 KB**. Design for ≤ 1 KB (the personal secret fits easily). Also (synthesis):
  - cloud restore on non-Pixel phones works only from Android 12;
  - Keystore-wrapped secrets restore as undecryptable data.
- **Proposal:** at first launch on a new phone with no restored data, show one short prompt suggesting linking from another device or restoring a cloud backup. The synthesis agrees and adds: make device linking the default recovery, and show backup state under "Your devices". **Not evaluated further;** there's no API to pull Block Store data later on a new device.

### D32. Backups hold every group's live secret
- **Evidence:** R7. A sealed backup is readable by anyone who reverse-engineers the format once.
- **Implication:** periodic key rotation (D5 rule 7) limits how long a stolen backup stays useful. No separate decision.

---

## UX

### D33. Name tags like "Alex · 4F2A" don't help
- **Evidence:** R6. Discord dropped discriminators after finding >40% of users didn't know theirs. The likeliest duplicate is the *same person's* unlinked second device.
- **Proposal:** "Is this your other device?" detection.
- **Note:** R6 also cites Signal adding private nicknames (2024). **You dropped nicknames**; I'm only recording the evidence.

### D34. The clipboard install trick is fragile
- **Evidence:** R4, R6.
  - Read only with window focus.
  - Toast on Android 12+.
  - Auto-cleared after 1 h on 13+.
  - Overwritten by any later copy.
- **Proposal:** check the clip description first (no toast); pass a **secret-free** "came from an invite" flag via Install Referrer to show a "tap your invite link again" screen. With D7, a clipboard leak is time-limited anyway.

### D35. Smaller UX fixes
- Widget time as an absolute ("Updated 14:05"), not "X ago", which freezes between redraws (R6).
- Late merges shouldn't move a row under the user's finger (R6).
- Glance backports checkboxes below Android 12, so that fallback isn't needed (R4).

### D36. Validation plan
- **Evidence:** R6. 5-person playtests find flow problems but not sync problems.
- **Proposal:** add a multi-day diary study with couples across OEMs, and a 20+ person code-entry test including non-native speakers.

---

## Policy, distribution, accounts

### D37. Android developer verification: check your Play Console soon
- **Evidence:** R8, plus my check of developer.android.com:
  - **2026-09-30** is the first *regional* enforcement date (Brazil, Indonesia, Singapore, Thailand); global rollout in 2027.
  - Play auto-registers ~99% of apps, **including your existing games** (check for any it missed).
  - Package names with few installs are registered first-come-first-served, so **reserving `com.kleiders.tasks` early** protects it.

### D38. Package name vs domain: ✅ DECIDED: `com.jurisgames.tasks` (2026-09-24)
- **Evidence:** R8. kleiders.com serves an unrelated AI fashion-design company. `com.kleiders` is your Play package prefix (your games), not a domain you control.
- **Play doesn't require package names to match a domain.** The mismatch matters only for:
  - Flathub (app ID must be a controlled domain; likely moot per D40);
  - possibly F-Droid (R8's claim; **I haven't verified that F-Droid rule**);
  - trademark risk (an active company brands as "Kleiders"; **not researched further**).
- **Options (both evaluated):**
  - (a) keep `com.kleiders.tasks`, consistent with your games;
  - (b) `com.jurisgames.tasks`, matching a domain you own.

  Either is free to choose **now**; it becomes permanent at first upload. Desktop IDs can use jurisgames regardless.
- **Either way:** use **jurisgames.com** for invite links and App Links (they work with any package name). That also removes the `github.io` host-takeover concern in D7/D41.

### D39. Windows: unsigned installers get blocked; the Microsoft Store is free *(your stated priority: GitHub Releases for PC / Linux / APK; this item stays open for when the Windows release happens)*
- **Evidence:** R8 (verifier upheld and strengthened).
  - Unsigned MSI → strong SmartScreen block.
  - Signing costs $120–300/yr and still warns at first.
  - The **Microsoft Store is free for individuals** (since 2025-09) and re-signs MSIX.
  - jpackage can't produce MSIX.
  - MSIX virtualizes `%APPDATA%` and deletes it on uninstall.
- **Proposal:** the Store as the primary Windows channel. Pick an MSIX packaging tool (**not evaluated yet**).

### D40. Flathub bans AI-generated manifests and scrutinizes AI-generated code: ✅ DECIDED: Flathub not pursued (2026-09-24)
- **Evidence:** R8. "Manifests must not contain AI-generated or AI-assisted content"; AI-generated code must be disclosed and may be rejected. This project is AI-assisted.
- **Correction (2026-09-24):** "likely closed" overstated it. The policy requires a **human-written manifest**, **human submission**, and **disclosure** of AI-generated code, with discretionary rejection "based on the extent or role of generated material". It isn't a ban on AI-assisted apps. Big apps like Spotify are unaffected because their Flathub packages are community-written wrappers around official binaries, which Flathub doesn't review. For us: possible, with honest disclosure and some rejection risk, after months of release history.
- **Implication (original):** Flathub is likely closed to us. Linux via AppImage and .deb/.rpm on our own releases. Flatpak outside Flathub is possible but loses discoverability.

### D41. GitHub Pages needs a public repo
- **Evidence:** R8. Free-plan Pages requires a public repo; private release assets aren't public; the unauthenticated API allows 60 requests/h per IP.
- **Proposal:** a separate **public** repo for the join page, releases and a static update manifest. The code repo stays private.

### D42. OneDrive: postpone
- **Evidence:** R8.
  - Microsoft app registration now needs an Azure (Entra) account with credit-card identity verification.
  - Publisher verification is out of reach for individuals.
  - **Since ~2026-08-23**, newly consented personal accounts get `403 serviceReadOnly` on `Files.ReadWrite.AppFolder` (reported by several developers, unacknowledged by Microsoft).
- **Proposal:** Google Drive only for now. **Good news:** `drive.file` needs **no** Google verification (non-sensitive scope), just an "In production" status.
- **Caveat:** Google sign-in for the F-Droid flavor is **likely impossible** (custom URI schemes are no longer supported for Android OAuth clients).

### D43. Exact alarms: extra cases
- **Evidence:** R8.
  - The permission is denied after a backup-restore onto a new Android 14+ device.
  - Revoking it kills the app and cancels all exact alarms.
- **Proposal:** re-request contextually after restore, and reschedule on grant.

### D44. Who holds the keys if you step away
- **Evidence:** R5. The Syncthing Android fork's maintainer vanished in 2025, leaving signing-key custody unclear.
- **Proposal:** back up the signing keys and write a succession note. "Backward compatible forever" needs it.

### D45. Invitees on iPhone have no path (new, from the synthesis)
- **Problem:** a shared list's value depends on the *other* person joining. iOS is ~60% of US mobile use (R6-1). The design has no iOS app, and the invite page offers only Play.
- **Options evaluated:**
  - (a) **Web guest client** (already an "extra" in the build order). Nostr works natively in browsers; iroh in WASM is relay-only and depends on n0's dev relays (R1), so a web guest would sync through Nostr only.
  - (b) **iOS app later.** The Rust engine is portable, but it's a second UI and a paid Apple developer account ($99/yr, conflicts with C2).
  - (c) **Accept Android + desktop only,** and say so on the invite page.
- **Confidence:** high that it matters for adoption; the choice is a product call.

---

## Engineering notes (no decision needed; recorded for the build)
- **iroh on Android:**
  - initialize the JNI context before creating an endpoint (otherwise it panics or silently uses 8.8.8.8);
  - call `Endpoint::network_change()` from a `ConnectivityManager` callback (iroh's Android monitor is a no-op);
  - shut the endpoint down in the background;
  - take a `MulticastLock` for mDNS (R1).
- **Build:** our NDK 29 build is already 16 KB page-aligned (verified). Don't use the published iroh-ffi AAR (4 KB-aligned, fails Play).
- **Size:** measured Rust core 7.07 MB installed / 4.33 MB compressed on arm64 with the size profile. Without the profile, R1's figures suggest ~2–3× larger (E6).
- **Loro:** require ≥ 1.16.2; restoring a deleted tree node creates a new TreeID; moving a task between lists is a cross-document copy (R3).
- **Nostr:** NIP-40 and NIP-09 support varies (e.g. nostr-rs-relay ignores `a` deletions); clock skew limits range from 30 s to 30 min, so a device whose clock ran fast can be locked out of its own slot (R2).
