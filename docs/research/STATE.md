# Research & Hardening: checkpoint

**Every loop iteration: read this file first, do the "Next action", update this file, commit + push.**
Goal: prove or disprove every claim in `docs/DESIGN.md` through research, experiments and attacks on our own system. Then fix what fails, with evidence. Done = every box below checked and the final report written.

## Safety rules (never break)
- **This is a technical prototype and research phase. Findings and solutions are proposals: never apply them to DESIGN.md or UX-PRINCIPLES.md. The user picks what stays.**
- No load tests, floods or attacks against third-party infrastructure (public Nostr relays, iroh relays, DHT nodes, STUN, Blossom). Heavy or adversarial tests run against **local** instances only.
- Probing public relays: at most a handful of small, well-formed events per relay, only to check acceptance and limits. Read NIP-11 info first.
- Nothing is published; the repo stays private. Emulators run headless.
- Install tools freely; never touch unrelated system config.

## Phases

### P0: Setup
- [x] Checkpoint file, research directory

### P1: Research (multi-agent). Reports in `docs/research/R*.md`
- [x] R1 iroh (connectivity, Android, relays, blobs, size)
- [x] R2 Nostr (acceptance, addressable events, limits, retention, PoW)
- [x] R3 Loro (history, shallow snapshots, sizes, perf, compacted-history imports)
- [x] R4 Android platform (WorkManager, shortService, charging, standby buckets, widgets, local-network permission, exact alarms, Block Store, Auto Backup)
- [x] R5 Prior art and their failures (BitTorrent / BEP44, Hypercore / Pear / Keet, Syncthing, Anytype, Delta Chat, Briar, Magic Wormhole, Automerge-repo)
- [x] R6 User experience evidence (P2P / local-first complaints, task app complaints, sharing flows)
- [x] R7 Security design review (invite codes, Argon2 params, key epochs, sealed backups, helper trust)
- [x] R8 Policy and feasibility (Play policies, data safety, FGS, Drive / OneDrive OAuth, GitHub Pages)
- [x] Adversarial verification of each report's key findings
- [x] Synthesis: `docs/research/FINDINGS.md` (claim → verdict → evidence → fix)

### P2: Experiments (local). Code in `research/experiments/`, results in `docs/research/E*.md`
- [x] E0 Toolchain: Android NDK, cargo-ndk, Rust Android targets
- [x] E1 Loro: history growth for realistic workloads, shallow snapshots, importing updates that depend on trimmed history, large-list performance
- [x] E2 Argon2id: timing across parameters on this PC; attacker GPU rate (hashcat benchmark if available)
- [x] E3 iroh: two local endpoints, local relay, connection paths, binary size impact
- [x] E4 Nostr: local relay (addressable replacement, size limits, rates) + polite acceptance probe of candidate public relays
- [x] E5 Helper network simulation: churn, replication factor, availability, reputation, eclipse
- [x] E6 Android size: release build of engine prototype (iroh + Loro) for arm64-v8a, stripped
- [x] E7 Invite codes: EFF list, entropy, derivation, expiry
- [x] E8 Key epochs: rotation prototype and tests

### P3: Decision list (the user decides; DO NOT edit DESIGN.md / UX-PRINCIPLES.md)
- [x] `docs/research/DECISIONS.md` (45 items, FINDINGS integrated): every issue found, with evidence, a proposed fix and alternatives, and a Keep / Change / Drop choice for the user. Nothing in the design changes until the user picks. **Only list alternatives that were actually evaluated with equal rigor; if there's one serious option, say so. State confidence and what was not checked for every item.**

### P4: Attacks on our own system (local only). Results in `docs/research/A*.md`
- [x] A1 Invite-code brute force (measured rates, not estimates)
- [x] A2 Rollback / withholding by helpers
- [x] A3 Junk flooding of helpers (fake groups)
- [x] A4 Eclipse / fake-helper positioning
- [x] A5 Malicious group member (mass delete, huge documents, format bombs, replaying old epochs, writing after removal)
- [x] A6 Parser fuzzing of engine message formats
- [x] A7 Metadata leakage analysis (relays, DHT, helpers)
- [x] A8 Invite / link / clipboard / QR exposure paths (keyboards, cloud clipboard, screenshots)

### P5: Final report
- [x] `docs/research/REPORT.md`: what held, what failed, what changed, what's still open

### P6: Data distribution layers (added 2026-09-24 at the user's request; highest priority: "this system defines this app and many others")
Prioritize layers that cover what we lack or doubt:
- **G1** offline store-and-forward independent of Nostr (acceptance and rate risk);
- **G2** internet P2P connectivity independent of n0 (address discovery + hole-punch coordination);
- **G3** waking sleeping Android phones without our own server;
- **G4** free relaying for failed hole punches (~30%, mostly phone↔phone on carrier NAT);
- **G5** discovery and presence signaling;
- **G6** large blob transfer.

Steps:
- [ ] (running: workflow wf_6f1ff1cc-d9e) L1–L8 research workflow (8 dimensions + independent verification). Reports: `docs/research/L*.md`
- [ ] Synthesis: `docs/research/LAYERS.md` (candidate → gaps covered → verdict → evidence)
- [x] Safe local experiments: E9 sizes done (E9-stack-sizes.md), E5b diurnal helper sim done (E5b-diurnal-helpers.md). Was: E9 arm64 size builds of libp2p (relay client + DCUtR + identify + kad + QUIC), str0m, webrtc-rs and veilid-core (`research/experiments/rust/e9_*`)
- [ ] DECISIONS.md: new "Distribution layers" section (proposals only; the user decides)

Note: WebSearch is exhausted for this session. Agents use WebFetch on known documentation URLs, `gh search` / `gh api`, crates.io / docs.rs APIs.

## Current state
- P1 research workflow launched (2026-09-24).
- E0 done (Rust 1.98.1, NDK 29.0.14206865, cargo-ndk 4.1.2, hashcat 7.1.2). E1, E2, A1 done.

- E4, E5, E7 done. E6 Android size builds running (crypto 0.30 MB, loro 1.74 MB, iroh 4.58 MB, nostr 1.50 MB per arm64 .so; combined pending).

## Next action
**P6 in progress:** data distribution layers research (see P6 below). Launch or collect the layers workflow, run safe local experiments where useful, then write `docs/research/LAYERS.md` and add a "Distribution layers" section to DECISIONS.md.

## Findings log (short; details live in the reports)
- E2/A1: RTX 3080 = 783 guesses/s at m=64MiB t=3 (4,018 at 19MiB, 81 at 256MiB). The 6-word estimate held. See E2-A1-argon2.md
- E1: 5 years of daily shared-list use = 0.35 MB, so size claims held. NEW: deleted items stay in Loro state forever; trash purge needs "document epochs" (re-create from live state). CRITICAL NEW: PeerID reuse (restore, two instances) silently diverges docs. Must regenerate the PeerID on restore, keep it out of backups, run the desktop as a single instance. See E1-loro.md
- E4: 8/8 major free relays accepted new keys + addressable kind 30078 + custom kind 31234, opaque 60 KB content; replacement correct. Most run strfry (128 KB msg limit; damus/primal 1 MB) -> chunking required; monoculture risk. Use NIP-78 kind 30078.
- E5: RAM-only phone-heavy helpers need r~15-20 for ~99% without member refresh; member refresh is essential; reputation gives +6-10 pts vs dishonest helpers; r>=20 tolerates 50% dishonest.
- E7: EFF large list: 34% of words have a valid 1-typo neighbor -> typos silently become other valid codes (92% of 6-word codes). Proposal: 8 words from EFF short list 2.0 (3-letter autocomplete, 0 typo neighbors, 82.7 bits > 77.5).
- E3: local iroh connect 2.4 ms; dial-by-ID via n0 669 ms. NEW: iroh defaults depend on n0 alone (relays + DNS/pkarr discovery); mDNS/DHT discovery not in core. Proposal: own discovery via Nostr + pkarr/Mainline DHT; explicit mDNS.
- E6: full Rust core arm64 = 7.07 MB installed / 4.33 MB compressed (iroh 4.6, loro 1.7, nostr 1.5, crypto 0.3). Estimated app ~15-20 MB installed, ~10 MB download: targets hold.
- A6 CRITICAL: fuzzing Loro 1.16.2 import (checksum recomputed like an attacker would) aborts the process within seconds; 4 distinct panic sites, 85-byte minimum input; double panic (poisoned lock) -> abort, catch_unwind can't help. Any member or disk corruption = app crash loop. Proposals: process-isolated import, crash-loop breaker, upstream report (needs the user's OK to publish).
- A5 CRITICAL: a member reusing another's PeerID causes permanent silent divergence (a real edit lost for one member) and frames the victim as author. Proposal: PeerID = hash(device key), signed updates, own-ops-only per device, checked on the isolated copy; drop the shared snapshot slot in favor of verified document epochs. Mass delete is recoverable via history (keep a min 30-day window). Size bombs: a 50 MB update is imported with no limit; add engine limits.
- A2-A4 (analysis): rollback only delays (monotonic signed reads); flooding degrades helpers but not delivery; gap: helpers can't tell legit reads, so cap young groups. Eclipse: the bootstrap hash must be keyed with the group secret; add IP/ASN diversity against sleepers.
- A7: one global iroh endpoint ID links a device's groups; the DHT discovery key must rotate with the secret. A8: link/QR carry the permanent secret, so every leak (clipboard history, cloud clipboard, browser sync, photo backups) is permanent. Proposal: links/QR carry short-lived invites (same mechanism as codes).
- E8: epoch simulator, 3000 randomized runs: concurrent add+remove re-included removed devices in 441 runs until the rule "every new epoch subtracts all known removals" was added; then 3000/3000 OK (no leaks to removed devices, convergence). Mutual removal empties small groups (38/3000).
