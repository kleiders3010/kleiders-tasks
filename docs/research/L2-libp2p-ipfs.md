# L2: libp2p / IPFS public infrastructure for NAT traversal and discovery

*2026-09-24. Dimension L2 of the P6 "data distribution layers" research. Target gaps: **G2** (internet P2P connectivity independent of n0: address discovery + hole-punch coordination), **G5** (discovery / presence signaling) and **G4** (free relaying when hole punching fails). This is a proposal document; DESIGN.md and DECISIONS.md are unchanged.*

*Method: read primary specs, config docs and source (libp2p/specs, ipfs/kubo, go-libp2p, rust-libp2p, iroh 1.2 source), ProbeLab measurement reports (2026), the Trautwein et al. paper abstract, crates.io API, GitHub API. No traffic was sent to any public relay, DHT node or bootstrap node. Important caveat on tooling: the WebFetch summarizer made up Kubo defaults ("30m / 128MB") on one call. Every number below was re-checked against raw source files with curl or `gh api`.*

---

## 1. Summary

1. **The mechanism is sound and well measured.** libp2p's DCUtR hole punching is coordinated over a *limited* circuit-relay-v2 connection. On the live IPFS network it succeeds **70% ± 7.1%** of the time (4.4 M attempts, 85k networks, 167 countries), and **82.9% for port-restricted-cone NATs**. The paper found **TCP ≈ QUIC** and that "success is independent of relay characteristics", with 97.6% of successes on the first attempt. go-libp2p and rust-libp2p interoperate at **100%** in ProbeLab's 2026 NAT testbed. The 2 min / 128 KiB circuit limits are more than enough for coordination: DCUtR messages are capped at 4 KiB.
2. **The public relay pool is real, independent of n0, and on by default.** Kubo turns on `Swarm.RelayService` (default `true`) as soon as AutoNAT finds the node publicly reachable. No ACL is set by default, so it accepts reservations from any peer. ProbeLab's April 2026 crawls see **~3,750 dialable Amino DHT servers per crawl (~84% Kubo)**, and **2,047 Kubo peers were dialable in all 84 crawls of one week**. These are thousands of independent operators, but the pool is **shrinking** (~20,500 visible peers per crawl on average May 2025–Apr 2026, down to ~8,100 in the last 30 days) and churns about 18% week to week.
3. **The endorsement is only implicit, and the surrounding infrastructure is being wound down.** No policy permits or forbids non-IPFS apps using Kubo relays. libp2p docs say limited relays let "every public node in the network … serve as a relay without high resource consumption". The IPFS Foundation's own public utilities (bootstrappers, delegated routing) are "best-effort … **not intended to be part of your critical path or production infrastructure**". In August 2026 the IPFS project announced that "the IP Shipyard team winds down their work" and began rate-limiting gateways.
4. **These relays can't carry iroh traffic.** They don't solve iroh's n0 dependency directly:
   - iroh 1.2's hole-punch candidates come from QAD (n0 relays), port mapping, local interfaces, or `add_external_addr`.
   - libp2p identify reports the NAT mapping of *libp2p's* socket, not iroh's.
   - iroh *can* run NAT traversal over a non-relay path through the `unstable-custom-transports` feature. But a hybrid (iroh QUIC tunnelled over a libp2p circuit) still lacks iroh's public port, unless the NAT preserves ports. That's unverified.
   - **The credible way to use this infrastructure is a parallel rust-libp2p stack** (identify + relay client + DCUtR, optionally AutoNAT v2 and Kademlia) that carries our sync protocol over its own direct connections.
5. **G4 (data relay for the ~30% of failed punches) stays open.** Circuits reset after 2 min / 128 KiB per direction. They are built for coordination, and using them as a data relay is outside their intent. They can carry at most a tiny exchange, never a relay service.
6. **Phones are the weak spot.**
   - A reservation needs a live connection to the relay. On Android that only exists while the app is open or during a WorkManager window, which is the same constraint iroh has.
   - ProbeLab found `gobind` (likely mobile-embedded go-libp2p) peers advertise DCUtR 99% of the time but hold a relay reservation only **~0.6%** of the time, because sessions are too short for AutoNAT + AutoRelay.
   - rust-libp2p has **no AutoRelay** (issue #4651, open since 2023). We would write relay selection ourselves and reserve immediately, without waiting for AutoNAT.
7. **Most promising takeaway for the helper network: reuse the protocols, not only IPFS's nodes.** Publicly reachable helper desktops (public IPv4/IPv6 or a UPnP-mapped port) could run circuit-relay-v2 *hop* (limited) plus an AutoNAT v2 server for the app's own users, exactly as every public Kubo node does for IPFS. Unlike an iroh-relay, a libp2p relay needs no web-PKI TLS certificate: peer IDs authenticate. That gives a G2 provider independent of both n0 and IPFS, which grows with the user base. Public Kubo relays serve as the bootstrap-era second provider.

**Verdicts in one line:**
- CONSIDER:
  - public Kubo relays for coordination;
  - DCUtR;
  - identify;
  - AutoNAT v2 (desktops);
  - bootstrap nodes (cold-start seed only);
  - rust-libp2p *alongside* iroh;
  - helper desktops as libp2p relays.
- REJECT:
  - circuits as a G4 data relay;
  - Amino DHT as a discovery channel on phones;
  - delegated routing HTTP;
  - the iroh-over-libp2p hybrid (for now);
  - rust-libp2p *instead of* iroh;
  - jvm-libp2p.

**Gaps covered:**
- G2: partially covered (desktops well; phones only while open).
- G5: small coordination signaling only while both peers are online, no store-and-forward.
- G4: not covered.
- G1, G3, G6: not covered (G6 only incidentally, over any direct connection).

---

## 2. Background: what exactly the libp2p NAT stack does

Dependency chain (ProbeLab DCUtR final report, 2026):

```
Identify (discover external address)
  → AutoNAT v2 (verify reachable; if no → behind NAT)
    → Circuit-Relay-v2 client (reserve a relay slot)
      → DCUtR (hole-punch over the relay to upgrade)
```

- **Identify** (`/ipfs/id/1.0.0`): `observedAddr` is "the connection source address of the stream-initiating peer as observed by the peer being identified … The initiator can use this address to infer the existence of a NAT and its public address." Any connected public peer acts as a free "STUN" observer, but only for the socket used on that connection.
- **Circuit relay v2**: B reserves a slot on relay R (TTL 1 h, refreshed while connected: "the reservation remains valid until its expiration, as long as there is an active connection from the peer to the relay"). A dials `/…/p2p/R/p2p-circuit/p2p/B`. Relays announce `Limit{duration,data}` and "reset the stream to the source and the stream to the destination" when a limit is exceeded. The spec's stated purpose: "limited relays and slot reservations … allows relays to effectively manage their resources and provide service at a small scale, thus enabling the deployment of an army of relays"; hole-punch coordination "doesn't require an unlimited relay connection."
- **DCUtR**: B→A `Connect{ObsAddrs}`, A→B `Connect{ObsAddrs}`, B measures the RTT, sends `Sync` and waits RTT/2; then both dial at once (TCP simultaneous open, or QUIC dial + random-UDP punches every 10–200 ms). The inbound peer "SHOULD retry twice (thus a total of 3 attempts)". Messages are ≤ 4 KiB.
- **Default limits** (all verified in source):

| Implementation | Circuit duration | Circuit data | Reservation TTL | Max reservations | Max circuits | Per-IP / per-ASN |
|---|---|---|---|---|---|---|
| go-libp2p `DefaultResources()` | 2 min | 128 KiB (`1 << 17`) | 1 h | 128 | 16 | 8 per IP, 32 per ASN, 1 per peer |
| Kubo `Swarm.RelayService` | `"2m"` | `131072` "in each direction" | `"1h"` | 128 | 16 | `MaxReservationsPerIP` 8, `PerASN` 32 |
| rust-libp2p `relay::Config::default()` | 2 min | 128 KiB | 1 h | 128 (4 per peer) | 16 (4 per peer) | rate limiter: per peer 30 per 2 min, per IP 60 per min (reservations and circuits) |

---

## 3. Candidate-by-candidate evaluation

### C1. Public IPFS (Kubo) nodes' circuit relay v2 as hole-punch **coordination**
- **Gaps:** G2 (coordination), G5 (small signaling while both are online). Not G1/G3/G4/G6.
- **Cost:** $0 to users and developer; no developer infrastructure. Traffic per coordination is a few KB.
- **Terms / permission:**
  - There is no ToS. These are volunteers' nodes running Kubo defaults.
  - Kubo docs describe the client role as: "Your node will automatically _use_ public relays from the network if it detects that it cannot be reached from the public internet."
  - libp2p docs: "In the case of IPFS, this allows every public node in the network to serve as a relay without high resource consumption."
  - Relays have no ACL by default (`acl: nil` in go-libp2p `relay.go`), so they accept any peer, IPFS or not.
  - **Nothing explicitly sanctions non-IPFS apps.** Operators could add ACLs, and Kubo could change defaults.
- **Operators and sustainability:**
  - ~3,750 dialable DHT servers per crawl, ~84% Kubo (ProbeLab Nebula, April 2026). 2,047 Kubo peers were dialable in 100% of 84 crawls over 7 days, which is the stable relay-capable core.
  - The DCUtR campaign (50,052 peers, May 2026) found 8.3% of peers directly reachable.
  - Many independent operators, but some large fleets exist: the `gala.games` Kubo build appears on ~7,700 nodes.
  - Trend is negative: visible peers ~20,500 → ~8,100 per crawl, and 18% of Kubo peers are "lost" week over week.
  - Stewardship is thinning: Shipyard is winding down (IPFS blog, 2026-08-25).
- **Reliability / limits:**
  - 2 min / 128 KiB per circuit.
  - Reservations: 128 per relay, 8 per IP, **32 per ASN per relay**. Phones on one carrier ASN share that quota on each relay, so selection must spread over many relays.
  - Success doesn't depend on which relay is used (Trautwein).
  - The main operational failure is not getting a reservation at all: 36% of DCUtR-advertising NAT'd peers never had one.
- **Privacy / metadata:**
  - The relay sees both peers' IPs, peer IDs and timing. Payload is end-to-end encrypted (Noise/TLS) on top of our own E2E.
  - Identify sends agent version, protocol list and listen addresses (including private LAN addresses unless filtered) to every connected peer. That makes the app fingerprintable and blockable.
  - The Amino DHT is continuously crawled, and ProbeLab publishes a **public ClickHouse dataset**. Any of our nodes running as DHT *servers* (desktops) would appear in it with IP, peer ID and protocols. Honeypots (as in the DCUtR study) also log clients that connect to them.
  - Peer IDs must rotate (per group / per epoch, D23) to avoid long-term tracking.
- **Android:**
  - A reservation needs a live connection, so it only works while the app is open or inside an expedited/periodic WorkManager window (network is cut ~5 s after leaving the app, D16). Same limit as iroh's home-relay connection.
  - Short sessions are the known failure: `gobind` peers held a reservation only ~0.6% of the time (ProbeLab). The mitigation is to skip AutoNAT on phones and reserve immediately on 1–2 relays (rust-libp2p: `listen_on(relay_addr/p2p-circuit)`). Time to reservation is unmeasured.
  - Needs only INTERNET permission. A client of others' relays isn't "providing proxy services" under Play's Device & Network Abuse policy.
- **Library maturity:** see C11 (rust-libp2p 0.57.0, MIT, 2026-09-11). There is no AutoRelay in rust-libp2p (#4651, open since 2023-10), so we must find relays ourselves. Options: a bootstrap → random-walk / identify scan for peers advertising `/libp2p/circuit/relay/0.2.0/hop`, cache the good ones, ship a seed list.
- **Abuse resistance:**
  - Relays can drop or refuse us but can't read or forge (peer-ID-authenticated end-to-end).
  - The risk is reversed: *we* would be seen as the abuser if our load shows up. With 10k concurrent foreground devices each holding 1–2 reservations, that is ~10–20k of ~256k slots in the stable pool (2,000 × 128). Noticeable, not dominant. Unmeasured.
- **Precedent:**
  - IPFS itself (Kubo, ~96% DCUtR enablement among vanilla Kubo).
  - Non-IPFS go-libp2p apps do sit in Amino (`other` 75 and `harmony` 42 dialable nodes).
  - **Berty, the best-known mobile libp2p messenger, does NOT rely on public relays.** Its `config.yml` pins its own rendezvous point and static relay (`51.15.25.224`, "svc ams 1").
  - n0 abandoned its libp2p-based IPFS implementation (beetle) in 2023: it "cannot reliably remain connected to the vast majority of peers on the network, if connection is possible at all".
- **Failure modes:**
  - Kubo changes defaults, or operators add ACLs against non-IPFS peers.
  - The network keeps shrinking.
  - Reservation refused (IP/ASN caps under CGNAT).
  - UDP black holes.
  - Symmetric NATs (~40% or worse).
  - Both peers must be online at the same moment.
- **Verdict: CONSIDER. Confidence: medium.** It's the only existing, independent, free, many-operator source of hole-punch coordination we found. It fits the protocol's intent (coordination, small scale). But it rests on an implicit norm, a shrinking volunteer pool and a stack we'd have to add. Treat it as a second provider next to n0 for G2, never the only one.

### C2. DCUtR (the hole-punch protocol itself, rust-libp2p `libp2p-dcutr` 0.15.0)
- **Gaps:** G2 (the punch). It needs a relayed connection first (C1 or C9).
- **Evidence:**
  - Trautwein et al. (arXiv 2510.27500, IMC '26): "baseline success rate of 70% ± 7.1% … statistically indistinguishable success rates for both TCP and QUIC (~70%) … success is independent of relay characteristics … 97.6% of successful connections established on the first attempt."
  - ProbeLab 2026 per-NAT-class results: port-restricted cone **82.9%** (3,042/3,669), APDF-unresolved 37.0%, symmetric 39.7% (probably misclassified), 100% go↔rust interop across punchable EIM×EIM cells.
  - Audit findings are minor. Both go and rust bail on protocol-stream errors instead of retrying. rust checks for an empty address list in the wrong order.
- **Caveat for our use:**
  - The measured population is IPFS nodes, mostly desktops and servers. Phone-on-cellular behind CGNAT is under-represented. R1's verifier already said none of these figures are grounded for phone↔phone.
  - TCP hole punching needs NATs that *drop* unsolicited SYNs; reject-style NATs break it (QUIC is immune).
  - The same EIM-mapping requirement applies as for iroh. DCUtR does **no** port prediction, so hard↔hard (symmetric) NAT remains unsolved, as in iroh.
- **Cost / terms / Android:** a protocol, $0. Runs over whatever relay is used. Android-feasible while the app is open.
- **Verdict: CONSIDER (adopt it if a libp2p stack is adopted). Confidence: high** on the measurements, **low** on phone↔phone rates.

### C3. Identify observed addresses (address discovery)
- **Gaps:** G2 (address discovery, the STUN-equivalent), but only for the socket the libp2p connection uses.
- **Evidence:**
  - Spec: the observed address lets "the initiator … infer the existence of a NAT and its public address."
  - Every connected public peer (thousands of Kubo nodes, or our helpers) is an independent observer. That's more providers than n0's QAD.
  - rust-libp2p's `libp2p-identify` 0.48.0 is mature (1.7 M recent downloads).
- **Limits:**
  - An observation is only valid for the NAT mapping of *that* socket. With EIM it's reusable towards any peer (what DCUtR relies on); with ADPM it's useless.
  - **It doesn't give iroh its public port** (see C10).
  - A single observer can lie. rust-libp2p/go-libp2p confirm observed addresses across several observers before using them.
- **Privacy:** observers learn our IP, which they would anyway by being connected.
- **Verdict: CONSIDER** (as part of a libp2p stack). **REJECT as an address source for iroh** unless experiment X3 shows NATs preserve ports. Confidence: high.

### C4. AutoNAT (v1/v2) on public Kubo nodes
- **Gaps:** G2 (reachability check: "am I dialable, should I be a relay/DHT server?").
- **Evidence:**
  - Kubo `AutoNAT.ServiceMode` defaults to enabled on publicly dialable nodes (V1+V2), throttled to **30 requests/min globally and 3 per peer per minute**.
  - ~50% of dialable Kubo advertise the v2 server (ProbeLab, April 2026).
  - v2 has amplification protection: servers "SHOULD ask for 30k to 100k bytes" before dialing an address that isn't the client's observed IP.
  - ProbeLab's testbed: 0% false negatives and 0% false positives for standard NATs, ~6 s to converge.
  - Weaknesses: 100% false positives for address-restricted NATs, no explicit UNREACHABLE signal for symmetric NATs, a UDP black hole blocking QUIC dial-back, and v1/v2 disagreement (go-libp2p's DHT consumes the flapping v1).
- **Android:** not needed on phones. Assume "private" and reserve immediately; waiting ~6 s+ for AutoNAT is what keeps mobile peers from ever reserving (C1). It's useful on desktops/helpers to decide whether to offer relay/AutoNAT service.
- **Verdict: CONSIDER for desktops/helpers only. Confidence: medium-high.**

### C5. Amino Kademlia DHT (provider records / peer routing) for discovery
- **Gaps:** G5 (rendezvous: finding the peer IDs and relay addresses of group members). Not presence messages.
- **Evidence:**
  - Provider records: "DHT Servers MUST implement a Provide Validity of `48h`"; clients "SHOULD republish Provider Records every `22h`"; provider addresses are kept 24 h (`amino/defaults.go`, IPFS kad-dht spec).
  - Value records: "Records with other prefixes [than `/pk/`, `/ipns/`] are not supported by the IPFS Kademlia DHT and MUST be rejected". So no arbitrary signed records, unlike the Mainline DHT's BEP44 (pkarr).
  - NAT'd devices are DHT *clients* and aren't findable by `FIND_PEER`, so discovery would have to go through provider records keyed by a rotating group-derived key.
  - Relay addresses in a provider record (valid 24 h) outlive the 1 h reservation.
- **Cost:**
  - $0, but heavy on phones. Each provide or lookup is a multi-hop walk (concurrency 10, bucket 20) with many fresh secure-channel handshakes. n0 measured DHT resolution "on the order of seconds" (2023).
  - Battery and data cost per lookup unmeasured.
- **Terms:** providing a CID for content you don't serve is outside IPFS's intended semantics. Gateways and indexers may try to fetch it.
- **Privacy:** anyone who can compute the key sees peer IDs and IPs. Keys must rotate daily with the epoch secret (D5/D23).
- **Redundancy:** discovery already has Mainline DHT/pkarr (signed mutable records, better fit) plus Nostr. Amino would be a third, heavier channel with worse record semantics.
- **Verdict: REJECT** as a phone discovery channel. It could be revisited on desktops only if pkarr/Mainline proves unreliable. Confidence: medium.

### C6. Public bootstrap nodes (`/dnsaddr/bootstrap.libp2p.io`, `va1.bootstrap.libp2p.io`)
- **Gaps:** cold-start entry into the Amino network, needed to find public relays (C1) if no cached peers exist.
- **Evidence and usage policy:**
  - The IPFS Foundation runs 4 go/rust nodes (sg1, sv15, am6, ny5; ny5 is rust-libp2p based) and 1 js node (va1). The live autoconf (`conf.ipfs-mainnet.org/autoconf.json`, version 2025080401) also lists `104.131.131.82`.
  - Policy (docs.ipfs.tech/concepts/public-utilities): "provide hosted public utilities to the community on a best-effort basis. As such, it is not intended to be part of your critical path or production infrastructure." Bootstrappers exist "for an IPFS node to join the Amino DHT".
  - A single organization operates them, and its engineering arm is winding down.
- **Mitigation:** each device caches the healthy public peers it met, like Kubo's "backup peers". Ship a seed list refreshed with releases. Use bootstrap.libp2p.io only as the last resort on first launch.
- **Verdict: CONSIDER, as a cold-start seed only, never a runtime dependency.** Confidence: high on the policy facts.

### C7. Delegated routing HTTP (`delegated-ipfs.dev/routing/v1`)
- **Gaps:** would let a phone read providers/peers over one HTTPS call instead of a DHT walk (G5 read side).
- **Evidence:**
  - Operated by the IPFS Foundation (someguy). It falls under the same "best-effort … not intended to be part of your critical path or production" statement.
  - Writes allowed only for `/routing/v1/ipns`.
  - Gateways are already rate-limited "and will be increasing" (2026-08 blog).
- **Verdict: REJECT.** One operator, explicit non-production policy, and it only reads C5's (rejected) records. Confidence: high.

### C8. Circuit relay v2 as a **data** relay for failed hole punches (G4)
- **Gaps:** G4.
- **Evidence:**
  - Every circuit is reset at 2 min or 128 KiB per direction. go-libp2p marks relayed connections as *limited*, and protocols must opt in.
  - The spec positions limited relays for coordination and small-scale service.
  - Earlier research (R5 P13, verifier) already refuted "second relay provider". The verifier noted a 1-year list snapshot (31–70 KB) *does* fit in 128 KiB.
  - Chaining new circuits every 2 minutes to emulate a relay is what the limits exist to prevent. It would make our app the visible abuser of volunteer bandwidth.
- **Scope of the "no":** a single circuit can legitimately carry the tiny exchange that accompanies coordination (version vectors, a small delta). It isn't a relay service and doesn't close G4.
- **Verdict: REJECT as a G4 provider. Confidence: medium-high.** G4 stays open in this dimension.

### C9. Helper desktops running circuit relay v2 *hop* + AutoNAT v2 server (the libp2p protocols on the app's own helper network)
- **Gaps:**
  - G2 (coordination and address observation independent of both n0 and IPFS).
  - G5 (signaling while online).
  - Partial G4: a helper can relay *its own groups'* traffic without limits (ACL on group peer IDs), and strangers' traffic only within limits.
- **Evidence:**
  - This is exactly IPFS's own model ("each public node would serve as a Relay").
  - Implementations exist (rust-libp2p `relay::Behaviour` with configurable limits and rate limiters; go-libp2p `ACLFilter` hook).
  - A libp2p relay needs a reachable TCP/UDP port but **no web-PKI certificate**: peers authenticate by peer ID. An embedded iroh-relay needs HTTPS/TLS and custom roots (R1 E3e), so this is simpler.
- **Constraints:**
  - Only publicly reachable desktops qualify: public IPv4, IPv6 with an open firewall, or a UPnP/PCP-mapped port. IPFS data suggests a small minority: 8.3% directly reachable in the DCUtR census, though DHT-server populations skew higher.
  - ISP CGNAT excludes many desktops, as noted in R1's verifier.
  - Phones must never run *hop* service for strangers. Play's "proxy services to third parties" risk (R8/D19) applies here more literally than to mailbox helpers.
  - Helper IPs become visible to strangers, as the design already accepts.
- **Scale:** at 1,000 desktop helpers and ~10% reachability, that's ~100 relays × 128 reservations. Enough for early scale; grows with users.
- **Verdict: CONSIDER. Confidence: medium.** The structurally strongest long-term answer to G2 in this dimension, because it removes both n0 and IPFS as dependencies. It depends on the helper network (D20) existing and on the reachable fraction, which is unmeasured for our users.

### C10. iroh using libp2p relays for coordination (hybrid)
- **Question:** is there any path for iroh to use a libp2p relay for coordination?
- **Findings (iroh 1.2.0 source, `socket.rs`, `remote_state.rs`, `transports/custom.rs`):**
  - iroh's NAT traversal runs *inside* an existing QUIC connection: `initiate_nat_traversal_round()` on a noq connection, over whatever path exists (relay **or custom transport**).
  - Local candidates are `local_direct_addrs`, built from four sources: the portmapper, the net_report (QAD against **iroh relays**), local interfaces, and user-configured addresses (`Endpoint::add_external_addr`, "advertised to peers together with any discovered external addresses").
  - The `unstable-custom-transports` feature ("may change without notice") exposes the `CustomTransport` / `CustomEndpoint` / `CustomSender` traits. They move iroh QUIC datagrams to `CustomAddr`s; registered IDs today are Test, Tor (experimental) and BLE (reserved).
  - **So a hybrid is possible in principle:**
    1. a `CustomTransport` that tunnels iroh datagrams over a libp2p circuit stream (≤ 128 KiB / 2 min is enough for the handshake and a NAT traversal round);
    2. plus `add_external_addr(public_ip:bound_port)`.
  - **The missing piece is iroh's own public port.** Identify reports the mapping of libp2p's socket, not iroh's. The only fix is assuming the NAT preserves the local port (bind iroh to a fixed port). That's the same heuristic iroh calls `Qad4LocalPort`, which it only uses when mapping varies by destination. Port preservation is common on home routers but not guaranteed. CGNATs often allocate port blocks or randomize ports. **Unverified.**
- **Other cost:** two stacks plus an unstable API, and the result is inferior to C11's plain parallel stack.
- **Verdict: REJECT for now.** Experiment X3 could reopen it. Confidence: medium.

### C11. rust-libp2p as a parallel connectivity stack (alongside iroh), or instead of iroh
- **Library facts:**
  - Versions: `libp2p` 0.57.0, released 2026-09-11 (MIT, MSRV 1.88). Previous 0.56.0 was 2025-06-27, a **15-month gap**. Sub-crates: relay 0.22.0, dcutr 0.15.0, identify 0.48.0, autonat 0.16.0, kad 0.49.0, quic 0.14.0, swarm 0.48.0.
  - Repo activity: 5,611 stars, 286 open issues, last push 2026-09-21.
  - Maintainers: **2 maintainers** listed. Notable users include Lighthouse, Forest, and qaul (Android mesh app, AGPL, on libp2p 0.56).
  - Android issues:
    - open: WebSocket+tokio doesn't work on Android (#5672);
    - open: `with_websocket` hardcodes the system DNS config, "breaks on Android" (#6529, 2026-07);
    - fixed: an if-watch Android ≥ 11 breakage (if-watch #10, 2021).
  - **No AutoRelay** (#4651, open since 2023).
  - Only 2 rust-libp2p nodes are dialable in the Amino DHT, so rust nodes are rare there, but interop is proven.
- **Binary size and dependencies (not measured):**
  - `libp2p-quic` pulls in `quinn` 0.11 with `rustls-aws-lc-rs` (aws-lc-rs, a C library), plus `ring`, `rcgen`, `x509-parser` and `yasna`.
  - iroh already ships `noq` (its quinn fork) with `ring`. So QUIC libp2p duplicates a QUIC stack and adds aws-lc: the largest size and cross-compile cost.
  - A **TCP-only** variant (`libp2p-tcp` + Noise + Yamux) avoids aws-lc. DCUtR over TCP is measured as effective as QUIC. It also works on networks that block UDP.
  - The size delta must be measured (X1). E6 set the baseline: full core 7.07 MB installed on arm64, iroh 4.58 MB of it.
- **Alongside iroh:**
  - Gives a second, n0-independent path for G2: identify + relay client + DCUtR over C1/C9 relays.
  - Our sync protocol would run over libp2p streams when iroh can't connect directly.
  - Costs: a second identity (derive from the device/group keys), a second socket, more battery while open, and roughly 1–4 MB native (a guess; X1).
- **Instead of iroh:**
  - Loses iroh's unlimited data relaying (G4, even if n0-dependent), QUIC multipath and LAN/mDNS integration already measured in E3.
  - Takes on the reliability issues n0 cited when leaving libp2p.
  - Our needs don't include IPFS content exchange.
- **Verdict:** **Alongside: CONSIDER (medium).** **Instead of iroh: REJECT (medium-high).**

### C12. jvm-libp2p (Kotlin/JVM) as the Android implementation
- **Evidence:**
  - v1.3.7 (2026-09-14), Apache-2.0.
  - README status: circuit-relay-v2 and autonat are ":lemon: prototype or beta, not tested in production". Hole punching, kad-dht and rendezvous are **not implemented**.
  - "We are not aware of anyone using it in production" on Android.
- **Verdict: REJECT. Confidence: high.** The engine is Rust anyway.

---

## 4. Comparison table vs gaps

Legend: ● covers, ◐ partial / conditional, ○ no.

| Candidate | G1 offline S&F | G2 addr + punch w/o n0 | G3 wake phone | G4 relay failed punches | G5 discovery / signaling | G6 blobs | Verdict | Confidence |
|---|---|---|---|---|---|---|---|---|
| C1 Public Kubo relays (coordination) | ○ | ◐ (desktops well; phones only while open) | ○ | ○ | ◐ (online-only, tiny) | ○ | CONSIDER | medium |
| C2 DCUtR | ○ | ● (mechanism; 70% / 83% PR-cone) | ○ | ○ | ○ | ○ | CONSIDER (with a stack) | high (desktops) / low (phones) |
| C3 Identify observed addr | ○ | ◐ (libp2p socket only; not iroh's) | ○ | ○ | ○ | ○ | CONSIDER / REJECT for iroh | high |
| C4 AutoNAT v2 | ○ | ◐ (reachability; desktops) | ○ | ○ | ○ | ○ | CONSIDER (desktops) | medium-high |
| C5 Amino DHT provider records | ○ | ○ | ○ | ○ | ◐ (rendezvous, heavy) | ○ | REJECT (phones) | medium |
| C6 Bootstrap nodes | ○ | ◐ (cold-start only) | ○ | ○ | ○ | ○ | CONSIDER (seed only) | high |
| C7 Delegated routing HTTP | ○ | ○ | ○ | ○ | ◐ (read-only) | ○ | REJECT | high |
| C8 Circuits as data relay | ○ | ○ | ○ | ○ (2 min / 128 KiB) | ○ | ○ | REJECT | medium-high |
| C9 Helper desktops as libp2p relays | ○ | ● (n0- and IPFS-independent, if enough reachable helpers) | ○ | ◐ (own groups; limited for others) | ◐ | ○ | CONSIDER | medium |
| C10 iroh over libp2p circuit (hybrid) | ○ | ◐ (only if the NAT preserves ports) | ○ | ○ | ○ | ○ | REJECT (for now) | medium |
| C11a rust-libp2p alongside iroh | ○ | ◐ | ○ | ○ | ◐ | ◐ (over direct conns) | CONSIDER | medium |
| C11b rust-libp2p instead of iroh | ○ | ◐ | ○ | ○ | ◐ | ◐ | REJECT | medium-high |
| C12 jvm-libp2p | ○ | ○ | ○ | ○ | ○ | ○ | REJECT | high |

**Not covered by this dimension at all:** G1, G3, G4 (only C9 partially), G6.

---

## 5. New risks surfaced

1. **The IPFS volunteer pool is shrinking and its steward is winding down.**
   - Visible peers per crawl fell from ~20,500 (May 2025–Apr 2026 average) to ~8,100 (recent 30 days; cause not established). Kubo churn is ~18% per week.
   - Shipyard is winding down (2026-08).
   - A layer built on public Kubo relays is only as durable as IPFS's own adoption.
2. **Acceptance risk mirrors Nostr (D13).** We would be using volunteers' capacity for non-IPFS traffic with no explicit permission. Kubo's per-ASN cap (32 per relay) and per-IP cap (8) already bite under carrier CGNAT. One Kubo release (an ACL, or turning relay service off) could close the path network-wide, since ~84% of dialable nodes run Kubo, a monoculture like strfry.
3. **Public measurement datasets de-anonymize participants.** ProbeLab crawls Amino ~12×/day and publishes a public ClickHouse dataset. Honeypot campaigns log connecting clients. Desktops acting as DHT servers or relays would be catalogued with IP, peer ID, agent and protocol list. This extends D23: protocol IDs and agent strings must be generic, and peer IDs rotated.
4. **Mobile libp2p nodes rarely complete the reservation dance** (`gobind` peers: 0.6% hold a reservation). Any libp2p design for phones must reserve immediately, without AutoNAT, and must accept that a backgrounded phone can't be reached this way at all.
5. **Size and build complexity:** libp2p-quic brings aws-lc-rs (a C library) next to iroh's ring, plus a second QUIC implementation. That's a supply-chain and cross-compile burden, and a size increase not yet measured.
6. **rust-libp2p maintenance bandwidth:** 2 listed maintainers, 15 months between minor releases, AutoRelay missing for 3 years. We would be writing and maintaining relay selection ourselves.
7. **The per-NAT evidence is desktop-biased.** DCUtR's 70% / 83% come from IPFS nodes; phone↔phone on cellular CGNAT is not represented. This can't be settled without our own field telemetry. (It confirms R1's verifier and X14 in FINDINGS.)

---

## 6. Suggested SAFE local experiments (no traffic to public infrastructure)

- **X1. Size delta.** Build a minimal rust-libp2p 0.57 feature set for `aarch64-linux-android` with the E6 release profile, in two variants:
  - (a) TCP + Noise + Yamux + identify + relay client + DCUtR;
  - (b) (a) + QUIC + AutoNAT + kad.

  Measure the stripped `.so` delta against the E6 core and list duplicated crates (`cargo tree -d`) and licenses (`cargo deny check licenses`). Decides whether C11a is affordable.
- **X2. Local NAT matrix for DCUtR.** Use Linux network namespaces or WSL2 + nftables with NATs of the following types:
  - EIM/APDF (port-restricted cone);
  - ADPM (symmetric);
  - port-preserving vs port-randomizing CGNAT-like.

  Run one local rust-libp2p relay (default limits) and two rust-libp2p peers. Measure success rate and time-to-direct for TCP and QUIC. ProbeLab's testbed design (dcutr-project) can be followed; their repos carry no license, so re-implement rather than copy.
- **X3. iroh hybrid feasibility (C10).** In the same namespaces, disable iroh relays and use an `unstable-custom-transports` tunnel over a local libp2p circuit plus `add_external_addr(public_ip:fixed_bound_port)`. Check whether iroh's NAT traversal round reaches a direct path behind port-preserving vs port-randomizing NATs. Settles whether identify can feed iroh at all.
- **X4. Circuit budget.** Replay E1 Loro update traces (live edit sessions, a 1-year snapshot) over a local relay with go/rust defaults (2 min / 128 KiB). Measure how much a single coordination circuit can carry, and verify reset behavior. Confirms C8's scope.
- **X5. Android lifecycle.** On a headless emulator with a local relay on the host:
  - time from app foreground to an accepted reservation, without AutoNAT;
  - reservation loss after backgrounding (expect ~5 s network cut);
  - whether an expedited WorkManager job can reserve, coordinate and sync inside one window.
- **X6. Fingerprint audit.** Capture the identify payload our node would send (agent_version, protocols, listen addresses) against a local peer. Minimize: generic agent, no private-LAN addresses to public peers, rotated peer IDs.
- **X7. Helper-relay reachability (C9).** On this PC's home network (local only), test whether a UPnP/PCP-mapped port lets a libp2p relay be reached from a namespace simulating an outside peer. Reuse E3.6's CGNAT check to estimate what share of desktops could serve as relays.

Explicitly **not** proposed: probing public Kubo relays, joining Amino to measure, or reserving on public relays. Those would send test traffic to third-party infrastructure (STATE.md safety rules).

---

## 7. Sources

**Specs**
- Circuit relay v2: https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md
- DCUtR: https://github.com/libp2p/specs/blob/master/relay/DCUtR.md
- Identify: https://github.com/libp2p/specs/blob/master/identify/README.md
- AutoNAT v2: https://github.com/libp2p/specs/blob/master/autonat/autonat-v2.md
- IPFS Kademlia DHT spec: https://github.com/ipfs/specs/blob/main/src/routing/kad-dht.md

**Config and source (defaults)**
- Kubo config docs (RelayService, AutoNAT, AutoConf, Bootstrap): https://github.com/ipfs/kubo/blob/master/docs/config.md
- go-libp2p relay defaults: https://github.com/libp2p/go-libp2p/blob/master/p2p/protocol/circuitv2/relay/resources.go
- go-libp2p `EnableRelayService`: https://github.com/libp2p/go-libp2p/blob/master/options.go
- go-libp2p relay (ACL nil, reservation tagging): https://github.com/libp2p/go-libp2p/blob/master/p2p/protocol/circuitv2/relay/relay.go
- rust-libp2p relay defaults: https://github.com/libp2p/rust-libp2p/blob/master/protocols/relay/src/behaviour.rs
- libp2p-quic dependencies: https://github.com/libp2p/rust-libp2p/blob/master/transports/quic/Cargo.toml
- Amino defaults: https://github.com/libp2p/go-libp2p-kad-dht/blob/master/amino/defaults.go
- IPFS mainnet autoconf: https://conf.ipfs-mainnet.org/autoconf.json

**Policy and sustainability**
- IPFS public utilities policy: https://docs.ipfs.tech/concepts/public-utilities/ (source: https://github.com/ipfs/ipfs-docs/blob/main/docs/concepts/public-utilities.md)
- "IPFS is moving beyond the sponsored gateways" (2026-08-25): https://blog.ipfs.tech/2026-08-beyond-sponsored-gateways/
- Waterworks community: https://github.com/ipshipyard/waterworks-community
- libp2p docs, circuit relay: https://github.com/libp2p/docs/blob/master/content/concepts/nat/circuit-relay.md
- libp2p docs, hole punching: https://github.com/libp2p/docs/blob/master/content/concepts/nat/hole-punching.md

**Measurements**
- Trautwein et al., "Challenging Tribal Knowledge – Large Scale Measurement Campaign on Decentralized NAT Traversal": https://arxiv.org/abs/2510.27500 (IMC '26 version arXiv:2604.12484)
- ProbeLab DCUtR project and final report (2026): https://github.com/probe-lab/dcutr-project, https://github.com/probe-lab/dcutr-project/blob/main/docs/dcutr-final-report.md
- ProbeLab AutoNAT v2 audit, Nebula analysis, NAT-stack best practices (2026): https://github.com/probe-lab/autonat-perf-audit, https://github.com/probe-lab/autonat-perf-audit/blob/main/docs/nebula-autonat-analysis.md, https://github.com/probe-lab/autonat-perf-audit/blob/main/docs/nat-stack-best-practices.md

**Libraries**
- rust-libp2p repo, README (maintainers, notable users), releases: https://github.com/libp2p/rust-libp2p
- crates.io API: https://crates.io/api/v1/crates/libp2p (and libp2p-relay, -dcutr, -identify, -autonat, -kad, -quic, -swarm)
- rust-libp2p issues: #4651 AutoRelay https://github.com/libp2p/rust-libp2p/issues/4651 · #5672 https://github.com/libp2p/rust-libp2p/issues/5672 · #6529 https://github.com/libp2p/rust-libp2p/issues/6529 · if-watch #10 https://github.com/libp2p/if-watch/issues/10
- jvm-libp2p: https://github.com/libp2p/jvm-libp2p

**iroh**
- iroh 1.2 `Endpoint` docs: https://docs.rs/iroh/latest/iroh/endpoint/struct.Endpoint.html
- Custom transport trait: https://github.com/n0-computer/iroh/blob/main/iroh/src/socket/transports/custom.rs
- `add_external_addr` and direct-address assembly: https://github.com/n0-computer/iroh/blob/main/iroh/src/socket.rs
- NAT traversal rounds: https://github.com/n0-computer/iroh/blob/main/iroh/src/socket/remote_map/remote_state.rs
- Transports: https://github.com/n0-computer/iroh/blob/main/TRANSPORTS.md
- n0 on leaving libp2p/IPFS (2023-02-17): https://www.iroh.computer/blog/a-new-direction-for-iroh

**Precedent apps**
- Berty network config (own rendezvous + static relay): https://github.com/berty/berty/blob/main/config/config.yml
- qaul: https://github.com/qaul/qaul.net

**Internal context:** `docs/DESIGN.md` §6–§8; `docs/research/DECISIONS.md` D11–D23; `R1-iroh.md` (C2c, C3, E3e); `R5-prior-art.md` (P13); `FINDINGS.md` (R5-13, X14, E3.6).
