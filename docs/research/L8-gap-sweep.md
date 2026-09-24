# L8: Gap sweep (infrastructure and protocols not yet listed)

*2026-09-24. Research only, no code built and no traffic sent to any relay, broker, tracker, DHT or server. Evidence comes from official docs, ToS pages, GitHub READMEs and source (via `gh api`), crates.io and docs.rs. WebSearch wasn't available, so candidates were found by checking known projects directly. Anything I couldn't verify is marked **unverified**.*

*Note: DECISIONS.md has no item D46 as of today, so this report works from D11–D23 and the P6 gap list in STATE.md.*

## Summary

**Bottom line: this sweep found no new layer that solves a gap cleanly.** It found two useful ideas, three weak supporting channels, and a longer list of confident rejections.

1. **Helper-hosted iroh relays with Let's Encrypt IP-address certificates** (CONSIDER, medium confidence). This is the most important new finding for **G2 and G4**.
   - Let's Encrypt made 6-day certificates for bare IP addresses **generally available on 2026-01-15**. They need no domain and no email.
   - `instant-acme` (Apache-2.0) supports both IP identifiers and the `shortlived` profile.
   - iroh 1.2 can add relays at runtime (`Endpoint::insert_relay`), and `iroh-relay` 1.2 runs embedded with manual certificates plus QUIC address discovery (QAD).
   - Together, a reachable desktop helper can run a **standard, web-PKI-trusted iroh relay that strangers' devices can use**. That makes the helper network a real second provider for relaying, public-address discovery and hole-punch coordination, alongside n0.
   - It only works on desktops with a publicly reachable port 80 or 443 (public IPv4 plus UPnP/PCP, or open IPv6). The share of desktops that qualify is **unmeasured**.
   - This is new beyond R1's "member-hosted relay" idea: that idea needed custom trust roots, so it could only serve a device's own groups.
2. **Public "Send" instances (the community fork of Firefox Send)** (CONSIDER, medium). This is the best new option for **G6**, and for the **large-snapshot part of G1**.
   - About 20 independently run instances accept anonymous, client-side-encrypted uploads.
   - Retention is 3–30 days, and size limits range from 50 MiB to hundreds of GiB.
   - A small pointer (URL plus key) can go in a Nostr slot, a helper, or a pkarr/Mainline record that any group member can keep alive.
   - Caveats: Mozilla shut the original service down over malware abuse, instances come and go, and the only Rust client is stale.
3. **Weak supporting channels for G5 only** (each CONSIDER, low):
   - **public BitTorrent trackers** as a rendezvous: 53 "stable" trackers on newTrackon (33 UDP, 20 HTTP);
   - **public MQTT test brokers** as real-time signaling while the app is open (Eclipse Mosquitto, EMQX, HiveMQ, shiftr.io). Every one of them says it's for testing;
   - **the Yggdrasil overlay** (about 250 of 400+ listed volunteer public peers online). It gives key-addressed reachability and relaying through volunteers (G2/G4). But it's alpha software, a Go runtime, needs an unstable iroh custom transport, and puts the "phone routes strangers' traffic" Play risk back on the table.
4. **G3 (waking a Dozing phone without our own server): nothing new exists.** Every wake-up path I checked needs push credentials that are tied to the app and held on a server:
   - OpenDHT/Jami's push proxy: "you only can send push notifications to an application with the related gateway";
   - FCM HTTP v1 needs service-account OAuth;
   - SMS is blocked by Play's SMS policy;
   - MQTT needs a live socket, which Android kills in the background.

   The only options remain UnifiedPush/ntfy and Web Push (covered elsewhere) and D17's "Tell Alex".
5. **G1 (offline store-and-forward independent of Nostr): still no strong answer.**
   - XMPP has about 15 free "category A" providers with in-app registration and 7–365 days of archive. It would mean hidden per-device accounts on servers run mostly by one person each, and 8 of 15 require a CAPTCHA. I'm rejecting it as a default layer.
   - OpenDHT keeps values for only 10 minutes.
   - MQTT retained messages come with no retention promise.
   - The helper network and Send (for large blobs) remain the main answers.
6. **Rejected with evidence** (details below): p2panda (same n0 dependency), Willow/Earthstar, Radicle, Matrix P2P/Pinecone, Reticulum/LXMF (license and an explicit stance against machine-generated code), Tox, Holochain, Nym, Lokinet, GNUnet, AT Protocol, ActivityPub, Automerge's public sync server, Jazz, ElectricSQL, Firebase Spark, Ditto, GUN, Scuttlebutt rooms, Mycelium (ThreeFold), trycloudflare, bore.pub, sslip.io/nip.io as a primary path, the PeerJS cloud, Tailscale DERP, IRC, SMS, 0x0.st/catbox, Swarm, Meshtastic and WebTorrent WSS trackers.

## Gap coverage at a glance

| Candidate | G1 offline S&F | G2 P2P w/o n0 | G3 wake phone | G4 relay | G5 signaling | G6 blobs | Verdict (confidence) |
|---|---|---|---|---|---|---|---|
| Helper-hosted iroh relays + LE IP certs | – | **Yes** (QAD + punch coordination) | – | **Yes** | Partial (relay as rendezvous) | – (possible, not advised) | **CONSIDER** (medium) |
| Send instances (timvisee/send) | Partial (large snapshots, days) | – | – | – | – | **Yes** | **CONSIDER** (medium) |
| Public BitTorrent trackers (UDP/HTTP, BEP 15/3) | – | Partial (IP only, not NAT port) | – | – | **Yes** (presence, rendezvous) | – | CONSIDER (low) |
| Public MQTT test brokers | Weak (retained msgs, no promise) | Partial (signaling channel only) | – | – | **Yes** (foreground) | – | CONSIDER (low) |
| Yggdrasil overlay (public peers) | – | **Yes** | – | **Yes** (via volunteers) | Partial | Weak | CONSIDER (low) |
| Mycelium (ThreeFold) overlay | – | Yes | – | Yes | Partial | – | REJECT (low): one company, 10 nodes, no terms |
| XMPP in-band-registration providers | Yes (7–365 d MAM) | – | – (push needs an app server) | – | Yes | Yes (7–31 d upload) | REJECT as default (medium) |
| OpenDHT / Jami network | 10-min values only | Partial | No (push gateway tied to the app) | – (TURN is Jami's) | Yes | – | REJECT (medium) |
| WebTorrent WSS trackers | – | Partial | – | – | Yes (online-only offers) | – | REJECT (low–medium) |
| All other checked ideas | – | – | – | – | – | – | REJECT (see §3) |

---

## 1. Candidates with a CONSIDER verdict

### 1.1 Helper-hosted iroh relays using Let's Encrypt IP-address certificates

**What it is.** A desktop helper that is reachable from the internet runs the standard `iroh-relay` server (plus QAD) inside the app. It gets a free, publicly trusted TLS certificate **for its own IP address** from Let's Encrypt, with no domain and no account email. It then announces `https://<ip>` as a relay through the helper network. Every client adds the chosen helper relays at runtime alongside n0's.

**Gaps covered:**
- **G2:** QAD gives public-address discovery, and the relay path does hole-punch coordination. iroh's own FAQ says relays are "the introduction, not just the fallback" (R1).
- **G4:** relaying for failed punches, including phone↔phone on carrier NAT, while both phones are online.
- **G5, partly:** peers that share a home relay can find each other.
- Not G1, G3 or G6.

**Evidence that the pieces exist:**
- **Let's Encrypt IP certificates are GA.** "These certificates are valid for 160 hours, just over six days." You select the `shortlived` profile (LE, 2026-01-15). Validation: "only the http-01 and tls-alpn-01 methods can be used" for IP certificates. DNS validation isn't allowed. IPv6 is supported (LE, 2025-07-01).
- **Rate limits fit:** "Up to 50 certificates can be issued per registered domain (or IPv4 address, or IPv6 /64 range) every 7 days" (LE rate-limit docs). A 6-day certificate renewed every ~3–4 days uses about 2 per week per IP.
- **ACME client:** `instant-acme` 0.8.5 (Apache-2.0, 2026-02-24, ~1.15 M downloads in 90 days) has `Identifier::Ip(IpAddr)` and `NewOrder::profile()`: "Set the profile to be used for the order" (docs.rs).
  - iroh's own ACME client (`n0-computer/tokio-rustls-acme`) only builds `Identifier::Dns` and uses tls-alpn-01 (source `src/acme.rs` line 129). So we'd use instant-acme and feed the certificate to iroh-relay's manual-certificate mode.
- **iroh-relay 1.2** (MIT/Apache-2.0): the `server` module is "a fully-fledged iroh-relay server over HTTP or HTTPS" with an optional QAD endpoint. It supports manual certificates (`rustls-cert-file-reader`) and can be embedded in an app (docs.rs). R1 verified an `AccessControl` trait and allowlists in 1.2.
- **iroh 1.2 client side:** `Endpoint::insert_relay` "Adds the provided configuration to the RelayMap…", and `remove_relay` removes one at runtime (docs.rs, iroh 1.2.0, 2026-09-09). `home_relay_status` exposes `auth_denied_reason()` for health tracking.
- **Fallback naming:** nip.io/sslip.io map `1-2-3-4.sslip.io` to an IP, and certificates for them "can be issued via the HTTP-01 challenge for each individual hostname". But neither is on the Public Suffix List (checked: absent from `public_suffix_list.dat`), so every user shares one Let's Encrypt limit. The operators say LE "bumped our limit from 50 to 250,000 certificates … but turned down our request for 500,000". It's also run by two individuals. Fine as an emergency fallback, not as the main path.

**Cost:** $0 for users and developer. Let's Encrypt is free, and the relay runs on the user's own machine.

**ToS / permission:** Let's Encrypt issues to anyone who proves control of the IP. No terms prohibit this use. **Unverified:** some residential ISPs' acceptable-use policies forbid running servers. The user is the operator here, as with Syncthing relays or BitTorrent seeding.

**Operators:** every qualifying desktop helper, so the count grows with the user base. It depends on one CA (ISRG) for account-less ACME with IP certificates. The alternatives I know of (ZeroSSL, Google Trust Services) require External Account Binding, i.e. a vendor account. **Not re-verified this session.**

**Reliability and limits:**
- Home desktops go offline (the D20 churn model applies).
- A 6-day certificate means an offline helper's certificate can lapse. It renews on startup before announcing itself.
- The IP must stay stable while the relay is announced. Dynamic IPs force a new certificate and a re-announce.

**Reachability (the key unknown):**
- The helper needs inbound TCP 80 or 443 for validation and inbound UDP for QAD.
- It fails behind ISP CGNAT (UPnP only maps the home gateway: R1 verifier), with no UPnP/PCP, when the ISP blocks inbound 80/443, or when the router firewalls inbound IPv6 by default.
- **The share of desktops that qualify is unmeasured.** It could be small.
- iroh's `portmapper` is already on by default on desktop (R1 C2d).
- The iroh client must accept a relay URL with an **IP-SAN** certificate. rustls supports IP server names, but iroh relay URL handling with bare IPs is **unverified**.

**Privacy / metadata:**
- The helper's IP is public. The design already accepts this for helpers (§7a).
- The relay sees which endpoint IDs talk to each other and when, but not content: iroh traffic is end-to-end QUIC/TLS.
- This worsens D23 linkability for anyone who routes through a hostile helper relay. Mitigations: rotating transport keys (D14 synthesis) and spreading groups over several relays.

**Android:** desktop-only by nature. Phones only use these relays as clients, like n0's. No new permissions.

**Library maturity:** iroh/iroh-relay 1.2 (stable 1.x, MIT/Apache) and instant-acme 0.8.5 (Apache-2.0). Binary cost is desktop-only (the relay server feature plus hyper), so it's no concern for the APK.

**Abuse resistance:**
- A relay is a bandwidth pipe. Anyone who knows the URL can relay iroh traffic through a volunteer.
- Mitigations: a per-client bandwidth cap (iroh-relay has rate-limit config), a total cap tied to the "Memory for helping others" setting, and relay use restricted to endpoints presenting a group-scoped token via `AccessControl`. The token scheme is a design task.
- Sybil helpers can offer malicious relays. That only costs availability and metadata, because iroh authenticates endpoints end to end, and D20's reliability records apply unchanged.

**Precedent:**
- Syncthing's community relay pool: about 100 volunteer relays for about 1M devices (R5). It shows volunteers do run relays, but through opt-in operator participation, not default-on.
- Tailscale's `derper` and n0's own relays show the relay model works. Nobody I found ships a default-on, auto-certified home relay.

**Failure modes:**
- Few helpers qualify, so the layer adds little.
- ISRG changes its IP-certificate policy.
- Windows firewall prompts (R1 NR10).
- A helper's ISP complains about server use.
- Churn at night (D20 correlated churn).

**Verdict: CONSIDER, medium confidence.** It's the only candidate in this sweep that gives a real, constraint-compliant second provider for relaying and address discovery. It fits the already-decided helper network. Its value depends entirely on the share of reachable desktops, which needs measuring (experiments X1–X2).

### 1.2 Public Send instances (community fork of Firefox Send)

**What it is.** Mozilla's Send, kept alive by Tim Visee's fork (`timvisee/send`, MPL-2.0 server). A curated list of public instances sits at `timvisee/send-instances` (1,135 stars, updated 2026-07-17). Uploads are encrypted in the client, and the key stays in the URL fragment. Each upload has an expiry and a download limit.

**Gaps covered:**
- **G6:** images and big attachments.
- **G1, partly:** large compacted snapshots that exceed Nostr's ~47 KiB payload (D12), handed to absent members over days.
- The pointer (URL plus key, about 150 bytes) travels over existing small channels: a Nostr slot, a helper slot, or a signed pkarr/Mainline record. Mainline records need republishing ("The DHT drops records after hours; republish periodically", pkarr README), but any group member or helper can republish a signed record without the key.

**Cost:** $0 for users and developer.

**Operators and limits** (from the instance list, 2026-09-24):
- About 20 instances run by independent individuals and small organizations in the NL, DE, US, CZ, TR, BE and FR.
- Size limits range from 50 MiB to 250 GiB, retention from 3 to 30 days, and download limits from 3 to 999,999.
- The list says: "This page does not give any promises or warranties with regard to instance security and reliability."
- The reference instance `send.vis.ee` shows 99.97% uptime on the community status badge.
- Server defaults: 2 GB maximum file, 7-day maximum expiry, 100 downloads maximum (`docs/docker.md`).

**ToS:**
- None of the instances I sampled publishes app-usage terms (`send.vis.ee/legal` returns 404).
- The upstream deployment guide warns operators: "Long expiration times are risky on public servers as people may use you as free hosting for copyrighted content or malware (which is why Mozilla shut down their `send` service)".
- Mozilla's own statement: "Some abusive users were beginning to use Send to ship malware and conduct spear phishing attacks" (2020-09-17).
- Automated app uploads aren't addressed. An app that makes the uploads for its users is a grey area. Be polite: an identifying User-Agent, a short expiry, a low download limit, and deleting via the owner token after everyone has fetched.

**Privacy:** content is end-to-end encrypted twice (Send's own encryption plus ours). The instance sees uploader and downloader IPs, sizes and timing.

**Android:** uploads and downloads fit WorkManager (network-constrained, preferably unmetered for big files). No special permissions.

**Library maturity:**
- `ffsend-api` 0.7.3 (MIT) was last released **2021-08-27**, so it's stale.
- `ffsend` CLI (GPL-3.0) was last pushed 2025-11.
- The Send v3 protocol (HTTP plus a WebSocket upload with an owner token) is small enough to reimplement. **Unverified:** that current instances still speak the protocol `ffsend-api` implements.

**Abuse resistance:** not our problem as a client, but the instances' abuse exposure is a risk to their survival.

**Precedent:** Thunderbird's FileLink provider for Send; `ffsend`.

**Failure modes:**
- Instances disappear or tighten limits. Mitigate by spreading over 2–3 instances and health-tracking them.
- An instance blocks the app's User-Agent.
- An expired upload before an absent member returns. Mitigate: members re-upload when asked, as with snapshots.

**Verdict: CONSIDER, medium confidence** for G6 and large-snapshot overflow. It needs the same "quiet citizen" rules as Nostr (D11). It doesn't replace a G1 mailbox for small, frequent updates.

### 1.3 Public BitTorrent trackers as a rendezvous (new angle on a covered item)

BitTorrent's DHT/BEP44 is covered elsewhere. **Public trackers** are separate infrastructure, which is why they're here.

**What it is.** Group devices announce under a rotating info-hash derived from the group's epoch key (D5 rule 6) and get back the IPs of other members currently announcing.

**Evidence:**
- newTrackon's API lists **53 stable public trackers** (33 UDP, 20 HTTP) out of 256 tracked (`/api/stable`, `/api/all`, fetched 2026-09-24).
- BEP 15: the announce request has explicit `ip` and `port` fields, and "most trackers will only honor the IP address field under limited circumstances". **The tracker returns the observed IP but the port the client declared.** So it gives no NAT-mapped port (it isn't STUN), only presence plus IP.

**Gaps:**
- **G5** (who is online, and at which IP), independent of Nostr and n0.
- **G2, partly:** useful mainly for helpers or desktops with a port-mapped fixed port.

**Cost:** $0.

**ToS:** open trackers accept any info-hash by design. No terms were found for the sampled operators.

**Operators:** many independent operators.

**Privacy:**
- Leaks IPs to tracker operators, and to anyone who learns the current info-hash (an ex-member until rotation).
- Worse than Nostr, which doesn't return peers' IPs.

**Android:**
- One-shot UDP/HTTP requests fit WorkManager.
- On carrier NAT the IP is shared and the declared port is useless.

**Library:** BEP 15 is about 200 lines; librqbit (Apache-2.0) has a client. **Not reviewed in depth.**

**Abuse / failure:**
- Trackers may blacklist unknown hashes. Most open ones don't.
- Tracker churn is high: only 53 of 256 are stable.

**Verdict: CONSIDER, low confidence.** A cheap extra presence channel for desktops and helpers, and a fallback helper-discovery channel next to the DHT. The IP leak must be weighed against D23.

### 1.4 Public MQTT test brokers

**What it is.** Publish/subscribe over WebSocket or TCP to public brokers. Trystero (MIT, 2.7k stars, pushed 2026-09-24) ships these defaults: `test.mosquitto.org:8081`, `broker.emqx.io:8084`, `public:public@public.cloud.shiftr.io`, `broker-cn.emqx.io:8084` and `broker.hivemq.com:8884` (source `packages/mqtt/src/index.ts`). Trystero ranks its strategies: "recommended in the order of MQTT, BitTorrent, and IPFS, based on robustness".

**Gaps:**
- **G5** (real-time presence and signaling while the app is in the foreground), independent of Nostr.
- It could carry iroh address exchanges for hole-punch coordination (G2 signaling part). It gives no address discovery.
- Retained messages would be a weak G1 with no retention promise at all.

**ToS (quotes):**
- Mosquitto: "You are free to use it for any application, but please do not abuse or rely upon it for anything of importance"; "it will often be running unreleased or experimental code".
- HiveMQ: "quick, temporary MQTT testing… not intended for private or production data."
- EMQX: "Built for developers to prototype, learn, and test MQTT".
- shiftr.io: "quickly test the system".

**Operators:** 4 companies or projects.

**Privacy:** the broker sees IPs and topic names. Use rotating, key-derived topics and encrypted payloads. Test brokers let anyone subscribe to most topics ("anybody could be listening").

**Android:** only while the app is open. A persistent socket in the background would need a foreground service (R4).

**Library:** `rumqttc` 0.25.1 (Apache-2.0, ~2M downloads in 90 days); Eclipse Paho for the JVM.

**Precedent:** Trystero apps use these brokers in production.

**Failure modes:**
- A broker restarts and loses everything.
- Brokers throttle.
- "Rely upon it for nothing of importance" means it can be a redundant, best-effort channel at most.

**Verdict: CONSIDER, low confidence**, as an opportunistic, never-required foreground signaling channel. It's in the same ethical position as D14 option (a) with n0. If the user prefers strict ToS compliance, reject it.

### 1.5 Yggdrasil overlay network (public peers)

**What it is.** An end-to-end encrypted IPv6 overlay. Each node's address comes from its key. A node dials 2–3 public peers outbound and is then reachable by key from anywhere: "It doesn't even matter if a node is behind a NAT — once a peering is established, traffic flows in both directions over that peering". Intermediate nodes forward traffic: "Yggdrasil nodes automatically pass traffic to help it get closer to its destination".

**Gaps:**
- **G2** (connectivity by public key, no n0, no hole punching needed).
- **G4** (volunteer public peers carry the traffic).
- **G5, partly** (reachable by key; no group presence).

**Operators:**
- The `public-peers` list has 509 peer URIs across ~40 country files.
- The live status page shows "approximately 400+ peer entries" and "roughly 250–270 peers are currently online" (hourly, 2026-09-24).
- Run by independent volunteers.

**Terms:** peers are published for public use, with no ToS. The README warns: "If you are using a home connection… you may end up carrying traffic for the rest of the network."

**Maturity:** "our official stance is that it is still alpha software" (FAQ). yggdrasil-go v0.5.14 (2026-06-19).

**License:** LGPLv3 with a static-linking exception ("permission to convey… a Combined Work that links statically or dynamically… without providing any Minimal Corresponding Source"), so it's usable in a closed or permissively licensed app.

**Android:**
- `contrib/mobile` exposes `Send`/`Recv` of raw IPv6 packets, and `yggstack` runs without TUN (userspace netstack plus SOCKS).
- So no VpnService is needed if we embed it in-process. But iroh would need its **unstable** custom-transport API (`add_custom_transport`, "unstable and gated behind the `unstable-custom-transport` feature") or a netstack.
- Go runtime size: the yggdrasil-android universal APK is 22.6 MB (asset size). The per-ABI cost is **unverified**.
- Persistent peer links only work in the foreground (R4).

**Privacy:** "Direct peers over the Internet will be able to see your IP address"; "it is not a goal… to provide anonymity". Content is end-to-end encrypted.

**Play risk:** a node peered with several public peers may forward strangers' traffic. That's the "proxy services" concern from D19/D20, so phones would have to be leaf-only (a single peer, never transit). **Unverified** that this can be enforced in v0.5.

**Precedent:** Yggdrasil's own apps. I found no mainstream consumer app that embeds it.

**Failure modes:**
- Volunteers are overloaded if the app scales. That's the same acceptance risk as Nostr, with far fewer operators than Nostr relays.
- Route instability in an alpha codebase.
- Two runtimes (Go plus Rust) in the APK.

**Verdict: CONSIDER, low confidence.** Architecturally it's the only multi-operator, n0-independent relaying fabric found. The cost (Go runtime, unstable iroh API, alpha status, volunteer load, Play proxy risk) is high. Worth a local experiment (X4), not a commitment.

---

## 2. Rejected, but checked in depth

### 2.1 XMPP providers with in-band registration: REJECT as a default layer (medium)

**Evidence** (the `data.xmpp.net/providers/v2/providers-A.json` dataset, 2026-09-24):
- **15 category-A providers**: free, and registration works from an app.
- 12 of 15 are run by a "private person", with bus factor 1–3.
- Message archive (MAM) retention runs from 0 to 365 days (7–90 typical).
- HTTP upload limits run from 20 MB to 10 GB, kept 0–31 days.
- **8 of 15 require a CAPTCHA for in-band registration.**
- yax.im's ToS forbids activities that "impose an undue burden on this server" and reserves deleting accounts after 6 months of inactivity.

**Why it fails:**
- It would cover G1 and G6. But it needs a **hidden account per device** on volunteer servers that never agreed to host a machine mailbox, which is the D13 acceptance problem with ~15 operators instead of hundreds.
- CAPTCHAs break seamless onboarding.
- Push (XEP-0357) needs an app server, so no G3.
- The Rust stack (`tokio-xmpp` 6.0.0, `xmpp-parsers`) is **MPL-2.0**, the kind of license D29 already flags.

Contrast with chatmail (covered elsewhere): chatmail operators exist *for* automatic accounts; XMPP operators don't.

### 2.2 OpenDHT / Jami network: REJECT (medium)

- MIT, C++17 plus GnuTLS, v4.4.0 (2026-09-04).
- "A value remains on the network for its lifetime (default 10 minutes)"; values up to 64 KiB. Value types are "hardcoded and known by every node", so no long-lived custom types are possible.
- It has a real-time `listen`, a plus over Mainline for G5, but no G1.
- Push support exists, but "you only can send push notifications to an application with the related gateway", i.e. the app's own FCM/APNs certificate on a push gateway. So no G3.
- The crates.io `opendht` crate is 0.5.2 from 2021 and **GPL-3.0**. The in-repo Rust bindings are thin FFI over a heavy C++ build.
- It adds little over pkarr/Mainline (covered) plus Nostr ephemeral events, at a large binary and maintenance cost.

### 2.3 Mycelium (ThreeFold): REJECT (low)

- Rust, Apache-2.0 (copyright TF TECH NV), v0.7.10 (2026-06-05), with a `mobile` crate.
- Same overlay model as Yggdrasil.
- The public fabric is **10 nodes run by one company** (DE, BE, FI, US, SG, IN): "A couple of public nodes are provided, which can be freely connected to".
- No terms, no SLA. It exists for the ThreeFold Grid.
- It would swap one single-provider dependency (n0) for another.
- Note: anyone can run a Mycelium public node, so helpers could, but then the iroh-relay approach (1.1) is simpler.

### 2.4 WebTorrent WebSocket trackers: REJECT (low–medium)

- Trackers relay WebRTC `offers`/`answer` objects between peers announcing the same info-hash (`bittorrent-tracker` `lib/server/parse-websocket.js`: `numwant` = number of offers; `to_peer_id` is required with `answer`).
- Trystero defaults to 5 of them (`open.ftorrent.com`, `tracker.webtorrent.dev`, `tracker.openwebtorrent.com`, `tracker.btorrent.xyz`, `tracker.files.fm:7073`). It says they have "far less relay redundancy than Nostr".
- Online-only, and it means repurposing an SDP-offer field.
- MQTT or UDP trackers (1.3, 1.4) serve the same G5 role more cleanly. Keep it only as a named fallback.

### 2.5 p2panda: REJECT as infrastructure (high); a useful library reference

- `p2panda-net` 0.7.1 (MIT/Apache, 2026-08-21) is built on iroh. Its examples hard-code n0 relays (`RELAY_URL = "https://euc1-1.relay.n0.iroh.link."`), and without "a bootstrap and relay" it only works on LAN (`p2panda/src/node.rs`).
- "APIs are not yet considered stable for production use."
- It brings **no new infrastructure** and shares D14's dependency.
- Worth reading for its confidential topic discovery (private set intersection), gossip and sync modules.

### 2.6 Willow / Earthstar: REJECT (high)

- Earthstar (LGPL-3.0) was last pushed 2025-04 and syncs with servers you run yourself ("Servers to sync with").
- `willow-rs` is archived on GitHub ("moved to codeberg.org/worm-blossom/willow_rs"). The sync and sideloading protocols are listed as "Eventually".
- No public infrastructure. The Willow Sideloading idea (data "delivered by any means possible") is a design idea we already have.

### 2.7 Radicle seed nodes: REJECT (high)

- Permissive seeds replicate "a full copy of all repository data available on the network" and need "a public, static IP address".
- Using a public git network as a hidden encrypted mailbox is misuse. Data replicates permanently, and operators are warned about "hosting data in a permissionless network".

### 2.8 Matrix P2P / Pinecone: REJECT (high)

- "Pinecone is incredibly experimental still." The last push was 2025-03.
- Dendrite (the P2P homeserver) is **archived** (2024-11).
- Regular Matrix needs homeserver accounts.

### 2.9 Reticulum / LXMF: REJECT (high)

- The Reticulum License forbids use "in the creation of an artificial intelligence, machine learning or language model training dataset".
- The README calls machine-generated derivatives "void grants" and says "all downstream projects using the infringing derivatives, are **directly** and personally liable". This project is AI-assisted, so that's an unacceptable legal and relationship risk.
- Also: Python reference implementation; the public testnet "has now been decommissioned".

### 2.10 Tox: REJECT (high)

- 28 bootstrap nodes listed, **19 UDP / 18 TCP up** (`nodes.tox.chat/json`, 2026-09-24).
- c-toxcore and tox-rs are **GPL-3.0**. tox-rs was last pushed 2024-01.
- No offline messages.

### 2.11 Holochain: REJECT (high)

- The Foundation's public bootstrap/relay "is not appropriate for production hApps, though, because it's low-bandwidth and has no uptime guarantees".
- Holochain's own Android runtime defaults to **n0's iroh canary relay** (`use1-1.relay.n0.iroh-canary.iroh.link`).
- License: Cryptographic Autonomy License (CAL-1.0).

### 2.12 Nym mixnet: REJECT (medium)

- "Once the zk-nym credential system is turned on, all Nym clients will have to present a valid credential to their Gateway on connection." Plans mention "a pay-per-play backend for large existing applications".
- That fails the $0 constraint at scale. The repo also contains GPL-3.0 parts.

### 2.13 Lokinet: REJECT (high)

- GPL-3.0, VPN/TUN-style onion routing, last release v0.9.14 (2025-07-28).
- Tied to the Oxen/Session network, which is covered elsewhere.

### 2.14 GNUnet: REJECT (medium)

- "still in an early alpha stage when it comes to software"; no mention of Android.
- The license wasn't confirmed on the page I fetched; it's a GNU project.

### 2.15 AT Protocol (Bluesky PDS): REJECT (high)

- Needs an account (a DID) per user or device on a PDS; account creation is rate-limited per IP ("100 per 5 minutes").
- Repositories are public. Record-write limits are generous (5,000 points/hour; blobs up to 50 MB).
- Hidden accounts on bsky.social would violate "no accounts" and invite moderation action ("bulk or spammy interactions are against the Community Guidelines").

### 2.16 ActivityPub: REJECT (medium; constraint-based, not fetched)

Needs accounts on a server, and delivery is server-to-server. No anonymous mailbox. I didn't fetch sources for this one.

### 2.17 Local-first sync services

- **Automerge's public sync server:** "For production software, you should run your own server, but for prototyping and development you are welcome to use ours on an 'as-is' basis" (automerge.org tutorial). Also a single operator and a different CRDT format. REJECT (high).
- **Jazz 2.0:** "the Jazz 2.0 alpha", and it runs on "our global storage cloud" (vendor). REJECT (medium; pricing page not reachable).
- **ElectricSQL:** "a Postgres sync engine"; requires "a Postgres database with logical replication enabled". There's no public instance. REJECT (high).
- **Firebase Spark:** the Realtime Database allows "100" simultaneous connections per database on the free plan, and Firestore 50K reads and 20K writes per day. That fails "free at any scale" and makes the developer the account holder. REJECT (high).
- **Ditto:** proprietary; free tier "10 cloud device connections". REJECT (high).
- **GUN:** no maintained public relay list. Its README tells you to deploy your own relay ("spin up a GUN relay peer"). REJECT (medium).
- **Scuttlebutt rooms/pubs:** `go-ssb-room` was last pushed 2023-09 and `ssb-server` 2022-06. The ecosystem is inactive. REJECT (medium).

### 2.18 Tunnels and "give a home PC a public name"

- **trycloudflare (Quick Tunnels):** no account needed, but "intended for testing and development only"; "a hard limit … 200 in-flight requests"; "We don't guarantee any SLA or uptime". REJECT (high).
- **bore.pub:** a single hobby instance of `ekzhang/bore` (MIT) with no terms. REJECT (high).
- **sslip.io / nip.io:** see 1.1. Fallback only.
- **PeerJS cloud:** "For production, you can host your own." WebRTC-signaling only, single operator. REJECT (high).
- **Tailscale DERP:** Tailscale's Terms bind "creating or administering a Tailscale account and accessing or using our Services". Its DERPs serve tailnets. Using them from a non-Tailscale app is outside the terms. REJECT (medium).

### 2.19 Other channels

- **IRC (Libera.Chat):** "if your bots are using Libera.Chat to relay information that is not meant for a human to directly observe and process, your usecase is likely not permitted". REJECT (high).
- **SMS as a wake-up (G3):** Play: "Only access Call Log or SMS permissions when your app falls within permitted uses". You must be the default SMS handler or fit a listed exception, and "Cross-device sync" covers syncing SMS content, not using SMS as a transport. Users also pay per message. REJECT (high).
- **0x0.st:** the operator says "CLANKERS ARE NOT WELCOME HERE", runs a single instance, and moderates uploads ("I have to moderate what is being uploaded here"). Retention is 30 days to 1 year, 512 MiB maximum. REJECT (high).
- **catbox/litterbox:** commercial use "requires prior approval" and the service "cannot function as a CDN". Logs IPs. REJECT (medium).
- **Ethereum Swarm:** "Postage stamps are used to pay for storing data on Swarm." That costs money. REJECT (high).
- **Meshtastic:** needs LoRa hardware ("use inexpensive LoRa radios"). REJECT (high). Offline proximity sync is already covered by §6 layer 9 (Nearby).
- **cjdns:** GPL-3.0; light check only (repo active, 2026-09-22). Its Hyperboria-style public peering has the same drawbacks as Yggdrasil plus copyleft. REJECT (low; not deeply evaluated).

---

## 3. What this means per gap

| Gap | Best new option found | Honest status |
|---|---|---|
| **G1** offline S&F without Nostr | Send instances (large snapshots only) | **Still open** for small, frequent slot updates. The helper network plus member devices (tier 1) remain the only real answer. XMPP was rejected on acceptance grounds. |
| **G2** P2P without n0 | Helper-hosted iroh relays with LE IP certs (QAD plus punch coordination); Yggdrasil as a long shot | **Partly closable.** Depends on the share of reachable desktops (unmeasured). |
| **G3** wake a sleeping phone | none | **Unchanged.** Every wake path needs app-bound push credentials on a server. Only UnifiedPush/ntfy/Web Push (covered elsewhere) and D17's "Tell Alex" exist. |
| **G4** relay for failed punches | Helper-hosted iroh relays; Yggdrasil public peers | Same dependency as G2. |
| **G5** discovery / presence | Public UDP/HTTP trackers, public MQTT brokers (foreground) | Several weak but independent channels exist. Each has ToS or privacy caveats. |
| **G6** large blobs | Send instances | Usable with quiet-citizen rules. Instance churn is the main risk. |

## 4. New risks surfaced

1. **The Let's Encrypt IP-certificate path makes volunteers into public servers.** ISP acceptable-use policies (some forbid servers), abuse complaints about relayed traffic, and Windows firewall prompts all fall on users who never explicitly volunteered, because D20 makes helping default-on. The helper setting's disclosure must say "your PC may relay encrypted traffic for other users".
2. **Single CA dependency.** Account-less ACME with IP identifiers appears to be ISRG-only. If Let's Encrypt changes the IP-certificate policy, the path falls back to sslip.io/nip.io under a globally shared rate limit. Not re-verified: whether other CAs issue account-less IP certificates.
3. **ToS consistency.** Several weak channels (MQTT test brokers, n0) say "testing only". Using them "best-effort, never relied on" is a position the user should take once, explicitly, and apply the same way everywhere (it overlaps with D14's Play "Device & Network Abuse" note).
4. **Tracker IP leakage.** Tracker-based presence returns member IPs to anyone holding the current info-hash. It must rotate with the epoch key, and it's weaker than D23's current model.
5. **Send's abuse history.** Public Send instances survive at operators' goodwill; Mozilla's service died of abuse. An app generating many automated uploads could push operators to add CAPTCHAs or block user agents.
6. **Reticulum's license.** It forbids AI-dataset use and its README claims AI-generated derivatives are "void grants". Any future temptation to borrow Reticulum/LXMF code or specs for this AI-assisted codebase is a legal risk. Avoid it entirely.
7. **Overlay networks put the Play "proxy services" issue back.** Yggdrasil and Mycelium nodes forward others' traffic by default ("The node **will still route traffic** as normal", Mycelium README). Any overlay on phones must be leaf-only.

## 5. Suggested SAFE local experiments (no public infrastructure touched)

- **X1: IP-SAN relay trust in iroh (local).**
  - Run a local `iroh-relay` 1.2 (server feature, manual certificates) on a LAN IP.
  - Issue it a certificate from **Pebble** (Let's Encrypt's local test ACME server) via `instant-acme` with `Identifier::Ip` and the `shortlived` profile.
  - Configure a client with Pebble's root through `ca_tls_config`, point it at `https://<lan-ip>`, and add or remove the relay at runtime via `insert_relay`.
  - Verify: IP-SAN verification works, QAD returns the observed address, and relayed traffic flows between two endpoints with direct paths blocked. Measure relay RAM and CPU per 100 relayed connections.
- **X2: reachability self-test design (offline simulation).**
  - Implement "can I be a relay?" as: portmapper result, plus a check of whether my external port 443 is free, plus a **peer-performed** inbound probe (a group member dials the candidate URL). Test it in network namespaces with a CGNAT, full-cone and firewalled-IPv6 matrix (R1's E3b setup).
  - No public probing. Real-world share of qualifying desktops: gather later through opt-in, in-app telemetry.
- **X3: Send round trip (local).**
  - Run `timvisee/send` in Docker locally.
  - Upload and download a 5 MB encrypted blob with a Rust client (ffsend-api, or a minimal reimplementation) using expiry, a download limit and owner-token deletion.
  - Put a pointer in a local Mainline testnet (the `mainline` crate's testnet) and republish it from a second, keyless node.
- **X4: Yggdrasil in-process (local).**
  - Run a 3-node local Yggdrasil network (no public peers) with yggstack/contrib-mobile. Prototype an iroh `unstable-custom-transport` over key-addressed Yggdrasil packets.
  - Measure added latency, CPU and the arm64 size of the gomobile AAR.
  - Check whether a single-peer leaf node can be prevented from forwarding.
- **X5: tracker rendezvous (local).**
  - Run a local `aquatic` or `opentracker`. Announce rotating info-hashes from two clients and confirm what IP and port data the other side learns, and what the operator sees.
- **X6: MQTT signaling (local).**
  - Run a local Mosquitto. Measure message size limits and retained-message behavior, and prototype iroh address exchange plus a simultaneous-dial trigger over MQTT between two local endpoints with the relay disabled.

## 6. Ideas checked and rejected (explicit list)

p2panda, Earthstar/Willow, Radicle, Matrix P2P/Pinecone (and Dendrite), Yggdrasil (CONSIDER, low), cjdns (light check), Reticulum/LXMF, Meshtastic, GNUnet, Nym, Lokinet, AT Protocol, ActivityPub (constraint-based, not fetched), OpenDHT/Jami, Tox, Holochain/kitsune2, Mycelium, public MQTT brokers (CONSIDER, low), WebTorrent WSS trackers, public UDP/HTTP trackers (CONSIDER, low), XMPP in-band-registration providers, Automerge public sync server, Jazz, ElectricSQL, Firebase Spark, Ditto, GUN, Scuttlebutt rooms, trycloudflare, bore.pub, sslip.io/nip.io, PeerJS cloud, Tailscale DERP, IRC (Libera), SMS, 0x0.st, catbox/litterbox, Ethereum Swarm, Send instances (CONSIDER), and Let's Encrypt IP certificates for helper relays (CONSIDER).

**Not evaluated** (ideas without enough evidence reachable with the available tools): Apple/Google "Find My" crowd networks as a data channel (almost certainly a ToS violation); Briar Mailbox beyond confirming its model (a user's own spare device as a Tor-reachable mailbox, the same as our tier-1 member device); Android Nearby Connections (already §6 layer 9, nothing new).

## 7. Sources

- Let's Encrypt, short-lived and IP certificates GA (2026-01-15): https://letsencrypt.org/2026/01/15/6day-and-ip-general-availability/
- Let's Encrypt, first IP-address certificate (2025-07-01): https://letsencrypt.org/2025/07/01/issuing-our-first-ip-address-certificate/
- Let's Encrypt rate limits: https://letsencrypt.org/docs/rate-limits/
- Public Suffix List: https://publicsuffix.org/list/public_suffix_list.dat
- nip.io / sslip.io: https://nip.io/
- instant-acme: https://crates.io/crates/instant-acme, https://docs.rs/instant-acme/latest/instant_acme/enum.Identifier.html, https://docs.rs/instant-acme/latest/instant_acme/struct.NewOrder.html
- n0 tokio-rustls-acme (DNS-only identifiers): https://github.com/n0-computer/tokio-rustls-acme/blob/main/src/acme.rs
- iroh 1.2 Endpoint and Builder: https://docs.rs/iroh/latest/iroh/endpoint/struct.Endpoint.html, https://docs.rs/iroh/latest/iroh/endpoint/struct.Builder.html
- iroh-relay 1.2: https://docs.rs/iroh-relay/latest/iroh_relay/
- Send instances list: https://github.com/timvisee/send-instances ; Send server config: https://github.com/timvisee/send/blob/master/docs/docker.md ; status badge: https://raw.githubusercontent.com/tdulcet/send-instances-status/HEAD/api/send-vis-ee/uptime.json
- Mozilla on the Send shutdown: https://blog.mozilla.org/blog/2020/09/17/update-on-firefox-send-and-firefox-notes/
- ffsend / ffsend-api: https://github.com/timvisee/ffsend, https://crates.io/crates/ffsend-api
- pkarr README: https://github.com/pubky/pkarr
- newTrackon: https://newtrackon.com/api/stable, https://newtrackon.com/api/all
- BEP 15 (UDP tracker): https://www.bittorrent.org/beps/bep_0015.html
- Trystero: https://github.com/dmotz/trystero (README, packages/mqtt/src/index.ts, packages/torrent/src/index.ts)
- bittorrent-tracker WS parsing: https://github.com/webtorrent/bittorrent-tracker/blob/master/lib/server/parse-websocket.js
- test.mosquitto.org: https://test.mosquitto.org/
- HiveMQ public broker: https://www.hivemq.com/mqtt/public-mqtt-broker/
- EMQX public broker: https://www.emqx.com/en/mqtt/public-mqtt5-broker
- shiftr.io: https://www.shiftr.io/try
- rumqttc: https://crates.io/crates/rumqttc
- Yggdrasil: https://yggdrasil-network.github.io/faq.html, https://yggdrasil-network.github.io/about.html, https://github.com/yggdrasil-network/public-peers, https://publicpeers.neilalexander.dev/, https://github.com/yggdrasil-network/yggdrasil-go (LICENSE, contrib/mobile/mobile.go), https://github.com/yggdrasil-network/yggstack, https://github.com/yggdrasil-network/yggdrasil-android/releases
- Mycelium: https://github.com/threefoldtech/mycelium (README, LICENSE, mobile/Cargo.toml)
- XMPP providers: https://providers.xmpp.net/, https://data.xmpp.net/providers/v2/providers-A.json ; yax.im ToS: https://yaxim.org/yax.im/tos/ ; tokio-xmpp: https://crates.io/crates/tokio-xmpp
- OpenDHT: https://github.com/savoirfairelinux/opendht, https://github.com/savoirfairelinux/opendht/wiki/API-Overview, https://github.com/savoirfairelinux/opendht/wiki/Push-notifications-support
- p2panda: https://github.com/p2panda/p2panda (p2panda-net README, examples, src/node.rs), https://crates.io/crates/p2panda-net
- Earthstar / Willow: https://github.com/earthstar-project/earthstar, https://github.com/earthstar-project/willow-rs
- Radicle seeder guide: https://radicle.dev/guides/seeder
- Pinecone: https://github.com/matrix-org/pinecone ; Dendrite: https://github.com/matrix-org/dendrite
- Reticulum: https://github.com/markqvist/Reticulum (LICENSE, README)
- Tox nodes: https://nodes.tox.chat/json ; https://github.com/TokTok/c-toxcore
- Holochain: https://github.com/holochain/docs-pages (running-network-infrastructure.md), https://github.com/holochain/android-service-runtime
- Nym: https://github.com/nymtech/nym (sdk/README.md, documentation/backup-pages/old-dev-docs/credentials.mdx)
- Lokinet: https://github.com/oxen-io/lokinet
- GNUnet: https://www.gnunet.org/en/about.html
- Bluesky rate limits: https://github.com/bluesky-social/bsky-docs/blob/main/docs/advanced-guides/rate-limits.md
- Automerge sync server: https://github.com/automerge/automerge.github.io (content/docs/tutorial/network-sync.md)
- Jazz: https://github.com/garden-co/jazz ; ElectricSQL: https://github.com/electric-sql/electric ; Firebase pricing: https://firebase.google.com/pricing ; FCM v1 auth: https://firebase.google.com/docs/cloud-messaging/send/v1-api ; Ditto pricing: https://www.ditto.com/pricing ; GUN: https://github.com/amark/gun
- trycloudflare: https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/
- bore: https://github.com/ekzhang/bore ; PeerJS: https://peerjs.com/peerserver.html
- Tailscale: https://tailscale.com/kb/1118/custom-derp-servers, https://tailscale.com/terms
- Libera.Chat policies: https://libera.chat/policies/
- Google Play SMS/Call Log policy: https://support.google.com/googleplay/android-developer/answer/10208820
- 0x0.st: https://0x0.st/ ; catbox FAQ: https://litterbox.catbox.moe/faq.php
- Swarm postage stamps: https://docs.ethswarm.org/docs/concepts/incentives/postage-stamps/
- Meshtastic: https://meshtastic.org/docs/introduction/
- Briar Mailbox: https://github.com/briar/briar-mailbox
- Earlier project research cited: `docs/research/R1-iroh.md`, `R4-android.md`, `R5-prior-art.md`, `DECISIONS.md` (D11–D23)

---

## Independent verification

*2026-09-24, skeptical verifier. I re-fetched primary sources myself instead of relying on the report's citations: the Let's Encrypt GA post, 2025-07 post and rate-limit page; the Public Suffix List (downloaded and grepped); iroh `main` source via `gh api` (`iroh-relay/src/client/tls.rs`, `iroh/src/net_report.rs`, `iroh-dns/src/dns.rs`); crates.io API for iroh, iroh-relay, instant-acme, ffsend-api, rumqttc, tokio-xmpp and opendht; the `timvisee/send-instances` README; the `tdulcet/send-instances-status` uptime JSON for every listed instance; the GitHub metadata for `timvisee/send` and `ffsend`; the Mosquitto, HiveMQ, EMQX and shiftr.io pages; Trystero's `packages/mqtt/src/index.ts`; newTrackon `/api/stable`; publicpeers.neilalexander.dev; yggdrasil-go `src/config/config.go`; `providers-A.json`; the OpenDHT wiki (raw); Nym's current zk-nym docs; GNUnet's about page; and the Mycelium README. I sent no traffic to any relay, broker, tracker, DHT or Send instance.*

### Verdict checks

| Candidate | Report | Verifier | Notes |
|---|---|---|---|
| Helper-hosted iroh relays + LE IP certificates | CONSIDER (medium) | **Upheld, with two corrections** | Facts confirmed: GA on 2026-01-15, "valid for 160 hours", "IP address certificates must be short-lived certificates", http-01 and tls-alpn-01 only, 50 per IPv4 or IPv6 /64 per 7 days. instant-acme 0.8.5 and iroh/iroh-relay 1.2.0 (2026-09-09) both exist. **Correction 1 (new, from code reading): bare-IPv6 relay URLs probably fail in iroh 1.2.** Both the relay TLS dial (`tls_servername`) and the QAD probe pass `url.host_str()` to rustls `ServerName::try_from`. For IPv6, `host_str()` includes the brackets (`[2001:db8::1]`), and neither the DNS-name nor the IP parser accepts them, so the result is `InvalidTlsServername`. IPv4 URLs parse as `ServerName::IpAddress`, and the DNS layer short-circuits IP hosts. So **the "open IPv6" half of the qualifying population is blocked unless iroh is patched upstream or the helper uses an sslip.io-style name.** Test this in X1. **Correction 2 (widens the population):** port 80 or 443 is needed only while a certificate is being validated, about every 3–5 days. The relay itself can serve on any port, since the certificate is bound to the IP, not the port. A short-lived UPnP mapping to a high internal port also avoids Linux's privileged-port barrier. LE's own caveat also applies: residential IPs "may change often, possibly without warning, and ... may be assigned to somebody else". |
| Public Send instances | CONSIDER (medium) | **Upheld, but weaker than stated** | 19 instances are listed (not ~20), with 3–30 days of retention and 50 MiB to 250 GiB size limits. **But the status repo's uptime badges show 3 instances at 25–68%, including the only 250 GiB one (25%) and the 999,999-download one (31%), and 3 more at 76–84%.** About 13 are above 96%. The **server repo `timvisee/send` was last pushed 2025-07-01** and `ffsend-api` on 2024-08, so the whole stack is dormant, not only the Rust client. The protocol is at least stable: ffsend 0.2.77 (2025-02) still builds on ffsend-api 0.7.3. For G6 it is secondary to L5's Google Drive blob-store option. Treat it as overflow, not "best option". |
| Public BitTorrent UDP/HTTP trackers | CONSIDER (low) | **Upheld** | Re-fetched `/api/stable`: 53 trackers (33 UDP, 20 HTTP) across about 45 distinct registrable domains. The BEP 15 declared-port caveat stands. Added risk: several stable trackers are piracy-adjacent, and tracker traffic is commonly blocked on school and corporate networks. |
| Public MQTT test brokers | CONSIDER (low) | **Not upheld for shipping: REJECT as a default, allow as dev/test only** | Quotes confirmed. Only Mosquitto allows "any application", and it adds "do not ... rely upon it" and "anybody could be listening". HiveMQ says "not intended for ... production data". EMQX and shiftr.io frame their brokers as test and prototype services. A shipped app sending traffic by default is production use for 3 of the 4 operators. That is a worse position than n0, which at least publishes rate limits for its free relays. G5 redundancy over Nostr plus trackers is marginal. Trystero's defaults (5 URLs) are confirmed but are not permission. |
| Yggdrasil overlay | CONSIDER (low) | **Upheld (low), with updated numbers and one extra cost** | Live page at 16:02 UTC: **308 of 447 peers online** (the report said about 250–270 of 400+). v0.5.14 (2026-06-19); yggdrasil-android was last pushed 2026-03. **Extra cost:** yggdrasil-go's config says "Yggdrasil has no concept of bootstrap nodes - all network traffic will transit peer connections" and tells users to "only peer with nearby nodes". Without direct peering, **100% of app bandwidth goes through volunteers**, which is worse than iroh relays, where most traffic goes direct after hole punching. The leaf-only concern is probably solvable by topology: one outbound peer, no `Listen`, multicast off. |
| XMPP IBR providers | REJECT (medium) | **Upheld; one count corrected** | `providers-A.json` today: 15 providers, 12 "private person", MAM 0–365 days, upload 20 MB–10 GB, kept 0–31 days. **9 of 15** (not 8) require a CAPTCHA. tokio-xmpp 6.0.0 is MPL-2.0 (confirmed). |
| OpenDHT / Jami | REJECT (medium) | **Upheld; the G3 reasoning is partly wrong** | Confirmed: 10-minute default lifetime, 64 KiB values, v4.4.0 (2026-09-04), and the crates.io crate is 0.5.2, GPL-3.0, 2021. **But the wiki also says the proxy "uses a GoRush or UnifiedPush push gateway".** A UnifiedPush endpoint is not tied to an app certificate, so a DHT proxy could wake a phone through the user's UnifiedPush distributor. This adds nothing over L4's direct UnifiedPush/ntfy route, and it depends on Jami's proxy operator (unverified for third-party use). The net verdict is unchanged. |
| Mycelium | REJECT (low) | **Upheld** | README: "A couple of public nodes are provided"; about 10 addresses, one company, v0.7.10 (2026-06-05). |
| WebTorrent WSS trackers | REJECT (low) | **Upheld** | Trystero's README: "far less relay redundancy than Nostr". |
| Nym mixnet | REJECT (medium) | **Upheld; the evidence is outdated** | The cited quote comes from `documentation/backup-pages/old-dev-docs`. Nym's **current** zk-nym page says "SDK integrations currently connect to the Mixnet without requiring credentials". Its stated model for apps is "a company purchasing zk-nyms to distribute to their app users" (crypto or fiat into NYM tokens). So it is free today, but the planned app model is paid and single-network. Rejecting it on $0 and dependency grounds still holds. |
| GNUnet | REJECT (medium) | **Upheld** | "still in an early alpha stage when it comes to software" confirmed; no mobile mention. |

### What the report missed or under-weighted

1. **iroh 1.2 likely can't use bare-IPv6 relay URLs** (brackets in `host_str()` break `ServerName::try_from`, in both the TLS and QAD paths). That shrinks the LE-IP-cert relay population to public-IPv4 desktops unless iroh gets an upstream fix. This is a small patch and worth an issue or PR to n0.
2. **Validation ports and serving ports are separate.** Only renewal needs 80 or 443 inbound, for a few seconds every few days. This lets helpers whose ISP blocks one of those ports still qualify (tls-alpn-01 on 443 or http-01 on 80), and it lets Linux helpers avoid root.
3. **`libp2p.direct` is on the Public Suffix List** (checked). Unlike sslip.io and nip.io, each `<peerid>.libp2p.direct` therefore gets its own LE rate-limit bucket. L2 already noted AutoTLS (single operator, Interplanetary Shipyard winding down). As a *fallback* naming path for IPv6-only helpers, it is better than sslip.io on rate limits but equal on dependency. L8's fallback discussion should cite it.
4. **Send instance health is uneven**, and the headline 250 GiB and 999,999-download figures come from instances at 25–31% uptime. The health tracker should weight instances by the public status data.
5. **Yggdrasil has no direct path**: all traffic transits peers. This matters more than the Go runtime for volunteer acceptance.
6. **Cross-report ranking:** L5's Drive blob store is ranked above Send for G6. L3's public STUN (ADOPT) already partly covers G2 address discovery, so the relay-plus-QAD value of the LE-IP path matters mainly for **G4 relaying and punch coordination**, not discovery.

### Bottom line

The report's facts hold up well: no fabricated numbers, and only small count drifts (CAPTCHA 9 not 8, Yggdrasil 308/447, 19 Send instances). The main finding, helper relays with LE IP certificates, survives with a **new technical blocker for IPv6**. Its value is now concentrated on public-IPv4 desktops with UPnP, which X1–X2 must measure. MQTT test brokers should drop to REJECT for shipped defaults. Send stays CONSIDER as a best-effort overflow channel on a dormant stack. No REJECT was overturned. G1 (small, frequent offline updates) and G3 remain open, as the report says.
