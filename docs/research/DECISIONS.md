# Pending decisions

*Only what's still open, most important first. Full analysis for each ID: `DECISIONS-FULL.md`. Already decided: `DECIDED.md`. Evidence: the `R*`, `L*`, `E*` and `A*` reports in this folder. Only alternatives that were actually evaluated are listed.*

---

## 1. Delivery system (defines this app and future ones)

**D13. Accept the Nostr risk for launch?**
- Nostr works today: 8/8 relays accepted our test.
- But it has hard rate limits (~8 writes/min per IP), a ~64 KB event limit, operators hostile to "relays as storage", one software running most relays, and relay churn.
- Without Nostr, everything keeps working except the "nobody online, no helpers yet" case and invites (see D55).
- **Yes / No.** "No" means revisiting a constraint.

**D55. Invites without Nostr** *(new)*
- Invite records (code / link / QR lookup) were designed around Nostr.
- **Proposal:** also store them with helpers and in the BitTorrent DHT (an encrypted invite fits its 1,000-byte records; ~2 h lifetime, so the inviter's app keeps re-posting).
- **Not evaluated in depth yet.**

**D47. Doorbell without a server** (wakes sleeping phones; replaces D17(b))
- (a) Web Push to Google's push endpoint, sent by member devices, with no server of ours. Behind an off switch until tested. Risks: an undocumented registration path, and Google sees who rings whom.
- (b) UnifiedPush only, for users who have a distributor app.
- (c) No doorbell: honest "last synced" times + "Tell Alex".
- **Needs your approval to test (a) on your phone** (a small test app for ~10 days).

**D49. Internet P2P and relaying without n0**
- (a) Helper desktops as iroh relays (Let's Encrypt IP-address certificates; public-IPv4 helpers only for now).
- (b) IPv6 / router-mapped direct connections + our own signaling.
- (c) iroh over a custom transport.
- (d) A parallel libp2p stack (weak on phones).
- (e) HyperDHT blind relays on helpers.
- **My lean:** (a) + (b) first. (c)–(e) after local lab tests.

**D51. Should default-on helpers also relay live connections for strangers?**
- (a) Default-on, like storage helping: maximum coverage.
- (b) A separate opt-in relay level: avoids ISP terms, abuse complaints and firewall prompts.
- (c) Own groups only.
- **My lean:** (b), until ISP reactions are known (not verified).

**D48. A second offline mailbox besides Nostr**
- (a) Session storage swarms.
- (b) The Veilid DHT (requires MPL-2.0).
- (c) chatmail relays (needs operator consent).
- (d) Google Drive / published Sheet (opt-in, needs accounts).
- (e) None: member devices + helpers + your machine.
- **My lean:** run the local tests first. Nothing is confident today.

**D14 + D50. How to treat free "best-effort" public services** (n0 relays, IPFS relays, public STUN, Send instances…)
- (a) Only services with written permission.
- (b) Use them as non-critical parallel layers, never counted toward principle 2.
- (c) Ask operators in writing first. **Asking n0 is outward contact: your call.**

**D52. Large content (images, big attachments)**
- (a) Keep postponing until iroh-blobs is stable.
- (b) Drive blob store (opt-in per uploader).
- (c) Public Send instances as overflow.
- (d) Helper and member desktops only.

---

## 2. Invites and linking

**D8 / D7 remainder.**
- **Expiry length** for the one invite (code = link = QR): 1 day, or longer (e.g. 3 or 7 days) for invites sent by chat and opened later. Expiry doesn't affect brute-force safety.
- **Confirm the word list:** 8 words from the EFF short list, 3-letter autocomplete (option a). The alternative evaluated was 6 words from a curated large list (b).

**D6. Device linking against phishing.**
- Agreed parts: link QR codes only accepted from "Your devices", a full-screen warning, "new device linked" alerts.
- Open: **require the devices to be near each other by default** (the synthesis recommends it) vs. keep that in reserve (my lean).
- Also open: **never silently merge a non-empty device.** This touches your earlier "linking merges your lists" decision.

---

## 3. Privacy and licensing

**D23 + D54. Metadata and privacy wording.**
- Relays and DHT nodes can link a device's groups; each new layer adds an observer (Google for rings, STUN operators, DHT nodes…).
- Options for the network identity: (a) a separate iroh identity per group (more battery and sockets, unmeasured) vs. (b) accept and document it.
- Decide which observers are acceptable; the privacy text gets updated before shipping.

**D29 + D53. Licensing.**
- Loro already pulls in MPL-2.0 crates; Veilid is MPL-2.0.
- (a) Amend principle 9 to allow MPL-2.0 (file-level copyleft, fine for closed source).
- (b) Replace Loro. **Not evaluated.**

---

## 4. Backup and recovery

**D30. Android backup without a screen lock isn't end-to-end encrypted.**
- (a) Turn Auto Backup off for those users.
- (b) Seal it, like cloud backups.

---

## 5. Product and platforms

- **D45. iPhone invitees have no path.** (a) Web guest client (syncs through Nostr only). (b) An iOS app later ($99/yr Apple fee, conflicts with "free for the developer"). (c) Android + desktop only, stated on the invite page.
- **D25. History checkpoints.** (a) Full history within a window, then a fresh document. (b) The same + small monthly snapshots stored separately (my lean).
- **D33. Duplicate names.** "Is this your other device?" detection. (Private nicknames were already dropped by you; the evidence for them is recorded in FULL.)
- **D39. Windows distribution** (decide when the Windows release happens): unsigned GitHub installers get a SmartScreen warning; the Microsoft Store is free.
- **D42. OneDrive:** postpone (Microsoft registration needs a credit-card check; an API bug since Aug 2026). Google Drive only for now.

---

## 6. The block: accept as a group unless you veto an item
No real alternative was found for these (platform facts or single fixes). Details in FULL.
- **Engine:** D5 key-rotation rules · D9 Argon2 64 MiB floor · D24 document epochs · D26 compact only at acknowledged versions · D27 ≥ 30 days of undo history + "undo changes by person" · D28 we own backward compatibility.
- **Nostr:** D10 dedicated event kind · D11 write budget, relay subsets, backoff, read-back checks · D12 40 KiB chunks.
- **Android:** D15 lower P2P expectations · D16 expedited-job flush + honest delay wording · D21 fix the permission table · D43 re-request exact alarms after restore.
- **Recovery:** D31 first-launch restore prompt, linking as default recovery · D32 periodic rotation limits stolen backups (info).
- **UX:** D18 "Saved → Sent → On Alex's phone" · D34 clipboard handling · D35 small UX fixes · D36 diary study + code-entry test.
- **Housekeeping:** D37 Play verification (in progress) · D41 separate public repo for join page and releases.
- **Superseded, no action:** D17 → D47 · D22 → D52 · D46 → D49.

---

## Later (not needed for launch)
- **Free hosting for operator-manifest select entries** (D56): Cloudflare Workers/Durable Objects (e.g. a small Nostr relay or mailbox; no UDP, so no iroh relay), Oracle Cloud Always Free VM (iroh relay + helper + Nostr relay), Google Cloud e2-micro. **Not evaluated yet.** Evaluate when extra guaranteed capacity is wanted.
- **Optional bonuses re-eligible under the clarified C2** (rejected earlier only for being developer-run): a doorbell worker (backup to D47's no-server path); Cloudflare Realtime TURN free tier (developer key; bills above 1,000 GB/month) for failed hole punches. Evaluate later as manifest entries.

## 7. Outward actions awaiting approval (none run)
- The doorbell test on your phone (D47).
- A Drive test on your Google account (D48d / D52b).
- Small probes: Nostr ephemeral events on 3–4 relays, one STUN request per operator, a read-only attach to the Veilid network.
- Contacting n0, Session's foundation, chatmail or Send operators.
