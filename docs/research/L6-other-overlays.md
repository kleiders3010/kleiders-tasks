# L6: Other P2P overlays (HyperDHT/Hyperswarm, Tor/Arti, I2P, Syncthing relays, BitTorrent / Mainline DHT / WebTorrent)

*2026-09-24. Research only. I changed no design files, built no code and sent no traffic to any relay or overlay. The only network access was web and API fetches of documentation, source and registry metadata. One read-only GET of the Syncthing relay-pool list and one of the Tor Onionoo summary were made to count operators.*

Gaps that each candidate is judged against:
- **G1** offline store-and-forward that doesn't depend on Nostr
- **G2** internet P2P connectivity that doesn't depend on n0 (address discovery plus hole-punch coordination)
- **G3** waking sleeping Android phones
- **G4** free relaying when hole punching fails
- **G5** discovery, presence and small signaling messages
- **G6** large blobs

---

## Summary

1. **None of these overlays closes G1 (offline store-and-forward) or G3 (waking a sleeping phone).**
   - Every storage primitive they have is short-lived by design:
     - HyperDHT records last **20 min**;
     - Mainline BEP44 items "MAY expire in 2 hours" (pkarr says "hours or days");
     - Tor, I2P and Syncthing relays need both ends online.
   - Nothing here can wake a Dozing phone.
   - This is a negative result, and it's firm. Nostr, member devices and the helper network stay the only offline paths.
2. **HyperDHT (Holepunch) is the strongest candidate for G2 and G4, and it fits the helper network well.** Verdict: **CONSIDER, medium confidence.**
   - **Hole punching is coordinated by DHT nodes, not by a relay operator.** The NAT-classification code includes birthday-style punching (256 sockets, 1,750 probes) for consistent↔random NAT pairs.
   - **Relaying is "blind".** The relay is reached with an ordinary DHT `connect`, so it can itself sit behind a hole-punchable NAT. That means **our own helpers (desktops and member devices) can act as the ~30% fallback relay without a public IP or TLS**, which iroh relays can't do (D14 option d).
   - **The caveats are serious:**
     - The only complete Rust port, `peeroxide`, is 3 months old, has one maintainer and 18 stars.
     - Bootstrap defaults to 3 Holepunch-run nodes.
     - The DHT is IPv4-only.
     - It would be a second transport stack next to iroh.
3. **Mainline DHT (BEP44 via `mainline` / `pkarr`) should be adopted for G5.** It also gives a partial, n0-independent G2: BEP42 says "all DHT responses SHOULD include a top-level field called ip" (the requester's reflexive address), so address discovery comes from many independent nodes. Verdict: **ADOPT for small signaling records only, medium-high confidence.**
   - It is already half in the design (pkarr).
   - **Configure it as DHT-only.** pkarr's default client also uses two Pubky-run HTTP relays.
   - Phones run as read-only clients.
   - Desktops and helpers can re-put other members' signed records so they outlive a Dozing phone.
4. **Tor onion services via Arti work around NATs completely (G2 and G4) on ~9,400 volunteer relays, but only while the host device is awake and online.** Verdict: **CONSIDER, narrowly: desktop-hosted onion service as a last-resort path to tier-1 member desktops and helpers, low-medium confidence.**
   - **Arti's own docs disagree about onion-service hosting.** The README says Arti "can run and connect to onion services", and the 1.8.0 changelog "Removed security disclaimer about onion service support". `doc/OnionService.md` still says "suitable for testing and experimentation only".
   - The `onion-service-service` API is excluded from semver guarantees.
   - **Precedent is poor.** Quiet (a "no servers, just Tor" app) drew battery complaints (24% in 8 h idle, Pixel 6a) and has built a central server (QSS) plus Android push.
5. **Rejected:**
   - **I2P** (i2pd / emissary): slow integration, transit routing on phones, ~15 volunteer reseed servers, no fit to any gap.
   - **Syncthing public relay pool:** the relays are "available to Syncthing users", using them means pretending to be Syncthing (`bep-relay` ALPN), and it needs a permanent TCP join, so the ethics and Android fit are both bad.
   - **WebTorrent / BitTorrent for blobs:** needs WebRTC trackers, publishes infohashes on a heavily monitored public DHT, has no store-and-forward, and offers nothing over an iroh or HyperDHT direct stream between members.

---

## 1. Holepunch HyperDHT / Hyperswarm

**What it is.** A Kademlia DHT (dht-rpc) whose nodes also relay connection handshakes and coordinate UDP hole punching between a client and a "server" keypair. Streams run over UDX (reliable UDP) and are encrypted end to end with Noise XX and SecretStream. Hyperswarm adds topic-based discovery on top.

**Gaps covered:**
- **G2: yes.**
- **G4: yes, via blind relays that we'd provide ourselves.**
- **G5: yes, but short-lived.**
- **G6: only between online peers.**
- **G1 and G3: no.**

**How connectivity works (from source):**
- Coordination comes from the DHT, not from a relay operator. The server announces itself to DHT nodes close to its key. The client's `PEER_HANDSHAKE` and `PEER_HOLEPUNCH` messages are routed through those nodes (`lib/constants.js` COMMANDS 0/1). README: "You can run servers on normal home computers, as the DHT will UDP holepunch connections for you."
- NAT classes are `FIREWALL = { UNKNOWN, OPEN, CONSISTENT, RANDOM }`.
  - `lib/holepuncher.js` uses `BIRTHDAY_SOCKETS = 256`, and `_randomProbes` sends `tries = 1750 // ~35s` when the local side is consistent and the remote is random.
  - When **both** sides are random it aborts with `HOLEPUNCH_DOUBLE_RANDOMIZED_NATS` (`lib/connect.js` l.668) unless a relay is configured.
  - So it goes further than iroh on hard NATs: iroh has "no port prediction" and "hard↔hard unsupported" (R1). But phone↔phone where both carriers use symmetric NAT still needs a relay.
- **Relaying is blind:** `relayThrough` can be a key, a list or a function (`selectRelay`). The relay is reached with `c.dht.connect(publicKey)` (`relayConnection`, `lib/connect.js` l.793–830), which is itself hole-punched, and the relay pairs streams by token.
  - blind-relay README: "a host may accept pairing requests from other hosts and relay UDX stream messages between them, similar to … TURN".
  - Pear's sidecar only relays when needed: `relayThrough: (force) => (force || this.dht.randomized ? this.relayPublicKey : null)`.
  - **So a helper desktop behind a consistent (punchable) NAT can relay for two phones on random NATs. No public IP or TLS certificate is needed.** This is the missing piece for D14/G4 inside our constraints.
  - peeroxide's docs add a caveat: a dedicated relay "should usually run at a stable, reachable UDP address… it is not designed to depend on its own holepunching sequence". **Unverified how well a NATed relay works in practice; see experiment X1.**
- Signaling and presence primitives (peeroxide `dht-primitives.md`, matching Node):

  | Primitive | Behavior |
  |---|---|
  | `mutable_put` | ~1,002 B value, Ed25519-signed, strict `seq`, **no salt** (one slot per keypair) |
  | `immutable_put` | ~1,000 B |
  | `announce` / `lookup` | multi-writer, "up to 20 announcers per topic per node" |
  | TTL | "Announcement records… 20 minutes", "Mutable / immutable LRU cache 20 minutes" |

  Fine for "I'm online at key K" presence. Useless for offline mailboxes.

**Cost to users and developer:**
- $0; no developer infrastructure is needed.
- Runs on Holepunch's public mainnet. The README says "The default bootstrap servers are publicly served on behalf of the commons."
- An isolated network is possible (`bootstrap: []`), but it needs "at least one persistent node" and "~30 mins for the node to become persistent". That would put us back in the hosting business unless helpers form the network.

**ToS / third-party use:**
- No ToS was found.
- "Publicly served on behalf of the commons" is the only statement, and it's permissive in spirit.
- Unlike n0's relays, there's no "development and hobby use only" clause.

**Operators and sustainability:**

| | |
|---|---|
| Bootstrap | 3 nodes, all Holepunch: `88.99.3.86@node1.hyperdht.org` (Hetzner), `142.93.90.113@node2`, `138.68.147.8@node3` (DigitalOcean) |
| After bootstrap | Routing and handshake relaying are done by every non-firewalled persistent node (Keet/Pear desktops, Holepunch nodes) |
| Mainnet size | **Unknown**: no public statistic found |
| Funding | Holepunch (Tether-funded; R5) |
| Activity | hyperdht v6.34.0 (2026-09-03), 409 stars, pushed 2026-09-23 |

- **Bootstrap is a single-provider dependency, but only at bootstrap.** A client can bootstrap from any node it knows, such as cached nodes or our own helpers. That's weaker than n0's grip, where relays are needed for every internet connection.

**Reliability and limits:**
- 20-min record TTL; ~1,000 B values; 20 announcers per topic per node; IPv4 only (`Ipv4Peer` in the wire format; `MAX_RELAY_ADDRESSES 3`).
- `randomPunchInterval` defaults to 20 s between random punches, which limits bursts.

**Privacy and metadata:**
- DHT nodes near an announced key learn the announcer's IP and that it is online.
- The server's firewall callback sees the client's IP ("remoteHandshakePayload contains their ip").
- Topics aren't secret ("Topic keys are not secret", peeroxide security model).
- The metadata picture is like pkarr's (D23): rotate announce keys and topics from the epoch key.

**Android feasibility:**
- **Precedent: Keet ships on Android and iOS** ("Keet is available for both Android and iOS devices", keet.io), using the JS stack inside Bare / bare-kit (Apache-2.0; Android `to.holepunch.bare.kit.Worklet`).
- For us, a Rust port avoids embedding a JS runtime.
- Phones should act as clients or foreground-only servers. The same Android limits apply (network cut ~5 s after backgrounding, D16).
- Birthday punching opens up to 256 sockets for ~35 s: a burst of radio activity, acceptable in the foreground and unmeasured on battery.
- No special permissions are needed beyond INTERNET.

**Library maturity:**

| Crate | Status |
|---|---|
| **`peeroxide` / `peeroxide-dht` / `libudx` 1.7.3** (MIT OR Apache-2.0) | Created 2026-04-25, last release 2026-07-12, 18 stars, 1 forked copy, ~5.6k downloads. **One human contributor** (32 commits) plus release bot. The repo has AGENTS.md files and may be AI-assisted. |
| `cowlicks/hyperswarm` (`hyperdht` 0.1.1, `hyperswarm` 0.1.0, `dht-rpc` 0.0.3, MIT) | Single release each (June–Aug 2026); early |

- peeroxide is pure Rust (tokio, RustCrypto, `if-addrs`; no C). It claims wire compatibility "validated via golden byte fixtures and live cross-language interop tests".
- It implements birthday sockets (`holepuncher.rs`: `BIRTHDAY_SOCKETS: usize = 256`, `for _ in 0..1750`) and a blind-relay client and server ("NAT holepunch hardening + blind-relay server implementation (#27)", 2026-07-12).
- It has Docker NAT-simulation tests.
- Binary size is unmeasured. Its dependencies overlap heavily with what we already ship (ed25519-dalek, blake2, chacha20poly1305, tokio).
- **Protocol-drift risk:** Holepunch ships hyperdht releases every few weeks, and a one-person port must keep up.

**Abuse resistance:**
- Any node can join; Sybil and eclipse attacks are possible ("susceptible to Sybil attacks in the general case"; "An attacker controlling all bootstrap nodes can eclipse a node").
- Noise XX with a known server key prevents impersonation; an attacker can only cause DoS.

**Failure modes:**
- Holepunch changes the protocol or retires public bootstrap nodes.
- The Rust port is abandoned.
- IPv4-only paths fail on IPv6-only networks without CLAT.
- Double-random NATs fail without a relay.
- Mainnet shrinks if Keet declines.

**Verdict: CONSIDER, medium confidence.** It is the only candidate here that gives relay-free hole-punch coordination **and** a relay model our own NATed helpers can serve. Before any commitment it needs X1 (local NAT matrix plus NATed relay) and X2 (Android `.so` size and battery). Treat peeroxide as something we might have to fork and own.

---

## 2. Tor onion services via Arti (Rust)

**Gaps covered:**
- **G2 and G4: yes, completely.** An onion service is reachable from anywhere regardless of NAT, with relaying done by Tor relays. But only while the host is running and online.
- **G5: partial** (the .onion address is a stable rendezvous).
- **G1 and G3: no.**
- **G6: discouraged** by Tor's bulk-traffic ethic.

**Maturity (evidence conflicts, so read carefully):**
- README (Aug 2026): "Arti is a full-featured Tor client… It can run and connect to onion services. We believe that Arti is suitable for general client usage."
- Arti 1.8.0 changelog (Dec 2025): "Removed security disclaimer about onion service support (!3464)".
  - Vanguards, restricted discovery ("Implement hidden service restricted discovery mode", 1.2.7) and `rate_limit_at_intro` exist.
  - Service-side PoW is "Experimental" (1.5.0); `hs-pow-full` links LGPL code.
- **But** `doc/OnionService.md` (last content change 2025-03) still says: "Arti's Onion Service (hidden service) support is **suitable for testing and experimentation only** and should not be used for anything you care about. It **may even compromise the privacy of your other uses of the same Arti instance!**" It also lists missing DoS protection.
- arti-client docs: experimental-feature APIs "are NOT covered by semantic versioning guarantees: we might break them or remove them between patch versions". arti-client is still 0.46 (2026-09-02), with "a certain amount of breakage between now and us declaring arti-client 1.x".
- License: MIT OR Apache-2.0 (LGPL only with `hs-pow-full`). Dependencies include bundled `rusqlite` for the directory cache. Binary size is **unmeasured**.
- `doc/Android.md`: "Arti does not have a stable Rust API yet… no proper bindings are provided". It references NDK 22 and is stale, but Android builds are reproducible (changelog).

**Cost:** $0, with no developer infrastructure.

**Operators:**
- **9,385 running relays** (Onionoo, 2026-09-24 15:00), run by many independent volunteers and organizations.
- Directory authorities are Tor-run.
- This is by far the most independent operator base among the candidates.

**Policy and ethics:**
- There are no ToS for using the network.
- Tor's long-standing ask is to avoid bulk traffic. "BitTorrent over Tor isn't a good idea" (2010) argues that bulk transfer strains donated capacity used by people with real anonymity needs.
- Small sync messages are fine; blobs over Tor are not.

**Reliability and latency:**
- Onion-service connection setup needs rendezvous and 6-hop circuits, a multi-second setup (not measured here).
- A service must keep intro-point circuits up and republish descriptors.
- Quiet reports "Hidden-service descriptor timeout wedges community creation" (#3594, 2026-09-19).

**Android:**
- A phone cannot host a reachable onion service in the background: the network is cut ~5 s after backgrounding (D16), which drops the intro circuits.
- Each WorkManager window would re-bootstrap circuits and refresh the consensus (consensus freshness is shorter than Doze gaps).
- **Precedent:**
  - Quiet #2152: "Quiet had consumed 24% of my battery since the last charge about 8 hours ago… no screen time" (Pixel 6a, Android 14). That app also runs Node, IPFS and OrbitDB, so not all of it is Tor.
  - Quiet now has QSS server-backed communities and "Android push: native QSS prerequisites" (#3460), which moves away from "no servers, just Tor".
  - Briar and Orbot keep Tor alive with foreground services.
- Tor apps are accepted on Play (Orbot, Briar, Tor Browser).

**Privacy:**
- Hides IPs from peers, which is better than any other layer.
- But network observers see that the device uses Tor. Tor is blocked on some networks and draws scrutiny in some jurisdictions. That's surprising for a grocery-list app.

**Abuse resistance:** strong for the service (PoW, rate limits at intro points). Clients are anonymous, so there's no IP-based rate limiting of peers; group-key authentication handles that.

**Failure modes:** censorship or blocking; slow cold bootstrap; API breakage between Arti patch versions; onion-service bugs.

**Verdict: CONSIDER, narrowly, low-medium confidence.**
- **Only use:** a desktop or tier-1 member device (always-on PC, headless helper) publishes one per-group onion service, used as the last-resort path when iroh, HyperDHT and member relays all fail.
- Phones are Tor *clients* only, in the foreground or during their sync jobs, for small messages.
- Not for phones as hosts, not for blobs, not on by default until Arti's onion-service docs agree with each other and X4 measures size and latency.

---

## 3. I2P (i2pd C++ / Java I2P / emissary Rust)

**Gaps covered:** G2 and G4 in principle (garlic-routed tunnels, NAT-agnostic), with both ends online. **G1, G3, G5 and G6: no** in a usable form.

**Maturity:**

| Implementation | Status |
|---|---|
| i2pd | BSD-3, v2.61.0 (2026-07-20), 4.2k stars; Android app `i2pd-android` (BSD-3) |
| emissary (Rust) | MIT, `emissary-core` 0.4.0 (2026-04-12), 248 stars ("lightweight and embeddable I2P router", NTCP2/SSU2, SAMv3) |
| Rust SAM library | `yosemite` 0.7.0 (MIT) |
| Older crates | `i2p` 0.0.1 (2017) and `i2p_client` (2020) are dead |

**Operators:**
- The network is routers run by users.
- Bootstrap depends on HTTPS **reseed servers**: 15 are hard-coded in i2pd `Config.cpp` and ~15 in emissary, all run by individual volunteers (e.g. `reseed.stormycloud.org`, `reseed-fr.i2pd.xyz`, `reseed.diva.exchange`).

**Integration:**
- i2pd's FAQ advice is "Run i2pd for a long time… to better integrate with the network".
- The I2P FAQ: "New routers will need time to startup and connect to the rest of the network." Its latency example gives 740 ms round trip for a 3-hop tunnel pair.
- Routers are expected to route transit traffic for others. On a phone that is the "proxy services to third parties" exposure that R8 flagged, and it costs battery.

**Android:** the router needs long uptime, which contradicts Doze and the 5-s background network cut. There are no in-process mobile precedents at our scale.

**Verdict: REJECT, high confidence.**
- It needs a long-running router, the transit burden is a Play-policy risk, and the network is small and slow.
- Its only benefit (NAT-agnostic reachability) is delivered better by Tor (larger network) or HyperDHT (lighter).

---

## 4. Syncthing public relay pool

**Gaps covered:** G4 only (forward-only relaying, both ends online).

**Protocol:**
- Relay protocol v1: a TCP "protocol mode" over TLS with ALPN `bep-relay`. The device ID is derived from the TLS client certificate (self-signed is fine).
- A **permanent join** is needed to be reachable ("waits for SessionInvitation messages"). Then there's a plaintext "session mode" that forwards bytes, with end-to-end encryption left to the application.
- The spec: "general enough that could be reused by other protocols or applications, as the data transferred… is completely obscure".

**Terms and ethics:**
- `strelaysrv` docs: "Syncthing relies on a network of community-contributed relay servers. Anyone can run a relay server, and it will automatically join the relay pool **and be available to Syncthing users**."
- The relay author on the forum: "in theory can be used for any sort of traffic", but "I can't see why anyone would want this in general."
- No permission covers third-party apps. Operators donated capacity to Syncthing users.
- We would have to present the `bep-relay` ALPN and act like Syncthing, which is close to impersonation.

**Operators:**
- **100 relays** in the pool (fetched 2026-09-24; flat since ~2018 per R5), each volunteer-run with its own `-global-rate` / `-per-session-rate` limits.
- The pool server is run by the project.

**Android:** a permanent TCP join can't survive backgrounding. Usable only while both apps are in the foreground.

**Verdict: REJECT, medium-high confidence** (ethics and terms, impersonation, background fit).
- The protocol is a good *model* for our own helpers' forwarding, but HyperDHT's blind relay is the better model because it is punchable and needs no public TCP port.

---

## 5. BitTorrent Mainline DHT (BEP44) for signaling; BitTorrent / WebTorrent for blobs

### 5a. Mainline DHT / BEP44 (signaling records)

**Gaps covered:**
- **G5: yes.**
- **G2: partial.** Reflexive address discovery comes from many independent nodes via the BEP42 `ip` field. The Mainline DHT gives no hole-punch coordination.
- **G1, G3, G4 and G6: no.**

**Spec limits (R5, re-confirmed):**
- Values ≤ 1,000 B; "MAY expire in 2 hours… SHOULD be re-announced once an hour"; one item per (key, salt).
- pkarr: "Records are ephemeral — The DHT drops records after hours"; "1000-byte limit — PKARR is for discovery, not storage".
- BEP43 recommends read-only nodes for phones.

**Address discovery:** BEP42: "all DHT responses SHOULD include a top-level field called ip, containing a compact binary representation of the requestor's IP and port."
- Querying several independent nodes gives our public mapping, and shows whether the NAT mapping is endpoint-independent (same port across nodes).
- That replaces QAD's role without n0. The *coordination* of simultaneous open still needs a channel: Nostr ephemeral events, the group doc, or HyperDHT.

**Operators:** "10+ million active nodes" (pkarr README). Default bootstrap in `mainline` 8.0.0 is `router.bittorrent.com`, `dht.transmissionbt.com`, `dht.libtorrent.org:25401` and `relay.pkarr.org`: four independent operators.

**Libraries:**
- `mainline` 8.0.0 (MIT, 2026-08-04; client, server and "Adaptive" mode that switches to server after 15 min with a public address; "The server implementation has no rate-limiting").
- `pkarr` 8.0.2 (MIT, 2026-09-23).
- iroh's `iroh-mainline-address-lookup` 0.5 sits on `n0-mainline` 0.6, a second implementation.
- **Caveat: pkarr's default client also uses `DEFAULT_RELAYS = ["https://pkarr.pubky.app", "https://pkarr.pubky.org"]`** (Pubky-run HTTP relays). Configure it as DHT-only or declare that dependency.

**Android:** read-only client queries in bursts during sync jobs, with no listening socket. Desktops can run in server or Adaptive mode and **re-put members' signed items** (a signed item is valid from anyone, which pkarr describes: "Friends or associates who care about keeping the records alive"). That covers the "phone asleep for 6 h, record expired" problem.

**Privacy and abuse:**
- Keys and salts derived from the epoch key are unlinkable to outsiders. DHT nodes see who puts and gets (by IP).
- Sybils: R5 cites ~300k measured in 2012; `mainline` ships vertical-Sybil mitigations.
- Avoid `announce_peer`/infohash use: it publishes IP:port to anyone who knows the hash, and the DHT is crawled heavily.

**Verdict: ADOPT for G5, and for reflexive-address probing as part of G2. Medium-high confidence.** The scope is narrow: per-device and per-group presence and endpoint records ≤ 1 KB, DHT-only, re-put by desktops and helpers. Not a mailbox.

### 5b. BitTorrent / WebTorrent for blobs (G6)

- **WebTorrent:** "uses WebRTC instead of TCP/uTP"; "we made a few changes to the tracker protocol"; browser peers "can only download torrents that are seeded by a WebRTC-capable torrent client".
  - It needs WebSocket trackers (signaling servers run by third parties) and a WebRTC stack.
  - Rust ports are immature (`webtorrent-rs` 1.0.2, 61 downloads).
- **Classic BitTorrent (`librqbit` 9.0.1, Apache-2.0, ~700 KB source crate):**
  - Swarming only helps with many seeders.
  - Group blobs have 2–10 interested devices, which a direct iroh or HyperDHT stream handles.
  - BEP55 hole punching needs "a relay peer" connected to both.
  - Announcing an infohash on the public DHT or trackers exposes IP↔blob interest on the most-monitored P2P network there is.
  - Nothing is stored while the uploader is offline, so there's no G1 value either.
- **Verdict: REJECT, high confidence (WebTorrent) and medium (BitTorrent).** For G6, D22's plan stands: a small custom ALPN "request by hash, BLAKE3 check" over whichever direct stream exists, with helpers (RAM, size-capped) possibly holding thumbnails only.

---

## Comparison vs gaps

| Candidate | G1 offline | G2 P2P w/o n0 | G3 wake | G4 free relay | G5 signaling | G6 blobs | $0 / no dev infra | Third-party use OK? | Rust maturity | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| HyperDHT / Hyperswarm | No (20-min TTL) | **Yes** (DHT-coordinated punching, birthday for random NAT) | No | **Yes, via our own helpers as blind relays** (punchable) | Yes (20 min, 1 KB, multi-writer topics) | Online peers only | Yes (bootstrap = 3 Holepunch nodes) | "publicly served on behalf of the commons" | Low (peeroxide 1.7.3, 3 months, 1 maintainer) | **CONSIDER** (medium) |
| Tor onion services (Arti) | No | **Yes** (NAT-agnostic), host must be awake | No | **Yes** (volunteer relays; small traffic only) | Partial | Discouraged | Yes | No ToS; bulk-traffic ethic | Medium (0.46; OS API not semver; docs conflict) | **CONSIDER, narrow** (low-medium) |
| I2P | No | Yes in theory; slow integration | No | Yes (tunnels) | No | Poor | Yes (volunteer reseeds) | Yes | Low (emissary 0.4) | **REJECT** (high) |
| Syncthing relay pool | No | No | No | Yes (TCP, both online) | No | Poor | Yes | "available to Syncthing users"; impersonation needed | n/a (Go) | **REJECT** (medium-high) |
| Mainline DHT BEP44 | No (≤ 2 h+, hours–days) | **Partial**: reflexive address via BEP42 `ip`; no coordination | No | No | **Yes** (1 KB, re-puttable by others) | No | Yes (4 independent bootstrap operators) | Open protocol | High (`mainline` 8.0, `pkarr` 8.0) | **ADOPT for G5** (medium-high) |
| BitTorrent / WebTorrent | No | No | No | BEP55 needs relay peer | No | Poor fit; public exposure | WebTorrent needs trackers | Yes | librqbit ok; WebTorrent-rs immature | **REJECT** |

---

## New risks found

1. **R-L6-1: none of the "decentralized overlays" solves G1 or G3.** Hopes that some existing P2P network could replace Nostr for offline delivery are unfounded. Nostr, member devices and the helper network remain the whole offline story (confirms D13's "no in-constraints alternative").
2. **R-L6-2: HyperDHT adoption risk.**
   - The protocol is controlled by one company (Holepunch/Tether) that releases often (v6.34.0 on 2026-09-03).
   - Bootstrap is 3 Holepunch-run hosts.
   - The only complete Rust port has a bus factor of 1 and may be AI-generated.
   - Adopting it likely means forking and owning peeroxide.
3. **R-L6-3: HyperDHT is IPv4-only.** It depends on Android's 464XLAT on IPv6-only carriers.
4. **R-L6-4: pkarr's hidden third-party relays.** The default pkarr client resolves through Pubky's HTTP relays. If D14's "Mainline DHT via pkarr" plan uses the defaults, it silently adds another single-provider dependency. Use `ClientBuilder` DHT-only.
5. **R-L6-5: Tor visibility.** Embedding Tor makes the app look like a Tor user to network observers (corporate, school, censored networks), and Arti's onion-service hosting docs contradict each other. Quiet, the leading Tor-only P2P app, added a central server and push. That is evidence against Tor as a mobile answer.
6. **R-L6-6: two or three transport stacks** (iroh QUIC, HyperDHT UDX, optionally Tor). Each has its own keys (D23 metadata linkage multiplies: HyperDHT server keys and onion addresses are long-lived identifiers, so derive and rotate them per group and epoch). There's also more battery and binary cost, and more attack surface for D3-style parser crashes.
7. **R-L6-7: DHT-node metadata.** On both Mainline and HyperDHT, the strangers' nodes nearest a key learn the IPs that put or get it, and when. Rotating keys (D23) limits linkage over time but not per-epoch observation.

---

## Suggested SAFE local experiments (no public network traffic)

- **X1: HyperDHT NAT matrix plus NATed blind relay** (local Docker or netns only).
  - Use peeroxide's own Docker NAT-simulation harness (TESTING.md) with an isolated bootstrap (`bootstrap: []` / `--no-public`).
  - Measure connect success and time for OPEN, CONSISTENT and RANDOM pairs, including RANDOM↔RANDOM, with a `relay_through` relay that is **itself behind a consistent NAT**. This confirms or refutes "helpers can be relays without public IPs".
  - Record packets sent during birthday punching (256 sockets, 1,750 probes) as a proxy for battery and data.
- **X2: Android footprint of peeroxide.** Cross-compile `peeroxide-dht` into the existing E6 size harness (arm64, opt-level z, LTO, strip, 16 KB alignment). Report the `.so` delta over the current 7.07 MB.
- **X3: Mainline BEP44 on a local testnet** (`mainline::Testnet`).
  - Epoch-salted per-device records.
  - The ~1,000 B payload budget after encryption.
  - Re-put of another member's signed item by a "desktop" node.
  - Read-only client behavior.
  - Reflexive `ip` field consistency across nodes behind a local NAT emulator (can we classify EIM vs symmetric mapping ourselves?).
- **X4: Arti footprint only.** Build `arti-client` with `onion-service-service` (+ `vanguards`, without `hs-pow-full`) for aarch64-linux-android and measure `.so` size and dependency licenses. Do not connect to Tor. Running behavior can later be measured on a private Chutney/Shadow network if X4 looks acceptable.
- **X5: iroh without n0 using externally learned addresses** (local netns NAT).
  - Two `RelayMode::Disabled` endpoints get their reflexive addresses from a local Mainline testnet (X3).
  - Exchange them over a local "signaling" channel, dial both sides at once, and check whether iroh establishes a direct path (`Endpoint::add_external_addr` plus address-only `EndpointAddr`).
  - Decides whether Mainline plus Nostr signaling can replace n0 for the punchable fraction without adopting HyperDHT.

---

## Sources

**HyperDHT / Holepunch**
- hyperdht README (bootstrap nodes, "publicly served on behalf of the commons", isolated network, ~30 min to become persistent): https://github.com/holepunchto/hyperdht, raw https://raw.githubusercontent.com/holepunchto/hyperdht/main/README.md
- hyperdht constants (COMMANDS, BOOTSTRAP_NODES, FIREWALL): https://raw.githubusercontent.com/holepunchto/hyperdht/main/lib/constants.js
- hyperdht connect.js (relayThrough, HOLEPUNCH_DOUBLE_RANDOMIZED_NATS, relayConnection via `dht.connect`): https://raw.githubusercontent.com/holepunchto/hyperdht/main/lib/connect.js
- hyperdht holepuncher.js (BIRTHDAY_SOCKETS 256, 1750 probes): https://raw.githubusercontent.com/holepunchto/hyperdht/main/lib/holepuncher.js
- blind-relay README: https://raw.githubusercontent.com/holepunchto/blind-relay/main/README.md
- Pear sidecar relayThrough usage: https://github.com/holepunchto/pear/blob/main/subsystems/sidecar/index.js
- autopass relayThrough usage: https://github.com/holepunchto/autopass/blob/main/index.js
- bare-kit (Android worklets, Apache-2.0): https://github.com/holepunchto/bare-kit
- Pears docs: https://docs.pears.com/ ; blind peering: https://docs.pears.com/p2p/how-to/blind-peering/
- Keet platforms / "no servers" claim: https://keet.io/
- peeroxide (Rust port): https://github.com/Rightbracket/peeroxide ; docs: `docs/src/concepts/dht-primitives.md`, `docs/src/relay/overview.md`, `docs/src/appendices/limits-and-performance.md`, `docs/src/appendices/security-model.md`, `TESTING.md`, `peeroxide-dht/src/holepuncher.rs`, `peeroxide-dht/Cargo.toml`
- crates.io: https://crates.io/crates/peeroxide , https://crates.io/crates/peeroxide-dht , https://crates.io/crates/libudx , https://crates.io/crates/hyperdht , https://crates.io/crates/hyperswarm , https://crates.io/crates/dht-rpc

**Tor / Arti**
- Arti README (status): https://gitlab.torproject.org/tpo/core/arti/-/blob/main/README.md (fetched via GitLab API)
- Arti `doc/OnionService.md` (limitations): https://gitlab.torproject.org/tpo/core/arti/-/blob/main/doc/OnionService.md
- Arti `doc/Android.md`: https://gitlab.torproject.org/tpo/core/arti/-/blob/main/doc/Android.md
- Arti CHANGELOG (1.5.0 PoW experimental; 1.8.0 "Removed security disclaimer about onion service support"; 2.6.0 2026-09-01): https://gitlab.torproject.org/tpo/core/arti/-/blob/main/CHANGELOG.md
- arti-client docs (semver caveats): https://docs.rs/arti-client/latest/arti_client/
- tor-hsservice docs: https://docs.rs/tor-hsservice/latest/tor_hsservice/
- Onionoo relay count (9,385 running, 2026-09-24 15:00): https://onionoo.torproject.org/summary?running=true&type=relay&limit=0
- Tor blog, "BitTorrent over Tor isn't a good idea" (2010-04-30): https://blog.torproject.org/bittorrent-over-tor-isnt-good-idea/
- Quiet README: https://github.com/TryQuiet/quiet ; battery issue #2152: https://github.com/TryQuiet/quiet/issues/2152 ; #2669: https://github.com/TryQuiet/quiet/issues/2669 ; QSS issues #2406, #3405, #3460, #3575, #3594: https://github.com/TryQuiet/quiet/issues/3594

**I2P**
- i2pd: https://github.com/PurpleI2P/i2pd ; reseed list in https://github.com/PurpleI2P/i2pd/blob/openssl/libi2pd/Config.cpp ; FAQ https://docs.i2pd.website/en/latest/user-guide/FAQ/
- I2P FAQ: https://i2p.net/en/docs/overview/faq
- emissary (Rust): https://github.com/eepnet/emissary ; https://crates.io/crates/emissary-core ; yosemite https://crates.io/crates/yosemite

**Syncthing relays**
- Relay protocol v1: https://docs.syncthing.net/specs/relay-v1.html (source https://github.com/syncthing/docs/blob/main/specs/relay-v1.rst)
- strelaysrv: https://docs.syncthing.net/users/strelaysrv.html
- Pool list (100 relays, 2026-09-24): https://relays.syncthing.net/endpoint
- Forum, "How to limit connections to relay?": https://forum.syncthing.net/t/how-to-limit-connections-to-relay/16314

**BitTorrent / Mainline / WebTorrent**
- BEP42 (ip field): https://www.bittorrent.org/beps/bep_0042.html ; BEP44: https://www.bittorrent.org/beps/bep_0044.html ; BEP43: https://www.bittorrent.org/beps/bep_0043.html ; BEP55: https://www.bittorrent.org/beps/bep_0055.html
- mainline crate README and bootstrap list: https://github.com/pubky/mainline (src/rpc.rs `DEFAULT_BOOTSTRAP_NODES`; src/rpc/server.rs `MAX_VALUES`)
- pkarr README, integration guide, default relays: https://github.com/pubky/pkarr (pkarr/src/lib.rs `DEFAULT_RELAYS`; docs/integration.md; docs/introduction.md)
- WebTorrent FAQ: https://github.com/webtorrent/webtorrent/blob/master/docs/faq.md
- librqbit: https://crates.io/crates/librqbit ; webtorrent-rs: https://crates.io/crates/webtorrent-rs

**Internal**
- `docs/DESIGN.md` §6–§8; `docs/research/DECISIONS.md` D11–D23; `docs/research/REPORT.md`; `docs/research/R1-iroh.md`; `docs/research/R5-prior-art.md`.

---

## Independent verification

*2026-09-24, separate verifier. Sources: GitHub API reads of the upstream source (hyperdht, peeroxide, pear, pubky/mainline, pkarr, n0-mainline, iroh, Quiet), crates.io API, the Tor GitLab API, bittorrent.org and docs.syncthing.net. I sent no traffic to any DHT, relay or overlay.*

### Verdict checks

| Candidate | Report | Result | Corrected verdict |
|---|---|---|---|
| HyperDHT via peeroxide | CONSIDER, medium | **Partly upheld.** The "our NATed helpers can be the relays" claim is weaker than reported. | CONSIDER, **low-medium** |
| Mainline BEP44 (mainline / pkarr) | ADOPT, medium-high in the body, medium in the summary | **Upheld, but narrower.** The partial-G2 claim is overstated. | ADOPT for **presence and endpoint records only**, **medium** |
| Tor onion services via Arti | CONSIDER narrow, low | Upheld | Unchanged |
| Syncthing relay pool | REJECT, medium | Upheld, with a nuance | Unchanged |

**1. HyperDHT: confirmed**
- Bootstrap is 3 nodes. The README says "publicly served on behalf of the commons" and "at least one persistent node" (lib/constants.js, README l.84).
- The `BIRTHDAY_SOCKETS = 256` and 1,750-probe loops are real.
- It aborts with `HOLEPUNCH_DOUBLE_RANDOMIZED_NATS` when both sides have random NATs.
- The relay is reached with `c.dht.connect(publicKey)` (connect.js l.809).
- peeroxide: created 2026-04-25; 18 stars; one human committer (32 commits) plus a bot; MIT OR Apache-2.0 per the workspace Cargo.toml.

**1. HyperDHT: missed, and it changes the relay story**
- **Node hyperdht allows only one random-NAT punch at a time per DHT instance, with at least 20 s between them.** Source: `index.js` l.54-56 has `this._randomPunchLimit = 1 // set to one for extra safety for now` and `randomPunchInterval` defaults to 20000.
  - The check runs on both sides (`connect.js` l.681-690, `server.js` l.588-592).
  - When the limit is hit, the server replies `TRY_LATER`. A connection that has no relay token of its own aborts instead of queueing. A client connecting *to* a relay is exactly that case.
- **What this means for a helper behind a consistent NAT.** Phones on random carrier NATs are the only users who need a relay. Each of them costs the helper one random punch (up to ~35 s).
  - So a NATed helper can admit about one such phone per 20-55 s.
  - Relaying two phones takes two serialized punches, so session setup takes 20 s or more.
  - peeroxide's own relay docs say a relay "should usually run at a stable, reachable UDP address... not designed to depend on its own holepunching sequence".
- **What the report gets right.** Pear added `pear blind-relay start` in Aug 2026: ordinary peers running `blind-relay` behind `dht.createServer`. So peer-run relays are an upstream pattern.
- **Corrected claim:** relays should be helpers that are **OPEN**: UPnP/PCP-mapped desktops (DESIGN §6 layer 4) or public-IP machines. NATed helpers are a slow last resort. X1 must measure this throttle.
- **Divergence risk.** A grep of peeroxide's `hyperdht.rs` and `holepuncher.rs` finds no equivalent throttle, so the Rust and Node implementations may already behave differently (not confirmed).
- **peeroxide has had no commit since 2026-07-12 (74 days).** In that time hyperdht shipped 6.33.1, 6.33.2 and 6.34.0. The commit titles look like bug fixes (orphaned holepunchers, suspended handshakes) rather than wire changes. The drift risk is real but slower than "every few weeks" suggests.

**2. Mainline BEP44: confirmed**
- BEP42: "all DHT responses SHOULD include a top-level field called ip".
- BEP44: "MAY expire in 2 hours... SHOULD be re-announced once an hour". It also says "Any node that's interested in keeping a blob in the DHT alive may announce it... without having the private key", so the desktop re-put plan is spec-sanctioned.
- pkarr `DEFAULT_RELAYS` are `pkarr.pubky.app` and `pkarr.pubky.org`. The README says "The default native client configures both DHT and relay backends" and that `ClientBuilder` can select DHT-only.
- Crate versions: mainline 8.0.0 and pkarr 8.0.2.

**2. Mainline BEP44: missed**
- **Reliability.** The maintainers' own `mainline-next/docs/problem-statement.md` (in pubky/mainline) says "The current architecture is unreliable and hard to diagnose under concurrency, network failures, and hostile traffic". It lists:
  - packet loss under concurrent queries (#38, open since 2024-11);
  - applications can't tell absent data from bootstrap or connectivity failure (#104, #61);
  - panics (#19, #23, #47).
  - A rewrite started in the same repo on 2026-09-22. Expect API churn.
  - One maintainer dominates: 416 commits against 35 for the next contributor.
  - So "Rust maturity: High" should read **medium**.
- **IPv4-only.** BEP32 isn't among the supported BEPs, and the rewrite's milestone 1 is "the IPv4 client". R-L6-3 (IPv4-only) applies to Mainline as much as to HyperDHT.
- **Latency.** pkarr's docs: "Traversing the DHT can take several seconds on a cache miss". There's no subscribe or push. That's fine for presence and endpoint records. It isn't a channel for time-critical hole-punch coordination, so the summary's "small signaling" wording should be narrowed.
- **The BEP42 reflexive-address claim needs one more condition.** The `ip` field reports the NAT mapping of *the socket that sent the query*.
  - To help iroh punch, the DHT queries must leave from iroh's own UDP socket.
  - Otherwise only the public IP is useful. The port is useful only on port-preserving EIM NATs, which carrier CGNAT rarely is.
  - `Endpoint::add_external_addr` exists in iroh, but socket sharing hasn't been verified. Add it to X5.
- **The default mode is Adaptive.** In that mode a desktop becomes a full BitTorrent DHT server after 15 min, and the server has "no rate-limiting".
  - Puts and re-puts don't need server mode.
  - Keep desktops in client mode unless we deliberately decide otherwise. The alternative means user PCs store torrent peer lists and draw BitTorrent-classified traffic, which some networks block.
- **Alternative library.** `n0-mainline` 0.6.0 is n0's fork of the same code, used by iroh's address lookup and pushed 2026-09-23. Compare it with pubky `mainline` before choosing.

**3. Tor via Arti**
- **Confirmed:**
  - `doc/OnionService.md` still says "suitable for testing and experimentation only", and its last content edit was 2025-03.
  - It is also stale: it lists missing Vanguards, which have since shipped.
  - The CHANGELOG has "Removed security disclaimer about onion service support".
  - Crate versions: arti 2.6.0, arti-client and tor-hsservice 0.46.0 (2026-09-02).
- **Caveat on the Quiet evidence.** Quiet #2152 dates from 2023-12 and involved C tor plus Node and IPFS, so it says little about Arti's own battery cost.
- **Missed precedent.** Tor's GitLab hosts "Tor VPN for Android" (built on `onionmasq`, an "Experimental Tunnel interface for Arti"). The Tor Project itself is productizing Arti on Android as a *client*. That supports "phones as Tor clients" but not phones as hosts.

**4. Syncthing relays**
- **Verdict upheld.** The docs say relays join the pool "and be available to Syncthing users".
- **Nuance:** calling it "impersonation" overstates it. The relay-v1 spec says it is "general enough that could be reused by other protocols", and `bep-relay` is simply the relay protocol's name.
- The real blockers are the scope of the volunteers' consent and the permanent TCP join on Android.

### Anything else missed
- R5 and FINDINGS already cover Keet's blind *peers* (the storage side). I found no L6-scope overlay that closes G1 or G3, so the negative result stands.

### Net effect on the plan
- The HyperDHT relay plan becomes "use OPEN (port-mapped or public) helpers as blind relays". "Any NATed helper" is a slow fallback.
- The Mainline plan stays but narrows:
  - client mode only, DHT-only;
  - presence and endpoint records only;
  - IPv4-only is noted;
  - pick between `mainline` and `n0-mainline`;
  - verify socket sharing with iroh in X5.
