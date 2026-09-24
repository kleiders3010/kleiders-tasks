# Research findings: design review of Kleiders Tasks

> **Note (2026-09-24):** this synthesis was written by a research agent before some user decisions. The package name is now `com.jurisgames.tasks` (not com.kleiders.tasks), and Flathub isn't pursued. **`DECISIONS.md` is the authoritative list for the user's choices**; §6 below is input, not applied.


*Synthesis of research dimensions R1–R8, each checked by an independent verification pass, plus the local experiments and attacks already run (E1–E8, A1–A8). Date: 2026-09-24. Subject: [DESIGN.md](../DESIGN.md) (draft of the same date).*

**How to read this.** Every verdict below is the **final** verdict: where a verifier corrected a report, the correction wins, and the table says what changed ("was Refuted"). Severity: Critical (silent data loss, crash loops, or full account takeover), High (a core promise fails or a principle is violated), Medium (a real problem with a known fix), Low (wording, polish, or edge cases). Full evidence and sources are in each report; its `## Independent verification` section holds the corrections.

Reports: [R1 iroh](R1-iroh.md) · [R2 Nostr](R2-nostr.md) · [R3 Loro](R3-loro.md) · [R4 Android](R4-android.md) · [R5 prior art](R5-prior-art.md) · [R6 UX evidence](R6-ux-evidence.md) · [R7 security](R7-security.md) · [R8 policy](R8-policy.md). Experiments and attacks: [E1](E1-loro.md) · [E2/A1](E2-A1-argon2.md) · [E3/E6](E3-E6-iroh-and-size.md) · [E4](E4-nostr.md) · [E5](E5-helper-sim.md) · [E7](E7-invite-codes.md) · [E8](E8-key-epochs.md) · [A2–A4](A2-A4-helper-attacks.md) · [A5](A5-malicious-member.md) · [A6](A6-parser-fuzzing.md) · [A7/A8](A7-A8-metadata-and-invites.md).

---

## 1. Executive summary

### Does the design hold?

**The architecture holds. Several mechanisms it depends on do not hold as specified.** A local-first Rust engine using Loro, a Nostr slot mailbox and iroh P2P, with no accounts and no servers, remains a sound foundation. Nothing found requires abandoning it. However, the mechanisms that keep data correct under real conditions don't work as written: untrusted imports, identity after backup restore, compaction, key epochs, invites and device linking. Neither do the promises the design makes to users about background delivery, recovery and provider independence. Every one of these problems has a concrete fix, and most fixes belong in format v1. They should be settled before engine code freezes any wire or storage format.

Tally of the 103 design claims checked: **20 confirmed, 63 partial, 15 refuted, 5 unverified.** Four claims changed verdict in the design's favor after verification (R1-4, R5-2, R6-10, R8-11) and three changed against it (R6-1, R8-12, R4-1 severity raised).

**What held, with evidence:**
- **App size.** The whole Rust core measured 7.07 MB installed and 4.33 MB compressed on arm64 with the size profile (E6), and the build is 16 KB-aligned by default with NDK r29. The size budget holds.
- **Data size.** Five years of daily shared-list use comes to 0.35 MB, and 10k tasks load in 28 ms on desktop (E1).
- **Invite-code entropy.** 6 words from the EFF large list give 77.5 bits. A 100-GPU farm would need about 20,000 years to crack one invite out of 3M (E2/A1, R7).
- **Nostr relays accept this traffic today.** 8/8 major free relays accepted fresh keys, custom addressable kinds and 60 KB of opaque content, and replaced addressable events correctly (E4).
- **Loro format compatibility in practice.** Blobs written by Loro 1.0.9, 1.10.8 and 1.16.2 round-trip in both directions (R3 verification).
- **Play policy.** P2P and Nostr apps are live on Play. shortService is exempt from Play's FGS criteria. The exact-alarm plan is correct. The Google code scanner needs no camera permission (R8, R4).
- **Mailbox strategy.** Member devices make the best mailboxes, and RAM-only helpers are safe for durability (R5).

### Act this week (time-sensitive)

1. **Android developer verification (R8, deadline 2026-09-30).** Play apps not registered by the deadline are removed. The verifier notes the deadline mainly affects existing apps that weren't auto-registered, and the "first come, first served under 50 installs" rule has no fetched source. Claiming the name is cheap, though. Check that the existing developer account's apps are registered, claim `com.kleiders.tasks`, and register the sideload signing key.
2. **Decide on the domain (R8).** `kleiders.com` currently serves an unrelated AI fashion-design SaaS (registered 2026-03-03). If the user doesn't own it, the permanent package name conflicts with F-Droid's and Flathub's app-ID rules and with trademark rules. An owned domain is effectively required anyway: Google's OAuth policy needs a homepage on a verified domain you own, and verified App Links, Flathub verification and a join-link origin the project controls all need one. Settle this before the first upload.

### Top 10 problems (ranked by severity)

**1. A malformed update crash-loops every member's app (Critical).**
- *Evidence:*
  - Fuzzing Loro 1.16.2 import, with the checksum recomputed as an attacker would, aborted the process in 40/40 sessions within 20 s. It hit 4 panic sites, and the smallest crashing input was 85 bytes (A6).
  - A second panic on a poisoned lock turns the first into an abort, so `catch_unwind` can't help.
  - Loro #1088 (snapshot imports cleanly, then panics on first read, "crash loop on every cold start") and #1106 (a non-unwinding abort triggered by `fork_at`) show the same pattern.
  - DESIGN sets `panic = "abort"`. Mailbox slots are re-fetched on every launch, so one bad slot kills every member's app on every start. Disk corruption does the same.
  - Sources: A6, R7-4, R3-2.
- *Fix:*
  - Import every untrusted blob first into a fresh `LoroDoc` built from bytes, in a disposable process (Android `android:process`, desktop child process). Import into the live document only if that succeeds.
  - Keep a quarantine journal: record "importing <hash> from <author>" before each import, and after a crash skip that blob and flag its author.
  - Enforce engine caps before import: update size, decompressed size, nesting depth and pending buffer.
  - Build with `panic = "unwind"`.
  - Run coverage-guided fuzzing in CI.
  - Report the crash sites upstream (publishing a bug report needs the user's OK).

**2. Cloned or forged identities silently and permanently fork documents (Critical).**
- *Evidence:*
  - Loro deduplicates operations by (PeerID, counter) with no content check.
  - Accidental PeerID reuse (E1, R3 verification) and deliberate squatting of another member's PeerID (A5) both produced permanent silent divergence: every import returned OK, and a real edit vanished for some members.
  - Auto Backup restore and device-to-device transfer copy app data to a second live phone, and D2D ignores `allowBackup=false` for target 31+. That clones the Loro PeerID, the per-group Nostr slot keys (the two devices overwrite each other's slots) and the iroh endpoint key (iroh forbids concurrent use).
  - Two desktop instances do the same.
  - Signing alone doesn't stop a member from equivocating on its own PeerID (R7 verification).
  - Sources: R3-6, R7-3, R4, R1, E1, A5.
- *Fix:*
  - Keep all identity material (device key, Nostr keys, transport key, PeerIDs) in no-backup storage, excluded from both cloud backup and device transfer.
  - A Keystore-bound install marker detects a restore. After one, mint a new identity and re-link through the personal group.
  - Run the desktop app as a single instance.
  - Give each install a PeerID bound to its device key by a signed registration in the group document.
  - Sign every update blob. Receivers enforce "a device may send only its own ops" by checking the version vector of the isolated copy (see #1).
  - Sign content hashes per op range, so equivocation becomes provable.
  - Rotate the PeerID on any doubt.

**3. Device linking can be phished to hand over all of a user's lists (Critical).**
- *Evidence:*
  - In 2025, Russia-aligned actors phished Signal's linked devices with QR codes disguised as group invites (Google GTIG).
  - Here, an attacker sends their own device-link QR styled as a list invite. The victim's phone joins the *attacker's* personal group, and the attacker approves on their own device, so the approval protects nothing.
  - Merge-on-link then hands over all the victim's current and future lists.
  - The 5-minute expiry doesn't help during a live chat.
  - Sources: R6-9.
- *Fix:*
  - Complete linking only over a direct local path (LAN or Nearby) by default. Put internet linking behind an extra warning.
  - Show a full-screen warning on the *scanning* device that names the account it will join and what that account will see.
  - Reject link payloads opened from chats, invite links or the join scanner.
  - Alert every device after a new link, with a Remove action.
  - Never merge a non-empty device silently.

**4. Compaction and the shared snapshot slot strand or silently drop edits (Critical).**
- *Evidence (reproduced on 1.16.2):*
  - A device that compacts at its own head permanently rejects other members' concurrent offline edits (`ImportUpdatesThatDependsOnOutdatedVersion`), and their later changes stay pending forever with no signal.
  - A device that misses one snapshot rotation imports the shallow snapshot with `Ok` but nothing applied.
  - A single unmerged head pins the root at the fork point.
  - One import order (the PR #1083 path) aborts the process.
  - Pending changes are lost on restart.
  - `fork_at` returns NotImplemented on shallow documents, so the "checkpoints at widening intervals" plan can't live inside one document.
  - The single "snapshot slot" is also impossible with per-device Nostr keys, because addressable events are keyed by pubkey (R2). It is unverifiable once updates are signed (A5).
  - Sources: R3-2, R3 new risks, R2-2, A5, E1.
- *Fix:*
  - Replace "shallow-snapshot compaction + shared snapshot slot" with **document epochs**. Compact only at a causally stable version acknowledged by every member device, minus a margin. Alternatively, re-create the document from live state, which each member verifies against its own state before switching.
  - Stale devices bootstrap into a fresh document and rebase their unsynced edits with a three-way app-level merge.
  - Keep raw received blobs until nothing pending references them.
  - Store checkpoints outside the live document.
  - Never import a shallow snapshot into a document with parked changes.

**5. Key epochs are underspecified: removed members and ghosts can keep access (High).**
- *Evidence:*
  - DESIGN has no rule for merging concurrent epochs, no protection against backdated edits by removed members, no policy for role-less mutual removal, and no authentication of new members.
  - E8's simulator re-included removed devices in 441/3000 runs until rule 4 was added ("every new epoch subtracts every removal its creator knows about").
  - Anyone holding an old or leaked secret can add a ghost device that later rotations include (the class of the 2023 Matrix attack).
  - Epoch keys posted to public relays have no forward secrecy against archives.
  - Sources: R7-2, E8.
- *Fix:*
  - Every *receiver* validates E8's rules 1–4.
  - A member addition is valid only under the current head epoch and only if signed by a current member device.
  - Each removal carries the remover's version vector, and ops by the removed author outside it are rejected (needs #2).
  - Mutual removal removes both, shown clearly with Undo.
  - Notify all members of every join.
  - Rotate periodically, and derive all network identifiers from the epoch key.
  - Reserve a hybrid X25519+ML-KEM suite ID now.
  - Bind version, group, epoch, slot and sender as AEAD associated data.
  - Consider adopting p2panda-encryption or Keyhive rules; p2panda is unaudited.

**6. Links, QR codes and the clipboard carry the permanent group secret (High).**
- *Evidence:*
  - Every leak is permanent. Paths include keyboard clipboard history and cloud clipboard sync, browser history sync, chat and photo backups, Microsoft Safe Links rewriting and detonating URLs, and in-app browsers injecting JavaScript.
  - The join page's origin is a trust anchor. A GitHub account takeover, username rename or domain lapse lets someone serve JavaScript that reads the fragment of every old link.
  - The clipboard handoff at install breaks after Android's 1-hour auto-clear.
  - Invite codes have no salt, so the pool of attackable invites grows forever.
  - Sources: R7-8, R4-10, R6-6, R8-14, R8-20, A8.
- *Fix:*
  - Links and QR codes carry an invite ID plus a 256-bit capability. It decrypts a short-lived, revocable invite record in the mailbox that holds only the current epoch key.
  - Codes work the same way, with Argon2 and a date-bucketed salt.
  - Joining always needs a user tap.
  - Serve verified App Links from an owned, GitHub-verified domain, with a minimal page, a CSP meta tag and no third-party scripts.
  - Clipboard: check the clip description first, read only after window focus, and clear the clipboard after use.

**7. The Nostr write policy would get the app rate-limited or banned (High).**
- *Evidence:*
  - Damus's reference filter allows 8 posts/min per IP and bans the IP for 1 h after 10 rejections, optionally at the Cloudflare edge. relay.damus.io banned an app that republished on every change (Sep 2026). khatru's default allows 2 events per 3 min.
  - Everyone behind a carrier NAT shares that budget.
  - DESIGN rewrites slots "every few seconds", flushes every group at once and uploads snapshot chunks in bursts.
  - NIP-78 (kind 30078, which E4 and STATE.md recommended) was amended on 2026-09-03 to owner-only reads under AUTH.
  - strfry's real event ceiling is 65,536 B of JSON, about 47 KiB binary.
  - "OK" doesn't mean stored. relay.damus.io announced a shutdown (it is still running). Most relays run strfry.
  - Sources: R2-1, R2-3, R2-6, R2 new risks, E4.
- *Fix:*
  - An engine-wide per-relay write budget (about 6 events/min, burst 5, shared across groups).
  - Rewrite slots at most every 30–60 s in the foreground, with jittered flushes.
  - Stop at the first `rate-limited:` and back off exponentially with full jitter.
  - Use one dedicated, publicly documented kind.
  - Chunks of at most 40 KiB with a generation/index/count/hash, switched atomically between two buffers.
  - A strictly increasing `created_at` per slot.
  - Judge relay health by reading events back.
  - Update the relay list at runtime from a signed list.

**8. iroh depends on a single provider, n0, for three roles (High, violates principle 2).**
- *Evidence:*
  - n0 provides the relays ("development and hobby use only", no SLA, throttled), public-address discovery and hole-punch coordination (STUN was removed in iroh 0.90, so "iroh + public STUN" doesn't exist), and default discovery (`dns.iroh.link`).
  - No independent relay operator for iroh 1.x was found.
  - Shipping on relays against their terms is also a Play "Device and Network Abuse" exposure.
  - The stable device key doubles as the endpoint ID, so n0's DNS works as a presence oracle.
  - Sources: R1-4, R1-6, R1-7, R5-12, E3, R8 verification.
- *Fix:*
  - Ask n0 for permission in writing and record the answer.
  - Make the relay list configurable and health-tracked.
  - Implement our own address lookup, fed by E2E-encrypted per-group address records on Nostr and in the group document.
  - Use a rotating transport key certified by the device key.
  - Make member relaying (layer 6) and desktop members the primary live fallback.
  - State the fallback honestly: without an iroh relay, P2P is LAN-only and sync goes through Nostr.

**9. Android background delivery is far slower than DESIGN promises (High).**
- *Evidence:*
  - On Android 15+, a still phone with the screen off enters Doze within about 1–3 min (15 s + 15 s on Android 16 and 17). Maintenance windows come at about +1, +3, +7 and +13 h.
  - Android 16 adds a 20 min/60 min job quota even for the ACTIVE bucket.
  - Android 15+ cuts background network 2–5 s after the app leaves the foreground, and the freezer terminates sockets.
  - Members who rarely open the app get about 3 job sessions per day.
  - The widget exemption doesn't cover Doze. The shortService notification deferral is rate-limited.
  - Competitors push notifications instantly (Bring!).
  - Sources: R4-1, R4-2, R4-3, R6-3, R6-4, R5-16.
- *Fix:*
  - Make an expedited WorkManager job the primary flush, enqueued *while the app is visible* (onPause and after each commit). shortService becomes optional.
  - Remove "usually within 15 minutes" and promise no number.
  - Show coarse per-member freshness and a "Why isn't this updating?" screen.
  - Offer a home-screen widget after joining.
  - Generate notifications only from merged state: batch them, show their age, drop stale ones, and scope them to recently used lists.
  - Design the content-free doorbell (UnifiedPush/FCM) now instead of waiting for complaints.

**10. Recovery has silent data-loss paths (High).**
- *Evidence:*
  - A user who skips restore in the setup wizard gets neither Block Store nor Auto Backup; only a same-device reinstall restores. The personal secret is then gone, the Nostr copies become unreadable, and solo lists are lost. That contradicts §11's "data is lost only if no second copy ever existed".
  - Auto Backup is readable by Google when no screen lock is set, which contradicts §9.
  - Block Store entries are 1 KB per the API reference.
  - Cloud restore works on non-Pixel phones only from Android 12.
  - Keystore-wrapped secrets restore as undecryptable data.
  - Backup stops silently above 25 MB, and any member can bloat a shared list past that.
  - Sources: R4-8, R4-9, R7-5, R6-14, R7 new risks.
- *Fix:*
  - When nothing was restored, first launch offers "Restore from your other device or a backup file". Device linking is the default recovery.
  - Show backup state under "Your devices".
  - Prompt for a cloud or folder backup when E2E isn't available.
  - Exclude the secrets file from backup without a screen lock (`disableIfNoEncryptionCapabilities`).
  - Keep Block Store entries at 1 KB or less.
  - Cap document sizes and show backup failures to the user.

### Just below the top 10

- **The helper network (§7a) rests on unproven assumptions (High, later phase).**
  - Opt-in volunteer participation is 0.01–0.3%, not 10% (R5-2).
  - The r formula ignores that RAM-only helpers lose slots on restart: E5 got 81% availability at r=10, where the formula predicts 99.9% (R5-3).
  - No precedent exists: Keet and Skype moved to operator-run nodes.
  - The bootstrap set can be eclipsed, and patient sybils can harvest metadata.
  - Phone helper chains are unreliable, and the FGS fallback is impossible (R4-4, R8-2).
- **Privacy claims are overstated (Medium).**
  - Per-group Nostr keys don't hide a device's groups from relay operators (R2-8, R7-7).
  - One stable iroh endpoint ID links all of a device's groups.
  - mDNS broadcasts public IPs, and Android's Private DNS setting is bypassed (R1).
- **There is no iOS path for invitees (High for sharing).** iOS is 60.7% of US mobile use, and the landing page offers only Play (R6-1).
- **Old clients can corrupt data from newer Loro features (High).** An old client publishing a full snapshot of a document with unknown container types loses data and desyncs new clients (R3-3).
- **Project continuity (High).** Briar, Syncthing-Android and Manyverse all stalled. Signing-key custody is a single point of failure, and the single private repository can't serve Pages or public downloads (R5, R6, R8).
- **Invite codes fail silently on typos (High for usability).** In 92% of 6-word codes, a single typo can produce another valid code. Four EFF words contain hyphens, which clash with the separator (R6-5, E7).

---

## 2. Every claim, with its final verdict

Columns: **ID** · **Claim** (as stated or implied by DESIGN.md, with section) · **Verdict** (final, after verification) · **Sev.** · **Evidence** (one line) · **Fix** · **Report**.

### R1: iroh

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R1-1 | iroh 1.x is stable and usable from Kotlin via UniFFI (§3) | Partial | Medium | 1.0 shipped 2026-06-15, now 1.2.0 with a wire-compatibility policy. iroh-ffi is supported again, but its published AAR is 4 KB-aligned (fails Play's 16 KB rule) and the fix is still unreleased after 5+ weeks. Each minor version gets 3 months of support. | Wrap iroh in our own engine and UniFFI layer; don't depend on iroh-ffi. NDK r28+. Quarterly upgrade task, cargo-audit. | [R1](R1-iroh.md) |
| R1-2 | iroh provides LAN discovery via mDNS (§6 L1) | Partial | Medium | mDNS lives in a separate pre-1.0 crate (0.5) that is off by default. iroh takes no MulticastLock. Android 17 (target 37) needs the local-network permission. By default mDNS advertises all addresses, including public IPs and the relay URL. | Add the crate. Hold a MulticastLock only in the foreground on Wi-Fi. Ask for the permission in context. On untrusted networks use `advertise(false)` or a private-address filter. | [R1](R1-iroh.md) |
| R1-3 | Direct IPv6 is a layer of its own (§6 L2) | Partial | Low | The publisher defaults to relay-only. Mobile IPv6 is firewalled, so a relay-coordinated open is still needed. The N0DisableRelay preset publishes an empty record. | Treat IPv6 as a path iroh chooses. Set the address filter explicitly in any relay-less mode. | [R1](R1-iroh.md) |
| R1-4 | Hole punching via "iroh + public STUN", ~90% direct (§6 L3) | Partial (was Refuted) | High | Punching works, but STUN was removed in 0.90. Address discovery (QAD) and coordination run through iroh relays. 90% is n0's own figure; an independent study measured 70% ± 7.1% on desktops. Hard-to-hard NAT is unsupported. | Rewrite L3 as "iroh (needs an iroh relay)". Drop the percentage. Measure (E3.3) and log direct vs relayed locally. | [R1](R1-iroh.md) |
| R1-5 | Router port mapping via iroh portmapper (§6 L4) | Confirmed | Low | On by default. The crate is pre-1.0 (0.19). Android 17 requires the local-network permission. | Keep on for desktop, turn off on Android. | [R1](R1-iroh.md) |
| R1-6 | Discovery via the Mainline DHT under a group-derived key (§6) | Refuted | High | The default is n0's DNS/pkarr server. The DHT is an opt-in 0.5 crate that publishes only the relay URL, under the *endpoint* key. Group-keyed records would be custom BEP44 code (1000 B limit, one signer). An endpoint ID alone still needs an address lookup. | Our own address lookup fed by E2E-encrypted per-group records (Nostr and the group document). Per-device DHT records on desktops only, optional. | [R1](R1-iroh.md) |
| R1-7 | n0 public relays are usable for free in production, with at least 2 providers (§2 P2, §6 L7) | Refuted | High | n0: "development and hobby use only", "Do not rely on it for production workloads", no SLA, rate-limited. No independent 1.x relay operator exists; Delta Chat still pins 0.35. | Written permission from n0. Configurable, health-tracked relay list. Prototype member-hosted relays (they need public reachability). State the Nostr fallback honestly. | [R1](R1-iroh.md) |
| R1-8 | iroh 0.35 relays are supported until 2026-12-31 | Confirmed | None | n0's v1 support table says so. | Target 1.x only, and expect the same sunset at 2.x. | [R1](R1-iroh.md) |
| R1-9 | iroh works on Android (background, network switches, battery) | Partial | Medium | Without the JNI context, iroh panics or silently falls back to public DNS (Cloudflare, Google, Quad9). Its network monitor is a no-op on Android. n0 says to shut the endpoint down in the background. Heartbeats every 5 s, QAD every 20–26 s. Private DNS is bypassed (confirmed in source). | Initialize ndk_context. Pass explicit DoT/DoH nameservers and disable the fallback. Call `network_change()` from Kotlin. Bind the endpoint only in the foreground or inside jobs. Keep `panic=unwind`. | [R1](R1-iroh.md) |
| R1-10 | iroh-blobs suits P2P image transfer (§4) | Partial | Medium | Its README says "not yet considered production quality". Versions 0.98 to 0.103 were all breaking. | A small custom protocol (request by BLAKE3 hash, verify, retry) for images up to ~1 MB. Adopt iroh-blobs only once it reaches 1.0. | [R1](R1-iroh.md) |
| R1-11 | Rust core is 8–15 MB per ABI; installed app ≤ 40 MB (§10) | Confirmed (E6 measured) | Low | R1's 14 MB is the untuned iroh-ffi build. E6 measured the whole core at 7.07 MB installed and 4.33 MB compressed on arm64, 16 KB-aligned. | Keep the size profile (it is load-bearing). CI checks for size budget and alignment. | [R1](R1-iroh.md), [E6](E3-E6-iroh-and-size.md) |
| R1-12 | iroh can run in a browser for a future web client (§16) | Confirmed | Low | Relay-only, ~1.1 MB gzipped, and dependent on dev-only relays. | Use Nostr over WebSocket as the web guest's transport. | [R1](R1-iroh.md) |

### R2: Nostr relays

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R2-1 | Free relays accept new keys, custom addressable kinds and opaque content (§7) | Partial | High | E4: 8/8 relays accepted all three today. But per-IP limits apply (noteguard: 8/min, 1 h ban after 10 rejections), and damus banned an app that republished on every change. offchain.pub rejects fresh keys by web of trust without declaring it. | Engine-wide per-relay write budget (≤ ~6/min, burst 5, across all groups). Stop at the first `rate-limited:`. Treat banned/blocked as down for ≥ 1 h. Probe acceptance live. Diversify operators and relay software. | [R2](R2-nostr.md) |
| R2-2 | Addressable events are replaced per (pubkey, kind, d) with correct tie-breaking (§7) | Partial | Medium | Ties go to the lowest id on strfry and khatru, and to the first-seen event on nostr-rs-relay. An older write gets "OK true" with nothing stored on khatru and nostr-rs-relay. Future-timestamp limits range from 30 to 900 s, so a device whose fast clock is corrected is locked out of its own slot. | Persisted `created_at = max(now, last+1)`. Read back to detect lockout. Readers pick the highest (created_at, then lowest id). | [R2](R2-nostr.md) |
| R2-3 | Snapshots are chunked under typical size limits (§7) | Partial | High | strfry's `maxEventSize` is 65,536 B of JSON (≈ 47 KiB binary). NIP-11 `max_message_length` is the websocket frame size, not the event size. A busy 5-year list needs 4–9 chunks, which exceeds 8/min in one burst. | Chunks ≤ 40 KiB with generation/index/count/hash. A/B double-buffered d-tags. Rewrite only changed chunks. Halve the size on "too large". | [R2](R2-nostr.md) |
| R2-4 | Relays drop expired invites (NIP-40); deletion works (§5) | Partial | Low | strfry, nostr-rs-relay and khatru honor NIP-40; memlay (snort) doesn't. The spec lets relays keep expired events. nostr-rs-relay ignores a-tag deletions. | Send invites only to NIP-40 relays. In-payload expiry (already designed). NIP-40 of +60–90 days on slots. Don't rely on NIP-09. | [R2](R2-nostr.md) |
| R2-5 | Reliability per "ACM study, 2023, 712 relays" (§7) | Partial | Medium | Wei & Tyson, CoNEXT'25, with data from Oct–Dec 2023. 18.5% of relays were dead within 3 months. "90% up > 80%" holds only after excluding dead relays. The paper has no retention data. relay.nostr.band is down. | Correct the citation. Make the relay list updatable at runtime. Diversify. | [R2](R2-nostr.md) |
| R2-6 | Operators tolerate encrypted non-social storage; NIP-78 is a precedent (§7) | Partial | High | NIP-78 was amended on 2026-09-03: kinds 78/30078 should be AUTH-gated and served only to their author. strfry AUTH-gates kinds 4/1059 by default. damus blocks file kinds. The custom kind 31234 is still accepted everywhere. | Never use 30078/78. Use one dedicated, publicly documented kind, with an operator info page. Keep writes frugal. Offer a bring-your-own/paid relay option. | [R2](R2-nostr.md) |
| R2-7 | Blossom accepts encrypted non-media blobs (§4) | Partial | Low | blossom.primal.net returns 415 for octet-stream, even when relabeled. blossom.band accepts media only. 24242.io deletes after 60 idle days. Verified to accept ciphertext: blossom.divine.video, blossom.ditto.pub, cdn.hzrd149.com. | Best-effort only. Upload octet-stream via /upload after a BUD-06 HEAD check. Store the SHA-256 next to the BLAKE3 hash. Keep a runtime server list. | [R2](R2-nostr.md) |
| R2-8 | Per-device-per-group keys make groups unlinkable and won't trip spam filters (§7) | Refuted | Medium | IP, one websocket per relay, combined REQ filters, multi-pubkey AUTH and simultaneous flushes all link keys. WoT relays reject fresh keys. Stable keys accumulate per-pubkey limits. | Reword §5, §7 and §9 (the keys hide groups only from other readers). Jitter flushes per group. At most one group's AUTH per connection. | [R2](R2-nostr.md) |

### R3: Loro CRDT

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R3-1 | Loro 1.x has a stable, backward-compatible binary format (§4, P8) | Partial | High | Top-level modes 3/4 are unchanged since 1.0, and the verifier's cross-version tests (1.0.9 and 1.10.8 against 1.16.2) all passed. But 1.9.0 dropped 0.x decoding, readers got stricter during 2026, and value kinds are a closed set, so a new one makes old clients reject the blob. There is no "forever" guarantee. The latest version is 1.16.2; there is no Rust 1.15.x. | Pin exactly (=1.16.2). Golden corpus tested in CI in both directions. Group feature floor for Loro-level features. | [R3](R3-loro.md) |
| R3-2 | Shallow snapshots trim safely, trimmed-history imports fail in a defined way, and checkpoints work via checkout/fork_at (§4, §10) | Partial | High | Reproduced on 1.16.2: an import can partially apply and still return an error; a rejected peer's later changes stay pending forever; a stale device importing a shallow snapshot gets `Ok` with nothing applied; an unmerged head pins the root at the fork point; `fork_at` returns NotImplemented on shallow documents; the PR #1083 path aborts the process. | Compact only at a causally stable version (document epochs). Fresh-document bootstrap plus three-way rebase. Checkpoints outside the document. Never import a shallow snapshot into a document with parked changes. | [R3](R3-loro.md) |
| R3-3 | Older clients preserve unknown data from newer versions (P8) | Partial (worse than reported) | High | App-level fields survive. For unknown Loro container types, an old client's full-snapshot export silently loses data (a counter went from 3.0 to 0.0), and new clients' state then diverges from their own oplog. Mergeable containers are safe. | Group feature floor. Old clients never publish snapshots of documents with unknown container types; they send updates only. Fix the container set now and keep the `counter` feature on. | [R3](R3-loro.md) |
| R3-4 | Movable tree, movable list and maps are production-ready (§4) | Partial | High | 2026 brought many critical fixes: tree convergence (in 1.13.9, not 1.15.1), import panics, unimportable shallow snapshots (fixed in 1.16.2). #1055 and #1068 are fixed although still open. Undoing a delete creates a new TreeID, and deleted nodes can't be moved back. | Require ≥ 1.16.2. Run Loro's fuzz actions in the network simulator. Soft delete. App-level task UUIDs. | [R3](R3-loro.md) |
| R3-5 | Performance is fine on low-end devices, and bytes per op fit the budget (§10) | Unverified (performance); size corrected | Medium | Nobody has measured low-end devices. Loro is 1.4–1.9 MB stripped on arm64 (measured), not R3's estimate of 4–7 MB. A single-op update costs ~92–101 B. 10k tasks load in 28 ms on desktop (E1). | Measure on a Cortex-A53/A55 phone (E12). | [R3](R3-loro.md) |
| R3-6 | PeerIDs must be unique, and authorship isn't authenticated | Confirmed | High | Reproduced: squatting another member's PeerID and counters causes permanent silent divergence, because Loro deduplicates by counter only. E1 reproduced the same with accidental reuse. | PeerID in no-backup storage with a lease marker. Rotate on doubt. Single instance on desktop. Signed updates (E16). | [R3](R3-loro.md) |
| R3-7 | Kotlin bindings exist, and size is acceptable (§3) | Partial | Low | loro-ffi configures Swift only, and there is no maintained Kotlin binding. NDK r29 builds are 16 KB-aligned by default. | Link the loro crate inside the engine, behind a domain-level UniFFI API. | [R3](R3-loro.md) |

### R4: Android platform

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R4-1 | Background updates arrive "usually within 15 minutes" (§8) | Partial (severity raised) | High | On Android 15+, a still phone enters Doze within ~1–3 min (15 s + 15 s on A16/A17). Deep-idle windows come at ~+1, +3, +7, +13 h. Android 16 caps even the ACTIVE bucket at 20 min per 60 min. OEM killers add to this. | Reword with no number. Detect throttling (standby bucket, stop reasons, exit info). Keep runs short. | [R4](R4-android.md) |
| R4-2 | A bound widget keeps the app in the ACTIVE bucket, so sync doesn't slow down (§8) | Partial | Medium | The mechanism is confirmed in android17-release. But the widget gives no Doze exemption, the ACTIVE quota still applies, and on Android 15 a force-stop grays the widget and stops jobs. OEM behavior is untested. | Reword: the widget keeps the app out of low buckets. Offer the widget after joining. OEM test (E11). | [R4](R4-android.md) |
| R4-3 | shortService can start at onStop, and its notification is held back 10 s so it's "usually invisible" (§8) | Partial | Medium | There is a 5 s visibility grace period. The 10 s deferral is rate-limited: another FGS within 2 min shows its notification immediately. The Task Manager lists the app. | Make an expedited job (CONNECTED) the primary flush, enqueued while the app is visible (onPause, after each commit). shortService becomes optional, without notification actions. | [R4](R4-android.md) |
| R4-4 | Charging phones aren't in Doze, and back-to-back jobs keep them reachable (§7a) | Partial | High | Jobs are quota-free while charging and can run 30 min. But "charging" means ≥ 90% or a rising level, so charge limiters vary. Android 15+ cuts background network within 2–5 s and freezes the process after 10 s. Batching can delay jobs up to 31 min. | Phones help only during their own job windows. Report reachability as a probability. Rely on desktops. | [R4](R4-android.md) |
| R4-5 | dataSync FGS is capped at 6 h per 24 h and faces Play review (§7a) | Confirmed | Medium | FGS timeout docs. dataSync use cases are user-initiated transfers. | Remove the FGS helper fallback. | [R4](R4-android.md) |
| R4-6 | ACCESS_LOCAL_NETWORK is needed for LAN sync; NEARBY_WIFI_DEVICES on SDK 36 (§12) | Partial | Medium | Mandatory only when targeting 37 on Android 17. It covers mDNS, multicast, CGNAT 100.64/10 and on-link IPv6. On 36 it's opt-in, so nothing is needed. Without it: EPERM or timeouts. | Request nothing on 36. On 37 request it in context. Treat EPERM as "path unavailable". Note that same-LAN IPv6 also needs it. | [R4](R4-android.md) |
| R4-7 | The SCHEDULE_EXACT_ALARM plan (§12) | Confirmed | Low | Denied by default on 14+. USE_EXACT_ALARM isn't for task apps. Inexact alarms can be up to 1 h late. The permission is denied again after a restore. The while-idle quota is 72/h. | `setAndAllowWhileIdle` fallback. Re-check on start. Re-arm on boot, update and time change. Re-prompt after a restore. | [R4](R4-android.md) |
| R4-8 | Block Store backs up the personal secret E2E and restores it on a new device (§11) | Partial (worse than reported) | High | 16 entries; the API reference gives `MAX_SIZE` = 1,024 B, while the guide says 4 KB. E2E needs Android 9+ with a screen lock. Cloud restore works on Pixel from Android 9, on other phones only from Android 12. Restore happens only in the setup flow. | Device linking as the default recovery. Design for 1 KB per entry. Never upload the raw secret without E2E. | [R4](R4-android.md) |
| R4-9 | Auto Backup: 25 MB, daily, E2E with a screen lock, D2D, restore before first launch (§11) | Confirmed (facts); a gap was missed | High | The facts hold. Missed: a user who skips the setup-wizard restore gets neither Auto Backup nor Block Store. D2D ignores `allowBackup=false` for target 31+. A restore clones device identifiers. | Exclude identity material from cloud backup and device transfer. clientSideEncryption flags. Offer a restore on first launch. | [R4](R4-android.md) |
| R4-10 | The clipboard works as a deferred invite on first launch (§5) | Partial | Medium | Reading needs window focus. `getPrimaryClip` shows a toast; reading the description doesn't. AOSP clears the clipboard after 1 h. The permanent secret is exposed to keyboards and clipboard sync. | Check the description first, after focus. Clear the clipboard after use. Copy a short-lived invite, never the secret. | [R4](R4-android.md) |
| R4-11 | Glance widget checkboxes need Android 12 (fallback: tap opens the app) (§12) | Partial | Low | Glance backports checkboxes below Android 12. A burst of redraws within one session costs one job. | Drop the fallback row. One `updateAll` per sync run. | [R4](R4-android.md) |
| R4-12 | The Google code scanner needs no camera permission (§12) | Confirmed | None | ML Kit docs. | Declare the install-time module so the first scan works offline. | [R4](R4-android.md) |
| R4-13 | Play In-App Updates confirms a newer version exists (P8) | Confirmed | Low | It works only for Play installs owned by the account. Update priority is set once, through the Publishing API. | Branch on the installer; use a signed manifest for sideloaded installs. | [R4](R4-android.md) |

### R5: Prior art

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R5-1 | The Mainline DHT handles group discovery, and helpers announce there, with r helpers XOR-closest to a daily group hash (§6, §7a) | Partial | High | BEP44: records ≤ 1000 B, may expire after 2 h, one per key+salt. BEP43: phones should be read-only nodes. BEP5 `get_peers` gives a rendezvous list, but no consistent "r closest" view, and the ~8 storing nodes can be eclipsed. | Use the DHT only for per-device endpoint records. Choose a real helper discovery mechanism before building §7a. | [R5](R5-prior-art.md) |
| R5-2 | About 1,000 helpers per 10k users (10%) (§7a) | Partial (was Refuted) | High | Refuted for opt-in volunteers: Syncthing relays ~0.01%, Tor ~0.29%. Unverified for default-on desktop helping, where precedents (the BitTorrent DHT, Kubo, Skype supernodes) reached high participation. Below ~0.1% the design is infeasible. | Size for 0.1–1% participation. Desktop default on. A dedicated helper mode. Measure in the beta. | [R5](R5-prior-art.md) |
| R5-3 | r ≥ ln(0.001)/ln(1−p) gives 99.9% availability (§7a) | Refuted | High | The formula uses reachability, but RAM-only helpers lose slots on every restart, so retention is what matters. E5 measured 81% at r=10 where the formula predicts 99.9%. Choosing helpers through the trust graph also correlates their churn. | Compute r from measured retention while the group's own devices are offline. Pick part of each set uniformly at random. Start at r=20 (E5). | [R5](R5-prior-art.md) |
| R5-4 | Hypercore/Keet shows serverless offline delivery for mobile works (§7a) | Refuted | High | Keet's blind peers are always-on nodes configured by the operator (100 GB by default). Its push notifications go through a configured gateway. Bootstrap nodes are fixed. | Member devices are the backbone. Plan the headless member build. Label open helpers experimental. | [R5](R5-prior-art.md) |
| R5-5 | Syncthing shows volunteer infrastructure works at scale | Partial | Medium | Its relays are forward-only, rate-limited and a last resort. About 100 relays stayed flat while users doubled. Discovery servers are paid for by the project (> €1,000/month). | Use Syncthing as a model for forwarding only, not storage. | [R5](R5-prior-art.md) |
| R5-6 | A background P2P sync app fits Play rules without special risk (P6, §8) | Partial | Medium | Syncthing-Android left Play (over storage access). The fork's signing-key custody is unclear since its maintainer vanished. | No phone-helper mode in v1. Organization account. Key escrow. Reproducible builds. | [R5](R5-prior-art.md) |
| R5-7 | The design avoids SSB/Manyverse failure modes | Partial | Medium | Invites and compaction avoid them. The helper network recreates the economics of volunteer mirrors (SSB pubs). | Keep invites independent of helpers. Make quotas and admission control mandatory. | [R5](R5-prior-art.md) |
| R5-8 | Member devices, including one's own always-on machine, are the best mailboxes (§7a tier 1) | Confirmed | Low | Briar Mailbox, Keet's advice to keep a desktop on, Ink & Switch's cloud peer. | Promote the headless member build to a planned phase. | [R5](R5-prior-art.md) |
| R5-9 | Local-first sync at scale works without any hosted nodes (C2, §13) | Unverified (no precedent) | Medium | Anytype, Automerge and Skype all ended up with hosted nodes. Joplin relies on the user's own cloud. | State the trade-off. Keep the status indicator honest. | [R5](R5-prior-art.md) |
| R5-10 | Free Nostr relays are fine at launch, and helpers fix acceptance later (§7) | Partial | High | Delta Chat hit email providers' limits and built its own relays. E4 shows acceptance is fine today, but rate limits (R2) apply now, and most relays run strfry. | Jittered backoff. A signed, updatable relay list. Acceptance monitoring. A dedicated kind (not 30078; see R2-6). | [R5](R5-prior-art.md) |
| R5-11 | 6-word offline codes beat short PAKE codes (§5) | Confirmed | Low | Short codes are safe only when PAKE limits the attacker to online guesses. | Keep. Add a parallel path where an online member answers invite lookups. | [R5](R5-prior-art.md) |
| R5-12 | n0 public relays are a production fallback (§6 L7) | Refuted (as production-grade) | High | Docs: "Suitable for development and testing"; traffic may be throttled. They're acceptable as best-effort, because the relayed share of traffic is small. | Written permission from n0. Make layer 6 and desktops the primary live fallback. | [R5](R5-prior-art.md) |
| R5-13 | libp2p circuit relays can be a second relay provider (§6, §15) | Refuted | Medium | Each circuit is limited to 2 min and 128 KiB. The relays exist to coordinate hole punching. Heavy dependency, no iroh interop. | Remove from §6 and §15. | [R5](R5-prior-art.md) |
| R5-14 | Per-group reputation makes bad helpers harmless, and eclipse buys little (§7a) | Partial | High | Data integrity holds; metadata doesn't (the KAX17 pattern). I2P says its identical daily H(key+date) rotation "is not particularly effective". | Secret-keyed bootstrap. IP/ASN diversity. Caps on each helper's share. Document the residual risk. | [R5](R5-prior-art.md) |
| R5-15 | RAM-only helpers are fine because members hold full data (§7a) | Confirmed (durability) | Low | Freenet precedent. RAM-only does lower availability (see R5-3). | Keep. Monitor the rate of missing slots. | [R5](R5-prior-art.md) |
| R5-16 | Timely change notifications need only WorkManager, no push (§6, §8) | Partial | Medium | Jami, Session, SimpleX, Keet and Delta Chat all use push or a persistent service. In the rare bucket, jobs get about 10 min per 24 h. | Scope default notifications. Design the doorbell early. | [R5](R5-prior-art.md) |

### R6: User experience evidence

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R6-1 | There's a market gap a free, account-free app can fill (§1) | Partial (was Confirmed) | High | The gap is real: Google Tasks can't share lists, Todoist's free plan is capped, and every competitor needs an account. But there's no iOS client: iOS is 60.7% of US mobile use, and iPhone invitees reach a landing page that only offers Play. | Bring the web guest client forward (view and check off). Make the landing page iOS-aware. | [R6](R6-ux-evidence.md) |
| R6-2 | The design avoids known P2P failure modes: battery, offline delivery, onboarding, key loss | Partial | Medium | Briar went into maintenance mode on 2026-07-09 citing exactly these. The design avoids recovery phrases and always-on services, but not misleading status or closed-app latency. | Honest status, closed-app latency mitigations, firewall handling. | [R6](R6-ux-evidence.md) |
| R6-3 | Minutes-to-hours background delay is acceptable; "Stay connected" can wait (§8) | Partial | Medium-High | With the app open, delivery is at parity (OurGroceries polls every 20 s while open). With the app closed, notifications and widgets lag, while Bring! pushes instantly. | Per-member freshness. Widget prompt. Test the "Tell Alex" nudge. Design the doorbell. | [R6](R6-ux-evidence.md) |
| R6-4 | The copy "usually within 15 minutes..." is honest (§8) | Partial | Medium | The rare bucket gets 3 job sessions per 24 h and restricted gets 1 per day. On stock Android, apps with a widget are exempt from the low buckets. | Promise no number. Show others' freshness. Keep the blocked-sync explainer. | [R6](R6-ux-evidence.md) |
| R6-5 | 6 EFF-large words are usable, and the large list is required (§5) | Refuted (as specified) | High | 4 words contain hyphens (1 in 325 codes). 14 homophone or variant pairs. 2,364 pairs one edit apart. A word needs 5.3 letters on average to identify. 7 BIP39 words give 77 bits, so the large list isn't required. | Choose the list via E7.1 and E7.3 (E7 favors 8 × EFF short 2.0). Chip-based entry UI. Consider a checksum word. | [R6](R6-ux-evidence.md) |
| R6-6 | The clipboard handoff carries the invite through the Play install (§5) | Partial | Medium | The focus rule, the toast, the 1 h auto-clear, and any later copy all break it. Firebase Dynamic Links is gone. | Read the description first. A secret-free Install Referrer flag triggers a "tap your link again" screen. Clear landing-page copy. | [R6](R6-ux-evidence.md) |
| R6-7 | First launch asks nothing; the name is asked at first share or join; permissions are asked in context | Confirmed | None | Android guidance and the Nostr design guide agree. | Put the name field inline on the join screen. Tie the notification permission to joining. | [R6](R6-ux-evidence.md) |
| R6-8 | Display name plus avatar plus tag tells members apart (§5) | Partial | Medium | Discord's discriminator tags failed (indirect evidence). The likeliest duplicate is the same person on a second, unlinked device. | Local nicknames. "Is this you on another device?" Renames shown in history. | [R6](R6-ux-evidence.md) |
| R6-9 | Approval on an existing device plus a 5-minute QR expiry makes linking safe (§5) | Refuted | Critical | Signal's linked-device phishing (GTIG, 2025). The approval happens on the attacker's own device, and merge-on-link hands over all lists. | Proximity requirement. Scanning-side warning. Reject link payloads in join paths. Alert all devices. Never merge silently. | [R6](R6-ux-evidence.md) |
| R6-10 | State is always visible, including "up to date" (UX principles) | Partial (was Refuted) | High | A device can know it merged the latest slot of every known device on the relays it reached, but not that it has everything. Anytype's "Synced" indicator misled users. | Never say "up to date". Show Saved → Sent → On Alex's phone. Coarse check-in times. | [R6](R6-ux-evidence.md) |
| R6-11 | No technical words, not even "sync" (UX principles) | Partial | Low | "Sync" is everyday vocabulary; transport jargon is the real problem. | Ban transport words. Allow sync, offline, updated. Add a "Why isn't this updating?" screen. | [R6](R6-ux-evidence.md) |
| R6-12 | Undo instead of confirmation works for all deletions (UX principles) | Partial | Medium | Undo shows only for the person who acted. There are no roles. DESIGN already has a 30-day trash. | Show the trash to every member, with who and when. Banner after bulk deletes. On a shared list, "Delete list" means Leave. | [R6](R6-ux-evidence.md) |
| R6-13 | The widget's "synced X ago" label shows useful freshness (§8) | Refuted (as specified) | Low | The label freezes between redraws, exactly when sync stalls. | Absolute time ("Updated 14:05"), or show it only when stale. | [R6](R6-ux-evidence.md) |
| R6-14 | Seedless recovery avoids key loss (§11) | Partial | Medium | Block Store fits, but users who skip restore and devices without E2E lose it. | Show backup state under "Your devices". Prompt for cloud or folder backup when E2E is unavailable. | [R6](R6-ux-evidence.md) |
| R6-15 | Playtests with ~5 people validate the UX (UX principles) | Partial | Medium | 5 users cover one homogeneous group. Sync and OEM problems only appear across days and devices. | More groups, a 2-week diary study, and a code-entry study with 20+ people. | [R6](R6-ux-evidence.md) |
| R6-16 | Invite links open the app directly from chat apps (§5) | Unverified | Medium | It depends on each app's in-app browser and Custom Tabs setup. The cited sources were weak. | E13 matrix. "Open in app" link and a browser hint on the page. | [R6](R6-ux-evidence.md) |
| R6-17 | Holding the splash ≤ 1.5 s, then showing changes live, is smooth (§8) | Partial | Low | Late merges shift rows under a finger, so the wrong item gets checked. | Pin the touched row. Delay reorders 500 ms. Animate inserts. | [R6](R6-ux-evidence.md) |
| R6-18 | Windows Firewall prompts the first time the app listens (§12) | Unverified (probably wrong) | Low | Delta Chat: UDP on a random port raised no prompt, while a fixed UDP port and TCP did. Inbound blocks mainly hurt LAN connections. | Firewall rule in the installer or MSIX, or a stable port. Don't pre-announce a prompt. Test (E3.8). | [R6](R6-ux-evidence.md) |

### R7: Security

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R7-1 | 6-word codes with Argon2id make offline multi-target brute force infeasible, and the tag leaks nothing (§5) | Partial | Medium | 77.55 bits. An RTX 4090 does 1,703 guesses/s (E2: RTX 3080, 783/s) at 64 MiB, t=3. With 1M invites: ~70,000 years at 100k guesses/s, ~412 years with 10k GPUs. DESIGN's 4-word figure ("minutes") is really ~10 h, and its 5-word figure ("days to years") is ~9 years. With no salt, the pool of attackable invites grows forever. The payload is the permanent secret. | Date-bucketed salt. Payload = current epoch key or a capability. Derive a tag of ≥ 128 bits via HKDF from the Argon2 output. A fresh throwaway key per invite event. Choose p by phone test. Correct the numbers. | [R7](R7-security.md) |
| R7-2 | Key epochs keep removed members out (§5) | Partial (underspecified) | High | No fork-merge rule, no protection against backdating, no mutual-removal policy, no authenticated additions. E8 reproduced resurrection of removed devices (441/3000 runs) until rule 4 was added. | E8 rules 1–4, receiver-side validation, a backdating rule, periodic rotation (see §6 here, §5 changes). | [R7](R7-security.md), [E8](E8-key-epochs.md) |
| R7-3 | Loro ops are unauthenticated; should updates be signed? | Confirmed | High | Loro's docs say duplicate PeerIDs corrupt documents. Kleppmann shows equivocating replicas never converge. Signing alone doesn't stop self-equivocation. | Signed blobs. PeerID bound to the signer. Own ops only. Signed per-range hashes as proof of equivocation. | [R7](R7-security.md) |
| R7-4 | All incoming data is untrusted: size limits, validation, fuzzing, rejection without crashing (P8) | Partial (worse than reported) | High | `panic=abort` stops UniFFI from catching panics. Loro #1106 aborts even with unwind, and its trigger was `fork_at`. A6: an 85-byte input aborts 1.16.2. P2P has no size cap (A5: a 50 MB update was imported). | Process-isolated import. Quarantine journal. Caps. Build a fresh `LoroDoc` from bytes instead of using `fork_at`. | [R7](R7-security.md), [A6](A6-parser-fuzzing.md) |
| R7-5 | Sealed backups defeat automated scanning; Auto Backup is E2E-encrypted with the screen lock (§9, §11) | Partial | Medium | Without a screen lock, Google can read the Auto Backup. Sealing is obfuscation: once someone reverse-engineers the format, every copy is exposed. Each backup holds all live group secrets. DPAPI is readable by same-user malware. | Correct §9. `disableIfNoEncryptionCapabilities` for the secrets file. Store only the personal secret. Wrap the seal key with the OS keychain where possible. | [R7](R7-security.md) |
| R7-6 | Helper trust resists sybil, eclipse, rollback, collusion and junk flooding (§7a) | Partial | Medium | Identities are free, so sybils can capture probation slots. The bootstrap position is computable, and landing near it takes ~10 bits of grinding. Colluding helpers defeat freshness comparison. Useful PoW limits are the same order as legitimate usage. | IP/ASN diversity. HMAC(epoch key, date) bootstrap. Signed receipts and version heads. Cap each member's influence. Epoch-signed helper sets. | [R7](R7-security.md) |
| R7-7 | Metadata exposure is limited to the §9 table, and groups are unlinkable (§7, §9) | Refuted | Medium | Relays link a device's groups by IP, connection, timing, REQ filters and multi-pubkey AUTH. The stable iroh endpoint ID published via n0's DNS is a presence oracle. iroh no longer uses STUN. | Rotating topic tags and slot IDs. Per-group flush jitter. Relay-only or encrypted address records. A rotating transport key. Update §6 and §9. | [R7](R7-security.md) |
| R7-8 | Secrets in URL fragments, the clipboard and QR codes are acceptably protected (§5) | Partial | High | Links and QR codes carry the permanent secret. Safe Links rewrites and detonates URLs. In-app browsers inject JavaScript. The keyboard can always read the clipboard. A web page can't mark a clip as sensitive. | A 256-bit capability for a short-lived, revocable invite record. A user tap before joining. Verified App Links. Sensitive-content flags. FLAG_SECURE on QR screens. | [R7](R7-security.md) |

### R8: Policy and feasibility

| ID | Claim | Verdict | Sev. | Evidence | Fix | Report |
|---|---|---|---|---|---|---|
| R8-1 | The shortService flush is Play-compliant without special review (§8) | Confirmed | None | Play's FGS policy explicitly exempts shortService. | Keep. Confirm in Play Console that no declaration is asked for (E15). | [R8](R8-policy.md) |
| R8-2 | Phone helpers can fall back to a dataSync FGS (§7a) | Refuted (also technically impossible) | High | Play allows an FGS only when user-initiated or perceptible. dataSync is capped at 6 h. On Android 12+, a background worker starting an FGS gets `ForegroundServiceStartNotAllowedException`, and no exemption applies. | Remove the fallback. | [R8](R8-policy.md) |
| R8-3 | A specialUse FGS works for a future "Stay connected" mode (§8) | Partial | Medium | specialUse is reviewed at submission. A toggle the user starts satisfies the background-start rules. | Keep it out of v1. If built, use specialUse with a justification. | [R8](R8-policy.md) |
| R8-4 | The Data safety form declares no collected data (§9) | Partial | Low | The E2E exemption is confirmed. But persistent endpoint IDs, pubkeys and IPs sent to relays count as pseudonymous data. Play requires a privacy-policy link inside the app. | Declare "Device or other IDs", or document why not. In-app privacy link. | [R8](R8-policy.md) |
| R8-5 | SCHEDULE_EXACT_ALARM, asked when a reminder is first set or received (§12) | Confirmed | Low | Tasks.org does the same. Revoking the permission kills the app and cancels alarms. Received reminders arrive in the background. | Receiver for permission changes. Prompt through a notification for received reminders. | [R8](R8-policy.md) |
| R8-6 | targetSdk 36, then 37 as Play requires (§12) | Partial | Low | 36 is required from 2026-08-31 (extension to 11-01). The 2027 deadline is unpublished; likely 37 around Aug 2027, which would enforce the local-network permission. | Keep. CI 16 KB check. Re-check mid-2027. | [R8](R8-policy.md) |
| R8-7 | Export compliance is declared as standard encryption (§9) | Partial | Low | Play Console has no export declaration. Under the EAR the app is mass-market 5D992.c with no report needed. Open-source crypto isn't subject. | Reword §9. Check the developer's own country's rules. | [R8](R8-policy.md) |
| R8-8 | P2P and Nostr apps face no Play policy problem | Confirmed | None | Amethyst, Primal, Briar, Jami, Syncthing-Fork and Orbot were all updated Jul–Sep 2026. | None needed. | [R8](R8-policy.md) |
| R8-9 | Helpers relaying strangers' encrypted data are acceptable on Play (§7a) | Partial | Medium (was High) | Play's proxy clause allows proxying for third parties only as the app's core purpose. But torrent clients that seed to strangers are live on Play. | Helpers off by default on Android. Prefer desktops. Keep strangers' helping out of the play flavor at first. | [R8](R8-policy.md) |
| R8-10 | drive.file needs brand verification; unverified means a warning screen and a 100-user cap (§11) | Refuted (with a caveat) | Medium | drive.file is a non-sensitive scope: no verification, no warning screen, no cap. But Google's OAuth policy requires a homepage on a verified domain you own. | Publish the OAuth app In production. Brand verification is optional. Use an owned domain. | [R8](R8-policy.md) |
| R8-11 | A free github.io domain is enough for Google verification (§15) | Unverified (was Refuted) | Medium | The evidence is weak, and github.io is on the Public Suffix List. OAuth policy wants an owned domain anyway. | Use an owned custom domain. Settle in E15 if still relevant. | [R8](R8-policy.md) |
| R8-12 | The foss flavor signs in to Google through the browser (§12) | Refuted (likely; was Partial) | Medium | Google: custom URI schemes are "no longer supported on Android", loopback is deprecated, a web client type isn't allowed on Android, and client secrets must never go in public repos. | Drop Google Drive from foss. Use SAF folder backup and OneDrive via MSAL. | [R8](R8-policy.md) |
| R8-13 | OneDrive needs only a free app registration (§11) | Partial | Medium | It needs an Entra tenant through an Azure free account (credit-card identity check). Publisher verification isn't available to individuals. An AppFolder 403 regression has been open since ~2026-08-23. | Personal accounts only. Re-test before phase 6. Keep Google Drive first. | [R8](R8-policy.md) |
| R8-14 | The clipboard invite is compatible with Play and Android (§5) | Partial | Medium | No policy problem. But the clipboard auto-clears after 1 h, a web page can't mark a clip sensitive, and gating on the description is weak for text/plain. | Short-lived invite. Overwrite the clipboard after joining. Keep "tap the link again". | [R8](R8-policy.md) |
| R8-15 | F-Droid inclusion later through the foss flavor (§12) | Partial | Medium | Needs FLOSS source. F-Droid bundles API keys in source, which conflicts with Google's policy. The app ID must not match someone else's domain. Rust via cargo-ndk is proven. | No automatic GitHub requests. Reproducible builds. Settle the domain. Decide on open source. | [R8](R8-policy.md) |
| R8-16 | Flathub is a viable Linux channel (§12) | Partial | Medium | The app ID needs domain control. Tray-only apps are rejected. "Manifests must not contain AI-generated or AI-assisted content", and AI-generated code must be disclosed. Apps with too little development history are rejected. | Human-written manifest, honest disclosure, submit after months of public history. AppImage/.deb as fallback. | [R8](R8-policy.md) |
| R8-17 | winget distribution is free (§12) | Confirmed | Low | Pull request to winget-pkgs. Needs direct release URLs and a clean uninstall. | Direct URLs. Prefer the msstore source. | [R8](R8-policy.md) |
| R8-18 | Microsoft Store works for an individual developer with MSIX (§12) | Confirmed | Medium | Free individual registration since 2025-09-10, and the Store re-signs MSIX. jpackage can't produce MSIX. MSIX virtualizes AppData and deletes it on uninstall. | Store MSIX (full trust) as the primary Windows channel. Test NIO, UDP and AppData (E15). | [R8](R8-policy.md) |
| R8-19 | A Windows MSI via GitHub costs nothing and gives acceptable UX (§12) | Refuted | Medium | Unsigned installers get a strong SmartScreen block. Paid signing still warns until reputation builds. Artifact Signing for individuals covers only the US and Canada. | Store first, winget via msstore, GitHub MSI only as a fallback. | [R8](R8-policy.md) |
| R8-20 | GitHub Pages hosts the join page for free (§5, §12a) | Partial | High | On the Free plan, Pages needs a public repo, but the design has one private repo. assetlinks.json must sit at the host root. A renamed username becomes claimable, making the host a trust anchor for fragment secrets. Pages logs visitor IPs. | A separate public site repo on an owned, GitHub-verified custom domain. Minimal page with a CSP meta tag. Capabilities in links, not secrets. | [R8](R8-policy.md) |
| R8-21 | GitHub Releases serve desktop update checks (P8) | Partial | Medium | Release assets of a private repo aren't public. The unauthenticated API allows 60 requests/h per IP; the verifier hit that limit. | Public releases repo. A signed latest.json on our own domain, fetched daily with jitter. | [R8](R8-policy.md) |

---

## 3. New risks the design didn't consider (deduplicated)

Risks raised by several dimensions are merged. "Raised by" lists every source, and "(v)" marks a risk found by a verifier.

### A. Engine integrity and data

| ID | Risk | Sev. | Raised by | Fix |
|---|---|---|---|---|
| NR-01 | Poison-pill crash loop: a malformed blob (from a malicious member, a bug or disk corruption) aborts the app on every launch, for every member | Critical | R3, R7, A6 | Process-isolated import, quarantine journal, caps, `panic=unwind`, CI fuzzing, upstream report |
| NR-02 | Identity cloning through Auto Backup restore, D2D or a second desktop instance: a shared PeerID (silent divergence), shared Nostr slot keys (mutual overwrites), a shared iroh endpoint key (forbidden concurrent use) | Critical | R1, R3, R4, R7, E1 | No-backup storage plus extraction rules for both cloud backup and device transfer. Keystore-bound install marker. Mint a new identity on restore. Single-instance desktop |
| NR-03 | Member forgery: squatting another member's PeerID drops their real edits everywhere and frames them; self-equivocation splits replicas even with signatures | High | R3, R7 (v), A5 | Signed blobs, PeerID bound to the signer, own-ops-only checked on the isolated copy, signed per-range hashes, rotation on detection |
| NR-04 | Compaction at a device's own head permanently rejects others' concurrent offline edits, and their descendants stay pending with no signal | Critical | R3 | Causally stable roots, or document epochs verified by all members |
| NR-05 | Missing one snapshot rotation strands a device (import returns Ok, nothing applied). Fetching a device slot before the snapshot follows the PR #1083 path and aborts | High | R3 (v) | Bootstrap into a fresh document from the snapshot before importing device slots. Never import shallow into a document with parked changes |
| NR-06 | A naive long-offline rebase gives old edits fresh timestamps, so they win last-writer-wins against newer edits | Medium | R3 | Three-way app-level rebase. Surface conflicts |
| NR-07 | Pending out-of-order changes are lost on restart (reproduced) | Medium | R3 (v) | Persist raw received blobs until nothing pending references them |
| NR-08 | Old clients that export full snapshots of documents with unknown container types silently lose data and desync new clients | High | R3 (v) | Group feature floor. Old clients send only updates for such documents |
| NR-09 | One unmerged concurrent head pins the shallow root at the fork point, so compaction silently does nothing | Medium | R3 (v) | Merge commit before compacting. Check `shallow_since_frontiers()` afterwards |
| NR-10 | Moving a task between lists crosses documents and isn't atomic: duplicates, lost edits, dropped fields | High | R3 | One document per group, or an explicit move protocol (E1.7) |
| NR-11 | Undoing or restoring a deleted tree node creates a new TreeID, which breaks reminders, widgets and links | Medium | R3 | Soft delete, purge at 30 days, app-level UUIDs |
| NR-12 | Two devices creating a child container under the same key hides one of them. Mergeable children are never trimmed, and can't be deleted once the owner node is gone | Medium | R3 (+v) | `ensureMergeable*` from day one. Purge mergeable children before `tree.delete`. Link devices by an app-level copy, not by importing history |
| NR-13 | Size bombs over P2P: a 50 MB update was imported with no limit | High | A5, R7 | Engine caps enforced before live import. Per-author rate limits |
| NR-14 | History-budget shrink: mass deletion shrinks the state-based budget, so compaction erases the undo path | Medium-High | R7, A5 | Budget from a high-water mark. A minimum 30-day window. "Undo changes by [person] since [time]" |
| NR-15 | A flood of updates with unsatisfiable dependencies; deep nesting overflows the stack | Medium | R7 | Cap the pending buffer with a timeout. Cap nesting depth before import |
| NR-16 | New validation rules conflict with "compatible forever": later caps make old and new clients diverge | Medium | R7 | Ship every limit in format v1. A per-group rules-version ratchet |
| NR-17 | Backup-quota DoS: one member bloats a list past 25 MB, and every member's Auto Backup stops silently | Medium | R7 | Per-document caps. Back up only what the device needs. Surface backup failures |
| NR-18 | Loro depends on im, sized-chunks and bitmaps: MPL-2.0 and unmaintained (principle 9 says MIT/Apache/BSD). RUSTSEC-2026-0255 becomes reachable once `catch_unwind` is used (R3's HIGH advisory claim was wrong; that one is patched) | Medium | R3 (corrected by v) | An explicit MPL-2.0 decision. cargo-deny exceptions. Ask upstream to replace im |
| NR-19 | Threading and re-entrancy panics or deadlocks in callbacks | Low | R3 | One actor thread per LoroDoc. No synchronous Loro calls from subscribers or UniFFI callbacks |
| NR-20 | Per-session PeerIDs grow version vectors (~1,800 sessions a year) and cost ~51 instead of ~12 B per op | Low | R3 | A per-install PeerID with a lease. Frontiers in handshakes |
| NR-21 | Change timestamps are opt-in, coarsened by merging, and unauthenticated | Low | R3 | Set them in the document factory. Label them as claimed |

### B. Groups, keys and invites

| ID | Risk | Sev. | Raised by | Fix |
|---|---|---|---|---|
| NR-22 | Device-link phishing hands over all of a user's lists | Critical | R6 | Top-10 #3 |
| NR-23 | Ghost members: anyone with a leaked or old secret adds a device that future rotations include | High | R7, E8 | Additions valid only under the current head epoch, signed by a current device. Join notifications |
| NR-24 | Epoch forks encrypt to the other branch's removed member. Removed members backdate edits. Removal wars. Mutual removal empties small groups (38/3000 runs) | High | R7, E8 | E8 rules. Removals bounded by version vector. Remove-both, with visible Undo. Rate-limit removals |
| NR-25 | The permanent secret in links, QR codes and the clipboard leaks through keyboard and cloud clipboard history, browser sync, chat and photo backups, Safe Links and in-app browsers | High | R4, R6, R7, R8, A8 | Capabilities. Sensitive flags. FLAG_SECURE. Clear after use |
| NR-26 | The join page's origin is a trust anchor: a GitHub account takeover, username rename or domain lapse lets JavaScript read the fragment of every old link | High | R8, R7 | Owned, auto-renewed, GitHub-verified domain. Hardened account. Minimal page with a CSP meta tag (Pages can't send headers). Capabilities instead of secrets |
| NR-27 | Link scanners run the landing page, so an automatic join from the fragment would create ghost members or burn invites | Medium | R7 (v) | Always require a user tap before joining |
| NR-28 | Unverified App Links and custom schemes can be intercepted by other apps; foss builds on de-Googled devices may fall back to the chooser | Medium | R7 (v) | Verified App Links at the root of an owned domain |
| NR-29 | Without a salt, the pool of attackable invites grows forever, and a cracked old invite still works | Medium | R7 | Date-bucketed salt. Epoch-key payload |
| NR-30 | Epoch keys wrapped with X25519 on public relays can be harvested now and decrypted later, and there is no forward secrecy against archives | Medium | R7 (v) | Reserve a hybrid X25519+ML-KEM suite ID now. Periodic rotation. Say so in §9 |
| NR-31 | Format version and group-ID hash sit outside the AEAD, with no context binding | Low-Medium | R7 (v) | Bind version, group, epoch, slot and sender as associated data |
| NR-32 | One Ed25519 key is reused for iroh TLS, app signatures and X25519 wrapping | Low | R1, R7 | Separate purpose keys, cross-signed |
| NR-33 | The same person joins from a second, unlinked device: a duplicate member and split history | Medium | R6 | Same-name detection. Ask "Already use Kleiders on another device?" in the join flow |
| NR-34 | Invite codes: typos silently form other valid codes, hyphens break the separator, the list is English-only with awkward words, and keyboards learn typed codes | Medium | R6, R7, E7 | New list (E7.1). Chips. No suggestions and no personalized learning. hintLocales. Curated words |
| NR-35 | Invite decoys: anyone who has a code can post competing events under the same tag | Low | R7 | Show the inviter's avatar or fingerprint. Prefer the earliest event. Mark "joined via code" |
| NR-36 | A rotation for ~300 devices exceeds some relays' caps, and a pre-rotation Block Store copy goes stale | Low | R7 | Chunk the rotation or deliver it per device. Update Block Store right after rotating |

### C. Transport and metadata

| ID | Risk | Sev. | Raised by | Fix |
|---|---|---|---|---|
| NR-37 | n0 is the only provider of relays, address discovery (QAD), hole-punch coordination and default discovery. Shipping on relays meant for development is also a Play "violates a service's terms" exposure | High | R1, R5, E3, R8 (v) | Written permission. Configurable relays. Our own address lookup. Layer 6 |
| NR-38 | The stable device key is the iroh endpoint ID: n0's DNS, resolvers, relays and the LAN (via mDNS) see one ID across all groups, which links them and gives ex-members a presence oracle. mDNS also advertises public IPs | Medium | R1, R7, A7 | A rotating transport key certified by the device key. Relay-only or group-encrypted address records. Passive or filtered mDNS away from home. Rotate on device removal |
| NR-39 | Android DNS: a missing JNI context means a panic or a silent fallback to Cloudflare/Google/Quad9, and Private DNS is bypassed (plain UDP) | Medium | R1 | Explicit DoT/DoH nameservers. Disable the fallback. Verify with a packet capture |
| NR-40 | A bound endpoint costs battery and data (5 s heartbeats, QAD every 20–26 s), and two phones' background windows almost never overlap | Medium | R1 | Bind only in the foreground or in jobs. Background jobs dial only always-on peers |
| NR-41 | iroh upgrade treadmill: 3-month support per minor version, public relays only for the latest release, client-side security fixes. Satellite crates are pre-1.0 | Medium | R1 | Quarterly upgrades. Exact pins. Thin adapters. cargo-audit and cargo-deny. A notice on stale builds |
| NR-42 | Memory growth in long-running endpoints (#4549) | Low | R1 | 24 h soak test. Watchdog restart |
| NR-43 | Desktop firewall behavior is unknown: a Windows prompt, or Linux UFW blocking inbound UDP | Low | R1, R6 | Installer rule or stable port. Silent relay fallback |
| NR-44 | Nostr per-IP rate limits with 1 h bans (possibly at the CDN edge, which also blocks reads), shared by everyone behind a CGNAT. Retry storms after outages look like abuse | High | R2, R5 | Write budget. Full-jitter backoff. Republish staggered by key hash. Hourly re-upload caps |
| NR-45 | The ecosystem is moving private kinds behind AUTH with owner-only reads | High | R2 | Dedicated documented kind. Watch the NIPs and strfry defaults |
| NR-46 | OK doesn't mean stored (shadowReject, stealth bans, silent drops on khatru and nostr-rs-relay). NIP-11 hides WoT gating. Subscriptions can die silently | Medium | R2 (+v) | Health from read-back over a separate connection. Liveness checks. Treat WoT refusals as permanent for fresh keys |
| NR-47 | Chunked snapshots aren't atomic: torn generations, partial acceptance | Medium-High | R2 | Generation/index/count/hash in every chunk. A/B pointer flip after read-back |
| NR-48 | Relay implementations break same-second ties differently, and a fast clock locks a device out of its own slot | Medium | R2 | Monotonic, persisted `created_at` |
| NR-49 | A shared (group-derived) slot key lets any member or ex-member overwrite the slot, or pin it with a future `created_at` | Medium | R7, R2 | Per-device slots. Ignore future-dated events. Epoch-derived keys |
| NR-50 | Kind fingerprint: one config line blocks all the app's traffic. At ~100k daily users the app would write ~60 GB/day into each donation-funded relay | High | R2 | Frugal writes. NIP-40 on slots. Operator outreach. Bring-your-own/paid relays. Member nodes |
| NR-51 | Relay churn and correlated failure: nostr.band is dead, snort is in-memory, damus announced a shutdown (still running), most relays run strfry, and operators overlap (nos.lol/nostr.mom; offchain/bitcoiner.social). Shipped lists go stale in installs that are never updated | High | R2 (+v), R5, E4 | Signed relay list refreshed at runtime, plus gossip in group documents. Diversity of operators and software |
| NR-52 | Subscription caps (20 per connection, 5 on some relays) versus one REQ per group; combining groups in one REQ links them | Low-Medium | R2 | ≤ 15 subscriptions per relay. An explicit decision on linkability |
| NR-53 | damus's AUTH doesn't work | Low-Medium | R2 | Detect `auth-required:` and fall back |
| NR-54 | Aggregators and relay-to-relay sync copy events to relays the app never chose | Low | R2 | Covered by encryption and invite expiry |
| NR-55 | NIP-13 PoW cost grows with event size (estimated ~1 min per 48 KB chunk at difficulty 20) | Low-Medium | R2 | PoW only for small events |
| NR-56 | Blossom addresses blobs by the SHA-256 of the ciphertext, while the design uses BLAKE3 | Low | R2 | Store both hashes |

### D. Android platform

| ID | Risk | Sev. | Raised by | Fix |
|---|---|---|---|---|
| NR-57 | The Android 15+ background network firewall cuts network 2–5 s after leaving the foreground (unconditional from Android 16), and the Android 14+ freezer terminates TCP sockets | High | R4 | All background networking inside jobs. Enqueue the expedited job while visible. Close sockets cleanly. 2–3 relays per background run |
| NR-58 | Doze starts within ~1–3 min on Android 15+, and deep-idle windows start at 1 h and double to 6 h | High | R4 (v) | Honest copy. Freshness display. Doorbell |
| NR-59 | "Charging" isn't the same as "plugged in"; job batching can delay up to 31 min; Android 16 counts jobs started in the foreground or alongside an FGS against the quota | Medium | R4 | Base behavior on observed stop reasons. Expedited jobs for user-caused uploads. A user-initiated data transfer job for big first syncs |
| NR-60 | On Android 15, a force-stop grays the widgets and stops jobs and alarms until the app is next opened | Medium | R4 (v) | Detect it through ApplicationExitInfo on the next open and explain |
| NR-61 | Samsung's sleeping and deep-sleeping lists may override the widget exemption, and deep-sleeping apps miss updates, causing version skew in groups | Medium | R4 | OEM test. A Samsung-specific explainer. Compatibility across several versions |
| NR-62 | Keystore-wrapped secrets in backed-up files restore as undecryptable data; security-crypto is deprecated | Medium | R4 | Exclude secure storage from backup. Treat a decryption failure as an empty store |
| NR-63 | Android 17 local network protection (target 37, likely required around Aug 2027) blocks LAN, mDNS, port mapping and on-link IPv6. How iroh's UDP stack handles EPERM is unknown | Medium | R1, R4 | Request in context. Filter local candidates when the permission is missing. Test (E3.4) |
| NR-64 | The skip-restore gap: without Block Store and Auto Backup, the personal secret and solo lists are lost | High | R4 (v), R6 | Restore offer on first launch. Device linking. Backup prompts |
| NR-65 | Play's 16 KB page-size requirement for the Rust .so | Medium (handled) | R1, R4, R8 | Default with NDK r28+/r29 (E6 verified). CI check with llvm-readelf and `zipalign -P 16` |
| NR-66 | The exact-alarm permission is lost after a restore, and reminders received in the background can't ask for it | Low | R4, R8 | Re-prompt through a notification. Inexact fallback with honest wording |
| NR-67 | Glance redraws cost job quota; Android 17 per-app memory limits; Play vitals for wake locks | Low | R4 | One `updateAll` per run. Scale caches to the device. No app wake locks |
| NR-68 | In-App Updates doesn't work for GitHub or F-Droid installs | Low | R4 | Branch on the installer. Signed manifest |

### E. Helper network (later phase)

| ID | Risk | Sev. | Raised by | Fix |
|---|---|---|---|---|
| NR-69 | Patient sybil helpers learn which IPs share groups, and ranking by reputation times capacity rewards them with more groups | High | R5, R7 | IP/ASN diversity, share caps, unlinkable slot IDs, optional reads through a relay hop, and the residual risk documented |
| NR-70 | The bootstrap set can be eclipsed: its position comes from an observable hash, and daily rotation doesn't help (I2P) | Medium | R5 (+v), R7, A4 | HMAC(epoch key, date). IP-bound helper IDs. Nostr in parallel |
| NR-71 | A free encrypted mailbox attracts malware command-and-control, which brings antivirus flagging and legal risk for operators | Medium | R5 | Admission only with a membership proof. Short TTLs. Per-source quotas. An operator FAQ |
| NR-72 | Forged helper sets amplify junk traffic, and helpers can't tell legitimate reads from fake ones | Medium | R7, A3 | Epoch-signed helper sets. Per-slot rate limits. Capacity caps for young groups |
| NR-73 | Play's proxy clause applies to phones relaying strangers' data | Medium | R8 | Desktops first. Not in the play flavor initially |
| NR-74 | §7a's sizes (50 KB per group, 256 KB quota) conflict with E1's measurements (70–353 KB per list) | Medium | R5 (v) | Redo the load table with E1's sizes |
| NR-75 | PoW scaled by load penalizes new legitimate groups, and RAM-only excludes enthusiasts' 24/7 servers | Low | R5 | Prefer quotas over PoW. Opt-in dedicated helper mode with encrypted disk |

### F. Product and UX

| ID | Risk | Sev. | Raised by | Fix |
|---|---|---|---|---|
| NR-76 | No iOS client, though most US households that share lists include an iPhone | High | R6 (v) | Bring the web guest client forward. iOS-aware landing page |
| NR-77 | Late, stale or mismatched notifications ("Alex added eggs" after the trip; a notification with no visible list change) | Medium | R5, R6 | Notifications only from merged state. Batch, show age, drop stale, scope |
| NR-78 | Phone-only groups, probably the common case, have no tier-1 mailbox, so Nostr carries everything | Medium | R5 | Budget relay redundancy. Measure their share in the beta |
| NR-79 | Members who rarely open the app drift into rare or restricted buckets. Presence displays reveal activity to ex-partners or former roommates | Medium | R6 (+v) | Offer the widget. Coarse presence ("today", "this week") or show it only in the diagnosis screen |
| NR-80 | Concurrent offline adds create duplicate items, and linking can create duplicate lists | Medium/Low | R6 | "Milk ×2, merge?" hint. "Combine lists?" offer |
| NR-81 | With no roles, any member can clear or delete a list for everyone | Medium | R6 | Visible shared trash. Banner after bulk deletes. On a shared list, Delete means Leave |
| NR-82 | Layout shift from background merges leads to the wrong item being checked | Low | R6 | Pin the touched row. Delay reorders |
| NR-83 | Maintainer burden and bus factor: Briar, Syncthing-Android and Manyverse all stalled, and signing keys are a single point of failure | High | R5, R6 | Organization developer accounts. Key escrow. Succession document. Reproducible builds. Open the engine. Self-serve diagnosis |
| NR-84 | Play review churn for sync-class apps | Medium | R5, R6 | Minimal permissions. Document every FGS use. F-Droid as a fallback |

### G. Distribution, policy and accounts

| ID | Risk | Sev. | Raised by | Fix |
|---|---|---|---|---|
| NR-85 | Android developer verification: package name and signing key must be registered. The 2026-09-30 deadline applies to existing Play apps; sideload enforcement covers 4 countries now and goes global in 2027 | Time-critical | R8 (severity lowered by v) | Check the account's existing apps. Claim `com.kleiders.tasks` now. Register the sideload key |
| NR-86 | `kleiders.com` currently serves an unrelated AI fashion-design SaaS (registered 2026-03-03) | High | R8 | Confirm ownership, or rename before the first upload |
| NR-87 | Google OAuth policy: the homepage must be on a verified domain you own, the client type must match the platform, and client secrets must never be in public repos. So Google Drive can't be in foss or F-Droid builds | Medium | R8 (v) | Owned domain. foss without Google Drive |
| NR-88 | OneDrive AppFolder 403 regression for new personal-account consents | Medium | R8 | Re-test before phase 6. Never silently fall back to the broad scope |
| NR-89 | Flathub's AI policy and development-history rule | Medium | R8 | Human-written manifest. Disclosure. Submit later |
| NR-90 | MSIX deletes virtualized AppData on uninstall | Medium | R8 | Choose the storage location deliberately. Backup prompt. Test |
| NR-91 | Windows signing costs money and still warns until reputation builds, which conflicts with C2 | Medium | R8 (+v) | Store MSIX as the primary channel |
| NR-92 | UGC policies assume the developer can moderate content, which E2E P2P can't | Medium | R8 | Terms at first share or join. "Remove and block". A Report action by email. Honest store listing |
| NR-93 | The single private repo can't serve Pages or public release downloads; the GitHub API allows 60 requests/h per IP | Medium | R8 | Public site and release repos. Signed latest.json on our own domain |
| NR-94 | Desktop infostealers can read DPAPI and Secret Service secrets | Medium | R7 | Say so honestly. Rotation. Store only the personal secret. Optional OS-bound wrapping |
| NR-95 | Smaller Play obligations: new personal accounts need a 12-tester, 14-day closed test; privacy policy linked in-app; Data safety IDs; Photo Picker only for images; F-Droid's NonFreeNet flag; DSA trader status | Low | R8 | Recruit testers early (reuse the playtest group). In-app policy link. The Photo Picker. No automatic GitHub fetches in foss |

---

## 4. Contradictions between dimensions, and how to resolve them

| # | Topic | Conflicting positions | Resolution | Settled by |
|---|---|---|---|---|
| X1 | Nostr event kind | R5-10's fix, E4's first version and STATE.md say "use NIP-78 kind 30078". R2 says never, because NIP-78 was amended on 2026-09-03 to owner-only reads under AUTH. | **R2 wins.** E4 has already withdrawn its recommendation; STATE.md's findings log still says 30078. Use one dedicated, documented addressable kind. The probe's kind 31234 behaved the same on all 8 relays. | Done (primary source). E4.7 re-probes the chosen kind |
| X2 | Event size limit | E4/STATE.md: "strfry 128 KB message limit, ~95 KB payload". R2: 65,536 B JSON, about 47 KiB binary. | **R2 wins.** The 128 KB is the websocket frame size. E4's own raw data (45 KiB accepted, nothing larger sent) agrees with R2. Plan chunks of at most 40 KiB. | Done. E4.1 confirms the threshold locally |
| X3 | When Nostr acceptance problems start | R5 says limits apply "from day one" (unsourced). E4 found 8/8 relays accept today. R2 cites real bans. | **Two separate risks.** Kind and content acceptance is fine today and is a *trend* risk (AUTH gating). Per-IP *rate limits* are real now and sourced (noteguard's config, damus banning an app, nostr-station#54). So the write budget is a launch requirement. | E4.1, E4.3 |
| X4 | Native size | R1 projected 18–28 MB for the core (iroh-ffi at 14 MB). R3 estimated Loro at 4–7 MB. E6 measured the whole core at 7.07 MB; R3's verifier measured Loro at 1.4–1.9 MB. | **The measurements win.** R1's and R3's numbers are untuned upper bounds, which shows the size profile is load-bearing. Switching to `panic=unwind` will add some size, still to be measured. | E6.1 |
| X5 | Tombstones and trash purge | E1 says shallow snapshots never purge deleted items, so trash purging needs document epochs. R3's verifier says a shallow snapshot at the head *does* purge tree and movable-list tombstones (down to 1.2 KB), but not map keys (25.9 KB) or mergeable children, and that E1 measured StateOnly exports. | **Both are partly right.** Shallow-at-head purges some models, but it is also the dangerous operation (causal stability, stranded devices, the abort path). Document epochs (re-create from live state and have members verify) remain the preferred mechanism: they purge everything and also replace the shared snapshot slot. Purge mergeable children while their owner still exists. | E1.3 |
| X6 | Can a stale device import the shallow snapshot? | E1 says yes. R3's verifier says the import returns Ok, applies nothing, and leaves everything pending. | **The verifier wins** (reproduced on 1.16.2). Stale devices must bootstrap into a fresh document and rebase. | Done. E1.2 builds the path |
| X7 | Checkpoints via `fork_at`/StateOnly | E1 says they work. R3 says NotImplemented on shallow documents. | **Both hold, under different conditions.** They work only on documents that haven't been trimmed. Store checkpoints outside the live document, exported before trimming. | E1.6 |
| X8 | Crash containment | R7's fix: `unwind` + `catch_unwind` + import into a scratch fork. R3's verifier: `catch_unwind` + `mem::forget` survived the #1083 case. A6: a double panic aborts, so `catch_unwind` can't help. R7's verifier: #1106 aborts even with unwind, and `fork_at` triggered it. | **`catch_unwind` is only a partial measure.** Process isolation is the one layer shown to contain every abort. Build the scratch copy from bytes, never with `fork_at`. Keep the quarantine journal in every case. | E9 |
| X9 | A "safe" Loro version | R3: 1.16.2 is the minimum safe version. A6: fuzzing aborts 1.16.2 within seconds. | 1.16.2 is the floor for bugs triggered by honest peers. It isn't safe against malicious input, so containment (X8) is needed regardless. | E9 |
| X10 | PeerID strategy | R7 and E1: random per session. R3: persistent per install with a lease marker, because per-session IDs grow version vectors (~1,800 sessions/year, 51 vs 12 B/op). A5: PeerID = hash of the device key. | **A per-install PeerID bound to a device key that is never backed up**, plus a lease marker and a single-instance lock, rotated whenever continuity is uncertain. A5's hash works only if the device key itself is regenerated on restore. Signatures bind the PeerID in every case. | E10, E16 |
| X11 | Snapshot slot and keys | R2: per-writer slots, or a group-derived per-epoch key with per-device d-tags. R7: a group-derived key lets any member or ex-member vandalize the slot or pin it with a future timestamp. A5: drop the shared snapshot slot for verified document epochs. | **Per-device keys and slots.** Snapshots become document-epoch snapshots that members verify. Use a group-derived *read* key only if AUTH-gated relays ever require it, and never as the integrity mechanism (inner device signatures still apply). | E4.5, E1.1 |
| X12 | RustSec status of Loro's dependencies | R3: an unpatched HIGH advisory (RUSTSEC-2020-0041). R3's verifier: patched in the version Loro uses; RUSTSEC-2026-0255 applies instead. | **The verifier wins.** The MPL-2.0 licensing issue still stands. | Done |
| X13 | Old clients and unknown containers | R3's fix: catch `UnknownContainer` and fall back to a full snapshot. R3's verifier: that fallback publishes a corrupting blob. | **The verifier wins.** Old clients never publish snapshots of documents with unknown container types; they send updates only. | E1.4 |
| X14 | Hole-punching success rate | DESIGN ~90%, R1 70% (libp2p study), R1's own estimate 60–80% for mobile. R1's verifier: none of these figures is grounded for phone-to-phone connections. | **Drop the number from DESIGN.** Measure it: a NAT matrix plus local direct-vs-relayed statistics per network type. | E3.3 |
| X15 | Peer discovery | R1: put endpoint IDs in invites and the group document and dial them. R1's verifier: an ID still needs an address lookup, which means n0 DNS. E3: Nostr plus pkarr on the Mainline DHT (no n0 needed). R1 and R5: the DHT is off by default partly so mobile apps don't look like BitTorrent, and BEP43 says phones should be read-only. R7 and A7: rotate keys and publish relay-only or encrypted. R5's verifier: BEP5 has a rendezvous primitive. | **Primary: our own address lookup fed by E2E-encrypted, epoch-rotated per-device address records** in the Nostr mailbox and group document. **Optional: per-device pkarr on the DHT, desktops only**; phones only read. n0 DNS is off by default, or used only with a rotating transport key. | E3.2, E3.11 |
| X16 | Helper participation | R5: 10% is refuted. R5's verifier: refuted for opt-in, unknown for default-on desktop helping. E5's notes use 0.01–0.3%. | Opt-in numbers apply to opt-in designs. For default-on desktops the rate is unknown, and it depends on what share of an Android-first app's users install the desktop app. The design must still work at 0.1%. | E5.1, beta metrics |
| X17 | What breaks the r formula | R5: diurnal, correlated churn. R5's verifier: mainly RAM-only *retention*, since random selection keeps availability close to independent. | **Both apply.** p must be the chance that a helper still holds the slot. Selecting through the trust graph creates correlation, so part of each set should be uniformly random. | E5.1 |
| X18 | When Doze starts | R4: 30 + 30 min. R4's verifier: 15 s + 15 s on Android 16/17, from the release branches (googlesource's `main` lags them). | **The verifier wins.** Cite AOSP release branches. | E11 |
| X19 | Network access in the rare bucket | R5's verifier and R6: network "Disabled", so effectively no background sync. R6's verifier: jobs have network while they run. | Jobs do get network while running. The limit is how rarely they run (about 3 sessions a day in the rare bucket), which comes to roughly daily sync for members who rarely open the app, unless they keep a widget. | E11 |
| X20 | When to enqueue the flush job | R4: at ProcessLifecycleOwner ON_STOP. R4's verifier: ON_STOP fires 700 ms after the app stops being TOP, so the start guarantee is gone. | **The verifier wins.** Enqueue from `onPause` and after each local commit while visible. | E11 |
| X21 | Block Store entry size | R4 and R6: 4 KB (the guide). R7's verifier: the API reference says `MAX_SIZE` = 1,024 B. | Design for 1 KB. The 32-byte secret and a wrapped seal key fit. | E10 |
| X22 | Phone helpers while charging | R4: fragile, "mostly refuted". R4's verifier: chained jobs can start sub-second while charging; the conditions are what's unreliable. R8's verifier: the FGS fallback is technically impossible. R8's verifier: the Play proxy risk is medium (torrent clients are on Play). | Chaining is *possible but not dependable*. Remove the FGS fallback. In v1 phones don't help strangers. | E11 (charging chains) |
| X23 | Google Drive setup | DESIGN: brand verification needed, 100-user cap. R8: not needed. R8's verifier: OAuth policy still requires a homepage on a verified owned domain. R8 said github.io is refuted; its verifier says unverified. | No review is needed for `drive.file`, **but an owned domain is**. The github.io question becomes moot. | E15 |
| X24 | Google sign-in in foss | R8: re-enable custom URI schemes. R8's verifier: they are "no longer supported on Android", and client secrets can't go in public repos. | **Drop Google Drive from foss**; use SAF folder backup and OneDrive via MSAL. | E15 (only if challenged) |
| X25 | Severity of the developer-verification deadline | R8: critical. R8's verifier: it targets existing apps that weren't auto-registered, and the first-come package rule is unsourced. | It matters less for an unpublished app, but acting costs nothing, so claim the package now. | Play Console check |
| X26 | Invite word list | R7: keep EFF large and replace the 4 hyphenated words, keeping indices stable. R6: clean EFF large, or use 7×BIP39, or 8×EFF short, decided by a user test. E7: 8×EFF short 2.0 (82.7 bits, no typo neighbors, 3-letter prefixes). R6's verifier: add a checksum. | Stable indices matter only after codes have been issued, so the list is free to change before launch. **E7's 8×short list has the best measured properties.** Confirm with the human study, and try a checksum word as one of its conditions. | E7.1, E7.3 |
| X27 | Argon2 lanes (p) | E2: p=1 as the floor. R7: p=4 with the `parallel` feature. | GPU attack cost scales with m·t, not p. p=4 lets a multi-core phone finish about 2× faster, and that time can go into a higher t. Decide from phone measurements. | E2.1, E2.2 |
| X28 | "Up to date" and the market claim | R6 refuted "up to date" and confirmed the market gap; its verifier reversed both to partial. | "Up to date" is unsafe *wording*, but visible sync state is a valid goal. The market gap is real on Android and blocked for iPhone invitees. | E14 |
| X29 | Recently deleted | R6: add a shared "Recently deleted". R6's verifier: DESIGN already has a 30-day trash. E1: purging from that trash doesn't work as written. | Make the existing trash visible to all members, and purge through document epochs. | E1.3 |
| X30 | Windows Firewall | DESIGN: a prompt appears. R1: it may. R6: random-port UDP raised no prompt in Delta Chat. | Unknown. Add an installer or MSIX rule, and don't pre-announce a prompt. | E3.8 |

---

## 5. Experiments to run next

These build on what has already been run (E1–E8, A1–A8). Safety rules from STATE.md apply: anything adversarial or high-volume runs only against **local** instances, and public probes stay polite and need approval where marked.

**Priority:** P0 = decides a format or data-model rule, so run it before engine code freezes formats. P1 = before the phase that uses it. P2 = before a later phase or a product decision. P3 = only when that feature is picked up.

| ID | Experiment | Answers | Source experiments | Priority | Where |
|---|---|---|---|---|---|
| **E1: Loro history and snapshots** | | | | | |
| E1.1 | Compaction policies in the network simulator (3–6 devices, random offline periods, Nostr slot model): at the local head, at the causally stable version, stable minus a margin, and document epochs. Count rejected, stranded and aborting cases. Reproduce #1040 and the #1083 ordering | R3-2, NR-04, NR-05, X11 | R3 E2 | P0 | Local |
| E1.2 | A stale device that missed one rotation: fresh-document bootstrap plus three-way rebase vs a naive re-apply. Count last-writer-wins flips | R3-2, NR-06, X6 | R3 E3 | P0 | Local |
| E1.3 | Tombstone purge per model (tree, movable list, map of maps, mergeable children): shallow snapshot at the head vs document epoch | X5, X29, NR-12 | E1 §2, R3 M1 | P1 | Local |
| E1.4 | Golden corpus and cross-version CI (1.0.9, 1.10.8, 1.13.9, 1.16.2), with unknown containers simulated by a build without `counter`. Verify the "old clients send updates only" rule | R3-1, R3-3, NR-08, X13 | R3 E1 + verifier | P0 | Local |
| E1.5 | Pending out-of-order changes survive a restart when raw blobs are persisted (a test of the fix) | NR-07 | R3 E11 | P1 | Local |
| E1.6 | Checkpoints over one year: StateOnly exported before trimming vs app-level delta-compressed JSON | X7 | R3 E7 | P2 | Local |
| E1.7 | Two devices move the same task to different lists concurrently while one edits it. Compare one document per group with one per list, including load time and memory at 20k tasks | NR-10 (data-model decision) | R3 E12 | P0 | Local |
| E1.8 | Soft delete and TreeID identity; a subtask added to a trashed parent survives | NR-11 | R3 E10 | P2 | Local |
| **E2: Argon2 timing and GPU rate** | | | | | |
| E2.1 | A real low-end ARM phone (Cortex-A53/A55): m ∈ {19, 46, 64, 128} MiB × t ∈ {1–3} × p ∈ {1, 4} with the `parallel` feature. Wall time, peak memory, thermals; compare with libsodium | R7-1, X27 | R7 E2, E2 open item | P1 | Physical device |
| E2.2 | hashcat on the local RTX 3080: p=4 vs p=1 at the chosen parameters | X27 | R7 | P2 | Local |
| E2.3 | Multi-target test: cost per guess doesn't depend on the number of invites, and a date-salted guess doesn't carry over to another day | NR-29 | R7 A1 | P2 | Local |
| **E3: iroh local endpoints and size** | | | | | |
| E3.1 | `presets::Minimal` with a local iroh-relay and a local iroh-dns-server: a configurable relay list, relay-to-direct path events, recovery after a relay restart | R1-7, NR-37 | R1 E3a | P1 | Local |
| E3.2 | Our own address lookup fed by encrypted per-device records (via Nostr and the group document) with no n0 DNS. Capture exactly what gets published under the default preset vs relay-only vs custom settings, including the mDNS advertisement | R1-6, NR-38, X15 | R1 E-privacy, R7 iroh audit | P1 | Local |
| E3.3 | NAT matrix (patchbay or network namespaces): easy/hard in every combination, carrier NAT, firewalled IPv6. Plus local direct-vs-relayed statistics in the prototype | R1-4, X14 | R1 E3b | P1 | Local |
| E3.4 | Headless Android emulator: JNI context init with no public-DNS traffic, Private DNS, Wi-Fi/cellular switches with and without `network_change()`, mDNS with and without a MulticastLock, the Android 16 RESTRICT_LOCAL_NETWORK opt-in, and EPERM handling in a target-37 build | R1-9, R4-6, NR-39, NR-63 | R1 E3c, R4 E8 | P1 | Emulator |
| E3.5 | Battery and cold start: an idle bound endpoint vs bind, sync, close every 15 min | NR-40 | R1 E3d | P2 | Physical device |
| E3.6 | A relay hosted by a member: first whether it is reachable at all (CGNAT), then an allowlist of group endpoint IDs plus a custom TLS root | R1-7 | R1 E3e | P2 | Local |
| E3.7 | A 24 h soak test with connection churn (memory, file descriptors) | NR-42 | R1 E3f | P2 | Local |
| E3.8 | Clean Windows 11 VM: firewall behavior with a random vs a stable UDP port, with an MSI rule, and in full-trust MSIX | R6-18, NR-43, X30 | R1 E-windows, R6 E-UX9, R8 E-7 | P2 | Local VM |
| E3.9 | Image transfer: a custom protocol vs iroh-blobs 0.103, with a network switch mid-transfer | R1-10 | R1 E-blobs | P3 | Local |
| E3.10 | Gzipped WASM size of the transport subset plus Loro, for the web guest client | R1-12, NR-76 | R1 E-wasm | P2 | Local |
| E3.11 | pkarr/BEP44 on a local Mainline testnet: per-device records vs a group-shared key (show the overwrite race), republish cost, lookup latency | R5-1, X15 | R1 E-group-discovery, R5 X4 | P3 | Local |
| **E4: Nostr, local relays and a polite public probe** | | | | | |
| E4.1 | Local strfry with the default config plus damus's noteguard config: replay a realistic day of writes under the proposed write budget. Test ties, fast-clock lockout, NIP-40 purge, NIP-09 a-tag deletion, and shadowReject returning OK without storing | R2-1–R2-4, NR-44, NR-46, NR-48, X3 | R2 E4a | P0 | Local |
| E4.2 | The same on local nostr-rs-relay and khatru, including the silent "OK true" drops | R2-2, NR-46 | R2 E4b | P1 | Local |
| E4.3 | CGNAT simulation: K clients behind one IP, and the ban threshold for each write budget. Retry storm after a 30-min outage, with and without jitter | NR-44 | R2 E4c, R5 X5 | P1 | Local |
| E4.4 | Chunk and generation protocol with E1's snapshot sizes: random chunk rejection must never produce a torn read | R2-3, NR-47 | R2 E4d | P0 | Local |
| E4.5 | Per-device keys vs a group-derived per-epoch key, including a relay with owner-only AUTH reads, slot vandalism and pinning with a future timestamp | X11, NR-49 | R2 E4e, R7 | P1 | Local |
| E4.6 | Relay-list aging: installs that are never updated for 1–5 years, with a shipped list only vs a signed manifest plus gossip | NR-51 | R5 X6 | P2 | Local (sim) |
| E4.7 | **Polite public probe (needs approval):** the chosen dedicated kind, read back from a second connection with a different key, a check after expiry, and `created_at` skew probes at +300 s and +1000 s | R2-6, X1 | R2 probe | P1 | Public, a handful of events per relay |
| E4.8 | Linkability demonstration on local strfry: one client in 5 groups (IP, connection, timing, REQ filters, multi-pubkey AUTH) | R2-8, R7-7 | R7 A7 | P2 | Local |
| E4.9 | NIP-13 PoW timing on a low-end phone at 1 KB and 48 KB | NR-55 | R2 E5 | P3 | Physical device |
| E4.10 | Local Blossom server: octet-stream uploads, 415 responses, idle expiry | R2-7 | R2 E6b | P3 | Local |
| E4.11 | Clock-skew survey: log NTP offsets on test phones for a week | NR-48 | R2 | P3 | Devices |
| **E5: Helper network simulation** | | | | | |
| E5.1 | Participation of 0.01/0.1/1/10% with diurnal time-zone profiles. Availability while the group's own devices are offline, using retention rather than reachability. Tier-1 members and Nostr modeled end to end. A uniformly random share of each set. E1's sizes. **Gates whether §7a gets built** | R5-2, R5-3, NR-74, X16, X17 | R5 X1, E5 follow-ups | P2 | Local (sim) |
| E5.2 | Cost of grinding IDs into the bootstrap set: plain hash vs keyed HMAC, with and without IP-prefix binding | NR-70 | R5 X2, R7 | P3 | Local |
| E5.3 | Sybil metadata aggregation over 90 days, with and without share caps, unlinkable slot IDs and ASN diversity | NR-69 | R5 X3 | P3 | Local (sim) |
| E5.4 | Collusion vs signed version heads; amplification through forged helper sets; capacity caps for young groups | R7-6, NR-72 | R7, A3 | P3 | Local (sim) |
| E5.5 | A headless member node as the mailbox for two phones that are never online together, vs Nostr only | R5-8 (supports promoting the headless build) | R5 X7 | P1 | Local + emulators |
| **E6: Android release size** | | | | | |
| E6.1 | The whole APK per ABI (Compose, UniFFI, engine, mDNS crate), `panic=unwind` vs `abort`, cargo bloat, a CI size budget, 16 KB alignment checked with llvm-readelf and `zipalign -P 16`, and a run on the 16 KB emulator image | R1-11, X4, NR-65 | R1 E6, R3 E5, R8 E-6 | P1 | Local |
| **E7: Invite codes** | | | | | |
| E7.1 | Word-list decision script: cleaned 6×EFF large (without hyphenated words, homophones or edit-distance-1 pairs) vs 8×EFF short 2.0 vs 7×BIP39, each with and without a checksum word. Report bits, typo neighbors, prefix uniqueness and length | R6-5, NR-34, X26 | R6 E-UX2, E7 | P0 | Local |
| E7.2 | Parser and entry tests: any separator or case, autocorrect, prefix autocomplete, no suggestions or personalized learning, hintLocales | R6-5, R7-1 | R7 E7 | P1 | Emulator |
| E7.3 | Human study with 20+ people (at least a third non-native English speakers, including right-to-left keyboard users): time to enter a code, first-try success, and mishearing over a phone call, across the candidate lists | R6-5, X26 | R6 E-UX1 | P1 | Human study |
| **E8: Key epochs** | | | | | |
| E8.1 | Extend the simulator: validation by each receiver, a malicious sender, data written during a fork, the backdating rule (needs E16), long partitions, a device lost mid-rotation, rotation size for 300 devices | R7-2, NR-23, NR-24, NR-36 | R7 E8, E8 limits | P0 | Local |
| E8.2 | Turn invariants I1–I3 into property tests in the engine's CI | R7-2 | E8 | P1 | Local |
| **New experiments** | | | | | |
| E9 | **Containing untrusted imports.** Coverage-guided cargo-fuzz of `import`, `import_batch` and `decode_import_blob_meta` with a mutator that fixes checksums. A prototype of process-isolated import on Android (`:import` process), including inside a WorkManager job: memory and latency. Quarantine journal across restarts. Caps (update size, decompressed size, depth, pending buffer). An lz4 bomb. `panic=unwind` + `catch_unwind` compared with isolation | R7-4, NR-01, NR-13, NR-15, X8, X9 | A6 (not yet done), R7 A5, R3 E8 | P0 | Local + emulator |
| E10 | **Restore and cloning.** Auto Backup (`bmgr`), D2D between two live phones, Block Store (cloud restore, D2D, skipped restore, reinstall with Backup on and off). Check that identity is excluded, the Keystore marker detects the restore, and a new identity and PeerID are minted. PeerID divergence with a pinned vs rotating ID. clientSideEncryption without a screen lock. The exact-alarm permission after a restore | NR-02, NR-64, R4-8, R4-9, R7-5, X10, X21 | R4 E9/E10, R3 E4, R7 N3, R8 E-10 | P0 | Emulators + devices |
| E11 | **Android background behavior.** Histogram of Doze gaps on Android 14/15/16/17 (unplugged, and with forced idle). Expedited job enqueued at onPause vs ON_STOP. shortService grace period and notification visibility. Timing of the background network firewall. Freezer socket termination and reconnect cost. Chained jobs overnight with and without a charge limiter. Standby bucket with a widget on Pixel, Samsung, Xiaomi and OnePlus after 3, 8 and 16 days. Job cost of Glance. Force-stop | R4-1–R4-4, NR-57–NR-61, X18–X20, X22 | R4 E1–E7, E12 | P1 | Emulators + devices |
| E12 | **Low-end device performance.** Cortex-A53/A55 at 1k, 5k and 20k tasks: cold open, full read, 400 concurrent imports, document-epoch rebuild, memory. Bytes per op with persistent vs per-session PeerIDs | R3-5, NR-20 | R3 E6 | P1 | Physical device |
| E13 | **Invite exposure and handoff paths.** Clipboard matrix on API 29–36 (focus, toast, checking the description first, the 1 h clear, installing after 65 min). App Links opened from WhatsApp, Telegram, Signal, SMS, Gmail, Instagram and Messenger. Fragment exposure through Safe Links, Slack and in-app browsers (a throwaway page that logs `location.hash` locally). Clipboard retention in keyboards and cloud clipboards. FLAG_SECURE. Windows clipboard exclusion formats | R4-10, R6-6, R6-16, R7-8, R8-14, NR-25–NR-28 | R6 E-UX3/4, R8 E-5, R4 E11, R7 A8 | P1 | Devices; a throwaway domain |
| E14 | **UX field studies (need a prototype).** A 2-week diary study with couples (latency with the app open and closed; A/B of the "Tell Alex" nudge and the freshness display). Widget label staleness. How people add a second device. Resistance to link phishing. Telling members apart by name | R6-3, R6-8, R6-9, R6-10, R6-13, NR-33, NR-79 | R6 E-UX5–8, E-UX10 | P2 | Human studies |
| E15 | **Policy and distribution dry runs.** Play Console: package registration, whether shortService needs a declaration, Data safety, pre-launch report (**the registration part is urgent**). Google OAuth `drive.file` published In production. Microsoft Graph AppFolder 403. MSIX full trust (NIO, UDP, AppData on uninstall, StartupTask). Offline flatpak-builder plus lint. A signed latest.json vs the GitHub API. In-App Updates through internal app sharing | R8-1, R8-10–R8-13, R8-16, R8-18, R8-21, R4-13, NR-85–NR-93 | R8 E-1–E-9, R4 E13 | P0 (registration), else P2 | Accounts + local |
| E16 | **Authenticated authorship.** A per-install PeerID bound to the device key. Signed update blobs. The own-ops-only check on the isolated copy. Reproduce forgery and squatting, then rotate on detection. Detect self-equivocation through signed per-range hashes (is `export_json_in_id_span` deterministic enough?). Signature overhead on a phone | R3-6, R7-3, NR-03, X10 | R3 E9, R7, A5 | P0 | Local |

**Suggested order:** first E15 (registration only), E9, E16, E1.1, E1.2, E1.4, E1.7, E8.1, E10, E4.1, E4.4 and E7.1. Together these fix the format-v1 rules. Then the P1 set, phase by phase. E5.1 decides whether §7a is ever built.

---

## 6. Proposed changes to DESIGN.md

These are proposals only. The user decides what changes (STATE.md rule), and DESIGN.md has not been edited. They are listed section by section, and the tags point back to this document.

### Header
- Confirm who owns `kleiders.com`. If the user doesn't own it, choose an owned domain and rename the package before the first upload, since `com.kleiders.tasks` is permanent (NR-86).
- Register the package and sideload key in Android developer verification now (NR-85).

### §1 Mission and constraints
- **C2:** add "Known unavoidable costs". The main one is an owned domain (~$10–15/year), which Google's OAuth policy requires for the homepage and which verified App Links, Flathub and a join-link origin the project controls all need (R8-10, NR-26). Note that Windows signing is avoided by distributing through the Store (R8-18/19). Also add that free third-party capacity comes with terms: written permission from n0, and staying within relay operators' rate limits (R1-7, R2-1).
- **C5:** say how iPhone invitees are served. The web guest client (view and check off) is part of the sharing promise, and the landing page detects iOS (R6-1).

### §2 Core principles
- **P2:** remove "STUN servers" (R1-4). Replace the provider list with a per-role table (live transport, public-address discovery, peer discovery, mailbox, blobs, relay-list distribution) that names at least two providers per role. Flag where only one exists today: iroh relays and address discovery come from n0 alone (R1-7, NR-37).
- **P5:** state what "never lossy" depends on: compaction only at causally stable versions, and raw blobs kept until their dependencies arrive (R3-2, NR-07).
- **P6:** add "no foreground-service fallbacks for background work". The only FGS is shortService, and it becomes optional (R8-2, R4-3).
- **P8:** add:
  - A **group feature floor**. Loro-level features (new container types, new value kinds, major Loro upgrades) are used in a group only once every member device advertises support. App-level fields can still be added freely (R3-1, R3-3).
  - **Old clients never publish snapshots of documents with container types they don't know**; they send updates only (X13).
  - Every validation limit ships in format v1, and later tightening uses a per-group rules version raised only when all members support it (NR-16).
  - Concrete mechanisms for "all incoming data is untrusted": import first in a disposable process, a quarantine journal, caps on size, depth and pending changes, and CI fuzzing with a mutator that fixes checksums (NR-01).
  - Desktop update checks read a signed `latest.json` from our own domain instead of the GitHub API (R8-21). The key for the required-update manifest is escrowed (NR-83).
- **P9:** acknowledge that Loro depends on MPL-2.0 crates (im, sized-chunks, bitmaps). Either accept file-level MPL-2.0 explicitly, with cargo-deny exceptions, or push upstream to replace them (NR-18).
- **Settings table:**
  - "Notify me when others change a list": on, scoped by default to recently used lists (R5-16).
  - "Help others sync": on for desktops by default, and not offered on Android phones in v1 (R4-4, R8-2, R8-9).
  - Backup row: foss has no Google Drive (R8-12).

### §3 Architecture
- Add an **import sandbox** between transports and documents: `android:process=":import"` on Android, a child process on desktop (NR-01).
- Each LoroDoc lives on one engine actor thread, and no synchronous Loro calls come from callbacks (NR-19).
- State that the engine exposes a domain-level UniFFI API and depends on neither iroh-ffi nor loro-ffi (R1-1, R3-7).
- The transport manager owns a custom iroh address lookup (X15).

### §4 Data model
- "Loro (1.x, stable format)" → "Loro pinned exactly (=1.16.2 today). Format stability is verified by our own golden corpus in CI, not assumed" (R3-1).
- Decide between **one document per group** (lists as subtrees, atomic moves) and one per list with an explicit move protocol (tombstone, forwarding pointer, deterministic winner). E1.7 settles it (NR-10).
- Tasks:
  - App-level UUIDs are the only external key.
  - Soft delete (a deletedAt field or a trash node); the real `tree.delete` happens only at purge.
  - `ensureMergeable*` for lazily created children.
  - Purge mergeable children before deleting their owner node (NR-11, NR-12).
- New **Authorship** subsection:
  - Each install gets a PeerID bound to the device key by a signed registration in the group document. It lives in no-backup storage with a lease marker.
  - Rotate it on restore, crash recovery, a failed single-instance lock, or an op arriving under our own PeerID that we didn't write.
  - Sign every update blob. Receivers enforce own-ops-only on the isolated copy.
  - Signed per-range hashes make equivocation provable.
  - "Who changed what" is labeled as claimed wherever it isn't verified (NR-02, NR-03).
- Blobs:
  - iroh-blobs → a small custom protocol (request by BLAKE3, verify, retry) until iroh-blobs reaches 1.0 (R1-10).
  - Blossom is best-effort, uploading octet-stream to servers verified to accept ciphertext.
  - Store the ciphertext's SHA-256 next to the BLAKE3 hash (R2-7).
- "Compaction (shallow snapshots) is available" → point to §10's document epochs.

### §5 Identity, groups and sharing
- **Device identity:** split the keys into:
  - an identity signing key;
  - a rotating transport key for the iroh endpoint, certified by the identity key;
  - an HPKE/X25519 wrapping key.

  None of them is backed up or transferred device to device. All are regenerated after a restore, which a Keystore-bound install marker detects (NR-02, NR-32, NR-38).
- Editorial: "Recovery simplifies: Block Store only needs the personal group secret…" is glued onto the display-names bullet. Split it out.
- **Display names:** add local nicknames (stored in the personal group), same-name detection ("Is this you on another device? Link instead"), and renames shown in history. Show the tag only in member details (R6-8, NR-33).
- **Linking:** replace "approval on an existing device" with:
  - Linking completes over a direct local path (LAN or Nearby) by default. Internet linking sits behind an extra warning.
  - The *scanning* device shows a full-screen warning naming the account it will join and what that account will see.
  - Link payloads opened from invite links, chats or the join scanner are rejected.
  - Every device gets a "New device linked" alert with Remove.
  - Merging a non-empty device first shows what will be merged (R6-9).
- **Key epochs:** specify:
  - E8's rules 1–4, validated by every receiver.
  - A member addition is valid only under the current head epoch and only when signed by a current member device.
  - A removal carries the remover's version vector, and ops by the removed author outside it are rejected.
  - Mutual removal removes both people, shown clearly with Undo, and removals are rate-limited.
  - All members are notified of every join and new device.
  - Periodic rotation for post-compromise security.
  - All network identifiers are derived from the epoch key.
  - Block Store is updated right after each rotation.
  - A hybrid X25519+ML-KEM suite ID is reserved.
  - Version, group, epoch, slot and sender are bound as AEAD associated data.
  - Prefer adopting p2panda-encryption or Keyhive rules over inventing new ones, noting p2panda is unaudited (R7-2, NR-23, NR-24, NR-30, NR-31).
- **Derived values:** the Nostr topic tag and discovery keys rotate per epoch and per day, and no plaintext group-ID hash goes on the wire (R7-7, A7).
- **Membership:** "knowing the secret makes you a full author" → "a device becomes a member when a current member device signs its addition under the current epoch; the secret alone isn't enough" (NR-23).
- **Link and QR:**
  - The payload is an invite ID plus a 256-bit capability. It decrypts a short-lived, revocable invite record in the mailbox that holds only the current epoch key.
  - The lifetime is a product call (A8 suggests 7 days for links and 1 day for codes).
  - Joining needs a tap.
  - Serve verified App Links on the owned domain (R7-8, NR-25–NR-28).
- **"Get the app":**
  - The page copies the capability link, not the secret.
  - A *secret-free* Install Referrer flag (`from_invite=1`) makes the app show "Tap your invite link again / Paste". The reason for rejecting Install Referrer no longer applies, because no secret passes through it.
  - The app reads the clipboard only after window focus, checks the clip's description first, and clears the clipboard after use.
  - Note the clipboard's 1-hour auto-clear (R4-10, R6-6).
- **Invite code:**
  - Choose the list with E7.1 and E7.3. E7 recommends 8 × EFF short 2.0 (82.7 bits, 3-letter autocomplete, no typo neighbors). The alternatives are a cleaned 6 × EFF large or 7 × BIP39 with chip entry, each with an optional checksum word.
  - Delete "so the large list is required" (7 BIP39 words give 77 bits).
  - Correct the brute-force figures. At 1M invites and 100k guesses/s: 4 words ≈ 10 h, 5 words ≈ 9 years, 6 words ≈ 70,000 years. With 10k GPUs, 6 words ≈ 400 years.
  - Add a date-bucketed salt. Derive the tag and key with HKDF from the Argon2 output, with a tag of at least 128 bits.
  - Argon2id floor: m = 64 MiB, t = 3, p = 1 (p = 4 with the `parallel` feature if phones allow).
  - A fresh throwaway Nostr key per invite event. Publish only to NIP-40 relays.
  - Code fields disable suggestions and personalized learning (R7-1, R6-5, E7, NR-29, NR-34).
- "Relays drop expired invites" → "Most relays drop expired invites; the app refuses them regardless" (R2-4).

### §6 Transport stack
- **Intro:** "about 90% direct-connection success" → "direct-connection rate to be measured (vendor figure ~90%; an independent desktop study found ~70%)" (X14).
- **Layer table:**
  - L1 → "iroh-mdns-address-lookup (separate pre-1.0 crate, off by default)": MulticastLock only in the foreground, the Android 17 permission, passive or private-address-only on untrusted networks (R1-2).
  - L2 → fold into L3 as a path iroh picks. Note that on-link IPv6 needs the Android 17 permission (R1-3, R4-6).
  - L3 → "iroh NAT traversal. Public-address discovery (QAD) and coordination go through an iroh relay." Drop STUN (R1-4).
  - L4 → desktop only, disabled on Android (R1-5).
  - L5 → "hard-to-hard NAT isn't supported by iroh; it would need patching; later" (R1-4).
  - L6 → promote to the primary live fallback when iroh relays are unavailable, preferring desktop members (R5-12).
  - L7 → "iroh relays: n0's public relays (best-effort, pending written permission), a configurable health-tracked list, member-hosted relays if E3.6 works". Remove libp2p circuit relays (R1-7, R5-13).
- **Discovery paragraph:** rewrite.
  - Primary: E2E-encrypted per-device address records per group (home relay URL plus optional IPs), published in the Nostr mailbox and the group document, keyed per epoch and rotated daily, and fed into a custom address lookup. Endpoint IDs travel in invites.
  - Optional: per-device pkarr records on the Mainline DHT, on desktops only; phones only read (BEP43).
  - n0's DNS/pkarr is off by default, or used only with rotating transport keys (R1-6, R5-1, X15).
- **Add an Android paragraph:**
  - Bind the endpoint only in the foreground and inside sync jobs.
  - Call `network_change()` from a NetworkCallback.
  - Use explicit DoT/DoH nameservers with the fallback disabled.
  - Keep `panic=unwind`.
  - Background jobs dial only peers that are always on (R1-9, NR-39, NR-40).
- **Doorbell:** "Deliberately not built" → "Designed in phase 5: a content-free UnifiedPush/FCM ping sent by any online member device (evaluate existing Nostr push services). Built when E14 shows closed-app latency hurts" (R5-16, R6-3).

### §7 Nostr mailbox
- **Slot model:**
  - Per-device slots only, each holding that device's own changes and signed with its device key.
  - Drop the single shared "snapshot slot": it is impossible with per-device keys and can't be verified (R2 risk, A5).
  - Snapshots become **document-epoch snapshots** with a causally stable root. One deterministic member publishes each, and the others verify it (X11, NR-04).
- **Kind:** one dedicated, publicly documented addressable kind, never 30078 or 78. Register it in the NIP kind list and publish an operator info page with expected volume and a contact (R2-6, X1).
- **Write rate:** replace the bullets with:
  - An engine-wide per-relay budget (about 6 events/min, burst 5, shared across groups).
  - In the foreground, rewrite a slot at most every 30–60 s.
  - Keep flush-on-leave, with 0–30 s of jitter per group.
  - Stop at the first `rate-limited:`. Treat `banned:`/`blocked:` as the relay being down for at least 1 h.
  - Full-jitter exponential backoff. Republishing staggered by a hash of the device key (R2-1, NR-44).
- **Timestamps:** `created_at` strictly increases per slot and is persisted. Readers ignore events dated further in the future than a skew window (R2-2, NR-48, NR-49).
- **Chunks:**
  - At most 40 KiB of binary per chunk, carrying generation, index, count and hash.
  - A/B d-tags, with the pointer flipped only after read-back.
  - Rewrite only changed chunks. Halve the size on "too large" (R2-3, NR-47).
- **Expiry:** add a NIP-40 expiration of +60–90 days to slots, refreshed with each rewrite (R2-4).
- **Relay list:** "shipped with the app and updated with app releases" →
  - Defaults chosen for diversity of operator *and* software (strfry, khatru, nostr-rs-relay, Ditto).
  - Refreshed at runtime from a signed manifest (own domain plus a Nostr event, signed with the developer key) and gossip in group documents.
  - Editable in diagnostics, for bring-your-own or paid relays.
  - Note that damus is at risk (NR-51).
- **Health tracking:**
  - Based on reading events back over a separate connection, not on OK replies. Subscription liveness checks.
  - At most 15 subscriptions per relay. At most one group's key authenticated per connection.
  - PoW only for small events (NR-46, NR-52, NR-55).
- **Unlinkability sentence:** → "Per-group keys stop other readers from linking a device's groups. Relay operators can still link them by IP, connection and timing (§9)" (R2-8).
- **Reliability paragraph:** correct the citation: Wei & Tyson, CoNEXT'25, data from Oct–Dec 2023. 18.5% of relays were dead within 3 months, "90% up >80%" excludes them, and the paper has no retention data (R2-5).
- **Acceptance paragraph:** acceptance risk exists *now* (per-IP limits, the trend toward AUTH). The answers are frugal writes, a documented kind, diverse operators, a runtime relay list and member devices, including headless member nodes. Helpers are supplementary (R5-10, X3).
- **Add:** phone-only groups depend on Nostr, so budget relay redundancy for them (NR-78).

### §7a Helper network
- **Status line:** "Experimental. There's no precedent; Keet, Skype and others moved to operator-run nodes. Build it only if E5.1 shows it is viable at 0.1–1% participation" (R5-2, R5-4).
- **Phones:**
  - Remove "Charging on Wi-Fi: heavy help… chain of back-to-back WorkManager jobs… foreground service only as fallback".
  - Phones help only their own groups, during their own job windows or while the app is open.
  - Helping strangers is desktop-only, and stays out of the play flavor until the policy question is settled (R4-4, R8-2, R8-9).
- **Discovery:** "helpers announce themselves on the DHT" → pick a real mechanism: signed helper ads on Nostr, a desktop-only overlay with IP-bound IDs, or gossip within groups (R5-1).
- **Bootstrap:**
  - Derive the position from HMAC(epoch key, date), not from a hash of the group ID.
  - Require IP-prefix and ASN diversity in each set.
  - Choose part of each set uniformly at random (R5-14, R7-6, NR-70, A4).
- **Adaptive replication:**
  - Replace the formula with measured retention while the group's own devices are offline. Start at r = 20 (E5).
  - Redo the size estimate with E1's sizes: 70–353 KB per list, not 50 KB per group (R5-3, NR-74).
- **Admission and integrity:**
  - A slot is admitted only with an epoch-signed membership proof. Per-slot write limits.
  - Replicate only to epoch-signed helper sets.
  - Signed store receipts and signed version heads.
  - Cap how much capacity new groups get, and cap each member's influence on reliability records (R7-6, NR-71, NR-72).
- **RAM-only:** keep it for default helpers. Add an opt-in "dedicated helper" mode with encrypted disk storage and operator-set caps (NR-75).
- **Trust tiers:**
  - Promote the headless member build to a planned phase that comes *before* helpers.
  - Desktop wording: "Keep this PC as your lists' mailbox".
  - An optional "old phone on charger" mode joins as a member, not a helper (R5-8).
- **Setting default:** on for desktops, off for phones.
- Editorial: the stray row "| Slow | Ranking only |" after the "Why old versions matter" paragraph belongs in the severity table above it.

### §8 Android background behavior and lifecycle
- **"Just left the app":**
  - An expedited OneTimeWorkRequest (CONNECTED), enqueued while the app is still visible (Activity.onPause and after each local commit), is the primary flush.
  - shortService becomes optional, with no notification actions.
  - "usually invisible" → "often invisible" (R4-3, X20).
- **Add:** Android 15+ cuts background network within seconds, and the freezer kills sockets. All background networking runs inside jobs, and sockets close within ~3 s (NR-57).
- **Background row:** add Doze timing (within minutes on Android 15+; maintenance windows from 1 h up to 6 h) and Android 16's ACTIVE-bucket quota (R4-1).
- **Widgets:**
  - Checkboxes on all supported versions (Glance backport).
  - An absolute time ("Updated 14:05"), or show it only when stale.
  - One `updateAll` per sync run.
  - "keeps the app in the active standby bucket" → "keeps the app out of low standby buckets (not out of Doze)".
  - Offer "Add to home screen" after joining (R4-2, R4-11, R6-13).
- **Honesty copy:** promise no number. Show coarse per-member freshness ("today", "this week"), and add a "Why isn't this updating?" screen (R6-4, NR-79).
- **Blocked-sync detection:** use `getAppStandbyBucket`, `isBackgroundRestricted`, ApplicationExitInfo, WorkInfo stop reasons and `getPendingJobReasonsHistory` (Android 16+) (R4-1).
- **Splash:** never move a row under an active touch; reorders wait 500 ms after the last input; mark "new from Alex" (R6-17).
- **Notifications:** generate them only from merged state; batch them, show their age, and drop stale ones; scope the default (NR-77).

### §9 Security and privacy model
- **Table:**
  - Nostr relays / helpers: add "can link a device's groups (IP, connection, timing, subscriptions)".
  - Replace the "iroh relays / STUN / DHT" row with "iroh relays (n0), n0 DNS/pkarr, DHT, DNS resolvers, same-LAN observers (mDNS)": transport endpoint IDs, IPs, who connects with whom. Drop STUN.
  - Google (Auto Backup): "Nothing readable **if a screen lock is set** (Android 9+). Without one, Google can read the backup, so the secrets file is excluded (`disableIfNoEncryptionCapabilities` / `requireFlags`)".
  - Own cloud: "sealed = obfuscation: unreadable to automated scanning, readable by anyone who studies the format".
  - Add a row for the join-page host: visitor IPs (R7-5, R7-7, R8-20).
- **Add:**
  - Epoch keys wrapped with X25519 on public relays have no forward secrecy against archives beyond rotation, and a post-quantum suite is reserved (NR-30).
  - Desktop OS keystores can be read by malware running as the same user (NR-94).
  - The accepted metadata risk explicitly includes linking a device's groups.
- **Play compliance:**
  - The Data safety form declares "Device or other IDs", or the doc records why not. The privacy policy is linked inside the app (R8-4).
  - Export: no Play declaration exists. Under the EAR the app is mass-market 5D992.c with no reporting. Check the developer's own country's rules (R8-7).

### §10 Storage
- **Add engine limits** (placeholders to tune): update ≤ 1 MB, document ≤ 20 MB, text field ≤ 100k characters, plus limits on nesting depth, the pending buffer and per-author write rate (NR-13, NR-15).
- **Compaction:** replace with **document epochs**.
  - Compact only at a version every member device has acknowledged, minus a margin, or re-create the document from live state, which each member verifies before switching.
  - Stale devices bootstrap into a fresh document and rebase their unsynced edits with a three-way merge.
  - "Re-applies its own unsynced changes as new edits" becomes a three-way app-level rebase (R3-2, NR-04–NR-06).
- **Trash purge:** "purged at compaction" → "purged by document epochs". Shallow-at-head purges only tree and list tombstones (X5).
- **Checkpoints:** stored outside the live document, as app-level delta JSON or StateOnly blobs exported before trimming. `fork_at` doesn't work on shallow documents (X7).
- **History budget:** use a high-water mark and a minimum 30-day window regardless of size, and add "Undo changes by [person] since [time]" (NR-14).
- **Keep** raw received blobs until nothing pending references them (NR-07).
- **Size:**
  - Update the estimate with E6's numbers: Rust core ≈ 7 MB installed, 4.3 MB download on arm64.
  - Release profile `panic = "unwind"` (measure the cost). Keep the size check and add a 16 KB alignment check (X4, NR-65).

### §11 Backup and recovery
- **Data-loss sentence:**
  - Add the skip-restore case.
  - When nothing was restored, first launch offers "Restore from your other device or a backup file".
  - Device linking is the default recovery (NR-64).
- **Block Store:**
  - Entries ≤ 1 KB (per the API reference). E2E only with a screen lock.
  - Cloud restore works on Pixel from Android 9 and on other phones from Android 12.
  - Never upload the secret without E2E. Update it after each rotation (R4-8, X21).
- **Auto Backup:**
  - Exclude all identity material.
  - `requireFlags` / `disableIfNoEncryptionCapabilities` for the secrets file.
  - Keep the backup set under ~20 MB and log quota overruns.
  - Detect a restore and mint a new identity (R4-9, NR-02, NR-17).
- **Google Drive row:** `drive.file` is non-sensitive: no verification, no warning screen, no 100-user cap. Publish the OAuth app In production. Brand verification is optional. OAuth policy requires a homepage on a verified domain we own (R8-10).
- **OneDrive row:** needs an Azure free account (credit-card identity check) and should be limited to personal accounts. Publisher verification isn't available to individuals. Re-test the AppFolder 403 regression (R8-13).
- **Sealed backups:** include only the personal-group secret, since everything else follows from it. Wrap the seal key with Block Store or the OS keychain where available (R7-5).
- **foss:** no Google Drive. Use SAF folder backup and OneDrive via MSAL (R8-12).

### §12 Platforms and distribution
- **Android row:**
  - Add developer-verification registration.
  - Add the 12-tester, 14-day closed test if the developer account is personal and new (NR-85, NR-95).
- **Older-versions table:**
  - Remove the checkbox-widget row.
  - The expedited job is the primary flush on every version, not just a fallback (R4-11, R4-3).
- **Permissions table:**
  - Local network row → "`ACCESS_LOCAL_NETWORK` only when targeting 37; nothing needed on 36" (R4-6).
  - Exact alarms: add re-prompting after a restore, and prompting through a notification when a reminder arrives in the background (R4-7, R8-5).
- **Desktop firewall sentence:** → an installer or MSIX firewall rule, or a stable UDP port; don't pre-announce a prompt (R6-18).
- **Windows row:** Microsoft Store MSIX (full trust) is the primary channel, winget via msstore second, and a GitHub MSI only as a fallback (signed through SignPath if open-sourced). Plan how MSIX handles AppData (R8-18, R8-19, NR-90).
- **Linux row:** AppImage and .deb first. Flathub later, with a human-written manifest, AI disclosure and months of public history (R8-16).
- **Flavors:** foss drops Google Drive and makes no automatic GitHub requests (R8-12, R8-15).
- **Join page:** a separate public repo on the owned domain, verified in GitHub, with a minimal page, a CSP meta tag, no third-party scripts and `assetlinks.json` at the root (R8-20, NR-26).

### §12a Engineering practices
- **Repositories:** a private engine/app repo, plus public repos for the site, join page, policies and release artifacts (NR-93).
- **Simulated-network tests:** add Loro's fuzz actions, compaction with offline devices, epoch property tests (E8's invariants), poison blobs, and restore-clone scenarios.
- **CI:**
  - cargo-fuzz of import with a mutator that fixes checksums.
  - Golden-corpus cross-version tests.
  - cargo-audit and cargo-deny (with the MPL exceptions).
  - Size budget and 16 KB alignment checks.
- **Upgrades:** exact pins, and a quarterly iroh/Loro upgrade task (NR-41).
- **Libraries:** `argon2` with the `parallel` feature. No iroh-ffi or loro-ffi. iroh-blobs only once it reaches 1.0. Add iroh-mdns-address-lookup.
- **Diagnostics:** local direct-vs-relayed statistics per network type. They stay on the device and are exported only with logs.
- **Continuity:** organization developer accounts, Play App Signing, an escrowed release-manifest key, a succession document, and reproducible builds (NR-83).
- **Word lists:** settle the list before the first code is ever issued (X26).

### §13 Free-forever economics
- Add the honest caveats: the owned domain, n0's permission, relay operators' goodwill (hence frugal writes), and that C2 covers infrastructure but not maintainer time (NR-83).

### §14 Open source
- Add that an open engine enables free SignPath signing, F-Droid inclusion and independent verification of the E2E claims. Also note that Google client secrets can't live in public repos (R8-12, R8-15, R8-19).

### §15 To verify during prototyping
- **Remove items that are now answered:**
  - IPFS circuit relays (refuted).
  - Block Store limits (16 entries of 1 KB; restore only in the setup flow).
  - The widget's standby bucket (confirmed in AOSP; OEMs remain to be tested).
  - Google brand verification (not needed for `drive.file`; an owned domain is needed anyway).
- **Update:**
  - iroh: n0's relays are dev-only, so ask permission.
  - Nostr: limits are answered, and rate limits are a new item.
- **Add** the P0/P1 experiments from §5 of this document.

### §16 Build order
- **Before phase 1:** the domain and package decision, and developer verification.
- **Phase 1:** add the import sandbox, quarantine journal and caps; signing and the PeerID policy; the golden corpus; format-v1 limits; and the one-document-per-group vs per-list decision.
- **Phase 2:** epoch rules with property tests; link-phishing defenses; capability links; the word list.
- **Phase 3:** write budget, dedicated kind, chunk protocol, runtime relay list, document epochs.
- **Phase 4:** our own address lookup, configurable relays, LAN via the mDNS crate.
- **Phase 5:** expedited-job flush, widget offer, honest status, doorbell design.
- **Phase 6:** restore offer and identity exclusion before Block Store and Auto Backup; foss without Google Drive.
- **Phase 7:**
  - Move the web guest client earlier, for iPhone invitees.
  - Build the headless member node *before* the helper network.
  - The helper network becomes conditional on E5.1.

### Outside DESIGN.md (for consistency, not edited)
- **UX-PRINCIPLES.md:**
  - "up to date" → Saved → Sent → On Alex's phone.
  - "No technical words, ever" → ban transport words and allow sync, offline and updated.
  - Pair undo with a visible shared trash.
  - Extend the playtest plan with a diary study and a code-entry study.
  - The linking flow needs the scanning-side warning (R6-9, R6-10, R6-11, R6-12, R6-15).
- **STATE.md findings log** still says "Use NIP-78 kind 30078" and "strfry 128 KB msg limit". Both are superseded (X1, X2).
