# Data distribution layers: candidate evaluation

*Status: research synthesis, 2026-09-24. Inputs: eight research dimensions (L1 to L8). Each one had an independent verification pass. Where the verifier corrected a verdict, the corrected verdict is used here. Context: [DESIGN.md](../DESIGN.md) and [DECISIONS.md](DECISIONS.md). The helper network (D20) is already decided as high priority and on by default on all devices. This document treats it as a given layer and does not re-evaluate it.*

**Gaps being addressed**

| Gap | What is missing today |
|---|---|
| G1 | Offline store-and-forward that does not depend on Nostr |
| G2 | Internet P2P (address discovery and hole-punch coordination) that does not depend on n0 |
| G3 | Waking a sleeping Android phone without a server we run |
| G4 | Free relaying for pairs whose hole punch fails (about 30%) |
| G5 | Discovery, presence and signaling |
| G6 | Large blob transfer |

**Hard constraints used throughout:** $0 at any scale, no infrastructure run by the developer, no user accounts, permissive licenses (principle 9), at least two independent providers per role (principle 2), and every channel runs in parallel (principle 4).

---

## 1. Executive summary

### The technical fact that shapes G2 and G4

iroh 1.x hole punching runs **inside an existing QUIC connection**. It is triggered from `remote_state.rs` after a connection exists, and it does nothing when there is no connection. iroh learns its own public IPv4 mapping only through QAD on iroh relays. It has no STUN client. Other address sources are the portmapper (UPnP/PCP/NAT-PMP), global IPv6, LAN interfaces and `add_external_addr`.

So any hole punch that avoids n0 needs one of these three things:

1. **Another relay that speaks iroh.** It provides QAD, coordination and data relaying in one stack.
2. **A first connection over some other path.** That means iroh's unstable custom-transport API. iroh then punches by itself but still lacks a reflexive IPv4 address.
3. **A directly reachable address.** That means IPv6, a UPnP mapping or a public IP, advertised through our own signaling.

Out-of-band coordination alone (a message over Nostr, Veilid or MQTT saying "dial now") cannot drive iroh's punch.

### Gap by gap

| Gap | Best candidates after verification | Can it be closed within the constraints? |
|---|---|---|
| **G1 Offline store-and-forward, not Nostr** | Helper network and member devices (already decided). Third-party options, all CONSIDER (low): **Session storage swarms**, using the authenticated private-namespace variant because public-outbox replace is test-only on the live network; **Veilid DHT** (retention via republish or rehydration; MPL license); **chatmail** relays; **Google Drive / published Sheet** as an opt-in, quota-capped layer. | **No, not with confidence.** No third-party network gives a second independent mailbox without a serious caveat: terms silent or unfavorable (Session, chatmail), weak retention and a license conflict (Veilid), or a developer-project quota and required accounts (Drive). The only in-constraints answer we control is still member devices, helpers and the user's own always-on machine. That confirms D13. Session is the closest technical fit and needs a local experiment plus a decision on its terms of service. |
| **G2 Internet P2P, not n0** | **Helper-hosted iroh relays with Let's Encrypt IP-address certificates** (CONSIDER, medium). **Public STUN** (ADOPT, but only for sockets we own). **Nostr ephemeral signaling** (ADOPT, medium). **Mainline DHT records** (ADOPT, narrowed). **IPv6 / UPnP direct** with our own signaling (CONSIDER, gated on a lab test). Second tier: a parallel rust-libp2p stack using public Kubo relays with DCUtR, HyperDHT, or iroh over a custom transport (Veilid, libp2p, ICE). | **Partly.** Closable for desktops with public IPv4, UPnP or IPv6, and wherever a reachable helper exists. **Not closable for phone-to-phone on carrier CGNAT over IPv4 without either n0 or a reachable helper relay.** No free third-party service does iroh-compatible QAD and coordination except n0. The public libp2p relays coordinate only libp2p connections, and the measured 70% success is conditional on getting a reservation, which mobile peers get 0.6% of the time. |
| **G3 Waking a sleeping phone** | **Web Push (RFC 8030/8291/8292 VAPID)** as the wire protocol (ADOPT, high). **FCM Web Push endpoints obtained through Play Services with no Firebase project and no gateway** (the embedded-fcm-distributor path). The verifier downgraded this to CONSIDER (low-medium), promoted to ADOPT only if on-phone tests E1 and E2 pass, and shipped behind an off switch. UnifiedPush external distributors as a user choice. While-idle alarm polling as a local accelerator. | **Mostly, for phones with Google Play Services, with caveats.** Google is the only provider for mainstream phones, so principle 2 cannot be met for this role. The registration side is undocumented, which is a Play policy enforcement risk and not only a breakage risk. Whether a high-urgency push actually wakes a phone in deep Doze, and whether FCM deprioritizes pushes that show no notification, are **not verified**. De-Googled phones only get it if the user installs a distributor. **This changes the premise of D17(b): the doorbell no longer needs a worker run by the developer.** |
| **G4 Free relaying for failed punches** | **Helper-hosted iroh relays with LE IP certificates** (CONSIDER, medium; public-IPv4 hosts only until an iroh IPv6 URL bug is fixed). Member relaying (already in the design). **HyperDHT blind relays on reachable helpers** (CONSIDER, low-medium). Veilid relays and Yggdrasil (low, lab only). Metered Open Relay (off by default, leaning REJECT). | **Not by any third-party free service.** No free TURN fits: every one needs a developer key, a developer account or payment, or explicitly "can and will go down". libp2p circuits are capped at 2 min / 128 KiB by design. **It can only be closed by our own users' reachable desktops (helpers).** The share of desktops that qualify is unmeasured. This revises D14 option (d): home desktops *can* be trusted relays when they have public IPv4 plus UPnP, via LE IP certificates. |
| **G5 Discovery, presence, signaling** | **Nostr ephemeral events** for real-time signaling (ADOPT, medium, pending a probe of ephemeral kinds). **Mainline DHT BEP44 in client mode, DHT-only** for presence and endpoint records (ADOPT, medium). n0 DNS stays as a third provider. Optional: Veilid DHT, BitTorrent trackers (low), libp2p rendezvous on helpers. | **Yes.** At least two independent providers exist for records (Nostr, Mainline, n0 DNS) and for signaling (Nostr ephemeral, n0 relays). Mainline is slow (several seconds on a cache miss, no push), so it serves presence and endpoints, not time-critical coordination. |
| **G6 Large blob transfer** | Direct iroh streams between members when both are online (existing plan, request by hash). **Google Drive as a blob store** (CONSIDER, medium; opt-in; uploader needs a Google account; not available in foss builds). **Public Send instances** (CONSIDER, low-medium; overflow only; stack dormant). Helpers and member desktops store blobs for their own groups. | **Partly.** Online hand-off is solved. Offline hand-off without accounts depends on volunteer Send instances (about 19, several with poor uptime, stack dormant since 2025). With a Google account, Drive works at low volumes: the ceiling is about 0.3M to 1M photo downloads per day per project, because of the 1 TB/day egress cap. D22 (postpone images) is still reasonable. |

### Where the user asked to prioritize (what we lack or doubt)
- **Most lacking:** G3 and G4.
  - G3 now has a real no-server path, but it is gated on two on-phone measurements.
  - G4 has no third-party answer. Its only answer is helpers acting as relays, which makes the helper network carry even more of the design.
- **Most doubted:** n0 (D14) and Nostr (D13).
  - n0 can be backed up for relaying and coordination only by helper-hosted relays.
  - Nostr cannot yet be backed up by an independent third-party mailbox with confidence.

---

## 2. Ranked candidate table

Ranking combines gap coverage, how much it covers what we lack or doubt, the verified verdict and the confidence. The verdicts are the corrected ones.

| # | Candidate | Gaps | Final verdict | Confidence | One-line reason | Report |
|---|---|---|---|---|---|---|
| 1 | Web Push protocol (VAPID, RFC 8030/8291/8292) as the doorbell wire format | G3 | **ADOPT** | high | Open standard: any holder of the VAPID key can ring FCM, autopush, ntfy or NextPush with no account. Rust crate `web-push-native` is MIT/Apache. | [L4](L4-wakeup-push.md) |
| 2 | Nostr ephemeral events as signaling (own encrypted format, not a draft NIP) | G2 coordination, G5 | **ADOPT** | medium | Strong precedents (Trystero, Amethyst, 0xChat). Not stored long-term (strfry keeps them 300 s). Ephemeral-kind acceptance on our relay set is unprobed, and events older than 60 s are rejected. | [L3](L3-webrtc-route.md) |
| 3 | Mainline DHT BEP44 (pkarr / mainline, or n0-mainline), client mode, DHT-only | G5 (presence and endpoints) | **ADOPT (narrowed)** | medium | 10M+ nodes and 4 independent bootstrap operators. Desktops may re-put members' signed items (the spec allows it). Crate is being rewritten, IPv4-only, lookups take seconds. Must disable pkarr's default Pubky HTTP relays. | [L6](L6-other-overlays.md) |
| 4 | Public STUN (Cloudflare, Google, Twilio, VoIP operators) | G2 (address discovery) | **ADOPT** | high on availability, medium on permission | Free, many operators. Only Cloudflare (and Metered) state free use in writing. Useful only for sockets we own (ICE or custom transport); iroh has no STUN client. | [L3](L3-webrtc-route.md) |
| 5 | Helper-hosted iroh relays with Let's Encrypt IP-address certificates | G2, G4, G5 (partial) | **CONSIDER** | medium | The only in-constraints second provider of QAD, coordination and relaying, all in iroh's own stack. Limited to public-IPv4 hosts until iroh handles bracketed IPv6 relay URLs. Qualifying share unmeasured, single CA. | [L8](L8-gap-sweep.md) |
| 6 | FCM Web Push endpoints via Play Services (embedded-fcm-distributor 3.x or clean reimplementation) | G3 | **CONSIDER; ADOPT only if E1 and E2 pass**, behind an off switch | low-medium | $0, no Firebase project, no gateway. Ships on Play in DAVx5 and Fedilab. Undocumented registration means a Play enforcement risk. Doze wake and deprioritization unmeasured. License ambiguous (LGPL repo vs Apache POM). | [L4](L4-wakeup-push.md) |
| 7 | IPv6 / UPnP direct iroh connection with Nostr + Mainline signaling (plus `add_external_addr` and simultaneous connect) | G2 (partial) | **CONSIDER**, gated on a NAT-lab test | medium | No new stack and no unstable API. IPv6 needs only firewall punching. The share of phone pairs that have IPv6 is the dominant unmeasured parameter. | [L3](L3-webrtc-route.md), [L2](L2-libp2p-ipfs.md) |
| 8 | Session storage swarms (authenticated private namespaces; public outbox only after the fix is released) | G1, G5 (poll), G6 (≤75 KiB) | **CONSIDER** | low | Closest fit to the slot model: 30-day TTL, renew without re-upload, 1,500+ staked nodes, independent of Nostr. ToS covers only the Session app. Outbox replace is test-only today. Protocol changing fast. Seed domains are malware IOCs. Needs our own Rust client (official libraries are GPL). | [L1](L1-store-forward-networks.md) |
| 9 | Veilid DHT as an encrypted slot mailbox (SMPL per-writer subkeys) | G1, G5 | **CONSIDER** | medium on facts, low on fit | Deployed, free, independent of n0 and Nostr, no operator rate limits. LRU of 128 records per node needs republish or rehydration. Android foreground only. Pre-1.0 wire breaks. Bus factor about 1. **MPL-2.0 conflicts with principle 9 and forces D29.** | [L7](L7-veilid.md) |
| 10 | iroh over a custom transport (Veilid first, or libp2p circuit / HyperDHT / ICE), letting iroh punch in-band | G2, G4 | **CONSIDER, lab only** | low | Source reading suggests iroh would punch its own IP paths once any first connection exists. Stacks unstable APIs, lacks reflexive IPv4, untested by anyone. | [L7](L7-veilid.md), [L2](L2-libp2p-ipfs.md), [L3](L3-webrtc-route.md) |
| 11 | rust-libp2p 0.57 parallel stack (TCP+Noise+Yamux, identify, relay client, DCUtR) over public Kubo relays and helper relays | G2, G5 | **CONSIDER** | medium | Real relay pool independent of n0 (≤2,047 stable Kubo nodes, permission implied). DCUtR 70% is **conditional** on getting a reservation, and mobile reservation is 0.6%. Second stack, no AutoRelay, rust-libp2p max_circuits default must be raised. | [L2](L2-libp2p-ipfs.md) |
| 12 | Google Drive as a blob store | G6 | **CONSIDER** | medium | Low request volume stays under the project quota. Uploader's own 15 GB. Readers need no account. 1 TB/day egress cap binds for photos. Opt-in, foss builds excluded, abuse false positives. | [L5](L5-user-cloud-mailbox.md) |
| 13 | HyperDHT / Hyperswarm via peeroxide (Rust) | G2, G4, G5 | **CONSIDER** | low-medium | Coordination by DHT nodes plus birthday punching. Blind relays run by our own helpers, but relays should be reachable helpers (one random-NAT punch per 20 s per node). Rust port has one maintainer and is 74 days behind upstream. Holepunch-run bootstrap. IPv4-only. | [L6](L6-other-overlays.md) |
| 14 | UnifiedPush connector with a user-chosen external distributor (ntfy, Sunup, NextPush) | G3 (de-Googled phones) | **CONSIDER** (user choice only) | high | Same Web Push sender, Apache-2.0 connector. Needs a second app, a battery exemption and a permanent notification. Never prompt for it. | [L4](L4-wakeup-push.md) |
| 15 | While-idle alarm polling (`setAndAllowWhileIdle`) | G3 (poll, not wake) | **CONSIDER** | low-medium | Local, no permission. Allowlisted apps get network access "unless data saver". Costs battery blame. Android 15+ and OEM behavior unverified. | [L4](L4-wakeup-push.md) |
| 16 | Google Drive public-link mailbox (API-key reads), or a publish-to-web Sheet read path | G1 (opt-in writers), G5 (slow) | **CONSIDER** | low | Documented and reader-anonymous. All usage is metered on the developer's one Cloud project: about 87k devices via the API, about 400k with published-Sheet reads. Charges planned later in 2026. Writers need Google accounts. | [L5](L5-user-cloud-mailbox.md) |
| 17 | Public Send instances (timvisee/send) | G6, G1 (large snapshots) | **CONSIDER** (overflow only) | low-medium | Anonymous, encrypted client-side, 3 to 30 days of retention, about 19 operators. Several instances have 25-84% uptime. Server and Rust client dormant. Original died of abuse. | [L8](L8-gap-sweep.md) |
| 18 | chatmail public relays (IMAP/SMTP) | G1, G6 (partial) | **CONSIDER** | low | Easiest to build with permissive crates. About 24 small volunteer relays built for Delta Chat. A shared-credential group mailbox may look like abuse. Operator consent needed in practice. | [L1](L1-store-forward-networks.md) |
| 19 | Veilid as a discovery / address-exchange channel for iroh | G5 | **CONSIDER** | medium | Can publish EndpointAddrs. Cannot coordinate iroh's in-band punch. | [L7](L7-veilid.md) |
| 20 | Veilid private routes and relays for small messages | G2, G4, G5 | **CONSIDER** | low | NAT traversal inside the network. 32 KiB cap, high latency, unknown relay capacity. | [L7](L7-veilid.md) |
| 21 | ICE-only: str0m `is` crate + public STUN + Nostr → iroh custom transport | G2 | **CONSIDER** | low | Small and permissive. The Firezone precedent does not carry over: it runs as a VPN service and uses its own TURN and portal. `is` has had 5 breaking releases in 6 months. Alternatives: `webrtc-ice`, `rice-proto`. | [L3](L3-webrtc-route.md) |
| 22 | webrtc-rs 0.21 full stack | G2, G6 (direct) | **CONSIDER** | medium-low | Mainly for a future browser/PWA guest (D45). Rewritten recently, one dominant maintainer. | [L3](L3-webrtc-route.md) |
| 23 | Public IPFS bootstrap nodes | G2 (cold start) | **CONSIDER** (last-resort seed only) | high | Best-effort by policy. Ship them as IP multiaddrs (DNS on Android is broken, rust-libp2p #6529). | [L2](L2-libp2p-ipfs.md) |
| 24 | AutoNAT v2 on Kubo | G2 | **CONSIDER** (desktops and helpers only) | medium | Decides whether a desktop offers relay service. Phones should skip it. | [L2](L2-libp2p-ipfs.md) |
| 25 | Tor onion services via Arti | G2, G4, G5 | **CONSIDER** (narrow) | low | Desktop-hosted onion as a last resort, phones as clients only. Arti's docs contradict each other on hosting maturity. Users visibly look like Tor users. | [L6](L6-other-overlays.md) |
| 26 | Public BitTorrent UDP/HTTP trackers as rendezvous | G5, G2 (IP only) | **CONSIDER** | low | 53 stable trackers, about 45 operators. Declared port, not NAT port. Leaks IPs to anyone who holds the current info-hash. Often blocked at work or school. | [L8](L8-gap-sweep.md) |
| 27 | Yggdrasil overlay | G2, G4 | **CONSIDER** (experiment only) | low | Multi-operator fabric, but volunteers carry 100% of traffic. Alpha, Go runtime, unstable custom transport. | [L8](L8-gap-sweep.md) |
| 28 | Metered Open Relay (public TURN with a static secret) | G4 | **CONSIDER** (off by default; leaning REJECT) | low | The secret is framed for one admin-run server, not mass-distributed apps. No quota stated for that use. "Can and will go down." | [L3](L3-webrtc-route.md) |

---

## 3. Proposed layered architecture

Every layer carries the same encrypted sync messages, in parallel (principle 4). "Proposed" means a layer at ADOPT, or a CONSIDER layer that is built only after its experiment passes. Items in *italics* are conditional.

### 3.1 By situation

| Situation | Layers active in parallel | Third parties involved |
|---|---|---|
| **A. Both online, same LAN** | iroh direct over LAN (mDNS / local discovery; Android 17 local-network permission, D21). Nostr slot writes and helper mailboxes continue at the D11 budget. | None for the data path. |
| **B. Both online, different networks** | **Discovery:** Nostr addressable "where am I" slot, **Nostr ephemeral signaling**, **Mainline DHT endpoint record**, n0 DNS. *Veilid record optional.*<br>**Direct path:** iroh with (1) n0 relays for QAD and coordination (best-effort, D14a); (2) ***helper iroh relays*** (LE IP certificates, inserted at runtime via `insert_relay`) for QAD, coordination and relaying; (3) ***IPv6 / UPnP / public-IP direct*** advertised through our signaling, with a simultaneous connect timed through Nostr ephemeral; (4) *lab-gated: a custom transport (Veilid / libp2p / HyperDHT / ICE) as the first connection, then iroh punches in-band.*<br>**Relayed fallback:** n0 relays, helper iroh relays, member relaying (a member desktop with a direct path to both), *HyperDHT blind relays on reachable helpers*.<br>**Mailboxes:** keep writing so a disconnect loses nothing. | n0 (best-effort), Nostr relays, Mainline DHT nodes, public STUN only if ICE or a custom transport is built, helper volunteers. |
| **C. One device offline** | **Mailboxes in parallel:** member devices (tier 1, on disk), always-on own machine, **helpers** (D20), Nostr relays (group's subset, D11), *Session private namespaces* or *Veilid DHT* (if O2 picks one), *Drive / published Sheet* (opt-in). **After every write, ring the offline members' doorbells (situation D).** | Nostr, helpers, optional second mailbox network, Google (opt-in). |
| **D. Phone asleep (Doze)** | ***Web Push ring*** from any online member device, or a member desktop ringing for a phone. Content-free, padded, batched, Topic header, VAPID key rotated every membership epoch. It reaches **FCM** via the Play Services endpoint, or the user's **UnifiedPush distributor**. The ring triggers an **expedited WorkManager job** that fetches from all mailboxes.<br>If there is no ring path, or the ring gets deprioritized: WorkManager windows (D16), *while-idle alarm poll*, honest per-member freshness, and "Tell Alex" through the user's messenger (D17a). | Google FCM (mainstream phones), user-chosen distributor operator. |
| **E. Large content (images, big snapshots)** | **Online:** direct iroh stream between members, request by hash (D22). **Offline:** member and helper desktops hold blobs for their own groups; *Drive blob store* (uploader opt-in, Google account); *Send instances* (overflow, pointer carried over small channels, re-publishable Mainline pointer). Nostr carries only ThumbHash-size previews and pointers. | Google (opt-in), Send volunteer operators. |

### 3.2 Provider independence per role (principle 2)

"Independent" counts organizationally separate operators or mechanisms. The helper network and member devices are **our own users**. They are listed separately because they are not third parties, but they depend on scale and participation.

| Role | Today (DESIGN + DECISIONS) | With ADOPTs | With CONSIDERs that pass their experiments | Meets principle 2? |
|---|---|---|---|---|
| Small offline mailbox | Nostr (1 ecosystem; strfry monoculture, D13) + member devices + helpers + own machine | same | + Session **or** Veilid (2nd third-party network); + Drive/Sheet (opt-in); + chatmail (low) | **No** at launch for third parties. Yes only if a second network passes, or once helpers reach scale. |
| Large offline content | none (Blossom rejected) | none | Drive (opt-in, account-bound), Send (about 19 operators, fragile), helper and member desktops | Weak. |
| Presence and endpoint records | n0 DNS, Nostr slot, pkarr (planned) | **Nostr, Mainline (DHT-only), n0 DNS = 3** | + Veilid, + trackers | **Yes.** |
| Real-time signaling | n0 relays (implicit) | **Nostr ephemeral + n0 = 2** | + helper relays, + Veilid app_message | **Yes.** |
| Address discovery for iroh's socket | n0 QAD, plus local portmapper / IPv6 / LAN | same | + helper-relay QAD (where helpers exist); + QUIC observed-address reports from a reachable member (unverified API) | Partial. Public STUN counts only for sockets we own. |
| Hole-punch coordination | n0 relays only | same | + helper iroh relays; + custom-transport first connection; + libp2p/DCUtR over Kubo (separate stack) | Only with helpers. |
| Data relay for failed punches | n0 relays + member relaying (own groups) | same | + helper iroh relays; + HyperDHT blind relays; (Open Relay: no) | Only with helpers. No third-party second provider exists. |
| Wake-up of a sleeping phone | none | Web Push sender (protocol only) | Google FCM (mainstream) + user-installed distributors + local alarm poll | **No.** Google is the only mainstream provider. That is structural on Android. |

### 3.3 Engine implications (reusable core, principle 7)

- **Capability-based mailbox interface.** The engine should model each mailbox as a set of capabilities instead of assuming Nostr's semantics:
  - `replace` (atomic replace or delete-then-write)
  - `max_size`
  - `ttl`
  - `renew`
  - `read_auth`
  - `append_only`
  - `push` / `subscribe`
  - `rate_budget`

  This matters because Nostr and Session outbox slots are replaceable, IMAP is a mailbox with delete, Waku is an append log, and Veilid uses per-writer subkeys with sequence numbers.
- **Doorbell as a transport-independent layer.** Endpoints are opaque URLs. Allow-list the host, not the path. Check that the target is a public IP. Follow no redirects. Drop an endpoint on a 404/410. Republish endpoints after every sync. Rotate VAPID keys each membership epoch.
- **Relay list as signed runtime config**, covering both n0 and helper relays, with per-operator health tracking (not per hostname).
- **One transport-key rotation policy** covering the extra long-lived identifiers these layers add: DHT keys, onion addresses, libp2p peer IDs, VAPID keys (D23).

---

## 4. Rejected candidates (do not re-propose)

| Candidate | Reason | Report |
|---|---|---|
| Waku Network (Logos Messaging) | Storage requires an RLN membership on an Ethereum L2 (paid in DAI on the mainnet design). 12 h to 2 days of append-log retention. Rust bindings stale since 2024. | L1 |
| SimpleX preset servers | Terms allow third-party apps only for "user-to-user messaging", with SimpleX branding. Single reader, delete on delivery. Haskell under AGPL. | L1 |
| Freenet (Rust) | Alpha, desktop-only, telemetry, forced upgrades, AGPL/LGPL. **Watch:** its contract model is the best conceptual fit for CRDT slots. | L1 |
| Briar Mailbox | A self-hosted device, not a network. AGPL. Briar is in maintenance mode. | L1 |
| Reticulum / LXMF | License bans use in AI training datasets and the README is hostile to AI-generated derivatives. Not OSI-approved. Python-only. | L1, L8 |
| Bitmessage | Small network, no Rust or mobile implementation (not checked deeply). | L1 |
| Amino (IPFS) DHT provider records | Rejects arbitrary records. 48 h / 22 h cycle. Duplicates Mainline and Nostr. Misuse of IPFS. | L2 |
| IPFS delegated routing (delegated-ipfs.dev) | Single operator, non-production policy, increasing rate limits. | L2 |
| libp2p circuit relay v2 as a data relay | 2 min / 128 KiB per circuit by design. Chaining circuits abuses volunteers. | L2 |
| iroh punching coordinated through a libp2p circuit (C10), for now | Unstable API, two stacks. Feasible only for UPnP / IPv6 hosts. Superseded by the general custom-transport lab item. | L2 |
| rust-libp2p replacing iroh | Loses iroh's unlimited relaying and multipath for relays that cannot carry data. | L2 |
| jvm-libp2p | Relay and AutoNAT are prototypes, no hole punching, no known production Android use. | L2 |
| Cloudflare Realtime TURN | Server-side key required. Usage over 1,000 GB billed to the developer. | L3 |
| ExpressTURN free plan | Static per-account credentials would be shared with the world. Terminable. | L3 |
| Twilio / Metered paid / Xirsys TURN, self-hosted coturn | Cost money or need infrastructure run by the developer. | L3 |
| str0m full stack | SFU-focused, P2P less tested, Android untested. (No TURN is a design choice.) | L3 |
| libdatachannel / datachannel-rs, Google libwebrtc AAR | MPL plus a C++ toolchain; libwebrtc is a 20.6 MB AAR and Kotlin-only. | L3 |
| gCompat-UP | Same mechanism as the embedded distributor, plus an install step. "Mainly for testing". | L4 |
| Our own WebSocket to Mozilla autopush | Needs a persistent connection Android kills. No permission. FCM bridge limited to Mozilla app IDs. | L4 |
| FCM HTTP v1 with an embedded service account | An extracted key lets anyone notify every user. The project can be disabled. | L4 |
| Developer Firebase project with a static VAPID key | Shared private key in the APK. Undocumented. Strictly worse than per-device VAPID. | L4 |
| Battery-optimization exemption | A to-do app is not an acceptable use under Play policy. The exemption does not wake anything in Doze. | L4 |
| ntfy.sh as a default or as a mailbox | Setup burden, best-effort terms, 250 messages/day per IP, 12 h cache. (Fine as a user-chosen distributor.) | L4 |
| Drive unauthenticated download URL (drive.usercontent.google.com) | robots.txt "Disallow: /" plus Google's automated-access terms. Undocumented and throttled. **Record this so it is not adopted later as an "optimization".** | L5 |
| Dropbox shared links | Discretionary production approval, 24 h sharing pause on a limit breach, low reach. | L5 |
| OneDrive anonymous links | Full-drive scope, readers need bearer tokens, AppFolder 403 regression open. | L5 |
| MEGA public links | App approval required, accounts, deletion after 3 months of inactivity, stale Rust crate. | L5 |
| 0x0.st, paste.rs, transfer.sh, catbox | Terms ban automated and backup use (0x0.st: "CLANKERS ARE NOT WELCOME"). Immutable URLs. Single operators. | L5, L8 |
| GitHub Gists / raw.githubusercontent | Writers need GitHub accounts. Bandwidth rules in the AUP. | L5 |
| Apps Script web app in the user's account | Each user must enable the API, probably needs a sensitive scope, and pushes code into users' accounts. | L5 (verify) |
| User's own Gmail as a mailbox | Restricted scopes requiring a security assessment. Part of the 2026 quota tiering. | L5 (verify) |
| I2P (i2pd, Java, emissary) | Long-running transit router, conflicts with Doze, Play proxy exposure, small and slow network. | L6 |
| Syncthing public relay pool | Operators consented to serve Syncthing users. Needs a permanent TCP join. Forward-only. | L6 |
| WebTorrent / BitTorrent for blobs | No store-and-forward. Exposes IP-to-infohash. No benefit for 2 to 10 devices. | L6 |
| Veilid for waking phones (G3) | No push mechanism. Node must be attached. | L7 |
| Veilid for large blobs (G6) | Block store is an empty stub. 32 KiB messages, 1 MiB records. | L7 |
| Veilid as a fix for iroh hole punching (out-of-band) | iroh's punch runs in-band in QUIC. Only the custom-transport route (item #10) is viable. | L7 |
| OpenArchive save-dweb-backend / veilid-iroh-blobs code | GPL-3.0, pinned to iroh 0.24 with forked crates, flaky tests. Reference only. | L7 |
| Public MQTT test brokers as a shipped default | 3 of 4 operators say testing only; Mosquitto says "do not rely". **Corrected from CONSIDER.** Allowed for dev and test only. | L8 |
| XMPP in-band-registration providers | Hidden per-device accounts on about 15 volunteer servers, 9 of which need a CAPTCHA. MPL Rust stack. | L1, L8 |
| OpenDHT / Jami | 10-minute values. Push only via its own proxy (a UnifiedPush gateway exists but adds nothing over L4). GPL crate from 2021. | L8 |
| Mycelium (ThreeFold) | Public fabric is about 10 nodes run by one company. | L8 |
| WebTorrent WebSocket trackers | Few operators, online-only offer relays. | L8 |
| p2panda | Built on iroh with n0 relays: the same dependency. | L8 |
| Willow / Earthstar | No public infrastructure. willow-rs archived on GitHub. | L8 |
| Radicle seed nodes | Misuse of git replication. Data is permanent. | L8 |
| Matrix P2P / Pinecone | Experimental. Dendrite archived. | L8 |
| Tox | Tiny network, GPL, no offline messages. | L8 |
| Holochain / kitsune2 | Test-only public server. Its Android runtime itself uses n0's relay. CAL license. | L8 |
| Nym mixnet | Free today, but its stated app model is companies buying credentials. Single network. | L8 |
| Lokinet | GPL, TUN-based, stale. | L8 |
| GNUnet | Early alpha. | L8 |
| AT Protocol (Bluesky PDS) | Per-device accounts, public repositories, bulk accounts violate the guidelines. | L8 |
| Automerge public server, Jazz, ElectricSQL, Firebase Spark, Ditto, GUN, Scuttlebutt rooms | Prototyping-only, vendor-run, self-host required, tiny free tiers, or inactive. | L8 |
| trycloudflare, bore.pub, sslip.io/nip.io as primary, PeerJS cloud, Tailscale DERP | Testing-only, hobby operators, shared rate limits, or outside their terms. | L8 |
| IRC, SMS, Ethereum Swarm, Meshtastic, cjdns | Forbidden by policy, paid, needs hardware, or GPL with Yggdrasil's drawbacks. | L8 |

---

## 5. New risks

### Dependency and permission
1. **Implied-permission services are everywhere.**
   - "Testing only" or "best effort": n0 relays, IPFS bootstrap and routing, MQTT brokers.
   - Implied permission only: Kubo relays, Google STUN, Twilio STUN.
   - Framed for a different use: Metered Open Relay's static secret.
   - Built for a flagship app, with terms silent on others: Session, chatmail, Syncthing relays.

   Each extra layer adds acceptance risk and a Play "Device & Network Abuse" exposure: the policy lists "access or use a service or API in a manner that violates its terms of service".
2. **The FCM Web Push registration is undocumented.** Google APIs Terms §2.c ("only by the means described in the documentation") combined with the Play policy above make this an **enforcement risk against the app**, not only a breakage risk. It has already changed IPC mechanism once. Confirmed Play shippers of the 3.x path are only DAVx5 and Fedilab.
3. **"Free" networks are drifting toward paid.** Waku RLN is priced in DAI. Session sells Pro because free usage is "not sustainable". Nym plans companies buying credentials. Google Workspace API charges above 400M units/day arrive later in 2026, and this also caps any Drive polling feature.
4. **Monocultures and single points.**
   - About 84% of dialable libp2p relays run Kubo.
   - IPFS stewardship (Shipyard) is winding down.
   - HyperDHT bootstrap is 3 Holepunch nodes.
   - Veilid cold start depends on Veilid Foundation DNS and keys until `bootstrap_records` ships.
   - LE IP-address certificates appear to come from ISRG only.
5. **Small maintainer bases.** peeroxide, Veilid and webrtc-rs each have about one dominant developer. rust-libp2p has 2 maintainers. pubky mainline is mid-rewrite; its own maintainers call the current version "unreliable ... under concurrency". `is` had 5 breaking releases in 6 months.

### Security and abuse
6. **Session network reputation.** Seed domains are listed as Mini Shai-Hulud malware C2 IOCs, and the malware used the network itself for exfiltration. Security tools may flag snode traffic in general.
7. **Web Push SSRF and amplification.** Members supply push endpoints, so a malicious member could make other devices POST to LAN hosts. Needs a host allow-list, a public-IP check and no redirects.
8. **Battery DoS by a member holding a VAPID key.** Needs receiver coalescing, a Topic header, and VAPID rotation every membership epoch.
9. **An embedded Google API key** (needed for account-free Drive reads) is extractable and can exhaust the shared project quota. It also conflicts with the Google APIs rule against embedding credentials in open-source projects.
10. **Veilid and Sybil attacks.** Open fanout-Sybil (#445) and node-tracking (#417) issues. Our phone nodes consuming relays and DHT slots could shorten retention for everyone.

### Helper-network exposure (grows with this research)
11. **Default-on helpers become public servers** if they run iroh relays with LE IP certificates, libp2p relays or HyperDHT blind relays. Consequences: ISP acceptable-use policies that forbid servers, abuse complaints about relayed traffic landing on users, Windows firewall prompts, and helpers catalogued by public crawlers (ProbeLab crawls the Amino DHT about 12 times a day with public datasets). The D20 disclosure must cover this.
12. **Veilid stores strangers' records on disk by default** (up to 256 MB) and routes for strangers on phones too. That contradicts §7a "RAM only, never disk" and reopens the Play "proxy services" question. Yggdrasil and Mycelium forward by default unless nodes are strictly leaf-only.

### Metadata (update §9)
13. New observers:
    - **Google** sees the sender IP → recipient Google account edge for every ring: the social graph of shared lists.
    - TURN operators see both peers' IPs.
    - STUN operators see presence timing (unencrypted on the path).
    - Mainline and HyperDHT nodes nearest a key see who puts and gets it, and when.
    - Anyone holding the current info-hash learns tracker members' IPs.
    - Cloud providers link a writer's real account to each group's mailbox.
    - Tor use is visible to network observers.
14. **More long-lived identifiers** (DHT server keys, onion addresses, libp2p peer IDs, VAPID keys, Drive file IDs) multiply D23 linkage. Each must rotate from the epoch key.

### Platform and technical
15. **Mobile reality undercuts P2P numbers.**
    - DCUtR's 70% is conditional on a reservation, and mobile reservation is 0.6%.
    - The measured populations are desktops, only about 4% of them behind symmetric NAT.
    - Carrier NAT rebinding (about 50-75 s on T-Mobile) forces periodic re-punching, which costs battery.
    - Android cuts background sockets after about 5-10 s.
16. **iroh's custom-transport API is unstable.** It had a send-path bug closed only on 2026-09-22, and its gating already changed once. Every "iroh without n0" route except helper relays and IPv6/UPnP depends on it.
17. **iroh 1.2 probably rejects bare-IPv6 relay URLs** (bracketed host passed to rustls `ServerName`). Until that is patched upstream, helper relays are limited to public-IPv4 hosts. This is from reading the source, not tested.
18. **Silent data loss patterns in DHT mailboxes.** In Veilid, writes at the same sequence number are silently dropped, and an evicted record reads back as empty. Session's 10 GiB node database fails stores when full, with no eviction. The engine must treat "empty" as "unknown", never as "deleted".
19. **Deprioritized rings.** Users who deny POST_NOTIFICATIONS, or rings that never show a notification, may be downgraded to normal priority, which is no better than WorkManager in Doze. Data Saver blocks network access in the allowlist window.
20. **License pressure on principle 9.**
    - Veilid core is MPL-2.0: adopting it forces D29.
    - Session, SimpleX, Freenet and Briar clients are copyleft: we must write our own clients.
    - embedded-fcm-distributor is ambiguous (LGPL repo vs Apache POM): reimplement it cleanly (about 500 lines).
    - libp2p-quic pulls in aws-lc-rs (C) next to ring.
21. **Binary growth is unmeasured** for every candidate stack (libp2p, `is`, webrtc-rs, peeroxide, veilid-core, arti) against the 7.07 MB E6 core. Veilid alone is estimated at +6 to 9 MB.

---

## 6. Safe local experiments (no load on public infrastructure), prioritized

Priority follows the user's instruction: first what we lack (G3, G4), then what we doubt (n0 for G2, Nostr for G1). Most items share one **NAT lab**: Linux network namespaces or WSL2 with nftables. It reuses R1's E3b setup and emulates these NAT types:
- port-preserving endpoint-independent mapping
- port-restricted cone
- symmetric / CGNAT-like
- IPv6 with a stateful firewall
- UPnP-capable router

| Pri | Experiment | Gap | What it decides | Source |
|---|---|---|---|---|
| **P1** | **Helper iroh relay with an LE IP-address certificate, via Pebble.** Issue the certificate through `instant-acme` (`Identifier::Ip`, shortlived profile). Run iroh-relay 1.2 embedded. Add and remove it at runtime (`insert_relay`). Block direct paths and verify IP-SAN validation, QAD and relayed traffic. Test a bracketed IPv6 URL to confirm or refute the `ServerName` bug. Measure RAM and CPU per 100 connections. | G2, G4 | Whether helpers can be a second provider for n0's roles. | L8 X1 |
| **P1** | **"Can I be a relay?" self-test** across the NAT lab: portmapper result, free external port, inbound probe from a peer. Separate the ACME validation port (80/443 every few days) from the serving port (any port). | G4 | How many helper configurations qualify. Real-world share only later, through opt-in telemetry. | L8 X2 |
| **P1** | **Doorbell on the user's own phone.** **Needs user approval:** it sends a handful of pushes through Google's FCM to the user's own device. Covers E1, E2, E3, E4, E5 and E8:<br>- **E1:** register a `wp:` subscription with a test APK and send from the desktop with `web-push-native`. Compare Urgency high, normal and none in forced and natural deep Doze, and log `delivered_priority`.<br>- **E2:** over 8 to 10 days, compare a notifying install with a silent one to see deprioritization.<br>- **E3:** on a ring, an expedited WorkManager job fetches from a **LAN** relay on Android 15, 16 and 17.<br>- **E4:** Topic and TTL collapse.<br>- **E5:** registration robustness, including a no-GMS emulator and microG.<br>- **E8:** old VAPID key rejected after re-registration. | G3 | Whether FCM Web Push is promoted to ADOPT. | L4 |
| **P1** | **While-idle alarm window test (E6).** Headless emulator, standby buckets forced with `am set-standby-bucket`, Data Saver on and off, socket to a LAN host inside the about-10 s allowlist window. | G3 | Local accelerator for phones without Google Play Services. | L4 E6 |
| **P2** | **iroh without n0 in the NAT lab** (relays disabled; one harness, several variants):<br>(a) global IPv6 plus a simultaneous connect timed through a **local** strfry with ephemeral events;<br>(b) UPnP-mapped desktop;<br>(c) `add_external_addr` with STUN from a side socket, on port-preserving vs randomizing NATs;<br>(d) QUIC observed-address reports from a reachable member, if the API exposes them;<br>(e) reflexive address from a local Mainline testnet (BEP42 `ip`), with a shared vs separate socket. | G2 | Whether cheap direct paths close much of G2 without a new stack. | L3 X2, L6 X5, L6 X3 |
| **P2** | **iroh over a custom transport, first connection then in-band punch.** Variants: (i) an isolated local **Veilid** network (`network_key_password`, `bootstrap_records`); (ii) a local libp2p circuit; (iii) an `is` ICE agent with a local STUN server. Measure whether iroh opens direct IP paths after the first connection, and the relay fallback latency. | G2, G4 | Whether any overlay can stand in for n0 as the first path. | L7 C4, L2 X3, L3 X1 |
| **P2** | **HyperDHT NAT matrix** with peeroxide's harness and an isolated bootstrap, including RANDOM↔RANDOM through a blind relay on (a) an open helper and (b) a consistent-NAT helper. Check the 20 s random-punch throttle and packet counts. | G2, G4 | HyperDHT as a relay/coordination provider. | L6 X1 |
| **P2** | **libp2p DCUtR NAT matrix** with a local relay: TCP vs QUIC, time to a direct connection. **Plus an emulator reservation test ranked above it:** time from foreground to reservation without AutoNAT, loss after backgrounding, and whether one expedited job can reserve, coordinate and sync. | G2 | Whether a libp2p stack is worth it on phones. | L2 X5, X2 |
| **P3** | **Capability-based mailbox interface with mock backends** (Nostr addressable, Session outbox and private namespace, IMAP, append log, Veilid SMPL). Property tests for GC, replace, renew, same-sequence collisions and "evicted reads as empty". | G1 | Precondition for adding any second mailbox. | L1 X6, L7 |
| **P3** | **Local session-storage-server** (MIT) with a mocked oxend or local stagenet. Covers: private-namespace store and delete; outbox replace with and without the unreleased fix; subaccount Write+Delete bits; clock tolerance; self-signed TLS pinning. **Needs user approval to build.** | G1 | Session viability. | L1 X1 |
| **P3** | **Veilid isolated network, 10 to 50 nodes, plus an Android emulator.** Measure cold and warm attach, SMPL write and read latency, LRU eviction above 128 records, whether **local rehydration on open** replaces republishing, attach-sync-detach inside an expedited job, and airplane-mode recovery (`restart_network`). Extend the E5 helper simulator with a 128-record LRU per node. | G1, G5 | Veilid retention on phones. | L7 |
| **P4** | **Combined size harness** (extends E6: arm64, opt-level z, LTO, strip, 16 KB alignment). Delta over 7.07 MB for: libp2p TCP-only and QUIC variants, `is` + `stun_codec`, webrtc 0.21, peeroxide-dht, veilid-core, arti-client (onion service), a Session-style client stub, `web-push-native`. Run `cargo tree -d` and `cargo deny` for duplicate crates and licenses. | all | Size and license cost of each layer. | L1 X2, L2 X1, L3 X4, L6 X2/X4, L7 |
| **P4** | **Local Send server (Docker).** Upload and download a 5 MB encrypted blob, test expiry, download limits and owner-token deletion. Republish the pointer on a local Mainline testnet from a node without the key. | G6 | Send as offline blob overflow. | L8 X3 |
| **P4** | **Drive quota model script** (purely local): units per device for group sizes 2 to 10, cadences of 15 min / 1 h / 6 h, API reads vs published-Sheet reads, and egress for blobs. Output device ceilings and back-off policy. | G1, G6 | Whether a Drive layer is worth building. | L5 X7 |
| **P5** | **Drive behavior tests (X1 to X4, X8)** on the user's test Google account. **Needs user approval.** Covers: `anyone` permission with `drive.file` on personal and Workspace accounts; poll staleness and ETags; owner fields leaking to link holders; unit cost of `alt=media`; whether a converting update keeps the publish state. | G1, G6 | Drive details. | L5 |
| **P5** | Local chatmail relay in a container: PGP/MIME through filtermail, IMAP APPEND vs SMTP, shared-credential logins, quotas. | G1 | chatmail viability. | L1 X3 |
| **P5** | Local tracker (aquatic / opentracker): what IP and port data peers and the operator learn from rotating info-hashes. | G5 | Tracker privacy cost. | L8 X5 |
| **P5** | Local 3-node Yggdrasil network: iroh over it, leaf-only enforcement, gomobile AAR size. | G2, G4 | Keep or drop Yggdrasil. | L8 X4 |
| **P5** | Local nwaku + Anvil, and the Freenet fdev simulator. Only if those candidates are reopened. | G1 | — | L1 X4, X5 |

**Outward actions that need explicit user approval** (not experiments; listed so they are not run by default):
- one polite ephemeral-kind round trip on 3 to 4 public Nostr relays, plus one STUN request per default STUN operator (L3 X6)
- one attach to the public Veilid network to read `estimated_network_size` and relay counts
- asking the Session Technology Foundation about third-party use of the storage network
- asking n0 in writing (D14)
- asking chatmail operators or Send operators for consent

---

## 7. Open decisions for the user

Only alternatives that were actually evaluated are listed.

### O1. Doorbell: revisit D17(b) under the new premise (G3)
**What changed:** the doorbell no longer needs a worker or any server run by the developer. Member devices sign VAPID and push directly to the phone's FCM Web Push endpoint.

- **(a) Build it as an optional layer behind a hidden off switch.** Promote to default only if E1 (high urgency wakes the phone in deep Doze) and E2 (no deprioritization for rings that show notifications) pass. Reimplement the registration code cleanly because of the license ambiguity.
- **(b) UnifiedPush only for users who already have a distributor, plus the while-idle alarm poll.** No Google path.
- **(c) No doorbell.** Honest per-member freshness plus "Tell Alex" (D17a).

**Confidence:** low-medium for (a).
**Not verified:** Doze wake behavior; deprioritization timing; whether Google or Play would act against the undocumented registration; microG support; how often phones run with Data Saver on.
**Cost of (a):** Google learns the sender-to-recipient graph for rings. This must be disclosed in §9.

### O2. A second offline mailbox provider beside Nostr (G1)
- **(a) Session storage swarms, private namespaces.** Best technical fit. Terms silent on other apps. Live outbox replace not guaranteed. Protocol unstable. Malware-IOC reputation. Needs our own client. Low confidence.
- **(b) Veilid DHT.** Weak retention (republish or rehydration). Android foreground or job only. Pre-1.0 wire breaks, which conflict with D28. **Requires accepting MPL-2.0 (D29 / principle 9).** Facts medium, fit low.
- **(c) chatmail relays.** Only with operator consent. Low.
- **(d) Google Drive or a published Sheet as an opt-in layer.** Capped at about 87k to 400k devices on the developer's project, charges coming. Writers need Google accounts. Not available in foss builds. Low.
- **(e) None.** Rely on member devices, helpers and your own always-on machine, and accept D13 as is.

**Recommendation:** run P3 before choosing. No option is confident today.
**Not verified:** Session operator stance; Veilid network size and capacity; real-world eviction cause in Veilid.

### O3. Path to internet P2P and relaying without n0 (G2, G4)
- **(a) Helper-hosted iroh relays with LE IP certificates.** Same stack. Public-IPv4 helpers only until the IPv6 bug is fixed. Medium.
- **(b) IPv6 / UPnP direct plus our own signaling.** Cheapest option. Value depends on the unmeasured IPv6 share. Medium, lab-gated.
- **(c) iroh over a custom transport** (Veilid, HyperDHT, libp2p or ICE as the first path). Unstable API. Low, lab only.
- **(d) A parallel rust-libp2p stack using public Kubo relays with DCUtR, plus libp2p relays on helpers.** Second stack. Weak on phones. Medium.
- **(e) HyperDHT blind relays on reachable helpers.** Low-medium.

**Recommendation:** (a) + (b) first, since both run in parallel with n0. Pick among (c), (d) and (e) only after the P2 lab results.
**Not verified:** share of qualifying helpers; phone-to-phone success on carrier CGNAT for any option.

### O4. One consistent rule for "best-effort" and "testing-only" public services
The same ethical and Play-policy question applies to n0 relays, Kubo relays (implied permission), IPFS bootstrap, Google and Twilio STUN, Metered Open Relay, Send instances and chatmail.
- **(a) Use only services with written permission for any app**, which today means essentially Cloudflare STUN, plus Nostr and DHTs as open protocols.
- **(b) Use best-effort services as non-critical parallel layers** with health tracking and graceful degradation, never counted toward principle 2.
- **(c) Ask operators in writing first** (outward contact).

Verification already changed one verdict (MQTT brokers → REJECT) on exactly this ground.

### O5. Should default-on helpers (D20) also take the public-server roles?
Candidate roles: iroh relay with LE certificate, libp2p relay, HyperDHT blind relay, AutoNAT server.
- **(a) Default-on, like storage helping.** Maximizes G4 coverage.
- **(b) A separate opt-in "relay" level.** Avoids surprising users with ISP-AUP, abuse-complaint and firewall-prompt exposure.
- **(c) Relay only for the helper's own groups** (member relaying), and never for strangers.

**Not verified:** how ISPs treat residential relays at this traffic level.

### O6. Large content (G6, D22)
- **(a) Keep postponing** until iroh-blobs is stable.
- **(b) Drive blob store**, opt-in per uploader.
- **(c) Send instances as overflow.**
- **(d) Helper and member desktops only.**

(b) is medium confidence and (c) is low-medium. Both were verified only on paper; P4 and P5 would test them.

### O7. Licensing (D29 interaction)
Adopting Veilid forces D29(a), accepting MPL-2.0. Every other candidate can stay permissive:
- Session and chatmail: write our own clients against the MIT/standard protocols.
- The FCM distributor: reimplement it cleanly.
- libp2p: TCP-only avoids the aws-lc-rs C dependency.

Decide whether a mailbox layer justifies MPL. Loro already raises the same question.

### O8. Metadata wording (extends D23)
Each adopted layer adds an observer:
- Google, for rings
- STUN operators
- DHT nodes, for records
- tracker operators, if trackers are used
- cloud providers, for Drive
- Tor visibility, for Arti

Decide which of these are acceptable, and update §9 before any of them ships.
