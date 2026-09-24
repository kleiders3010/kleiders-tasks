# R5: Prior art and their failures

*Research dimension R5. Date: 2026-09-24. Method: official docs, source code, specs, measurement papers, issue trackers and forum posts. Research only: nothing was installed, built or sent to public infrastructure beyond normal page fetches.*

*Caveats: the session's web-search budget ran out partway through, so later evidence comes from fetching known URLs directly. Sources marked **[snippet]** were seen only as search-result excerpts, not fetched in full. Two points are **unverified**: how Keet sends mobile push notifications, and the classic BitTorrent swarm-death statistics (Kaune et al., 2010). Neither is relied on for a verdict.*

---

## Summary

- **No prior system runs an open, volunteer-run storage or mailbox network with reputation that serves offline mobile users at scale.** Every system that delivers to offline phones depends on one of three things: operator-run always-on nodes (Keet blind peers, Anytype nodes, Session staked nodes, SimpleX, Delta Chat chatmail), a push server run by the project (Jami's DHT proxy, Delta Chat's notification proxy, Session's push server), or a persistent foreground service (SimpleX Android, Bitchat, Syncthing-Android). §7a's helper network would be the first of its kind. Treat it as research, not as the answer to Nostr acceptance risk.
- **Volunteer participation is 1 to 3 orders of magnitude below what §7a assumes.** §7a assumes ~1,000 active helpers per 10k users (10%). Syncthing has **~100** relays (counted 2026-09-24) for **~1,000,000** active devices (2024): about 0.01%. Tor has ~8,000 relays for ~2.5M daily users (~0.3%), and Tor actively recruits operators and funds relay associations. On Gnutella, 70% of users shared nothing and 1% served 50% of responses. **Below ~1% participation, the helper network cannot meet its own r = 12–30 target at 10k users.**
- **The availability formula assumes helpers fail independently. Real churn is diurnal and correlated.** Mainline DHT shows a "very clear daily churn pattern", and half of its sessions last under 200 minutes. §7a spreads trust through people, which pulls helpers toward the same social circle and time zone as the group. Those helpers sleep when the group's own desktops sleep, which is exactly when the mailbox is needed.
- **Mainline DHT / BEP44 is fine for per-device endpoint records but cannot serve as a helper directory.** A BEP44 record holds at most 1,000 bytes, one per (key, salt), and may expire after 2 h. BEP43 says battery- and NAT-limited devices (phones) should be read-only nodes. About 300k Sybil nodes were measured in 2012, and BEP42's ID-to-IP binding is not mandatory. iroh's DHT lookup is **not enabled by default**. By default iroh discovers peers through **n0's hosted DNS/pkarr server**.
- **The "free public service" pattern keeps ending in "not for production".** n0 says its public iroh relays are "suitable for development and testing", rate-limited, with no guarantees, and points production users to paid plans. Automerge's public sync server has "no reliability or data safety guarantees". Magic Wormhole's server is free "until volume or abuse makes it infeasible". libp2p v1 open relays "proved to be an expensive proposition". v2 now cuts each relayed connection at **128 KiB / 2 min**, so **IPFS circuit relays are not a usable second relay provider** (§6 layer 7, §15).
- **Delta Chat is the closest precedent to building on Nostr, and it had to walk away from free third-party infrastructure.** Classic email providers imposed sending limits, spam rejection and OAuth "bureaucracy". Delta Chat built chatmail relays and a **central push proxy** (notifications.delta.chat). This confirms §7's "acceptance is the real long-term risk" and argues against deferring mitigation to a later phase.
- **Keet (Holepunch, Tether-funded) is the closest architectural cousin, and it moved from peers-only to operator-run blind peers.** Early advice was to keep your desktop Keet open and bookmark the room (this matches our tier 1). The current system uses blind peers: a configured list of always-on servers with a 100 GB default disk budget, reached through fixed bootstrap nodes (node1–3.hyperdht.org). Keet invites also need a member online to accept them.
- **Open volunteer networks attract patient Sybils.** KAX17 ran up to 900+ Tor relays from 2017 onward, with a 16% chance of being a user's guard. Tor removed ~1,000 relays tied to a crypto-incentive scheme in 2023. §7a's per-group reputation protects data integrity. It does **not** protect metadata: a helper that behaves well for years and collects which IPs share groups is *rewarded* with more groups.
- **Android lesson from Syncthing: sync apps die of Play policy churn and maintainer burnout, not of protocol bugs.** The official app was retired in Dec 2024 after repeated Play rejections over storage access ("our time again for a random denial"). Its fork's maintainer then vanished in 2025, leaving signing-key custody unclear. Manyverse's lead developer stopped in April 2024. "Backward compatible forever" needs a bus-factor and key-custody plan.
- **What holds up:** RAM-only, evictable helpers with full copies on every device (the Freenet/Hyphanet lesson is already absorbed); compaction and budgets (SSB's permanence problem is avoided); long offline-safe invite codes (consistent with Magic Wormhole's PAKE math); member devices and your own always-on machine as mailboxes (Briar Mailbox, Keet's early advice, Ink & Switch's "cloud peer"). NIP-78 (kind 30078) explicitly allows non-Nostr apps to use relays "as personal databases".

---

## Claim-by-claim table

| # | Claim (from DESIGN.md) | Verdict | Severity if wrong | Key evidence (short) | Proposed fix |
|---|---|---|---|---|---|
| P1 | Mainline DHT (pkarr / BEP44) is the decentralized discovery layer for group peers, and helpers "announce themselves on the DHT" (§6, §7a) | **Partial** | **High** (helper part); Low (endpoint part) | BEP44: ≤1,000 B, "MAY expire in 2 hours", one item per (key, salt); BEP43: phones should be read-only; iroh DHT lookup off by default, "slower than DNS"; ~300k Sybils (2012); BEP42 optional | Use the DHT only for per-device endpoint records. Build helper discovery as its own overlay or as gossip inside groups. Enable DHT lookup explicitly and keep a second discovery channel. |
| P2 | Volunteers will supply ~1,000 active helpers per 10k users; helping is like torrent seeding (§7a estimate) | **Refuted** (no precedent) | **High** | Syncthing ~100 relays / ~1M devices; Tor ~8k relays / ~2.5M users; Gnutella 70% share nothing, 1% serve 50% | Size for 0.01–1%. Make helping default-on on desktop only, plus dedicated headless helpers. Treat the helper network as supplementary. |
| P3 | r ≥ ln(0.001)/ln(1−p) gives ~99.9% availability (§7a) | **Refuted** as stated (assumes independence) | **Medium-High** | Mainline DHT daily churn pattern; half of sessions < 200 min; trust spreads through people, so helpers share time zones | Model correlated availability. Select helpers for time-zone and ASN diversity. Measure hour-of-day availability per helper. |
| P4 | Hypercore/Keet show serverless P2P can serve offline mobile users (implied precedent for §7a) | **Refuted** as a precedent for open volunteers; **confirms** tier 1 | **High** | Blind peers are always-on servers picked from a configured key list (`DEFAULT_BLIND_PEER_KEYS`, 100 GB default); earlier advice was to leave your desktop Keet on; fixed bootstrap nodes; pairing needs an online member | Keep tier 1 (member devices) as the main answer. Promote the headless member node. Treat tier 2 as an experiment. |
| P5 | Syncthing shows free, volunteer-run infrastructure works at scale (§2 principle 2, §7a analogy) | **Partial** | **Medium** | Relays are forward-only, rate-limited, used as a last resort, ~100 of them, flat since 2018 (112). Discovery is run by the project: 3 servers, ~15 TB/month, "well over 1000 euros per month" at cloud egress prices | Use Syncthing relays as the model for *forwarding*, not for *storage*. Don't count on volunteers scaling with users. |
| P6 | Android can host a background P2P sync app within Play rules (§2 principle 6, §8) | **Partial** (holds for our narrow use; Syncthing shows the failure modes) | **Medium** | Syncthing-Android retired Dec 2024 ("Google making Play publishing something between hard and impossible and no active maintenance"); repeated storage-access denials; fork maintainer disappeared Dec 2025 | No sensitive permissions. Keep phone-helper mode out of the first Play release. Plan key custody and succession. |
| P7 | Volunteer always-on mirrors plus invite codes give smooth onboarding (SSB pubs vs our helpers and invites) | **Confirmed lesson**: the design avoids the SSB traps, but the helper network recreates pub economics | **Medium** | Pubs overloaded ("could easily overload with data from thousands of people"), private invites, GB-scale initial sync; rooms stopped storing data; lead developer quit in April 2024 | Keep invites independent of helpers (already so). Per-group quotas and PoW are necessary. Expect helper operators to throttle. |
| P8 | Member devices, including your own always-on machine, make the best mailboxes (§7a trust tier 1) | **Confirmed** | **Low** | Briar Mailbox: spare phone on power and Wi-Fi, asks to ignore battery optimizations; Keet's "connect your always-on desktop"; Ink & Switch "cloud peer" | Raise the headless/NAS member build from "optional, low priority" to a planned phase. Nudge desktop users to stay online. |
| P9 | Local-first sync at scale is possible without hosted nodes (§1 C2, §13) | **Unsupported by precedent** (Anytype chose company nodes plus freemium) | **Medium** (strategic) | Anytype: "Remote backup nodes operated by Anytype" by default; P2P only on the local network; file quota | None required. Accept that we trade reliability for C2 and state it honestly. Keep Nostr redundancy high. |
| P10 | Free third-party transport (Nostr) is fine for now; helpers later address acceptance (§7, §7a "When") | **Partial**: the risk is real and arrives earlier than "at scale" | **High** | Delta Chat: classic providers have "sending limits and overzealous spam rejections"; dropped Google OAuth over "bureaucracy"; had to build chatmail and a central push proxy | Use NIP-78 kind 30078. Rate-limit and back off with jitter. Update relay lists without app releases. Build acceptance monitoring before launch. |
| P11 | 6-word codes are safe without anyone online; short PAKE codes rejected (§5) | **Confirmed** (consistent with Magic Wormhole) | **Low** | Wormhole: 2 words work only because PAKE gives "1-in-65536" per online attempt; needs both parties online and one rendezvous server that is run "until volume or abuse makes it infeasible" | Keep. Also let an online inviter device answer invite lookups directly, as a parallel channel. |
| P12 | iroh public relays are a usable always-on fallback (§6 layer 7) | **Refuted** for production use | **High** | n0: public relays "suitable for development and testing", "Rate-limit traffic", "no uptime or performance guarantees"; production means Pro or Dedicated plans or self-hosting | Treat layer 7 as best-effort only. Get n0's written consent for production use or drop the dependency. Member relaying (layer 6) and Nostr carry the load. |
| P13 | libp2p circuit relays on IPFS nodes could be a second relay provider (§6, §15) | **Refuted** | **Medium** | circuit-v2 defaults: 2 min and 128 KiB per direction, 16 circuits; built for hole-punch coordination; v1 open relays "proved to be an expensive proposition" | Remove it from §6 and §15. Don't add libp2p (binary size, no benefit). |
| P14 | Per-group reputation makes helper misbehavior harmless; eclipse attacks buy little (§7a) | **Partial**: data integrity holds; metadata and bootstrap eclipse do not | **High** | KAX17: ~900 relays, since 2017, 16% guard probability; I2P: 8–15 floodfills clustered in keyspace can capture a key; IPFS DHT censorship "with minimal effort and cost" (NDSS 2024) | Diversity caps (ASN, /16, operator). Derive the bootstrap position from a secret-only label. Unlinkable slot IDs. Cap any single helper's share of groups. |
| P15 | RAM-only helpers with eviction are acceptable because every device has full data (§7a) | **Confirmed** | **Low** | Hyphanet: "Documents that are never requested are eventually removed through disuse"; persistence only by reinsertion. Our slots are "unpopular" by nature, but members republish. | Keep. Monitor how often slots are found missing. |
| P16 | A serverless mobile app can deliver timely change notifications with WorkManager only, with no push and no persistent service (§6 doorbell, §8) | **Partial**: fine for sync, weak for "Notify me when others change a list" (default on) | **Medium** | Jami: "push notifications are the only way to notify mobile users while the app is not in the foreground"; Session and Delta Chat run push servers; SimpleX and Bitchat use a foreground service | Keep the honest wording. Reconsider notification defaults. Design the doorbell as a Nostr- or helper-side pusher via UnifiedPush early. |

---

## Details per claim

### P1. Mainline DHT / BEP44 as the discovery layer (partial, high for helpers)

**What the design says.** Peers find each other through iroh discovery (DNS / pkarr records on Mainline DHT) under a key derived from the group (§6). Helpers "announce themselves on the DHT", and first-time joiners use "the *r* helpers closest to a daily-rotated hash of the group ID (XOR distance)" (§7a).

**Evidence.**
- BEP44 limits (spec text): "Storing nodes MAY reject put requests where the bencoded form of v is longer than 1000 bytes", "it's not safe to assume storing more than 1000 bytes will succeed", and "Without re-announcement, these items MAY expire in 2 hours… they SHOULD be re-announced once an hour." A mutable item is addressed by (public key, salt ≤ 64 B), so readers must already know both. pkarr's README says it outright: "PKARR is for discovery, not storage".
- One record per (key, salt). iroh's own DHT-discovery notes call this a "fundamental bottleneck for multi-peer discovery", and records expire "after 1 to 2 hours". A group-derived keypair gives every member the same signing key, so members overwrite each other's record, and anyone holding the group secret can forge any member's record.
- Phones are poor DHT citizens. BEP43 recommends read-only mode for devices "behind a restrictive NAT", on "a fixed data plan", or "where traffic adversely affects battery life". Read-only nodes are not added to routing tables. HyperDHT, the closest analog overlay, needs "~30 mins for the node to become persistent". A phone helper therefore cannot be a routable DHT node.
- Sybils and eclipse. Wang & Kangasharju (GLOBECOM 2012) measured about 300,000 Sybil nodes in Mainline DHT and "horizontal and vertical" routing-table attacks "happening in the real network". BEP42 ties node IDs to IPs, but "Enforcement is not mandatory", and attackers with large IP blocks are not stopped.
- iroh defaults. DHT lookup "is not enabled by default. It lives in the separate `iroh-mainline-address-lookup` crate", and it comes "at the cost of slower lookups than DNS". The default is the n0-run pkarr publisher and DNS server (dns.iroh.link). Principle 2 asks for "at least two independent providers" per role. By default, discovery has one hosted provider.

**Mapping.** The DHT works for **per-device endpoint records** keyed by the device's own key. It is weak for **group-level rendezvous** and unusable as a **helper directory or XOR-closest selection**. Mainline has no primitive that says "list the helpers near key X". A get_peers/announce_peer (BEP5) infohash returns only IP:port and would expose every helper's IP to everyone. "Closest helpers to H(group)" needs an overlay in which helpers are the routing nodes.

**Fix.**
1. Per-device pkarr records, published under each device's own key (as iroh already does). The group doc carries device IDs, and for first join the invite payload includes the inviter's device IDs.
2. Enable `iroh-mainline-address-lookup` **and** keep n0 DNS and Nostr as parallel discovery channels. That satisfies principle 2.
3. Helper discovery options:
   - (a) a desktop-only helper overlay (Kademlia among helpers, IDs bound to an IP prefix as in BEP42, bootstrapped from several sources);
   - (b) helpers publish signed self-descriptions to Nostr (a kind for helper ads) and clients sample from them;
   - (c) groups learn helpers only through members' gossip.
   Pick one before building §7a. The design currently hand-waves it.

### P2. Enough volunteers (refuted as an assumption, high)

**Evidence.**

| System | Volunteers | Users | Ratio | Notes |
|---|---|---|---|---|
| Syncthing relays | **100** (pool endpoint, 2026-09-24); 112 in Apr 2018 | ~1,000,000 active devices (Sep 2024) | ~0.01% | Forward-only, no storage, rate-limited, a "last resort" |
| Tor relays | ~8,000 (mid-2025) [snippet, secondary sources] | ~2.5M daily users [snippet] | ~0.3% | Active recruitment, funded relay associations, EFF challenge |
| Gnutella (2000) | 30% shared anything | – | 1% of hosts served ~50% of responses | Classic free-riding result |
| Keet | Blind peers are operator-run servers, not volunteers | – | – | See P4 |

**Load math** (the design's own numbers: 5k groups × 50 KB × r 12–30 ≈ 3–7.5 GB network-wide at 10k users):

| Participation | Helpers at 10k users | Per-helper RAM | Feasible? |
|---|---|---|---|
| 10% (design) | 1,000 | 3–7.5 MB | Yes |
| 1% | 100 | 30–75 MB | Desktops only (phones are capped at 16–32 MB) |
| 0.1% | 10 | 300–750 MB, and r > 10 cannot be met with "one copy per group per helper" | **No** |
| 0.01% (Syncthing ratio) | 1 | – | **No** |

At 1M users and 1% participation: 500k groups × 50 KB × 12–30 ≈ 300–750 GB spread over 10k helpers, or 30–75 MB each. That's feasible only on desktops.

**Fix.**
- Size for 0.1–1%, not 10%.
- Default "Help others sync" **on for desktop** (the torrent-seeding analogy only holds with default-on) and off on phones.
- Lower r by accepting Nostr as the co-primary mailbox.
- Add an explicit, visible "dedicated helper" mode for enthusiasts: a headless build that runs 24/7, the Syncthing-relay-operator equivalent.

### P3. Independence assumption in r (refuted, medium-high)

**Evidence.**
- Mainline DHT daily users varied "between 15 and 27 million per day with a very clear daily churn pattern". About half of sessions last under 200 minutes, and 3% exceed 9,000 minutes (Wang & Kangasharju measurement).
- §7a explicitly spreads helper trust "through people", which yields helpers with the same geography and habits as the group.
- The failure the mailbox exists for is "author edits on a phone, then everyone else is offline". That tends to happen at night or on weekends, when home desktops (helpers) are also off.

**Fix.**
- Compute r from *conditional* availability, meaning P(helper up | group's own desktops down), using per-helper hour-of-week uptime profiles in the reliability records.
- Add a diversity rule: a helper set must span ≥ 3 UTC-offset buckets and ≥ N ASNs.
- Simulate before building (experiment X1).

### P4. Keet / Hypercore / Pear (refuted as precedent for open volunteers; supports tier 1)

**Evidence.**
- Blind peers are "a dedicated replicator that stores and serves Hypercores without decrypting". They are "always-on", and "Blind peering is one way of putting the server in serverless: always-on reachability like a hosted service". Clients are pointed at explicit keys (`BLIND_PEER_KEYS = [...]`, `DEFAULT_BLIND_PEER_KEYS`). The client picks peers by XOR proximity among *configured* peers (default `pick` 2) and prefers different "groups".
- The server side has a "default disk budget [of] 100 GB" and a `--trusted-peer` flag to control who may announce. This is operator infrastructure with admission control, not an open pool. Who operates Keet's default blind peers is not documented publicly. Given the configured-key model, it is almost certainly Holepunch.
- Before blind peers, Keet's official advice (2023) was: "until this launches here is how to keep data availability high for Keet Mobile chats 1. Connect to the same room with your always-on desktop Keet App 2. Bookmark the room 3. Ask your friends to do the same" [snippet, @keet_io on X]. That is exactly our tier 1, and Keet judged it insufficient.
- HyperDHT bootstraps from fixed hosts node1/2/3.hyperdht.org.
- Blind pairing (invites) requires a member online to process the candidate (`onadd` callback, then `candidate.confirm`).
- Keet mobile has push notifications. Its changelog lists fixes such as "coordinating the push handler with a lockfile" (v4.3.1/v4.5.1). **Unverified:** who sends the pushes (docs and privacy statement are silent). Any FCM or APNs push requires a sender with credentials.

**Fix.**
- Keep tier 1 as the backbone.
- Plan the headless member node early (P8).
- Label §7a tier 2 as experimental.

### P5. Syncthing infrastructure (partial, medium)

**Evidence.**
- The relay pool is community-run. The pool server tests each relay's reachability on join (`client.TestRelay(..., 3)` retries), evicts relays after `evictionTime = time.Hour` without re-announcement, keeps a permanent-relays list, returns HTTP 429 when the 64-deep join queue is full, and blocks IPs after >10 failed joins (source: `cmd/infra/strelaypoolsrv/main.go`).
- Relays are forward-only and E2E encrypted. The relay "knows your IP and device ID" and "can see the amount of traffic". Operators set `-global-rate` and `-per-session-rate`.
- Capacity (2018 thread, fetched): 112 relays, ~23,000 sessions, ~900 Mbps in use against ~10 Gbps estimated capacity, with ~16% of connections relayed. Developer Jakob Borg: "traffic will grow to swallow all available bandwidth". Honest statistics and "rogue relays" were a concern for relay selection.
- Global discovery is **run by the project**. The Sept 2024 infrastructure report: active devices went from ~500k to "almost 1,000,000"; 3,000–4,000 requests/s; 3 replicated instances; ~15 TB/month; egress on major clouds would be "well over 1000 euros per month". Also: "Client retries create cascading effects lasting hours after outages."

**Mapping.**
- Volunteer relaying works when it is (a) forward-only, (b) a last resort, (c) rate-limited by the operator, and (d) backed by a project-run discovery service. The relay count stayed flat while users doubled.
- Our helpers *store* data. That is a heavier ask than Syncthing relays and has no equivalent volunteer base.
- Our Nostr and iroh dependencies play the role of Syncthing's paid discovery servers, except we don't pay, so we have no control.

### P6. Android lessons from Syncthing (partial, medium)

**Evidence.**
- Official retirement notice (2024-10-20): "Google making Play publishing something between hard and impossible and no active maintenance". Jakob Borg: revival requires "someone to step up and put in long term effort"; the app had been on "life support for a long time".
- Issue #2064 (2024-02-27): "It's our time again for a random denial of the access to device storage by google play". The repository was archived 2024-12-03.
- Syncthing-Fork (heise, 2025-12-08): the maintainer became unreachable, the repository reappeared under a new account, and "it is unclear whether the new account has access to the signing keys of the original app".
- Community workarounds for Doze involved Termux plus Tasker.

**Mapping.**
- Our app keeps data in app-private storage, so the specific storage-permission trap doesn't apply.
- The general pattern does: repeated review churn, volunteer maintenance, and signing-key custody.
- Phone helper mode (back-to-back WorkManager chains, §7a) is the part most likely to attract a Play review question ("why is this app running constantly while charging?").

**Fix.**
- Ship v1 without phone-helper mode.
- Use Play App Signing plus an organization developer account with a documented successor.
- Escrow the desktop release-manifest signing key, which §2.8 makes security-critical.
- Aim for reproducible builds so F-Droid can verify.

### P7. SSB / Manyverse (lessons confirmed; the design mostly avoids them)

**Evidence.**
- Pubs: "pubs were never intended for discovery and onboarding of strangers. The pub concept is meant as an always-online mirror of data for a relatively tight-knit community". With public invite codes, "the server could easily overload with data from thousands of people", so operators moved to private codes. New users faced "too much data".
- Rooms (2021) "do not store any user data" and only connect online peers.
- Initial sync pulled gigabytes and indexing took hours [snippet].
- Staltz's retrospective: the permanence of the append-only log had to give way ("deletes… must play an important role").
- Staltz ended his work on SSB, Manyverse and PPPPP on 2024-04-05 ("My last update").

**Mapping.**
- Our invite codes don't need a pub (good).
- Loro compaction and budgets avoid permanence bloat (good).
- The **helper network is structurally a pub network**: volunteer always-on mirrors for strangers' groups. SSB pub operators throttled and closed when load came from strangers. Expect the same (§7a quotas and PoW are necessary, not optional).
- A single funded lead developer is a single point of failure for the "forever" promise.

### P8. Member devices and your own always-on device as mailbox (confirmed, low)

**Evidence.**
- Briar Mailbox: "install the Mailbox app on a spare Android device… leave the Mailbox device connected to power and Wi-Fi". One mailbox per user, reached over Tor. The app asks for permission to ignore battery optimizations [snippet, F-Droid listing].
- Ink & Switch found "NAT traversal… is unreliable", and peers cannot connect if one "closes their laptop lid before the other user has connected". Their answer is a cloud peer that can "store a copy of the document, and forward it to other peers when they come online".

**Fix.**
- Promote the headless engine build ("own always-on server joins as member", §7a / §16.7) to a planned phase right after P2P.
- Offer "keep this PC as your lists' mailbox" wording in the desktop app.
- Consider an "old phone on a charger" mode that is a *member* (tier 1), not a helper. That is Briar Mailbox, but trusted, because it's yours.

### P9. Anytype any-sync (precedent contradicts C2; strategic, medium)

**Evidence.**
- Default sync is "Remote backup nodes operated by Anytype". Anytype acts as a "blind orchestrator". There are four node types: sync, file, consensus and coordinator.
- P2P is local-network only.
- Free storage covers files; above quota, "new files stop syncing".

**Mapping.** The best-funded local-first product with mobile users chose company nodes plus freemium. Our C1/C2 constraints rule that out, so we must accept lower worst-case reliability and say so.

**Fix.** No design change. Keep Nostr redundancy (5–8 relays) and make the "up to date" state honest (UX-PRINCIPLES).

### P10. Free third-party transport and acceptance risk (partial, high)

**Evidence (Delta Chat).**
- Classic providers "have sending limits and overzealous spam rejections"; push "does not work with classic providers". Google OAuth support was discontinued because of "Google adding more and more bureaucracy in that area".
- Result: Delta Chat built **chatmail relays** ("cheap and dumb servers"), added multi-relay support, and runs a **central notification proxy** at notifications.delta.chat.
- Even with FCM, some Samsung phones woke the app but couldn't get network access (issue #3281, 2024).

**Mapping.** Email was "free public infrastructure run by others" that tolerated app traffic until it didn't. Nostr relays are the same class of dependency. §7 already names acceptance as "the real long-term risk". Prior art says it bites before large scale: per-IP and per-pubkey rate limits and kind allow-lists apply from day one.

**Fix.**
- Publish as **NIP-78 kind 30078** (the NIP lists "Allowing apps unrelated to Nostr to leverage Nostr relays as personal databases" as a use case), so relays that allow-list kinds are more likely to accept us.
- Use exponential backoff with jitter on every relay write.
- Treat the relay list as **data**: signed by our key, refreshable from GitHub Pages or a Nostr event, and gossiped through group documents, not only shipped in app releases (see new risk N2).
- Add an acceptance dashboard to diagnostics before launch.

### P11. Invite codes vs Magic Wormhole (confirmed, low)

**Evidence.**
- Wormhole: "For each use of the tool, an attacker gets a 1-in-65536 chance of success". This is safe only because PAKE allows one online guess. The default servers are "freely available until volume or abuse makes it infeasible to support". A 2021 failure showed the relay out of disk ("Nospaceleftondevice") [snippet].
- Keet's blind pairing and SSB pub invites both need an online counterpart.

**Mapping.** Our offline invites trade PAKE for ~77 bits of entropy. That is consistent with this math (R7 and E7 cover the numbers). Our only critical dependency is that at least one of 5–8 relays keeps the invite event for 24 h.

**Fix.** Keep the design. Add a parallel path: an online inviter device (or any member device) answers "invite lookup tag" queries over iroh. This is principle 4 applied to invites.

### P12. Public test services are not production infrastructure (refuted for iroh relays, high)

**Evidence.**
- iroh docs: "Public relays are suitable for development and testing"; they "Rate-limit traffic to prevent abuse" and "Carry no uptime or performance guarantees"; production means Shared/Dedicated/Enterprise plans or self-hosting.
- Automerge: "as an experimental service has no reliability or data safety guarantees… run your own sync server for production apps."

**Mapping.** §6 layer 7 plans to rely on n0 public relays in production at any scale. That conflicts with the provider's stated intended use, and principle 2 (two providers) is unmet for relays. Sync still works via Nostr thanks to principle 4, but "live" collaboration for the ~10% of pairs that can't hole-punch degrades.

**Fix.**
- Ask n0 in writing whether a free, open-source, non-commercial app may use public relays in production, and at what volume.
- Make member relaying (layer 6) and desktop members the primary live fallback.
- Document layer 7 as best-effort.
- (R1 should confirm the current n0 relay policy text.)

### P13. IPFS circuit relays as second relay provider (refuted, medium)

**Evidence.**
- go-libp2p relay v2 defaults: `ReservationTTL: time.Hour`, `MaxReservations: 128`, `MaxCircuits: 16`, `Duration: 2 * time.Minute`, `Data: 1 << 17 // 128K`.
- Spec: v1 had "continuous over-subscription of relays", and "running a relay proved to be an expensive proposition requiring dedicated hosts". v2 is built for hole-punch coordination.
- Other IPFS availability lessons: provider records expire after 48 h and must be republished every 22 h (kubo #9389 [snippet]). Free infrastructure disappears: Cloudflare moved its public gateway traffic to the IPFS Foundation in May 2024, and NFT.Storage Classic stopped accepting uploads on 2024-06-30 [snippets]. The IPFS DHT can be censored "with minimal effort and cost" (NDSS 2024).

**Fix.** Delete the "evaluate libp2p circuit relays" items (§6 layer 7, §15). They would add a large libp2p dependency for a 128 KiB / 2-min pipe.

### P14. Reputation vs Tor/I2P-style adversaries (partial, high)

**Evidence.**
- KAX17: "operated since at least 2017, peaking at over 900 malicious servers", with a "16% chance of connecting through KAX17 guard relays, 35%… middle". It evaded detection by omitting contact info and spreading over "50+ autonomous systems". Tor removed batches in 2020 and 2021.
- Tor (2023-11-20) removed relays tied to a crypto-token scheme: "such financial schemes present a significant threat to the network's integrity".
- I2P: "creating a small number (8-15) of floodfill routers clustered closely in the keyspace" lets an attacker capture a key's lookups [snippet, I2P netdb docs]. In February 2023, attacker floodfills accepted and then dropped requests, and tunnel builds collapsed [snippet].
- Cwtch (untrusted group servers): "Current proof-of-work protections are insufficient against resourced adversaries."

**Mapping.**
- *Integrity*: fine. Signatures, CRDT merges and multi-channel delivery make dropping or rollback low-impact, as the design argues.
- *Metadata*: a well-behaved Sybil operator gains rank, advertises capacity, and by design receives **more** groups. It observes which IPs read and write which slots, and therefore which IPs share a group. That is a social graph. §9 accepts "the uploader's IP" per operator, not the aggregation across many Sybil helpers.
- *Bootstrap eclipse*: the bootstrap set is "the r helpers closest to a daily-rotated hash of the group ID". If that hash input is observable (the Nostr messages carry a "group ID hash"), an attacker can precompute tomorrow's position and mint helper IDs near it. Cost is about 2^k keypair generations for a k-bit prefix, which is cheap. The joiner still has Nostr, so the result is denial of service or metadata, not corruption.

**Fix.**
- Derive the bootstrap position from a **secret-only HKDF label** that never appears on the wire.
- Bind helper IDs to IP prefixes, BEP42-style.
- Per group: at most one helper per /16 (IPv4) or /48 (IPv6) and per ASN.
- Cap any helper's share of the network's groups, which stops capacity from buying reach.
- Use unlinkable per-slot identifiers so one helper can't tell that two slots belong to the same group.
- Consider fetching helper data via an iroh relay hop to hide reader IPs. That costs latency but blunts IP harvesting.

### P15. RAM-only helpers and data loss (confirmed, low)

Hyphanet: "Documents that are never requested are eventually removed through disuse"; content stays only if popular or reinserted (KeepAlive plugin). Our slots are unpopular by definition, but members refresh them and hold full copies, and §7a states the only loss case. The design has already absorbed this lesson.

### P16. Mobile notifications without push or a persistent service (partial, medium)

**Evidence.**
- Jami: "Android and iOS prevent apps from staying active in the background for too long". Their DHT proxy "sends a push notification through Apple's or Google's notification services". Users reported "DHT works sometimes but often completely fails—messages never delivered" (2021).
- Session's "fast mode" exposes the Account ID and push token "to an STF operated push notification server"; its slow mode polls.
- SimpleX Android runs a background service costing "a few percents of battery per day".
- Bitchat: "A foreground service keeps the mesh alive" [snippet].
- Delta Chat: push through a central proxy.

**Mapping.** §8's WorkManager-only approach is right for *sync* of a task list. The default-on "Notify me when others change a list" will arrive 15 minutes to hours late on idle phones. For shared groceries ("add milk"), that is exactly when users notice.

**Fix.**
- Keep the honesty text (§8).
- Consider making change notifications default-on only for lists the user interacted with recently.
- Design the doorbell early as "any online member device or helper posts a content-free UnifiedPush/FCM ping". Nostr-side push services exist in the ecosystem. Evaluate them in R2 or R4.

---

## New risks (not in DESIGN.md)

| # | Risk | Severity | Evidence | Fix |
|---|---|---|---|---|
| N1 | **Hosted defaults inside "serverless" stacks.** iroh discovery defaults to n0's DNS/pkarr server; iroh public relays are for dev/test; HyperDHT, and Mainline's usual bootstrap routers, are fixed hosts run by companies. Principle 2 ("two independent providers per role") is unmet by default for discovery and relays. | High | iroh docs; HyperDHT README; pkarr | Audit every default endpoint. Enable DHT lookup. Cache bootstrap nodes from past sessions. List each role's two providers in the design. |
| N2 | **Stale relay lists in old installs.** "Forever backward compatible" means 5-year-old installs keep running, but the relay list only updates with app releases. Free services disappear (Cloudflare IPFS gateway handoff, NFT.Storage Classic, Wormhole's "until… infeasible"). | High (over years) | P10, P13 sources | Signed relay-list updates fetched from GitHub Pages and a Nostr event. Gossip working relays through group docs. Never rely on the shipped list alone. |
| N3 | **Retry and republish storms.** After a relay outage, every client reconnects and re-uploads snapshots ("A device that finds a relay missing data re-uploads it"). Syncthing saw "cascading effects lasting hours after outages". Storms look like abuse and trigger bans. | Medium | Syncthing infra report | Full-jitter exponential backoff. Stagger republishing by a hash of the device key. Cap re-uploads per relay per hour. |
| N4 | **Metadata harvesting by patient Sybil helpers**, rewarded by reputation and capacity weighting (KAX17 pattern). | High | P14 | Diversity caps, share caps, unlinkable slot IDs, optional relay-hop reads. State the residual risk in §9. |
| N5 | **Bootstrap eclipse through an observable group-ID hash.** | Medium | I2P netdb; IPFS NDSS 2024; BEP42 optional | Secret-only derivation, IP-bound helper IDs, Nostr in parallel (already there). |
| N6 | **Bus factor and signing-key custody** threaten the "forever" promise (Syncthing-Android, Syncthing-Fork 2025, Manyverse 2024). | Medium-High | heise 2025; Syncthing forum; Open Collective | Organization accounts, key escrow, succession doc, reproducible builds, open-source the engine early (§14). |
| N7 | **Play review churn for sync-class behavior** (recurring denials for Syncthing). Phone-helper chains may read as background abuse. | Medium | Syncthing #2064 | Keep phone-helper mode out of the Play flavor until reviewed. Minimal permissions (R8). |
| N8 | **Helpers as a free covert channel.** A small-message, encrypted, replicated, anonymous mailbox is what malware command-and-control wants. It creates legal and reputational risk for helper operators and possible antivirus flagging of the desktop app. (DHT-based botnets are widely reported; not re-verified in this pass.) | Medium | Cwtch PoW note; general | Short TTLs, per-source write quotas, only slots whose writes carry a group-membership proof verifiable by the helper (e.g., a signature by a key in a helper-visible group key list), a clear operator FAQ. |
| N9 | **Notification expectations.** Default-on change notifications will be late, and apps like Jami got "messages never delivered" reviews. | Medium | P16 | See P16. |
| N10 | **Phone-only groups are the common case** for an Android-first app. They have no tier-1 mailbox, so Nostr is load-bearing, not auxiliary. Phone helpers are active at night on chargers, when members are asleep anyway. | Medium | Keet 2023 advice; P3 | Say so in §7. Budget relay redundancy for phone-only groups. Measure the share of phone-only groups in the beta. |
| N11 | **Load-scaled PoW penalizes legitimate new groups during attacks** and is "insufficient against resourced adversaries". | Low-Medium | Cwtch | Prefer quotas and history-priority. Keep PoW bounded. Exempt slots pre-authorized by a member's signature. |
| N12 | **RAM-only rule excludes the most valuable volunteers:** 24/7 headless servers run by enthusiasts, the Syncthing-relay-operator profile. They lose everything on each restart. | Low-Medium | P2, P5 | Allow an explicit "dedicated helper" mode with opt-in encrypted disk persistence and operator-set caps (consent-based, unlike the default desktop helper). |

---

## Suggested experiments (local only)

| ID | Question | Setup | Success criterion |
|---|---|---|---|
| X1 | Does §7a reach 99.9% slot availability at realistic participation and correlated churn? | Extend E5: 10k simulated users; helper participation 0.01% / 0.1% / 1% / 10%; per-helper diurnal on/off profiles by time zone (sessions exponential, median ~3 h, as in Mainline); trust-spread selection vs diversity-constrained selection | Availability ≥ 99.9% at P(helper up \| group's own members down); report the minimum participation that works |
| X2 | Cost of eclipsing a bootstrap set | Local Kademlia-style helper overlay (e.g., `mainline` crate testnet or a custom overlay); attacker mints IDs near H(groupID‖date) with and without secret-only derivation and IP-prefix binding | Keygen time per k-bit prefix; fraction of bootstrap set captured |
| X3 | Metadata aggregation by Sybil helpers | Same simulator; attacker controls fraction f of helper capacity and behaves perfectly for 90 days | Fraction of groups and member IPs observed vs f, with and without share caps, unlinkable slot IDs and ASN diversity |
| X4 | BEP44 on a local DHT testnet | `mainline` testnet in-process: per-device pkarr records, group-shared keypair contention, salt use, republish traffic per device per day | Bytes/day per device; lookup latency; confirm the overwrite race with a shared group key |
| X5 | Retry storm | Local strfry or nostr-rs-relay with rate limits; 5k simulated clients; 30-min relay outage | Peak events/s after recovery with and without jitter; whether the relay's own rate limiter would ban clients |
| X6 | Relay-list aging | Use relay death and churn rates from R2's measurements to simulate installs that never update for 1–5 years; compare shipped-list-only vs group-gossip plus signed-manifest refresh | Probability an old install has ≥ 2 working relays |
| X7 | Headless member as mailbox | Local desktop or headless engine as a tier-1 member for two phone emulators that are never online together | Delivery latency distribution vs Nostr-only |
| X8 | Is a 128 KiB / 2-min relay useful at all? (to close §15 formally) | Local libp2p relay v2 with defaults; measure typical Loro update and snapshot sizes from E1 | Share of sync sessions that fit; expected: snapshots don't fit, so drop the item |

---

## Sources

**BitTorrent / DHT / pkarr / iroh**
- BEP 44 (mutable items): https://www.bittorrent.org/beps/bep_0044.html
- BEP 43 (read-only nodes): https://www.bittorrent.org/beps/bep_0043.html
- BEP 42 (DHT security extension): https://www.bittorrent.org/beps/bep_0042.html
- Wang & Kangasharju, Real-world Sybil attacks in Mainline DHT (GLOBECOM 2012): https://nymity.ch/sybilhunting/pdf/Wang2012a.pdf [snippet: ~300k sybils]
- Mainline DHT measurement (Cambridge): https://www.cl.cam.ac.uk/~lw525/MLDHT/
- pkarr README: https://github.com/pubky/pkarr
- iroh DHT discovery docs: https://docs.iroh.computer/connecting/dht-discovery
- iroh DHT discovery design notes: https://hackmd.io/vwgYERaBTHKnJUF7s-WLig
- iroh relays (public relays for dev/testing): https://docs.iroh.computer/concepts/relays
- Adar & Huberman, Free Riding on Gnutella (2000): https://firstmonday.org/ojs/index.php/fm/article/view/792

**Hypercore / Pear / Keet**
- Blind peering explanation: https://docs.pears.com/p2p/explanation/blind-peering/
- Availability and blind peering: https://docs.pears.com/explanation/availability-and-blind-peering/
- Keep data available with blind peering: https://docs.pears.com/how-to/blind-peering/keep-data-available-with-blind-peering/
- blind-peering client: https://github.com/holepunchto/blind-peering
- blind-peer server: https://github.com/holepunchto/blind-peer
- blind-pairing: https://github.com/holepunchto/blind-pairing
- HyperDHT README (bootstrap nodes, persistence): https://github.com/holepunchto/hyperdht
- Keet mobile changelog: https://github.com/holepunchto/keet-mobile-releases/blob/main/CHANGELOG.md
- Keet support, security & privacy: https://support.keet.io/security-and-privacy/
- Keet on X, 2023 availability advice [snippet]: https://twitter.com/keet_io/status/1620393030875492354

**Syncthing**
- Discontinuing syncthing-android (2024-10-20): https://forum.syncthing.net/t/discontinuing-syncthing-android/23002
- syncthing-android issue #2064 (Play storage denial): https://github.com/syncthing/syncthing-android/issues/2064
- heise, Syncthing-Fork key handover (2025-12-08): https://www.heise.de/en/news/Key-handover-in-the-dark-Syncthing-fork-community-raises-alarm-11107337.html
- Infrastructure report (discovery), 2024-09-19: https://forum.syncthing.net/t/infrastructure-report-discovery-stuff/22819
- Relay network capacity thread (2018): https://forum.syncthing.net/t/relay-network-capacity/11470
- Relay pool endpoint (100 relays on 2026-09-24): https://relays.syncthing.net/endpoint
- Relay pool server source: https://raw.githubusercontent.com/syncthing/syncthing/main/cmd/infra/strelaypoolsrv/main.go
- Relaying docs: https://docs.syncthing.net/users/relaying.html
- Relay server docs: https://docs.syncthing.net/users/strelaysrv.html
- Discovery server docs: https://docs.syncthing.net/users/stdiscosrv.html

**SSB / Manyverse**
- Manyverse Open Collective updates ("My last update", 2024-04-05): https://opencollective.com/manyverse/updates
- Announcing SSB Rooms: https://www.manyver.se/blog/announcing-ssb-rooms/
- On permanence and safety (2022): https://www.manyver.se/blog/2022-10-permanence-and-safety/
- SSB overview, initial sync [snippet]: https://srcco.de/posts/secure-scuttlebutt-ssb.html

**Briar**
- Briar Mailbox README: https://github.com/briar/briar-mailbox/blob/main/README.md
- Briar Mailbox on F-Droid [snippet]: https://f-droid.org/packages/org.briarproject.mailbox/
- Download Briar Mailbox: https://briarproject.org/download-briar-mailbox/

**Anytype**
- Sync & Backup docs: https://doc.anytype.io/anytype/data/sync-and-backup
- any-sync: https://github.com/anyproto/any-sync

**Delta Chat**
- FAQ: https://delta.chat/en/help
- chatmail notifiers (central proxy): https://github.com/chatmail/notifiers
- Gmail provider notes: https://providers.delta.chat/gmail
- deltachat-android #3281 (FCM wake, no connectivity): https://github.com/deltachat/deltachat-android/issues/3281

**Magic Wormhole**
- Docs, welcome: https://magic-wormhole.readthedocs.io/en/latest/welcome.html
- Relay error issue #414 [snippet]: https://github.com/magic-wormhole/magic-wormhole/issues/414

**Automerge / local-first**
- Automerge network sync (public server disclaimer): https://automerge.org/docs/tutorial/network-sync/
- Ink & Switch, Local-first software: https://www.inkandswitch.com/essay/local-first/

**IPFS / libp2p**
- Trautwein et al., Design and Evaluation of IPFS (SIGCOMM 2022): https://arxiv.org/abs/2208.05877
- Content Censorship in the IPFS DHT (NDSS 2024): https://arxiv.org/abs/2307.12212
- kubo #9389 provider record 22h/48h [snippet]: https://github.com/ipfs/kubo/issues/9389
- Cloudflare IPFS gateway transition [snippet]: https://blog.cloudflare.com/cloudflares-public-ipfs-gateways-and-supporting-interplanetary-shipyard
- NFT.Storage Classic decommission [snippet]: https://nft.storage/nft-storage-classic
- libp2p circuit relay v2 spec: https://raw.githubusercontent.com/libp2p/specs/master/relay/circuit-v2.md
- go-libp2p relay v2 default limits: https://raw.githubusercontent.com/libp2p/go-libp2p/master/p2p/protocol/circuitv2/relay/resources.go

**Tor / I2P / Hyphanet / Cwtch**
- KAX17 (The Record): https://therecord.media/a-mysterious-threat-actor-is-running-hundreds-of-malicious-tor-relays
- Tor network health update (2023-11-20): https://blog.torproject.org/tor-network-community-health-update/
- Tor metrics (relay and user counts via secondary summaries [snippet]): https://metrics.torproject.org/
- I2P network database (floodfill Sybil) [snippet]: http://geti2p.net/en/docs/how/network-database
- I2P 2023 floodfill attack [snippet]: https://theoverlay.ghost.io/what-is-a-ddos/
- Hyphanet FAQ: https://github.com/hyphanet/wiki/wiki/FAQ
- Cwtch server design: https://docs.cwtch.im/security/components/cwtch/server

**Mobile "serverless" apps**
- Jami and proxies (2025-05-05): https://jami.net/jami-and-proxys/
- Jami forum, phones need configuration (2021): https://forum.jami.net/t/on-phones-jami-requires-configurations-in-order-to-work/969
- Session push notifications: https://sessionapp.zendesk.com/hc/en-us/articles/4439028541849-How-do-push-notifications-work-on-mobile-platforms
- Session FAQ (swarms, stake) [snippet]: https://getsession.org/faq
- SimpleX instant notifications (2022): https://simplex.chat/blog/20220404-simplex-chat-instant-notifications.html
- SimpleX servers operated by Flux (2024) [snippet]: https://simplex.chat/blog/20241125-servers-operated-by-flux-true-privacy-and-decentralization-for-all-users.html
- Bitchat architecture (foreground service, Nostr fallback) [snippet]: https://dev.to/lina_atar_b33f8cb45364a91/messaging-after-the-internet-inside-bitchats-bluetooth-mesh-nostr-fallback-and-offline-security-bih
- Berty challenges: https://berty.tech/challenges

**Nostr**
- NIP-78 (arbitrary custom app data, kind 30078): https://raw.githubusercontent.com/nostr-protocol/nips/master/78.md

---

## Independent verification

*Adversarial re-check, 2026-09-24. Method: direct fetches of primary sources (specs, source code, READMEs, papers, official docs), chosen to be independent of the report's sources where possible. Web search was unavailable (session budget exhausted), so no search-snippet evidence is used here. Nothing was installed or sent to public infrastructure beyond page fetches.*

### Verdicts after re-check

| # | Original verdict | After re-check | What changed |
|---|---|---|---|
| P1 | Partial | **Upheld (partial)**, one sub-claim corrected | Mainline *does* have a "list peers under key X" primitive: BEP5 `announce_peer` / `get_peers` on any infohash (how every torrent swarm is found). The real obstacles are (a) a rendezvous returns a partial, inconsistent list, so readers can't agree on "the *r* XOR-closest helpers", and (b) the ~8 storing nodes can be eclipsed. The objection that it "exposes every helper's IP" adds no new cost, because §7a already accepts helper IPs being visible. |
| P2 | Refuted | **Not upheld as stated → Partial / contingent** | The report measures *opt-in dedicated operators* (Tor relays, Syncthing relays). §7a is a *default-on, in-app* feature "like torrent seeding" (default still undecided). Default-on precedents reach high participation: every BitTorrent client is a Mainline DHT node (15–27M nodes/day); Kubo "starts as a DHT client and automatically switches to server mode when reachable"; Skype ran on user supernodes until 2012. **Correct verdict:** refuted for opt-in (0.01–0.3%); unverified for default-on desktop helping, where the answer depends on the desktop install share of an Android-first app, which is unknown. The 10% figure has no evidence either way. |
| P3 | Refuted | **Upheld, but the main reason is different** | Bhagwan et al. (Overnet, 2003) found diurnal patterns but "significant independence between host pairs" when hosts are picked **at random**: 80% of pairs are within ±0.2 of independence. So correlation mostly comes from §7a's *selection bias* (trust spreads through people), not from churn itself. The bigger flaw the report missed: the formula uses **reachability** *p*, but helpers are RAM-only, so **every restart is a permanent loss** of the slot. The relevant *p* is P(helper still holds the slot), which is far lower. The project's own E5 shows this: at r = 10 with desktops 50% online, availability is 81% with no member refresh, against 99.9% predicted. |
| P4 | Refuted | **Upheld**; the report's "unverified" item is now resolved | `blind-peering` README: blind peers must be configured ("You should always set this, otherwise there are no mirrors to contact"), with a `group` field for "where the blind peer is hosted". **Keet-style push goes through blind peers:** `sendNotification` asks "the closest blind peer to create a `blind-push` notification … and forward it to its configured push gateway". The blind-peer CLI has `--push-gateway-key` and `--trusted-peer`, and logs `"maxBytes":"100GB"`. So Keet's offline delivery *and* push both rest on operator-configured always-on nodes. |
| P5 | Partial | **Upheld** | Add the counter-lesson: ~100 volunteers carry ~1M Syncthing devices because each relay's load is light (forward-only, last resort). For helpers, total RAM supply vs demand is what matters, not the ratio. At 1M users that is 300–750 GB (§7a numbers). At 64–256 MB per desktop, that takes ~1,200–12,000 helpers online at once (0.1–1.2% of users), and more once downtime and E1's larger group sizes (missed item 6) are counted. |
| P6 | Partial | **Upheld** | Confirmed: repo archived 2024-12-03; README: "Google making Play publishing something between hard and impossible". heise (2025-12-08): maintainer unreachable, project taken over by account "researchxxl", "unclear whether the new account has access to the signing keys". The Play trigger was all-files storage access, which this app doesn't need. |
| P7 | Partial | **Upheld** | The Rooms 2 spec describes rooms purely as intermediaries that "improve connection reliability" (tunnels, aliases), with no feed storage. This is consistent with the report. |
| P8 | Confirmed | **Upheld** | Briar's own page confirms: "a helper app … that lets you receive encrypted messages from your contacts while Briar is offline", on a spare device with power and Wi-Fi. (The battery-optimization detail stays [snippet].) |
| P9 | Unverified | **Upheld, with scoping** | Local-first apps *do* run at scale without developer-hosted nodes by using the **user's own cloud account**: Joplin syncs via Dropbox, OneDrive, Nextcloud, WebDAV, S3 or the file system, with Joplin Cloud optional. C3 ("no accounts") and C4 ("seamless") rule out that pattern as the default. Skype, the largest user-hosted network, moved to "Microsoft-operated supernodes in May 2012" and to central servers in 2017 (Wikipedia). So the claim holds for C2 + C3 + offline mobile delivery. |
| P10 | Partial | **Upheld (partial), timing claim weakened** | The quote "sending limits and overzealous spam rejections" is **not on delta.chat/en/help today**. The substance is supported elsewhere: chatmail.at says "avoiding classic e-mail server rate limits"; providers.delta.chat/gmail says "Due Google adding more and more bureaucracy in that area, we dropped support for that". The central proxy is confirmed ("The central Delta Chat notification proxy decrypts the device token…"). But the report's claim that rate limits and kind allow-lists "apply from day one" is unsourced, and this project's **E4 found 8/8 major relays accepting new keys, kind 30078 and 60 KB payloads today**. The stronger near-term risk is E4's **strfry monoculture**: one policy change hits most relays at once. |
| P11 | Confirmed | **Upheld** | Wormhole docs: "a 1-in-65536 chance of success"; "Both clients must be run within an hour or so of each other"; the server is free "until volume or abuse makes it infeasible to support". |
| P12 | Refuted | **Upheld** (as "not a production-grade fallback") | iroh blog (shared relays, 2026): public relays are "rate-limited based on the total current unauthenticated traffic", then "throttling your traffic", and users hitting limits are told to "upgrade to a Pro plan or self-host". They are not going away ("No"). Automerge: "no reliability or data safety guarantees … run your own sync server for production apps". Layer 7 is legitimate only as best-effort. R1 adds that n0's DNS/pkarr docs do permit production use. |
| P13 | Refuted | **Upheld; the rationale needs fixing** | Kubo `config.md` confirms relay-service defaults `ConnectionDurationLimit "2m"`, `ConnectionDataLimit 131072`, `MaxCircuits 16`, `ReservationTTL "1h"`. But "snapshots don't fit" is weak: E1 measured a 1-year list at 70 KB full / 31 KB state-only, which fits in 128 KiB. Better reasons to drop it: the relay is meant for hole-punch coordination, each circuit resets, it adds a heavy libp2p dependency with no iroh interop, and it uses IPFS operators' capacity for non-IPFS traffic, an acceptance risk of its own. |
| P14 | Partial | **Upheld**; one mechanism corrected, one strong source added | **I2P's own netdb docs** say its daily keyspace rotation, H(key + YYYYMMDD), the same idea as §7a's "daily-rotated hash of the group ID", "is not particularly effective": "An attacker can precompute numerous router hashes in advance, and only a few routers are sufficient to 'eclipse' a portion of the keyspace within a half hour after rotation." Correction: seeing the *hash* of the group ID on Nostr lets an attacker precompute the bootstrap position only if that position derives from the published hash, or if helpers learn the group ID itself (e.g. as a slot key). The secret-only HKDF fix is right either way. Minor: the Tor 2023-11-20 post says "a large number of relays" and gives no "~1,000" count. |
| P15 | Confirmed | **Upheld** | No new evidence against. |
| P16 | Partial | **Upheld, somewhat stronger** | Android's bucket table: **Rare = regular jobs up to 10 min per 24 h, network "Disabled"; Restricted = once per day**. Group members who rarely open the app (typical for someone receiving a shared list) get effectively no background sync unless a widget keeps them active. Jami (2025-05-05): "Android and iOS prevent apps from staying active in the background for too long". Session fast mode: "Account ID and unique push notification token to an STF-operated push notification server". Keet: blind-push via push gateway (P4). |

### Important things the report missed

1. **Skype supernodes are the canonical precedent for §7a, and they went hosted.** Skype was the largest default-on, user-hosted infrastructure ("any client with sufficient bandwidth and processing power could become a supernode"). It became "entirely powered by Microsoft-operated supernodes in May 2012", then moved to central servers in April 2017 (Wikipedia: Skype, Skype protocol). The exact reasons, including mobile, weren't verifiable from these pages. The direction of travel was not. §7a should cite this as the main cautionary precedent for default-on helping.
2. **User-contributed storage features tend to be removed.** Wuala's storage trading (disk on other users' machines) was "discontinued" in 2011, and the service closed in 2015. CrashPlan's free backup to friends' computers ended with CrashPlan for Home in 2018 ("Backups to friends/family are not supported in the new product"). Neither page gives the technical reason. The pattern supports treating tier 2 as an experiment.
3. **The *r* formula conflates reachability with retention (RAM-only).** Blake & Rodrigues (HotOS 2003, "Pick Two") show that cooperative storage is bounded by the repair traffic needed to replace data lost to churn, and that hosts which keep their data across downtime are much cheaper. RAM-only helpers never keep it: every restart is a departure. With task-list-sized data the bandwidth is affordable (e.g. 300–750 GB re-replicated per ~8 h session at 1M users ≈ 10–26 MB/s network-wide), but *p* must be modeled as retention. **Fix:** redefine *p* in §7a as "probability the helper still holds the slot at read time" and take it from E5-style measurement, not uptime.
4. **Daily key rotation is not an anti-eclipse defense** (I2P, above). §7a's bootstrap relies on it, so its claim that eclipse "buys an attacker almost nothing" applies only to the steady-state set. **Fix:** secret-only derivation, plus a random rather than XOR-closest bootstrap sample from a multi-source helper list.
5. **Random selection is what buys independence** (Bhagwan 2003). The P3 fix should say: pick part of each group's helper set *uniformly at random* from the global pool (not through the trust graph), which restores approximate independence. Time-zone and ASN caps are then a second layer.
6. **§7a's size inputs conflict with E1.** §7a assumes ~50 KB per group and a 256 KB per-group quota per helper. E1 measured one shopping list at 70 KB (1 year) to 353 KB (5 years) as a full snapshot, or 31–155 KB state-only, and deleted items never leave the state. A multi-list group can exceed the 256 KB quota within a few years, and network-wide RAM is likely several times the §7a estimate. Rerun the P2 load table with E1 sizes.
7. **Tor numbers are stale in the report.** Tor metrics, 2026-09-20: **9,416 relays, ~3.28M daily users (~0.29%)**, against the report's ~8,000 / ~2.5M. The conclusion is unchanged.

### Sources used in this verification
- BEP 5 (announce_peer / get_peers): https://www.bittorrent.org/beps/bep_0005.html
- BEP 42 / 43 / 44 (re-read for exact text): https://www.bittorrent.org/beps/bep_0042.html, https://www.bittorrent.org/beps/bep_0043.html, https://www.bittorrent.org/beps/bep_0044.html
- Kubo config (relay-service defaults, `Routing.Type auto`): https://github.com/ipfs/kubo/blob/master/docs/config.md
- I2P network database (keyspace rotation "not particularly effective"): https://i2p.net/en/docs/overview/network-database
- Bhagwan, Savage, Voelker, Understanding Availability (IPTPS 2003): https://cseweb.ucsd.edu/~voelker/pubs/avail-iptps03.pdf
- Blake & Rodrigues, High Availability, Scalable Storage, Dynamic Peer Networks: Pick Two (HotOS 2003): https://www.usenix.org/legacy/events/hotos03/tech/full_papers/blake/blake.pdf
- Mainline DHT measurement (daily churn, session lengths): https://www.cl.cam.ac.uk/~lw525/MLDHT/
- HyperDHT README ("~30 mins for the node to become persistent", default bootstrap nodes): https://github.com/holepunchto/hyperdht
- blind-peering README (configured mirrors, `sendNotification` → push gateway): https://github.com/holepunchto/blind-peering
- blind-peer and blind-peer-cli (`--trusted-peer`, `--push-gateway-key`, 100 GB): https://github.com/holepunchto/blind-peer, https://github.com/holepunchto/blind-peer-cli
- syncthing-android (archived 2024-12-03): https://github.com/syncthing/syncthing-android
- heise on Syncthing-Fork (2025-12-08): https://www.heise.de/en/news/Key-handover-in-the-dark-Syncthing-fork-community-raises-alarm-11107337.html
- SSB Rooms 2 spec: https://ssbc.github.io/rooms2/
- Briar Mailbox: https://briarproject.org/download-briar-mailbox/
- Magic Wormhole docs: https://magic-wormhole.readthedocs.io/en/latest/welcome.html
- iroh shared relays (2026): https://www.iroh.computer/blog/shared-relays
- Automerge network sync: https://automerge.org/docs/tutorial/network-sync/
- Anytype sync and backup: https://doc.anytype.io/anytype/data/sync-and-backup
- Joplin sync targets: https://joplinapp.org/help/apps/sync/
- Delta Chat FAQ (notification proxy): https://delta.chat/en/help; chatmail: https://chatmail.at/; Gmail provider notes: https://providers.delta.chat/gmail
- Session push notifications: https://sessionapp.zendesk.com/hc/en-us/articles/4439028541849-How-do-push-notifications-work-on-mobile-platforms
- Jami and proxies (2025-05-05): https://jami.net/jami-and-proxys/
- Android standby-bucket limits: https://developer.android.com/topic/performance/power/power-details
- Tor metrics CSVs (relays, users): https://metrics.torproject.org/networksize.csv, https://metrics.torproject.org/userstats-relay-country.csv
- Tor community health update (2023-11-20): https://blog.torproject.org/tor-network-community-health-update/
- Wikipedia: Skype, Skype protocol, Wuala, CrashPlan: https://en.wikipedia.org/wiki/Skype, https://en.wikipedia.org/wiki/Skype_protocol, https://en.wikipedia.org/wiki/Wuala, https://en.wikipedia.org/wiki/CrashPlan
- Project experiments cross-checked: E1-loro.md (sizes), E4-nostr.md (relay acceptance), E5-helper-sim.md (availability)
