# E10: NAT lab for D49 (internet P2P and relaying without n0), measured

Date: 2026-09-24. Code, scripts and raw results: `research/experiments/nat-lab/` (README explains how to rerun).
Question (DECISIONS D49, LAYERS §6 P1/P2, L8 X1/X2): how can two app instances connect over the internet, directly or relayed, **without n0's relays or DNS**?

**Short answer from the data:**
- Which pairs can get a **direct** path depends on the two NAT types, not on the option used. Options (a), (b) and (c) all went direct on the same 8 of 13 NAT pairs.
- Only the relay-carrying options, **(a) helper iroh relays** and (c) a custom first path, connected **all 13 pairs**. Option (b) connected at most 8, with 0% for the other 5.
- The 5 relay-only pairs all have a symmetric/CGNAT IPv4 side facing a peer that is neither full-cone nor UPnP-mapped. That includes **phone-like CGNAT on both sides**. Nothing in the lab got these pairs direct. They connect only through a relay.

**Build (a) first and (b) alongside it. Don't build (c).** Details in §7.

---

## 1. Setup

### Versions
| Item | Version |
|---|---|
| iroh / iroh-relay / iroh-base | **1.2.0** (same as E3/E6), features `unstable-net-report`, `unstable-custom-transports`; iroh-relay `server` |
| noq / noq-proto, portmapper, netwatch | 1.3.0, 0.19.3, 0.19.3 |
| instant-acme / rcgen / rustls | 0.8.5 / 0.14.10 / 0.23.45 |
| Pebble (local Let's Encrypt test CA) | v2.10.1 release binary, `shortlived` profile = 576,000 s (160 h, like LE IP certificates) |
| miniupnpd (nftables backend) / miniupnpc | 2.3.4 / Ubuntu 24.04 package |
| Rust | 1.98.1 |
| Host | WSL2 Ubuntu 24.04.1, kernel 5.15.153.1-microsoft-standard-WSL2, nftables 1.0.9, AMD Ryzen 7 5800X (16 threads) |

### Topology
All nodes are Linux network namespaces. None is linked to the root namespace, so no lab packet can reach the internet. The only external traffic in this whole task was package, crate and binary downloads. No n0 relay, n0 DNS or pkarr server was configured anywhere. Endpoints used `RelayMode::Disabled`, `RelayMode::Custom(<lab helpers only>)` or `Custom(empty)`, with no address lookup.

```
                         nl-inet  "internet core" (bridge, or the delaying switch)
                          7.7.7.1/24, 2001:db8::1/64 (routes the LAN /64s)
   ____________________________|______________________________________________________
  |        |          |          |          |          |                             |
nl-ca   nl-stun    nl-h1      nl-h2     nl-load    nl-r-cgn .31              nl-r-<x> .20+i (WAN)
.5      .6 + .7    .10        .11       .12        (symmetric CGN)           NAT/firewall router
Pebble  STUN x2    helper     helper    load gen        | 100.64.11.0/24          |  10.i.0.0/24
                   relay 1    relay 2                nl-r-upcgn (UPnP)        nl-c-<x> 10.i.0.2
                   + TCP fwd                             |                    (v6 routers also:
                   (opt. c)                          nl-c-upcgn               2001:db8:i::2)
```

| Router (client) | Type | Behaviour (nftables) | STUN check from 2 servers (local port 40000) |
|---|---|---|---|
| eim1, eim2 | **EIM**: full cone, port-preserving | `masquerade` + DNAT of all inbound UDP to the client | `7.7.7.21:40000` twice |
| prc1, prc2 | **PRC**: port-restricted cone | `masquerade` (port-preserving, endpoint-independent mapping, addr+port-dependent filtering) | `7.7.7.23:40000` twice |
| sym1, sym2 | **SYM**: symmetric / CGNAT-like | `masquerade fully-random` | `:23405` and `:65446`, varies by destination |
| v6a, v6b | **V6+SYM4**: phone-like dual stack | IPv4 as SYM; global IPv6 `2001:db8:i::2` behind a stateful firewall (inbound only for established flows) | IPv4 varies by destination |
| upnp1, upnp2 | **UPNP** | PRC + miniupnpd (UPnP IGD, NAT-PMP/PCP) | `7.7.7.29:40000` twice |
| upcgn | **UPnP behind CGN** | PRC + miniupnpd home router whose WAN is 100.64.11.2 behind the SYM `nl-r-cgn` | `7.7.7.31`, varies |

All routers drop unsolicited WAN input to themselves, as real home routers do (see §6, artifact 1). "Public" IPv4 is `7.7.7.0/24` on purpose: miniupnpd, like real routers, refuses to map ports when its WAN address is reserved. IPv6 is `2001:db8::/32`.

**Latency.** WSL2's kernel has neither `sch_netem` nor an NFQUEUE target. So `natlab wanemu`, a userspace learning switch built on AF_PACKET, replaced the core bridge and delayed every frame by **20 ms per core hop**. Resulting RTTs:
- Direct IPv4 client↔client: **40 ms**.
- Client→helper relay→client: **≈ 84 ms**.
- Routed IPv6 client↔client: **≈ 80 ms**, because it crosses the core twice.

There is no loss and no jitter. The load test ran on the kernel bridge with no delay.

### Method
- **One case** = two fresh `natlab peer` processes.
  - Keys come from fixed seeds.
  - Router conntrack tables are flushed before each case, and leftover UPnP mappings are deleted.
  - Each peer waits 3 s (settle), then publishes its `EndpointAddr` to a file. That file exchange stands in for **our own signaling**.
  - B dials A. In "simultaneous" variants both dial at a shared wall-clock instant (+500 ms), which stands in for Nostr-ephemeral timing.
- **Timeouts:** connect 10 s, then 6 s of path observation.
- **Repetitions:** 3 for the main (a) and (b) variants, 1 for the others.
- **Metrics** (all measured from dial start):
  - **Connect**: handshake done.
  - **TTFB**: a 1-byte echo round trip completes.
  - **Direct after**: the first time the *selected* path is an IP path.
- Raw JSON lines per case: `results/raw/<exp>__<pair>__r<n>/{a,b}.log`. Per-case rows: `results/<exp>.jsonl`.

---

## 2. Experiment 1: baseline (relays disabled, no discovery, no portmapper, one-sided dial)

**0/13 pairs connected** (all timeouts). Every client knows only its private LAN address (10.x), plus its IPv6 global on the v6 routers. The one-sided IPv6 dial is dropped by the peer's stateful firewall. This confirms the expected failures: without address discovery, even full-cone NATs are unreachable.

---

## 3. Experiment 2: option (a), helper-hosted iroh relays

### 3.1 IP-address certificate (the L8 "X1" path), verified end to end
- `instant-acme` 0.8.5 → Pebble, with `Identifier::Ip` for **IPv4 and IPv6 in one order** and profile `shortlived`.
- http-01 validation was served by a throwaway responder on port 80.
- **Issued in 1.03–1.04 s.** The certificate has `IP Address:7.7.7.10, IP Address:2001:DB8::10`, is valid 160 h, and the chain is 2 PEMs.
- Pebble sent 3 validation GETs per identifier.
- instant-acme's built-in `Order::finalize()` needs its non-default `rcgen` feature. We built the CSR ourselves with rcgen `SanType::IpAddress` and called `finalize_csr`.
- The certificate was fed to the embedded `iroh_relay::server` via `CertConfig::Manual`.
- The client trusts it via `Builder::ca_tls_config(CaTlsConfig::custom_roots(<Pebble root>))`. **IP-SAN verification works** for both the relay TLS and the QAD QUIC handshake.
- **Fallback also works:** a self-signed CA plus an IP-SAN leaf came online in 244 ms. A client given the wrong CA never comes online.

### 3.2 NAT matrix, helper relay h1 only (`RelayMode::Custom`, 20 ms/hop)
| Pair | Types | Connected | Direct | Connect ms | TTFB ms | Direct after ms | Final path |
|---|---|---|---|---|---|---|---|
| eim1-eim2 | EIM / EIM | 3/3 | 3/3 | 42 | 93 | 42 (first path) | direct v4 |
| eim1-prc2 | EIM / PRC | 3/3 | 3/3 | 44 | 93 | 44 (first path) | direct v4 |
| prc1-prc2 | PRC / PRC | 3/3 | 3/3 | 85 | 176 | **259** | direct v4 (hole punch) |
| eim1-sym2 | EIM / SYM | 3/3 | 3/3 | 42 | 92 | 42 (first path) | direct v4 |
| prc1-sym2 | PRC / SYM | 3/3 | **0/3** | 84 | 174 | – | **relay** |
| sym1-sym2 | SYM / SYM | 3/3 | **0/3** | 83 | 173 | – | **relay** |
| v6a-v6b | V6+SYM4 / V6+SYM4 | 3/3 | 3/3 | 84 | 182 | **340** | direct **v6** (hole punch) |
| v6a-prc2 | V6+SYM4 / PRC | 3/3 | **0/3** | 84 | 176 | – | **relay** |
| upnp1-prc2 | UPNP / PRC | 3/3 | 3/3 | 43 | 92 | 43 (first path) | direct v4 |
| upnp1-sym2 | UPNP / SYM | 3/3 | 3/3 | 43 | 93 | 43 (first path) | direct v4 |
| upnp1-upnp2 | UPNP / UPNP | 3/3 | 3/3 | 43 | 93 | 43 (first path) | direct v4 |
| upcgn-prc2 | UPnP-behind-CGN / PRC | 3/3 | **0/3** | 84 | 182 | – | **relay** |
| upcgn-sym2 | UPnP-behind-CGN / SYM | 3/3 | **0/3** | 84 | 174 | – | **relay** |
| **Total** | | **39/39** | 24/39 (8/13 pairs) | | | | |

- **QAD works through a helper relay.** Every client learned its public mapping, e.g. `7.7.7.23:39347` for PRC. On SYM clients it learned the mapping *towards the relay*. The first net report finished at ≈ 418 ms.
- **Home relay online:** median **541 ms**, range 532–556 ms (n = 130 endpoint starts).
- **Timing:**
  - When the peer's published address is reachable (full cone or UPnP), iroh races it against the relay and the direct path wins: connect 42 ms = 1 RTT.
  - Otherwise the first path is the relay (≈ 84 ms). The hole-punched direct path is selected at ≈ 259 ms for PRC and ≈ 340 ms for IPv6.
  - Relayed TTFB ≈ 175–182 ms, which is 2 relayed RTTs.
- **Variants** (1 rep each, same 13 pairs; `results/relay_*.jsonl`):
  - **Dial with endpoint id + relay URL only** (`relay_idonly`, no IPs from signaling): 13/13 connected. It went direct on the same 8 pairs, but always via the relay first (direct at 257–342 ms). **The relay alone is enough to coordinate; signaling IP addresses only speeds up the first path.**
  - **The two peers home on different helpers** (`relay_cross`, A on h1, B on h2): 13/13 connected, direct on the same 8. Connect is slower (≈ 208 ms instead of 84) because the dialer first opens a connection to the peer's relay.
  - **Portmapper disabled** (`relay_noportmap`): same results, except **upnp1-sym2 drops to relay-only**. So the UPnP mapping is what turns "UPnP vs symmetric" into a direct path.

### 3.3 Runtime `insert_relay` / `remove_relay` (`results/relaymgmt/`)
| Test | Result |
|---|---|
| R1: start `RelayMode::Disabled`, then `insert_relay(h1)` | **Does nothing.** Not reflected in `addr()`, never online (10 s). With `Disabled` there is no relay transport to insert into. |
| R2: start `RelayMode::Custom(RelayMap::empty())`, then `insert_relay(h1)` | Works. In `addr()` after **420 ms**, online after +116 ms. |
| R3: home h1, `insert_relay(h2)`, then `remove_relay(h1)` | Inserting h2 doesn't change the home relay. After removing h1, the home relay moved to h2 in **421 ms** and came online. |
| R4: sym1↔sym2 connected **only via h1**; dialer calls `remove_relay(h1)` | The existing connection **keeps working over h1**: every later ping succeeded (≈ 82 ms) over the next 10 s. The published address still listed h1 for ≥ 10 s, because no other relay was available. After `insert_relay(h2)` the home relay became h2. Re-inserting h1 switched the home relay back. |

**Engine rule:** start every endpoint with `Custom(empty or seed map)`, never `Disabled`, if helpers are to be added later. `remove_relay` doesn't tear down live relayed connections.

### 3.4 Bracketed IPv6 relay URL (the reported `ServerName` bug): **confirmed**
The client in v6a used `https://[2001:db8::11]:8443` against a dual-stack relay with a valid IPv6-SAN certificate (`results/ipv6_relay_url/`):
- Relay TLS: `Failed to connect to relay server: unable to connect: Invalid TLS servername` (10 retries in 12 s).
- QAD: `QADv6: probe failed: QUIC connection failed: invalid server name: [2001:db8::11]`.
- The endpoint never comes online. `net_report` can still pick it as `preferred_relay`, because the HTTPS latency probe handles brackets.
- The **same relay via `https://7.7.7.11:8443` came online in 250 ms**.

So helper relays are limited to IPv4-literal URLs, or DNS names, until iroh strips the brackets before `ServerName::try_from`. A side effect: with IPv4-literal relays, QAD never reports `global_v6`. IPv6 direct paths come only from interface addresses. That was enough in the lab.

### 3.5 Group-token admission (abuse control)
The embedded relay ran with an `AccessControl` that allows only clients whose `auth_token` (client side: `RelayConfig::with_auth_token`) matches (`results/relay_token/`):
- **Correct token:** online in 257 ms.
- **Wrong or missing token:** never online. `home_relay_status().auth_denied_reason() = "bad group token"`. Each client retried about 9 times within its 4 s online timeout.
- **QAD still answers denied clients.** The QUIC address-discovery listener has no access control. Address reflection stays open to anyone. That costs little, but it is not gated.

### 3.6 "Can this desktop be a helper?" (L8 X2, `results/helper_nat/`)
| Helper position | UPnP mapping (TCP 80, 443, UDP 7842) | ACME IP certificate | Relay usable by SYM↔SYM clients |
|---|---|---|---|
| Behind a UPnP router (upnp2, WAN 7.7.7.30) | OK (`AddPortMapping` via miniupnpc) | **Issued**: http-01 through the mapping, 2.0 s | **Yes.** Clients online in 544 ms. **QAD through the NATed helper worked**: client learned `7.7.7.25:24746`. sym1↔sym2 relayed, connect 88 ms. |
| UPnP home router behind a CGN (upcgn) | **Refused.** IGD "not connected"; miniupnpd: `private/reserved address 100.64.11.2 is not suitable for external IP` | **Failed**: Pebble timed out on `7.7.7.31:80` | No |
| Plain NAT, no UPnP (prc2) | "No IGD found" | **Failed**: timeout on `7.7.7.24:80` | No |

A helper qualifies exactly when it has a public IPv4 address or UPnP on the router that holds the public address. The certificate is bound to the IP, not the port, so port 80 is needed only during validation. The relay itself can use any mapped port; `:8443` and `:9443` were used above.

### 3.7 Helper relay resources (`results/load/`; kernel bridge, no delay; fresh relay per run)
Setup: N relay-only endpoints (`clear_ip_transports`) in nl-load, all homed on h1. N/2 relayed connections. Hold phase: 20 s with one echo per connection per second. Then 50 MB pushes with 1, 5 and 10 pairs. CPU is % of one core on the Ryzen 7 5800X, measured from the relay process only. The load generator runs on the same host.

| N endpoints online (relayed conns) | Relay RSS idle → all online → conns open | CPU during hold (N/2 echoes/s) | 1 pair | 5 pairs | 10 pairs | RSS peak |
|---|---|---|---|---|---|---|
| 20 (10) | 11.2 → 13.0 → 33.5 MB | 0.25% | 437 Mbit/s, 133% | 1.70 Gbit/s, 427% | 2.04 Gbit/s, 442% | 52 MB |
| **100 (50)** | 11.3 → 17.4 → **48.1 MB** | **0.95%** | 387 Mbit/s, 132% | 1.54 Gbit/s, 424% | 2.05 Gbit/s, 452% | **62 MB** |
| 200 (100) | 14.9 → 24.4 → 55.1 MB | 1.8% | 390 Mbit/s, 127% | 1.67 Gbit/s, 441% | 2.29 Gbit/s, 497% | 70 MB |
| 400 (200) | 15.0 → 28.7 → 60.2 MB | 3.4% | 430 Mbit/s, 135% | 1.83 Gbit/s, 484% | 2.32 Gbit/s, 478% | 73 MB |

- All endpoints came online: 400/400 in 1.5 s. Every relayed connection opened, and 0 of 7,200 echoes failed.
- **Idle cost is small.** About 34 KB of RSS per connected client, and under 2% of one core for 100 active relayed connections at sync-like message rates.
- **Bulk relaying is expensive.** About 24–27 core-seconds per GB (1 pair: 50 MB in 0.92–1.03 s at ≈ 1.3 cores). A helper needs the relay's per-client rate limit (`Limits::client_rx`) plus the "memory for helping others" cap.

---

## 4. Experiment 3: option (b), direct only (relays disabled, our own signaling)

| Pair | Types | b1: one-sided dial, portmapper on | b2: simultaneous dial | b3: simultaneous + STUN prediction |
|---|---|---|---|---|
| eim1-eim2 | EIM / EIM | ✗ | ✗ | **3/3 direct**, 43 ms |
| eim1-prc2 | EIM / PRC | ✗ | ✗ | **3/3**, 44 ms |
| prc1-prc2 | PRC / PRC | ✗ | ✗ | **3/3**, 43 ms |
| eim1-sym2 | EIM / SYM | ✗ | ✗ | **3/3**, 43 ms |
| prc1-sym2 | PRC / SYM | ✗ | ✗ | ✗ 0/3 |
| sym1-sym2 | SYM / SYM | ✗ | ✗ | ✗ 0/3 |
| v6a-v6b | V6+SYM4 ×2 | ✗ (firewall) | **3/3 direct v6**, 83 ms | **3/3 v6**, 84 ms |
| v6a-prc2 | V6+SYM4 / PRC | ✗ | ✗ | ✗ 0/3 |
| upnp1-prc2 | UPNP / PRC | **1/1**, 42 ms | **3/3**, 43 ms | **3/3**, 44 ms |
| upnp1-sym2 | UPNP / SYM | **1/1**, 43 ms | **3/3**, 43 ms | **3/3**, 46 ms |
| upnp1-upnp2 | UPNP / UPNP | **1/1**, 42 ms | **3/3**, 42 ms | **3/3**, 44 ms |
| upcgn-prc2 | UPnP-behind-CGN / PRC | ✗ | ✗ | ✗ 0/3 |
| upcgn-sym2 | UPnP-behind-CGN / SYM | ✗ | ✗ | ✗ 0/3 |
| **Pairs connected** | | **3/13** | **4/13** | **8/13** (24/39 runs) |

What each piece contributes:
- **iroh's portmapper maps automatically.** It needs no configuration on desktop, and it used UPnP `AddAnyPortMapping` against miniupnpd, not NAT-PMP/PCP. It advertises the router's WAN IP with the mapped port (e.g. `7.7.7.29:61359` for local port `42729`). That address alone lets **any** peer dial in, including symmetric ones, with a one-sided dial.
  - Behind a CGN, miniupnpd refuses the mapping, and nothing is advertised (`published: ["10.11.0.2:…"]` only).
- **Global IPv6 through stateful firewalls works only with a simultaneous dial.** One-sided: 0/1. Simultaneous: 3/3 at 83 ms, i.e. 1 v6 RTT. It needs no STUN, relay or portmapper: iroh advertises the interface's global address, and both sides' outbound packets open their own firewall.
- **STUN from a side socket:** probe two STUN servers from a separate socket, then call `add_external_addr(public_ip : iroh's bound v4 port)`.
  - The prediction is right **exactly when the NAT preserves ports**: EIM, PRC and UPnP routers. The side socket showed `port_preserved: true` and `varies_by_dest: false` on them.
  - On SYM and CGNAT the side socket showed `varies_by_dest: true`. The predicted address was wrong and those pairs failed.
  - With the prediction plus a simultaneous dial, PRC↔PRC and EIM↔SYM connect directly **without any relay**, at 1 RTT.
  - Without STUN (b2), port-preserving NATs are useless: iroh only knows its 10.x address.
- iroh has a related built-in heuristic, `DirectAddrType::Qad4LocalPort` (QAD IP + fixed local port). It applies only when QAD says the mapping varies by destination (≥ 2 relays) **and** the socket is bound to a fixed port (source reading). So it doesn't help the relay-less case.
- **Lab artifact found and fixed.** Linux routers that *accept* unsolicited WAN input (no input-drop rule) create conntrack entries for the peer's early packets. The router then remaps the client to a new port (`…dport=29172` instead of 48816), which breaks port preservation during simultaneous open. The first PRC↔PRC relay run failed to punch because of this. Real home routers drop WAN input; the lab routers now do too.

---

## 5. Experiment 4: option (c), reduced form (custom transport as the first path)

- **Unstable API:** `unstable-custom-transports`.
- **The custom transport:** a deliberately dumb **TCP datagram forwarder** on helper h1 (`ctrelay`, 7.7.7.10:9000). It stands in for any non-iroh first path: a libp2p circuit, a HyperDHT blind relay, a Veilid route.
- **Setup:** iroh keeps its IP transports. Relays are disabled. The dialer adds the peer's `TransportAddr::Custom`.
- **Measured:** whether iroh then opens and selects direct IP paths in-band.

| Pair | ct_only: first path | ct_only: IP paths opened | ct_stun (+ STUN prediction): result |
|---|---|---|---|
| eim1-eim2, eim1-prc2, prc1-prc2, eim1-sym2 | custom, 84–106 ms | none | **direct v4 selected at 258–259 ms** |
| prc1-sym2, sym1-sym2, v6a-prc2, upcgn-prc2, upcgn-sym2 | custom, 83–85 ms | none | custom only (no IP path) |
| v6a-v6b | custom, 84 ms | **v6 path opened** (hole-punched through both firewalls, 339 ms) but not selected | same |
| upnp1-{prc2,sym2,upnp2} | **direct from the start**, 43 ms (portmapped address) | yes | direct selected at 257–261 ms |
| **Connected** | **13/13** | | **13/13**; direct selected in 7 pairs + 1 v6 opened = the same 8 pairs as (a) and (b) |

**Findings:**
- **iroh does hole-punch in-band over a foreign first path.** The candidate exchange runs inside the QUIC connection (`n0_nat_traversal`, `remote_state.rs`), and it opened IPv4 and IPv6 paths through PRC NATs and stateful firewalls.
- **But the candidates are only iroh's own direct addresses.** With no relay there is no QAD, so behind IPv4 NAT it needs the STUN prediction exactly as in (b).
  - Per source reading, QUIC `ObservedAddr` path events are received and **ignored** (`NoqPathEvent::ObservedAddr => {}`). The public `PathEvent` has no observed-address variant, so "observed-address reports from a reachable member" (LAYERS P2 d) aren't available through iroh 1.2's API.
- **The v6 path wasn't selected only because of lab RTT.** Routed v6 crosses the delay core twice (≈ 82 ms RTT), which ties with the custom path. `BiasedRttPathSelector` biases only against *relay* paths, not custom ones. On real networks a direct path would usually win, but an app using (c) should install a `PathSelector` that prefers IP.
- **API gaps:**
  - `Endpoint::addr()` / `watch_addr()` **never include custom-transport addresses**; they merge only IP and relay. Our signaling must add them. The first ct run failed entirely until the dialer did.
  - `CustomAddr` needs `iroh-base` and `noq-udp` as direct dependencies.
- **Conclusion:** (c) adds **no direct-path coverage** beyond (a) + (b). Its only value is as another *relay* type, i.e. a fallback path, and it carries all the risk of the unstable API.

## 6. Not run, lab artifacts and caveats

- **(d) rust-libp2p/DCUtR and (e) HyperDHT: not run (time).**
  - Because direct coverage depended only on the NAT pair, neither is expected to add direct paths on the 5 relay-only pairs, with one exception.
  - HyperDHT's randomized "birthday" punching (L6) is the **only candidate technique** that might turn **PRC/CGN ↔ SYM** into direct. **Not verified.** It is the one open question the lab could still answer (L6 X1).
- **Artifacts:**
  1. The router input-accept conntrack effect (§4). It is fixed, but it shows that real routers which accept WAN input would break simultaneous open.
  2. IPv6 paths are about 2× the IPv4 RTT in this topology.
  3. Latency is a userspace switch with no loss or jitter, and timings are single-host.
  4. NAT behaviours are Linux netfilter only: masquerade, fully-random, and a DNAT full cone. No real router firmware was tested.
  5. The UPnP router is miniupnpd, and iroh used UPnP, not PCP/NAT-PMP.
  6. The "SYM" type is the worst case: fully random ports per destination. RFC 6888 requires CGNs to use endpoint-independent mapping, and many carriers do. On such CGNs, the PRC-style results (hole punch works with QAD or STUN) would apply instead.
- **Pebble ≠ Let's Encrypt:**
  - LE validates from **multiple network perspectives**; Pebble used one. Rate limits (50 per IP per week) don't apply in Pebble.
  - ISPs that block inbound TCP 80 would stop issuance. That wasn't tested.
- **Not measured:**
  - Real phones and carrier NATs: mapping type, UDP timeouts, IPv6 firewall policy, Android background socket death.
  - Windows desktops as relays: firewall prompts.
  - The real share of qualifying helpers.
  - NAT rebinding during a session.
  - The relay under hostile load.

---

## 7. Recommendation for D49 (data-backed)

Coverage in the lab, as pairs with at least one path out of 13 (pairs with a direct path in brackets):

| Option | Connects | Direct | Needs | Can't cover |
|---|---|---|---|---|
| (a) helper iroh relays | **13/13** | 8/13 | A reachable helper: public IPv4 or UPnP router. IP certificate via ACME. `Custom` relay map. | Helpers behind CGN or without UPnP can't serve. No helper available means no relay. IPv6-literal URLs (bug). |
| (b) direct: IPv6 + UPnP + STUN prediction + simultaneous dial | 8/13 | 8/13 | Our signaling (addresses + timing), one STUN server for the side socket | Every pair with a symmetric/CGNAT IPv4 side not facing full cone/UPnP (5/13): **0%**, with no fallback |
| (c) custom first path + in-band punch | 13/13 (via the forwarder) | 8/13 (with STUN) | Unstable API, a relay-like first-path provider, own `PathSelector`, custom addresses in our signaling | Adds nothing over (a) + (b). Its fallback is just another relay. |

**Build order:**
1. **(a) Helper-hosted iroh relays: build first.**
   - It is the only no-n0 option that connected every NAT pair, including **symmetric ↔ symmetric (phone-like CGNAT on both sides) and IPv6 phone ↔ IPv4-only peer**. Those pairs were always relayed, never direct.
   - With (a) the app also gets QAD, hole-punch coordination (PRC↔PRC direct at ≈ 3 relayed RTTs) and runtime management (start with `Custom(empty)`; `insert_relay`/`remove_relay` work).
   - The pieces are all verified: instant-acme IP certificates, embedded iroh-relay with `CertConfig::Manual`, `ca_tls_config`, and a token gate through `AccessControl`.
   - Cost per helper is small for sync traffic: about 50–60 MB RSS and under 2% of one core for 100 relayed connections. It needs a rate limit for bulk traffic (≈ 25 core-seconds per GB).
   - **Limits:**
     - Only desktops with public IPv4 or a UPnP router qualify. None behind CGN did in the lab.
     - IPv4-literal URLs until the iroh bracket bug is fixed. Worth an upstream PR: strip `[]` before `ServerName::try_from` in `client/tls.rs` and in the QAD probe.
     - QAD is open to non-members.
2. **(b) Direct paths through our own signaling: build alongside (a), in parallel, since it's cheap.**
   - Components:
     - Publish iroh's own addresses, including global IPv6 and the portmapped address.
     - Add `add_external_addr(STUN public IP : iroh port)` from a side-socket STUN probe, but **only when the probe shows port preservation**.
     - Dial from both sides at a time agreed via signaling.
   - It matches (a)'s direct coverage exactly (8/13) at 1 RTT, with no helper and no n0.
   - It is the **only** way two IPv6-capable phones behind stateful firewalls get direct without any relay (3/3, 83 ms).
   - One-sided dialing isn't enough (3/13); the simultaneous dial is load-bearing.
   - **It can't cover symmetric/CGNAT IPv4 unless the other side is full cone or UPnP-mapped, and it has no fallback.**
3. **(c) Don't build it now.** Same direct coverage as (a) + (b), an unstable API, and extra glue. Revisit only if a non-iroh relay network (libp2p circuit, HyperDHT) is chosen as an additional **fallback** where no helper exists.
4. **Next experiment worth running:** HyperDHT birthday punching for PRC ↔ SYM (L6 X1). It is the one technique that might shrink the relay-only set.

**What remains uncovered without n0:** two IPv4-only phones on symmetric (endpoint-dependent) CGNATs, when **no reachable helper relay** is in the group's helper set. Every option here either needed a relay for that pair or failed. It is covered only by (a), and only if helpers exist.

**Confidence:**
- **High** that the mechanisms work as measured with iroh 1.2 on Linux netfilter NATs.
- **Medium** on transfer to real networks. Carrier CGNAT mapping types, the IPv6 share and firewall policy on mobile, the share of desktops that can be helpers, and Let's Encrypt multi-perspective validation behind home routers are all **unverified**.
