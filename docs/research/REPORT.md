# Research phase report

*2026-09-24. Scope: prove or disprove the design in `DESIGN.md` through multi-agent research (8 dimensions, each independently verified), local experiments, and attacks on our own system. All fixes are **proposals** in `DECISIONS.md`. DESIGN.md is unchanged.*

## Bottom line
**The core idea holds:** local-first CRDT sync, Nostr as a mailbox, P2P when possible, no accounts, no hosting. It builds, it's small (7 MB native on Android), real relays accept our traffic today, and invite-code security holds against measured GPU attacks.

**The research also found five things that would have broken it in production.** None requires abandoning the architecture:
1. **Any group member, or one corrupted update, could crash every member's app in a loop** (Loro import aborts on crafted input). Fix prototyped: process-isolated import, which contains the crash. (D3)
2. **A member could silently and permanently split the group's data and forge edits as someone else.** Restoring a phone backup could do the same by accident. Fix: device-bound, signed authorship; identity never backed up. (D1, D2)
3. **Key rotation as designed leaks new keys to removed devices under concurrency, and lets old secrets add "ghost" devices.** Fix: a set of rules validated in 3,000 randomized simulations. (D5)
4. **Invite links and QR codes carry the permanent group secret,** so any leak (clipboard history, browser sync, chat logs, photo backups) grants access forever. Fix: short-lived invites. (D7)
5. **Device linking can be phished** (the 2025 Signal attack pattern), and our "linking merges your lists" rule makes it worse. Fix: a separate linking path, warnings, and notifications. (D6)

**And two strategic realities that weaken the design's assumptions:**
- **iroh's internet path depends on n0's free relays, which are officially "development and hobby use only".** There is no second provider and no in-constraints alternative. (D14)
- **Nostr relay operators dislike "relays as storage" and rate-limit hard** (~8 events/min per IP at Damus). The mailbox works today but must be a quiet citizen, and could be restricted at scale. The volunteer helper network isn't a launch-scale answer. (D11, D13, D20)

## What held (with evidence)
| Claim | Evidence |
|---|---|
| App stays small | Rust core (iroh + Loro + Nostr + crypto) = **7.07 MB installed / 4.33 MB compressed** on arm64, 16 KB-aligned (E6) |
| Data stays small with history | 5 years of daily shared-list use = **0.35 MB** (E1) |
| Large lists are fast | 10k tasks: create 31 ms, load 28 ms on desktop (E1) |
| Long-offline devices can rejoin | Rebase path works where direct import fails (E1) |
| Relays accept our traffic today | 8/8 major free relays accepted new keys, custom addressable kinds, opaque 60 KB content, correct replacement (E4) |
| Invite codes resist brute force | RTX 3080 measured 783 Argon2id guesses/s at 64 MiB; 6-word codes hold (A1) |
| Helper network can tolerate dishonest helpers | at r ≥ 20, ~99.5% availability even with 50% dishonest helpers, *if* enough helpers exist (E5) |
| Rollback by helpers only delays | signed, monotonic reads (A2) |
| Google Drive backup is easier than planned | `drive.file` needs no verification (R8) |
| P2P / Nostr apps are fine on Play | many shipped in 2026 (R8) |

## What failed or needed correction
See `DECISIONS.md` for all 45 items (`FINDINGS.md` has the full synthesis: 103 claims, 20 confirmed, 63 partial, 15 refuted, 5 unverified). Most significant beyond the five above:
- **My own earlier recommendation (NIP-78 kind 30078) was wrong:** the spec changed on 2026-09-03. Use a dedicated kind. (D10)
- **Android is stricter than §8 assumes:** background network is cut ~5 s after leaving the app, the process freezes at 10 s, and Doze windows reach 6 h, which widgets don't prevent. "Usually within 15 minutes" is wrong for idle phones. (D16, D17)
- **Deleted tasks never leave a Loro document;** the "30-day trash purge" needs document re-creation, as does the checkpoint plan. (D24, D25)
- **The invite word list lets typos fail silently** (34% of words are one typo from another valid word). Evaluated fixes: 8 short-list words, or 6 curated large-list words. (D8)
- **Phone helpers conflict with Play policy.** (D19)
- **Loro depends on MPL-2.0 crates and unmaintained ones,** including a memory-safety advisory that triggers on panics. (D29)
- **Windows unsigned installers get blocked,** but the Microsoft Store is free. **Flathub bans AI-generated manifests,** which likely excludes us. (D39, D40)
- **Invitees on iPhone have no path at all.** A shared list only works if the other person can join (D45).
- **Time-sensitive:** Android developer verification (regional enforcement from 2026-09-30). The package name should be settled before the first upload: kleiders.com isn't yours, jurisgames.com is. (D37, D38)

## Still open (not verified)
- Real low-end Android measurements: Argon2 timing, Loro performance, and the cost of a separate import process.
- Battery and socket cost of a per-group iroh endpoint (privacy option D23).
- Correlated (time-zone) churn in the helper model; sleeper-attack simulation.
- The rest of the key-epoch rules in a real engine: backdating rejection and data written during forks.
- Real-user code-entry and reading error rates; a multi-day sync diary study (D36).
- An MSIX packaging tool for the Microsoft Store (D39).

## Artifacts
- Research: `R1`–`R8` (each ends with an independent verification), `FINDINGS.md` (synthesis).
- Experiments: `E1` Loro, `E2`+`A1` Argon2/GPU, `E3`+`E6` iroh and size, `E4` Nostr, `E5` helpers, `E7` invite codes, `E8` key epochs.
- Attacks: `A2`–`A4` helpers, `A5` malicious member, `A6` fuzzing + isolation prototype, `A7`–`A8` metadata and invites.
- Code: `research/experiments/` (Python + Rust; crash inputs in `rust/e1_loro/fuzz_out/crashes/`).
