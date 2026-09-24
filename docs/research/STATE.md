# Research & Hardening: checkpoint

**Every loop iteration: read this file first, do the "Next action", update this file, commit + push.**
Goal: prove or disprove every claim in `docs/DESIGN.md` through research, experiments and attacks on our own system. Then fix what fails, with evidence. Done = every box below checked and the final report written.

## Safety rules (never break)
- No load tests, floods or attacks against third-party infrastructure (public Nostr relays, iroh relays, DHT nodes, STUN, Blossom). Heavy or adversarial tests run against **local** instances only.
- Probing public relays: at most a handful of small, well-formed events per relay, only to check acceptance and limits. Read NIP-11 info first.
- Nothing is published; the repo stays private. Emulators run headless.
- Install tools freely; never touch unrelated system config.

## Phases

### P0: Setup
- [x] Checkpoint file, research directory

### P1: Research (multi-agent). Reports in `docs/research/R*.md`
- [ ] R1 iroh (connectivity, Android, relays, blobs, size)
- [ ] R2 Nostr (acceptance, addressable events, limits, retention, PoW)
- [ ] R3 Loro (history, shallow snapshots, sizes, perf, compacted-history imports)
- [ ] R4 Android platform (WorkManager, shortService, charging, standby buckets, widgets, local-network permission, exact alarms, Block Store, Auto Backup)
- [ ] R5 Prior art and their failures (BitTorrent / BEP44, Hypercore / Pear / Keet, Syncthing, Anytype, Delta Chat, Briar, Magic Wormhole, Automerge-repo)
- [ ] R6 User experience evidence (P2P / local-first complaints, task app complaints, sharing flows)
- [ ] R7 Security design review (invite codes, Argon2 params, key epochs, sealed backups, helper trust)
- [ ] R8 Policy and feasibility (Play policies, data safety, FGS, Drive / OneDrive OAuth, GitHub Pages)
- [ ] Adversarial verification of each report's key findings
- [ ] Synthesis: `docs/research/FINDINGS.md` (claim → verdict → evidence → fix)

### P2: Experiments (local). Code in `research/experiments/`, results in `docs/research/E*.md`
- [ ] E0 Toolchain: Android NDK, cargo-ndk, Rust Android targets
- [ ] E1 Loro: history growth for realistic workloads, shallow snapshots, importing updates that depend on trimmed history, large-list performance
- [ ] E2 Argon2id: timing across parameters on this PC; attacker GPU rate (hashcat benchmark if available)
- [ ] E3 iroh: two local endpoints, local relay, connection paths, binary size impact
- [ ] E4 Nostr: local relay (addressable replacement, size limits, rates) + polite acceptance probe of candidate public relays
- [ ] E5 Helper network simulation: churn, replication factor, availability, reputation, eclipse
- [ ] E6 Android size: release build of engine prototype (iroh + Loro) for arm64-v8a, stripped
- [ ] E7 Invite codes: EFF list, entropy, derivation, expiry
- [ ] E8 Key epochs: rotation prototype and tests

### P3: Fixes
- [ ] Apply confirmed fixes to `docs/DESIGN.md` (+ UX-PRINCIPLES.md), each citing its evidence

### P4: Attacks on our own system (local only). Results in `docs/research/A*.md`
- [ ] A1 Invite-code brute force (measured rates, not estimates)
- [ ] A2 Rollback / withholding by helpers
- [ ] A3 Junk flooding of helpers (fake groups)
- [ ] A4 Eclipse / fake-helper positioning
- [ ] A5 Malicious group member (mass delete, huge documents, format bombs, replaying old epochs, writing after removal)
- [ ] A6 Parser fuzzing of engine message formats
- [ ] A7 Metadata leakage analysis (relays, DHT, helpers)
- [ ] A8 Invite / link / clipboard / QR exposure paths (keyboards, cloud clipboard, screenshots)

### P5: Final report
- [ ] `docs/research/REPORT.md`: what held, what failed, what changed, what's still open

## Current state
- P1 research workflow launched (2026-09-24).
- E0 toolchain install in progress.

## Next action
Collect the P1 reports when the workflow finishes, then continue with P2 experiments.

## Findings log (short; details live in the reports)
- (none yet)
