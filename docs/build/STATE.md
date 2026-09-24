# Prototype build: checkpoint

**Every loop iteration: read this file first, do the "Next action", update this file, commit and push.**

**Goal:** a fully functional prototype of everything technical in `docs/DESIGN.md` (v2), without polished UX. Milestones M1–M5; each ends runnable and tested.

## Rules
- Work **headlessly**: no GUI windows. Emulators run with `-no-window`. The user accepts the performance cost.
- **DESIGN.md is the spec.** Where implementation forces a design change, record it here under "Design deviations" and tell the user. Never silently diverge.
- Pending design items (D48, D49, D51, D25, images, "More often") get **pluggable slots**, not full implementations, until decided.
- **Tests:** simulated-network harness for sync logic; property tests for the key-epoch rules; fuzzing for parsers. Heavy network tests only against **local** instances; public relays get at most a few polite test events.
- Dependencies are pinned to exact versions (principle 8). Release profile uses `panic = "unwind"`.
- The Rust workspace lives at the repo root (`crates/`) and excludes `research/`.

## Milestones

### M1: Engine core (Rust)
- [x] Workspace, crates (`kt-engine`, `kt-tasks`, `kt-cli`), pinned deps, CI-style test script
- [x] Crypto primitives: Ed25519 signing, X25519 key wrapping, XChaCha20-Poly1305 with associated data, HKDF derivations, BLAKE3 hashing
- [x] Device identity + PeerID binding (D1); identity store separate from backed-up data
- [x] Personal group and people (display name, list index): `kt-tasks/src/app.rs` (linking semantics in M3)
- [x] Groups and key epochs with the E8 rules 1–8 validated by every receiver (D5)
- [x] Documents (Loro, pinned 1.16.2): group doc + list docs; signed update envelopes; own-ops-only check (D2)
- [x] Import sandbox: validate in a disposable child process; quarantine journal / crash-loop breaker (D3)
- [x] Storage: atomic writes, snapshot + update log, per-group layout
- [x] Soft limits: piece splitting, priority lanes, per-author throttling (D4)
- [x] Document epochs: propose / verify / switch; rebase path; removal cutoffs; concurrent proposals (D24, D26): `kt-tasks/tests/doc_epochs.rs`
- [x] Simulated-network harness + property tests (convergence, removal, impersonation, poison containment): `crates/kt-engine/tests/sim.rs`
- [x] `kt-cli` (`kt`): lists, tasks, share / join bundles, send / recv message files, epochs, info. Two-device demo verified

### M2: Transports
- [x] Mailbox slots in the engine (`replica/slots.rs`): sealed per-device slots under rotating topics, forward slots across key epochs, epoch records first, signed per-document status, previous-generation carry
- [x] Capability-based mailbox interface (`kt-net/src/mailbox.rs`)
- [x] Nostr mailbox (`kt-net/src/nostr.rs`): kind 35711, per-device slots, per-relay budget (6/min, burst 5), backoff on `rate-limited:`, 40 KiB chunks with double buffering, read-back health, relay subset per topic
- [x] Sync driver (`kt-net/src/sync.rs`): rewrites only changed / due / lost slots; state persisted across processes
- [x] Local test relay (`kt-net/src/testrelay.rs`, `kt relay`) with rate-limit / size / silent-drop / offline knobs; `kt sync`
- [x] Runs on Android: the `kt` CLI cross-compiled (arm64) on the test tablet syncs with the PC; sandboxed imports work there
- [x] Public relay session (organic, approved 2026-09-24): all edits synced over 7 real relays, kind 35711 accepted everywhere, no rate limits, 100% read-back (`docs/research/E11-public-nostr-session.md`)
- [x] iroh P2P (`kt-net/src/p2p.rs`): whereabouts in sealed slots, slot store, member-to-member exchange with delivery acks, non-members refused, member relaying (store-and-forward)
- [x] Helper relay (`kt-net/src/helper_relay.rs`, `kt helper-relay`): embedded iroh-relay server, token access, per-client cap. **Verified PC ↔ tablet** with Nostr dead: the tablet's sync reached the PC through the helper relay (tunneled over USB with `adb reverse`), 3 slots sent and confirmed, 3 received, 0.62 s on the tablet
- [ ] Helper relay TLS: Let's Encrypt IP certificates via ACME (validated in E10 against a local CA; needs a publicly reachable machine to test for real)
- [ ] iroh P2P (LAN with rotating IDs, direct, relays per D49 outcome, member relaying)
- [x] Discovery via slots (whereabouts) and the **Mainline DHT** (`kt-net/src/dht.rs`, engine `replica/discovery.rs`): sealed, device-signed records at member-only locations that rotate with key epochs. Local testnet test + **public DHT from the tablet**: published and found by the other member device, ~20 s per publish+lookup round
- [x] LAN discovery (`kt-net/src/lan.rs`, D57): beacons with 10-minute rotating tags and sealed device-signed whereabouts, padded entry counts; members learn LAN addresses and dial directly; outsiders learn nothing. Tested on loopback; multicast on a real LAN not yet tested (it would trigger a Windows firewall prompt on this PC)
- [x] P2P membership proof: `Hello` carries the dialer's device-signed whereabouts naming its endpoint, so a responder serves a member it hasn't heard of yet
- [ ] n0 DNS discovery as an optional best-effort layer (a config flag on the iroh endpoint)
- [x] Live mode (`kt-net/src/live.rs`): Nostr live subscriptions push members' slots (29 ms locally); knocks (ephemeral kind 25711, throwaway keys, sealed whereabouts) make online members dial each other at once (D49 (b) signaling)
- [ ] Helper storage protocol (basic) + reliability records
- [x] Operator manifest (`kt-net/src/manifest.rs`): signed with a separate operator key, add-only merge with the baseline, no downgrade, last good copy persisted; fetch over HTTPS and Nostr (kind 35712, any author, only the signature counts)
- [ ] Operator manifest, real-world part (needs the user): generate the real offline operator key (same handling as the release keystore), choose the hosting URLs (jurisgames.com, the public repo)

### M3: Sharing
- [ ] Invites (8-word code, link, QR; all layers; 30 days; multi-use; cancel)
- [ ] Device linking (D6 protections; reusable codes valid a few hours; Undo with key rotation)

### M4: Apps
- [ ] UniFFI bridge
- [ ] Minimal Compose UI (Android + desktop)
- [ ] Android lifecycle (expedited flush, periodic sync) + widget
- [ ] Headless emulator smoke tests

### M5: Lifecycle and backup
- [ ] Android backup (sealed without a lock), Block Store, restore prompt
- [ ] Exports, desktop backup folder, Google Drive (opt-in)
- [ ] Notifications from merged state

## Current state
- **M2 in progress (2026-09-24).** 66 tests passing. Nostr sync works end to end locally (CLI + tests) and PC ↔ tablet (Samsung SM-T290, Android 11, 2 GB: 0.14 s per sync round). D49 settled with lab data (E10): helper iroh relays + direct via own signaling.
- Android binary size: the `kt` CLI with Nostr + iroh + the relay server is 9.8 MB (arm64 release, stripped). E6's 7.07 MB figure had no relay server and no CLI; re-measure the real app library in M4.
- Found and fixed in M2:
  - the host parsed untrusted blobs before the sandbox (a regression from a duplicate-skip shortcut): the sandbox worker now reports op ranges and the host decides from those;
  - slot fingerprints changed between processes (hash-map order), so every run rewrote every slot;
  - ops made right before a compaction were lost to members that hadn't received them;
  - parked compaction proposals weren't retried;
  - `wss://` relays panicked without a TLS crypto provider.
- **M1 complete (2026-09-24).** 42 tests passing (`scripts/test.sh`): 29 engine unit, 5 network simulations, 3 domain, 5 document-epoch scenarios. The two-device CLI demo works across process restarts, including sharing, concurrent edits and a document epoch (4,018 → 627 bytes).
- Fixed during M1: new members couldn't read pre-join history (Add epochs now carry wrapped history keys); the outgoing queue and own-op send positions are now persisted (the CLI's short-lived processes exposed both).

## Next action
1. Helper storage protocol for strangers (RAM), reliability records, selection.
2. Then M3 (invites, device linking).

## Design deviations
- ~~Epoch records travel unencrypted in M1~~: on the network they now travel only inside sealed slots (M2). The direct join bundle still carries them as-is (M3 invites replace it).
- **Slot event kind 35711, not 31234:** the E4 probe's kind 31234 is registered as NIP-37 "Draft Event". 35711 is unregistered (NIPs README, 2026-09-24). *Needs noting in DESIGN §7.*
- **Previous-generation carry:** after a document epoch each device keeps its own old-generation ops in its slot until every member's signed status shows the new generation (max 30 days). Needed because a device may have edited just before compacting. A device two or more generations behind still can't catch up through slots alone (it needs a member's full export). *Known gap; compactions are rare.*
- **D5 rule 5 made concrete:** removals carry per-document cutoff counters instead of an opaque version-vector digest. Receivers reject a removed device's ops beyond the cutoff; the post-removal document epoch will be rebuilt at the cutoff. *Implementation detail within the design's intent.*
- ~~Own-op send position isn't persisted~~: fixed in M1 (persisted per document, with a durable outgoing queue).
- Groups follow DESIGN §4/§5: a group (a circle of people, the sharing unit) holds one **group document** (`meta.kind=group`, name, `lists` map) plus **any number of list documents**. Solo lists live in the personal group (which also holds the profile and index docs and cannot be shared). Sharing a group shares all its lists, including ones added later. `move_list` copies the live state (task IDs kept) into a new doc in the target group and unlists the old one. Covered by `kt-tasks/tests/app_groups.rs`.
- **Add epochs carry history keys:** earlier epoch keys, wrapped for the new device only, so a new member can read existing content. *Addition the design implied (sharing a list shares its contents); to be noted in DESIGN §5.*
