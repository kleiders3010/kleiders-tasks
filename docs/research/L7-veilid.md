# L7: Veilid as a transport and mailbox layer

Research date: 2026-09-24. Scope: whether Veilid (veilid-core, Veilid Foundation / Cult of the Dead Cow) can fill gaps G1–G6 next to iroh and Nostr (DESIGN.md §6, §7, §7a, §8; DECISIONS D11–D23).

**Method.** I read the veilid-core **source at `main` (post-0.5.7)** (downloaded as a tarball from GitLab and read, not built), the Veilid CHANGELOG, the developer book sources, the GitLab issue tracker (API), crates.io metadata and the Debian package index. I also read these precedents:
- OpenArchive's `save-dweb-backend`, `veilid-iroh-blobs`, `save-rust` and the Save Android app's source;
- VeilidChat's GitLab repo;
- **veilist**, a Veilid-based collaborative to-do list app with a detailed engineering log;
- `veilid-vc` and `stigmerge`.

I downloaded one public APK (veilist 0.9.2) and read the sizes of its native libraries without installing it. I sent no traffic to the Veilid network, its bootstrap servers or any relay.

**Versions (crates.io / GitLab, 2026-09-24):**
- `veilid-core` **0.5.7** (2026-07-19), MPL-2.0, MSRV 1.89;
- 38,198 all-time downloads and 2,714 recent (crates.io's "recent" window);
- unreleased work on `main` as of 2026-09-22.

VeilidChat's latest tag is `v0.5.12-internal` (2026-09-13).

---

## Summary

1. **Veilid is the closest thing that exists to the helper network we planned, already deployed.** Every app instance is a full node: DHT storage for others, relaying, private routing and address validation all happen on users' own devices. It needs no accounts, has no terms of service and is free. It is **independent of both n0 and Nostr.** That's why it deserves a serious look.
2. **The DHT maps well onto our slot mailbox (G1).** SMPL schema records give each writer (device) exclusive subkeys: up to 256 members, 1,024 subkeys, **32 KiB per subkey** and **1 MiB per record**. Values are signed, carry a monotonic sequence number and are encrypted by default since 0.5.0. There's no per-IP write limit like Nostr's ~8 events/min, and no operator can ban a "relays as storage" use, because storage is the point of the design.
3. **Retention is weak, and it's measured, not theoretical.**
   - Each node keeps at most **128 records for strangers** (LRU, ≤ 256 MB on disk). Writes need **5 nodes**, reads **3 matching**.
   - veilist saw a real shared list "nobody touched for days … evicted from every node holding it". It emptied a user's list and forced a **6-hourly republish**.
   - Mailbox data therefore survives only while some member device keeps rewriting it. On Android that happens only in the foreground or during jobs.
4. **Android is the weak point.** No Veilid app syncs in the background:
   - VeilidChat and veilist are foreground-only.
   - Save runs Veilid in a `specialUse` foreground service, and its DWeb feature is **off by default** (`isDwebEnabled = false`).
   - A cold attach takes **30–60 s** before a private route can be built (veilid-vc). A warm reattach takes ~6 s (veilist).
   - After an OS-level network drop, the node **never recovered** in veilist's measurements until they added a `network restart` watchdog, which brought recovery down to 42–55 s.
   - A backgrounded node "believes it is ready and can reach nobody".
   - Veilid's own "background refresh / notifications" issue (#499) is open. **G3 is not solved.**
5. **Connectivity (G2/G4) is n0-independent for Veilid's own traffic, but it doesn't fix iroh.**
   - VICE (Veilid Internet Connectivity Establishment) does public-address detection through other nodes, then signalling via inbound relays, reverse connections, hole punching and relay fallback, all without STUN/TURN servers.
   - It works only for Veilid's own sockets. It can't discover iroh's NAT mapping or coordinate iroh's hole punch.
   - Options to use it for iroh: carry iroh rendezvous messages over Veilid, or run iroh over Veilid through iroh 1.2's **`unstable-custom-transports`** feature. Both are experimental.
   - Cold start depends on the **Veilid Foundation's bootstrap** (`bootstrap-v1.veilid.net`, records signed by Foundation keys). Unreleased `main` adds `bootstrap_records`, which lets an app ship bootstrap records and skip DNS.
6. **Maturity is the main risk.**
   - The project is pre-1.0. **6 of the 10 releases from 0.4.7 to 0.5.7 list breaking API changes** (0.4.7, 0.4.8, 0.5.0, 0.5.2, 0.5.3, 0.5.4), and 0.5.0 changed the **wire format** (old nodes are cut off).
   - One developer wrote **96 of the 118 commits on `main` in 2026**.
   - Security issues are open: a Sybil attack on fanout (#445), node-identity tracking (#417), and private-route message length leaking hop count (#392).
   - Network size isn't published anywhere I could find.
   - VeilidChat still has "no official builds".
7. **Size.** veilist's arm64 `libveilid_flutter.so` is **11.8 MB installed / ~5.2 MB deflated**. We'd add most of that to our 7.07 MB core; some crates like tokio and rustls would be shared (the increment is an estimate, not measured).
8. **License.** veilid-core is **MPL-2.0**: file-level copyleft, the same class we already accepted through Loro (D29). The precedents we'd most like to copy (**save-dweb-backend, veilid-iroh-blobs, veilist**) are **GPL-3.0**. Read them for ideas only; don't copy their code.
9. **Verdict: CONSIDER** Veilid as a **fourth, optional mailbox and signalling layer** behind the transport manager, desktop-first and default-off on phones until measured. Don't make it a core dependency now. Confidence: **medium** on the facts, **low** on whether the public network can carry our load.

---

## 1. Architecture (what Veilid actually is)

- **Every app is a node.** "All Veilid applications running `veilid-core` are 'nodes', and they are all equal in the eyes of the network." Nodes "help each other out to the best of their ability and incur no penalty for not being able to assist other nodes" ([networking page](https://veilid.com/how-it-works/networking/)).
- **Capabilities** (source `veilid_capability.rs`, dev book `concepts/nodes.md`): `ROUT` (private/safety routing), `SGNL` (signalling), `RLAY` (relay for NATed peers), `DIAL` (validate others' public addresses), `APPM` (app messages), `DHTV` (DHT storage). `TUNL` (tunnels) and `BLOC` (block/file store) are **"Not currently implemented"**.
  - All are on by default. I found **no mobile-specific defaults** in `veilid_config.rs`: phones relay and store for strangers unless the app sets `capabilities.disable`.
- **Messaging primitives:** `app_message` (fire-and-forget) and `app_call` (request/response), each capped at **32,768 bytes** (`operation_app_call.rs`, `operation_app_message.rs`).
  - Sending to a raw NodeId is compiled out unless the **`footgun-nodeid-target`** feature is enabled ("Features that go against 'Natural Security'"). Normal traffic goes through the receiver's **private route** and the sender's **safety route**.
- **Private routes** hide the receiver and safety routes hide the sender. The default is `default_route_hop_count: 1`, maximum 4. The app publishes route "blobs" (typically in a DHT record) and must republish them when routes die (dev book `private-routing.md`).
  - veilid-vc's author: "Private routes die on their own schedule"; "Attaching to the public network takes 30–60 seconds before a route can be built".
- **Bootstrap:** a DNS TXT lookup of `bootstrap-v1.veilid.net` (default in `veilid_config.rs`). Records are signed by "Primary / Secondary / Backup Veilid Foundation bootstrap signing key[s]". Unreleased `main` adds `network.routing_table.bootstrap_records`, which "can be used to join the network without resolving `bootstrap` at all". A node with a warm routing table reconnects from its persisted table.
- **Isolated networks:** `network_key_password` (a separate network key) plus custom bootstrap records allow a private Veilid network. That would need our own public-IP bootstrap and relay nodes, so it's not usable within our constraints.
- **WASM:** a full node runs in the browser over WebSockets.
  - veilist's production web build had to be served over **http**, not https, because "veilid's wasm dials bootstrap over ws://, which an https page cannot open" (WSS was deprecated in 0.5.0).
  - Relevant to D45 (web guest for iPhone invitees), but awkward.

## 2. DHT storage (G1: offline store-and-forward; G5: presence/change signals)

**Record model** (dev book `concepts/dht.md`, source `veilid_api/types/dht/*`):
- A record key is a hash of the schema and the owner key. The schema is immutable.
- `DFLT` schema: only the owner writes. `SMPL` schema: the owner plus named members, each with an exclusive subkey range.
- Limits from the source: `MAX_SUBKEY_SIZE = 32768`, `MAX_RECORD_DATA_SIZE = 1_048_576`, `MAX_SUBKEY_COUNT = 1024`, `MAX_MEMBER_COUNT = 256`.
- Every subkey has a sequence number; "A storage node accepts a value only if its sequence number exceeds the one it already holds." `min_seqnum` can close a subkey permanently.
- Values have been encrypted by default since 0.5.0 ("Encryption is enabled by default for all DHT operations").
- **Fit with our slot model (§7):** one SMPL record per group epoch, one member slot per device, a snapshot slot owned by the writer that compacts. Because the schema is immutable, the device count is fixed per record. That fits our document epochs (D24), since a new epoch means a new record. Our 47 KiB Nostr chunks must shrink to ≤ 32 KiB subkeys, with a 1 MiB total per record.

**Replication and consistency** (`VeilidConfigInternalDHT` defaults):
- `set_value_count: 5`, `get_value_count: 3`, `consensus_width: 10`, fanouts of 5–6, 10 s timeouts.
- Writes made while offline are queued and flushed by the node (`offline_subkey_writes`, checked every 1 s).
- "Rehydration" re-pushes a record to the network when an owner opens it (`rehydrate.rs`; recheck interval 300 s).
- **DHT transactions** (atomic multi-subkey commits, ≤ 32 records) arrived in 0.5.0 and are still being tuned heavily in unreleased `main`.

**Retention: the critical number.** `VeilidConfigDHT` defaults on native builds:
- `remote_max_records: 128`;
- `remote_max_storage_space_mb: 256` ("Disk cap for remote DHT record storage");
- the remote subkey cache is sized to RAM/32.

The record index is an **LRU** (`record_index.rs`). Other people's records are **stored on disk** (`record_store/mod.rs`: "backed to the TableStore for persistence").

There is no time-based expiry, only LRU pressure plus node churn. Rough capacity: N DHT nodes × 128 records / ~5–10 copies ≈ **13–25 records per node** of network capacity, shared by every app on the network (arithmetic, not measured).

**Real-world evidence (veilist DESIGN.md / ROADMAP phase 25):**
> "storage nodes hold other people's records under an lru capped at `remote_max_records` (128 a node off-web) … a list nobody touched for days was evicted from every node holding it."

The result: two devices "declared themselves synced having read nothing", and the next write **wiped the list for every member**. Their fix:
- keep the device's own contribution locally;
- never treat an empty network read as complete;
- republish every **6 h** while the app is in the foreground.

Our design already keeps full data on every device (§7, "never the only copy"), which avoids the wipe. It doesn't fix delivery to an absent member once all writers have gone quiet.

**Other DHT footguns found in real use (veilist phase 23, 26):**
- **Overlapping writes to one subkey silently lose data.** "veilid stamps a write with the sequence number of the value the writer held when the write STARTED … the dht keeps the first value it receives at a sequence and discards the rest, however much newer … no error and no retry." Writes must be serialized per (record, subkey).
- A watch update can arrive **without its value** (several changes coalesced), so the reader must re-read.
- `open_dht_record` on a joiner returns `TryAgain` until the record is reachable.

**Change signals (G5):**
- DHT **watches** push value-change notifications to watchers: `max_watch_expiration_ms: 600000` (10 min, renewed automatically), `public_watch_limit: 32` and `member_watch_limit: 8` per record per node.
- veilist: "the watch only delivers changes that arrive live"; "backgrounding closes every watch".
- `app_message` over private routes can carry small presence or "dial me" messages.
- Both work **only while the node is attached**.

## 3. NAT traversal and relaying (G2, G4)

- **VICE** (dev book `concepts/nat_traversal.md`): "Veilid implements its NAT traversal in a fashion modeled after STUN, but completely in-network without the use of specialized 'STUN servers'. Veilid relaying is like TURN, but is only applied when no other STUN-like establishment can be applied."
  - Address detection is done by peer nodes with `DIAL`. On unreleased `main`, the "validation quorum scales down to the available validator pool (minimum 2)".
  - Signalling goes through each node's chosen **inbound relay**.
  - Then reverse connection, UDP hole punch (`hole_punch_receipt_time_ms: 5000`) and finally inbound relaying.
  - UPnP is on by default (`upnp: true`).
- **Who relays:** any node with public inbound dial info and `RLAY`. In practice these are volunteer `veilid-server` operators (for example Docker images and a "free Veilid node in the GCP free tier" Terraform repo found on GitHub), desktops with UPnP and Foundation nodes. I found no published count of relay-capable nodes.
- **Limits:** there's no per-relay bandwidth quota in the source, only connection limits: `max_connections_per_ip4: 32`, `max_connection_frequency_per_min: 128`, punishments per IP/node.
- **Scaling concern (inference):** phones on carrier-grade NAT are `OutboundOnly` and each needs an inbound relay. A popular app full of phones adds demand for relays but supplies none. The Veilid team's own simulation ticket (#415) expects "many more relayed nodes … when we have a lot of apps coming online".
- **Latency:** Save's README says: "Due to the high latency of Veilid tunnels, it's faster to ask random peers for data than it is to get the list of peers that have data". veilid-vc exists because "nobody has published the answer" on private-route latency. **No public numbers exist.**
- **What this means for iroh (the actual G2 gap):**
  - Veilid can't tell iroh its public mapped address, because that's a different socket and therefore a different NAT mapping. It also can't run iroh's disco/holepunch exchange.
  - It **can** carry iroh `EndpointAddr`s and "connect now" signals over private routes. That's a discovery channel independent of n0's DNS, comparable to our planned Nostr "where am I" slot, with better metadata properties.
  - The full answer would be **iroh over Veilid**: iroh 1.2 exposes `Builder::add_custom_transport(Arc<dyn CustomTransport>)`, "unstable and gated behind the `unstable-custom-transport` feature". QUIC datagrams fit easily under the 32 KiB cap.
  - That stacks two unstable APIs, private-route churn and unknown latency. Nobody has done it. Save doesn't use iroh networking at all: it uses iroh-blobs 0.24 purely as a verified blob store and moves the bytes over Veilid `app_call` tunnels.

## 4. Android and desktop feasibility

| Topic | Evidence |
|---|---|
| Background sync | None of the precedents do it. VeilidChat manifest: only INTERNET / NETWORK_STATE / WIFI_STATE, no service. veilist: "decided: a backgrounded app syncs when it is next opened. no foreground service … and no WorkManager job." Save: `SnowbirdService` with `foregroundServiceType="specialUse"`, and DWeb is `isDwebEnabled = false` by default. Veilid issue #499 "Background Data Refresh / Notifications" (opened 2025-09-18) is **open** and lists UnifiedPush / FCM / APNs as the only candidates. |
| Attach time | Cold: "30–60 seconds before a route can be built" (veilid-vc). Warm reattach: "~6s" (veilist phase 14). After a `network restart`: "PublicInternet ready in 977ms" (veilist). |
| Network changes | veilist phase 24, Android, OS radio cut: recovery "never within 180s"; `detach()` "never returns" in that state. A `debug("network restart")` watchdog → 42–55 s. Unreleased Veilid `main` adds fixes ("Properly drop TCP connections when offline … as is common on cellular", "Fix UDP stalls on cellular/CLAT464"); issue #512 on OS-level interface notifications is still open. |
| Backgrounded process | veilist: "the node believes it is ready and can reach nobody" (every SetValueQ/GetValueQ times out while in the background with no FGS). |
| Fit with our Android plan (D16) | An expedited or periodic WorkManager job would have to attach, possibly build routes, sync and detach inside the window. A warm attach (~6 s) is plausible. Cold, it's 30–60 s. **Not measured inside a real job window.** |
| Battery | No measurements published. A node keeps a routing table of up to ~1,500 entries (#179), pings liveness, tests routes and serves DHT and relay for others. Unreleased `main`: "Steady-state ping traffic roughly halves". Save's risk assessment: "no mobile battery optimization strategies". |
| Keystore | ProtectedStore fails on Android in some setups (#482, open). veilist: "its keystore-backed secure storage fails to initialize in a release build, so startup must allow a file-storage fallback". |
| Play policy | Default capabilities make a phone relay, route and store (on disk, ≤ 256 MB) for strangers. That's the same "proxy services to third parties" exposure as phone helpers (D19/D20). Disabling `RLAY`/`ROUT`/`DHTV` on phones avoids it but makes phones pure consumers. |
| Build | Veilid removed `rust-android-gradle` and requires NDK 28.2 for the Flutter plugin. We'd link `veilid-core` directly into our Rust engine, so there's no Flutter; **there are no Kotlin/JVM bindings**, which we don't need. Local-network (LAN/mDNS) support is **not implemented** (#2 open since 2022, #387). iroh must stay the LAN layer. |
| Desktop | Linux/Windows/macOS are first-class (veilid-server packages; veilist ships a Linux build). This is where Veilid fits best. |

## 5. Maturity, sustainability, operators

- **Releases:** 0.4.0 (2024-10) → 0.5.7 (2026-07-19), with active work on `main` (last activity 2026-09-22). The CHANGELOG lists "API Breaking Changes" in 0.4.7, 0.4.8, 0.5.0, 0.5.2, 0.5.3 and 0.5.4, and unreleased breaking changes for WASM and Flutter.
  - 0.5.0: "Many on-the-wire encoding changes", with envelope versions now FOURCC codes. A sideloaded APK that stops updating eventually falls off the network.
  - VeilidChat mentions "One sunsetted DH call remains until veilid 0.9.0", so more breaks are planned.
  - A new crypto suite (VLD1, ML-DSA / ML-KEM) is coming; the book warns that suite upgrades "may need additional processing".
- **API stability tooling** (from 0.5.4): "`cargo public-api` support … rejects API changes in CI otherwise". Doc comments now cover the whole public API. There's also a `mock-api` feature for offline tests.
- **Team:** 2026 commits on `main`: Christien Rioux 96 of 118, the next contributor 8. **The bus factor is about 1.**
- **Organisation:** the Veilid Foundation, Inc., a 501(c)(3) in Virginia with a board of three. It's donation-funded (Stripe). I found no published funding, grants or network statistics; the blog has two posts, the latest from 2024.
- **Network operators:** there's no operator list and no size figure. The network is every running app plus volunteer `veilid-server` nodes and Foundation bootstrap nodes. The network-size estimator in `routing_table/network_estimator.rs` exists but reports only locally.
- **Ecosystem:** VeilidChat ("no official builds yet"), Save DWeb (disabled by default), veilist (GPL, early development but "reported from real use"), stigmerge (MPL), reunicorn/Coagulate, and a long tail of hobby repos. crates.io: 2,714 recent downloads.

## 6. Privacy and metadata

- **Better than our current stack on IP exposure.** With safety and private routes, storage nodes and peers don't learn a device's IP or NodeId. That would ease D23 (linkability) and the §7a concern that "helpers' IP is visible to strangers".
- **Caveats:**
  - A node's NodeInfo (IP and dial info) is published to the routing tables of the whole network. A long-lived NodeId can be tracked by anyone who learns it (#417, open).
  - Route-length leakage (#392, open).
  - Save's own risk assessment: "IP privacy is mostly between peers and isn't guarding against an adversary with full network visibility"; Veilid "lacks innocuous masking".
- Encrypted DHT values still expose record keys, schema member public keys, sizes and write timing to the storage nodes.

## 7. Abuse resistance

- Writes must be signed by a schema member, and storage nodes validate schema and signature on set ("SECURITY: Reject invalid DHT schema and value signatures on inbound set value", 0.5.4). So strangers can't overwrite our slots.
- Anyone can **create** records and fill nodes' 128-record LRUs. Nothing like proof of work or quotas per creator exists; LRU eviction is the only defence. A spammer can therefore shorten everyone's retention.
- A Sybil attack on fanout (#445, open) can "return old signed information … starving access to some target record". Our monotonic/CRDT merge limits the damage to delay, as with helper rollback (A2).
- Connection-level punishments per IP and NodeId exist.

## 8. Precedent: OpenArchive `save-dweb-backend`

- **What it does:** "Veilid provides peer discovery, routing, and public key cryptography … Iroh is used for blob-addressed storage and hash verification … blob transfer is mediated by `veilid_iroh_blobs` tunnels built on Veilid routes rather than plain direct Iroh peer connections."
  - Group = a Veilid DHT record; each member's repo = its own record holding its route blob and collection hash.
  - Data goes over `app_call` tunnels in chunks with stop-and-wait ordering.
- **Versions:** veilid-core git tag v0.5.5, `iroh = "0.24.0"`, and **forked iroh crates** patched "to relax hickory-resolver pin for veilid-core 0.5.5 compat". So using both libraries together caused dependency conflicts. Unreleased Veilid `main` drops hickory-resolver entirely, which should ease this.
- **Reliability signal:** save-rust's README says "Veilid/DHT tests can be slow and flaky; P2P tests … get 15 retries to improve pass rate."
- **Operational model:** they built a "Backups Server (Cloud/PC)", an always-on peer (PR #17). That's the same conclusion as our tier-1 "your own always-on machine".
- **Shipping status:** the Save Android app ships the Rust backend and a `specialUse` foreground service, but `AppConfig.isDwebEnabled = false`.
- **License:** GPL-3.0 (save-dweb-backend, veilid-iroh-blobs). Use it as a design reference only.
- **Takeaway:** the one real Veilid+iroh combination uses **Veilid for everything on the network** and iroh only for content addressing. It's evidence that Veilid can stand in for iroh's network layer when privacy is the goal. It isn't evidence that they combine well as parallel transports.

## 9. Candidate-by-candidate verdicts

### C1. Veilid DHT as an additional encrypted slot mailbox (G1, partly G5)
- **Gaps:** G1 (independent of Nostr), G5 (watches while online).
- **Cost:** $0 for users and developer, no developer-run infrastructure (bootstrap is the Foundation's). Users' devices store up to 256 MB of strangers' encrypted records on disk (configurable) and spend bandwidth serving them.
- **ToS:** none exist. The network is explicitly for apps: "the most basic discoverability layer for communication between nodes". The Foundation "will speak out publicly about any immoral uses".
- **Limits:** 32 KiB/subkey, 1 MiB/record, ≤ 256 members, LRU 128 remote records per node, write to 5 nodes and read from 3, watch ≤ 10 min (renewed).
- **Failure modes:**
  - Eviction after days of silence; silent loss on overlapping writes.
  - `TryAgain` on first open.
  - Nodes stuck after network changes on Android.
  - Wire breaks between versions.
  - Unknown network capacity.
- **Android:** foreground or job-window only; needs a warm routing table to be fast.
- **Library:** Rust, MPL-2.0, pre-1.0, ~11.8 MB arm64 `.so` in a real APK.
- **Verdict: CONSIDER (medium confidence).**
  - Worth prototyping as a third parallel mailbox (principle 4). It removes the "no in-constraints alternative to Nostr" statement in D13, **for desktops at least**.
  - It doesn't replace Nostr, and it doesn't solve retention while every member is asleep; it has the same "someone must republish" property.
  - Phones should start as consumers (`DHTV`/`RLAY`/`ROUT` disabled) until Play risk and battery are measured.

### C2. Veilid private routes and relays as n0-independent messaging for sync (G4, G5; G2 for Veilid-native traffic)
- **Gaps:** G4 (relaying through other nodes when a direct path fails), G5 (small signalling), G2 only if the sync traffic itself moves onto Veilid.
- **Cost:** $0. Relaying is done by volunteer and peer nodes.
- **Limits:** 32 KiB messages, private-route churn, "high latency" (Save), unmeasured throughput, hop count 1–4.
- **Operators:** unknown count of relay-capable public nodes. Phones on carrier-grade NAT need them, and there's no evidence the network has spare relay capacity for a large new app.
- **Privacy:** better than iroh relays (IPs hidden behind routes).
- **Verdict: CONSIDER (low confidence).**
  - Good for small live messages ("I changed slot X", "here's my iroh address") when iroh can't connect and n0 is unavailable.
  - Not a bulk sync path until latency and relay capacity are measured.
  - It could also be the **transport for our helper network (§7a)**, hiding helper IPs, but that inherits all of Veilid's maturity risks.

### C3. Veilid as the discovery and address-exchange channel for iroh (G2, discovery half)
- Publish iroh `EndpointAddr`s (with UPnP-mapped or IPv6 addresses) in an encrypted DHT slot, or push them over `app_message`. This replaces n0's DNS for discovery.
- It **doesn't** provide iroh's public-address discovery (QAD) or hole-punch coordination.
- **Verdict: CONSIDER (medium confidence)** as one more discovery source next to Nostr and pkarr. **REJECT as a fix for iroh's hole punching (high confidence):** Veilid can't observe iroh's NAT mapping.

### C4. iroh over Veilid via `unstable-custom-transports` (G2 + G4 for iroh)
- Technically plausible: iroh 1.2 has `add_custom_transport`, and Veilid carries unreliable datagrams up to 32 KiB.
- It stacks two unstable APIs. Nobody has done it: Save avoided iroh networking entirely.
- Latency over private routes is unknown, so QUIC congestion control over multi-hop routes is a real question. Route death forces transport-level path changes.
- **Verdict: CONSIDER only as a lab experiment (low confidence).** Don't plan around it.

### C5. Veilid for waking sleeping Android phones (G3)
- **REJECT (high confidence).** A node has to be running and attached. Veilid has no push mechanism, and #499 proposes only FCM, UnifiedPush or APNs. No Veilid app receives anything in the background without a foreground service.

### C6. Veilid for large blobs (G6)
- There's no block store ("BLOC … Not currently implemented"). Apps chunk over 32 KiB `app_call`/`app_message` (Save, veilid-vc, stigmerge). No throughput data is public, and Save reports high latency. DHT records cap at 1 MiB and are subject to the same LRU.
- **REJECT for now (medium confidence).** Keep images postponed (D22). If done over P2P, prefer iroh's direct connections.

### C7. Reusing OpenArchive `save-dweb-backend` / `veilid-iroh-blobs` code
- GPL-3.0, pinned to iroh 0.24 with forked iroh crates, and the feature ships disabled.
- **REJECT as a dependency (high confidence).** Keep it as an architecture reference (a group record plus a per-member record holding a route blob, then tunnels).

## 10. Comparison vs. gaps

| Gap | C1 DHT mailbox | C2 routes/relays | C3 iroh discovery | C4 iroh-over-Veilid | C5 wake | C6 blobs |
|---|---|---|---|---|---|---|
| G1 offline store-and-forward w/o Nostr | **Partial**: yes while some member republishes; LRU eviction after days | — | — | — | — | — |
| G2 internet P2P w/o n0 | — | Veilid-native only | Discovery half only | Yes in theory (unstable) | — | — |
| G3 wake sleeping Android | ✗ | ✗ | ✗ | ✗ | **✗ REJECT** | — |
| G4 relaying for failed punches | — | **Yes** (volunteer relays; capacity unknown) | — | Yes in theory | — | — |
| G5 presence / small signals | Watches (online only) | **Yes** (`app_message`) | Yes (address slot) | — | — | — |
| G6 large blobs | ✗ (1 MiB/record) | Slow chunking | — | Possible | — | **✗ REJECT** |
| Verdict | CONSIDER (med) | CONSIDER (low) | CONSIDER (med) / REJECT for punching | CONSIDER-lab (low) | REJECT (high) | REJECT (med) |

## 11. New risks introduced by adopting Veilid

1. **Forced upgrades by wire breaks.** Pre-1.0 protocol changes (0.5.0) cut off old nodes. That conflicts with "backward compatible forever" (D28) and with sideloaded APKs that update rarely.
2. **Bus factor ≈ 1** (96 of 118 commits in 2026 by one developer). If development stops, the network degrades as apps churn.
3. **Strangers' data on users' disks** (≤ 256 MB by default). This contradicts §7a's "RAM only, never disk" principle unless `DHTV` is disabled or capped. It's also a Play and user-trust question.
4. **Phone as relay/router for strangers by default**: the D19/D20 Play "proxy services" exposure again, plus battery.
5. **Silent data loss patterns** (same-sequence write collisions, eviction reads that look empty). These must be designed around; veilist lost a real list to them.
6. **A new single provider for cold start**: Foundation bootstrap DNS and signing keys. It's weaker than n0's role (needed only at first start, and records can be cached or shipped), but still a dependency.
7. **Dependency friction with iroh**: Save needed forked iroh crates over a hickory-resolver pin. It's improving on Veilid `main`.
8. **Binary growth**: an estimated +6–9 MB installed per ABI (based on the 11.8 MB measured `.so`, minus shared crates; unverified).
9. **Network-capacity free-riding**: a large influx of phone nodes (outbound-only, often offline) could degrade the network for everyone, and nothing tells us its current size.

## 12. Suggested SAFE local experiments (no public-network traffic)

1. **Size (extends E6):** add `veilid-core` (`default-tokio`, no `debug-api`, no `backtrace`) to the existing size-profile Rust core with iroh, Loro and Nostr. Measure the arm64 `.so` delta and check for duplicate crates (rustls/ring, tokio, sqlite).
2. **Isolated Veilid network on one PC:** use `network_key_password` plus `bootstrap_records` (unreleased `main`) or the `virtual-network` feature. Run 10–50 nodes, then measure:
   - cold and warm attach time;
   - SMPL slot write/read latency and consensus;
   - behaviour of overlapping writes (to confirm veilist's same-sequence loss);
   - LRU eviction when > 128 records per node;
   - memory and CPU per idle node.
3. **Android emulator inside the isolated network:** check whether a node can attach, sync one slot and detach within an **expedited WorkManager job** and a 10-min periodic job, from a warm and a cold routing table. Repeat after toggling airplane mode to reproduce veilist's stuck state and test `restart_network()` recovery.
4. **`mock-api` feature:** check that a Veilid mailbox adapter can sit behind our transport-manager trait with fully offline CI tests.
5. **Retention model in E5 (helper simulator):** add a Veilid-like layer (128-record LRU per node, 5 copies, churn at phone and desktop uptimes, an attacker creating junk records). Estimate time-to-eviction for idle groups against the republish interval.
6. **Paper check before any public test:** confirm with the Veilid maintainers (outward contact, your call) the current public network size, the share of relay-capable nodes and their view on a to-do app adding thousands of phone nodes. **Public-network measurements** (cold attach time, record survival over days) would send traffic to strangers' nodes and need your explicit approval.

## 13. Bottom line for the design

Veilid is the only evaluated option that is simultaneously free, serverless, account-free, independent of n0 and Nostr, and **already provides relaying and third-party storage by peer devices**. That's exactly the missing half of D13/D14. It should be tracked as a candidate fourth layer.

Its weaknesses sit exactly where our hardest problems are:
- Android background operation (it can't help);
- retention while every member is asleep (LRU days);
- long-term stability (pre-1.0, wire breaks, one maintainer).

Recommendation: prototype C1+C3 behind the transport-manager interface, desktop-first, after experiments 1–3. Decide on phones only with measured attach time, battery and Play-policy data.

---

## Sources

**Veilid (primary):**
- Source code `main`, read 2026-09-24: https://gitlab.com/veilid/veilid. Files cited: `veilid-core/src/veilid_config.rs`, `storage_manager/constants.rs`, `storage_manager/record_store/**`, `storage_manager/rehydrate.rs`, `veilid_api/types/dht/mod.rs`, `veilid_api/types/dht/schema/*.rs`, `veilid_api/types/veilid_capability.rs`, `rpc_processor/coders/operations/operation_app_call.rs`, `operation_app_message.rs`, `network_manager/relay_worker.rs`, `routing_table/network_estimator.rs`, `veilid-core/Cargo.toml`
- CHANGELOG: https://gitlab.com/veilid/veilid/-/blob/main/CHANGELOG.md
- Developer book (sources `concepts/dht.md`, `nat_traversal.md`, `nodes.md`, `bootstrap.md`, `private-routing.md`, `apps/api/appmessaging.md`, `admin/upgrading.md`, `veilid-foundation.md`): https://gitlab.com/veilid/developer-book and https://veilid.gitlab.io/developer-book/
- Networking overview: https://veilid.com/how-it-works/networking/
- Site: https://veilid.com/ · Foundation: https://veilid.org/ · blog: https://veilid.org/blog/
- VeilidChat status: https://veilid.com/chat/ · repo: https://gitlab.com/veilid/veilidchat (CHANGELOG, `android/app/src/main/AndroidManifest.xml`)
- Issues: #2 https://gitlab.com/veilid/veilid/-/issues/2 · #179 · #387 · #392 · #400 · #415 · #417 · #420 · #445 · #482 · #499 · #512 (same URL pattern)
- crates.io: https://crates.io/api/v1/crates/veilid-core
- Debian package index (veilid-server 0.5.7 deb: 4,453,192 B amd64 / 4,153,660 B arm64): https://packages.veilid.net/apt/dists/stable/main/binary-amd64/Packages
- Wikipedia (release date, license): https://en.wikipedia.org/wiki/Veilid

**Precedents:**
- OpenArchive save-dweb-backend (README, Cargo.toml, PR list): https://github.com/OpenArchive/save-dweb-backend
- veilid-iroh-blobs: https://github.com/OpenArchive/veilid-iroh-blobs
- save-rust README: https://github.com/OpenArchive/save-rust
- Save Android (`AppConfig.kt`, `AndroidManifest.xml`, `SnowbirdService.kt`, `SnowbirdBridge.kt`): https://github.com/OpenArchive/Save-app-android
- Save DWeb privacy and risk assessment: https://hackmd.io/@s74XZjUBQDuPPS04AgPvow/HkacodHgyx
- veilist (DESIGN.md, ROADMAP.md, release 0.9.2 APK): https://github.com/khimaros/veilist
- veilid-vc: https://github.com/chouithegewy/veilid-vc
- stigmerge: https://github.com/cmars/stigmerge

**iroh:**
- `iroh` 1.2.0 features incl. `unstable-custom-transports`: https://crates.io/api/v1/crates/iroh
- `Builder::add_custom_transport`, `address_lookup`, `external_addr`: https://docs.rs/iroh/1.2.0/iroh/endpoint/struct.Builder.html

**Internal:** `DESIGN.md` §6–§8, `research/DECISIONS.md` D11–D24, D28, D29, D45, `research/REPORT.md`, `research/R1-iroh.md`.

---

## Independent verification

Verifier pass on 2026-09-24. I used different routes from the researcher where I could: the GitLab REST API (issues, commits, trees), raw source files on `main`, the crates.io API, the GitHub API (veilist, Save, save-dweb-backend, save-rust, veilid-vc, iroh), and the veilist 0.9.2 APK's zip central directory, read with an HTTP range request. I sent no traffic to the Veilid network.

### Facts re-checked and confirmed
- **crates.io:** `veilid-core` 0.5.7 (2026-07-19), MPL-2.0, MSRV 1.89, 38,198 downloads, 2,714 recent.
- **`veilid_config.rs` on `main`, native builds:** `remote_max_records = 128` and `remote_max_storage_space_mb = 256`. The WASM build uses **64 records and 128 MB**, so web guests (D45) keep even less. Other defaults: `set_value_count = 5`, `get_value_count = 3`, `consensus_width = 10`, watch 600,000 ms, watch limits 32 public / 8 member, `default_route_hop_count = 1`, `upnp = true`, `hole_punch_receipt_time_ms = 5000`, `max_connections_per_ip4 = 32`. Bootstrap is `bootstrap-v1.veilid.net`, signed by three Foundation keys, and `bootstrap_records` is empty by default.
- **`dht/mod.rs`:** `MAX_SUBKEY_SIZE = 32768` and `MAX_RECORD_DATA_SIZE = 1_048_576`. **`operation_app_call.rs`:** request and answer are each capped at 32768 bytes.
- **CHANGELOG:** 0.5.0 enabled encryption by default for all DHT operations (#300) and brought "many on-the-wire encoding changes" (#453).
- **The block store is an empty stub.** `block_store/mod.rs` has an empty `BlockStoreInner`, and `BLOC` sits behind the `unstable-blockstore` feature. This makes the C6 REJECT stronger.
- **Issue states from the GitLab API:** #2, #387, #392, #415, #417, #445, #482, #499 and #512 are all **open**.
- **Commits on `main` since 2026-01-01:** 118 in total, 96 by one author, the next author 8. **Bus factor ≈ 1 confirmed.**
- **veilist** (GPL-3.0; 0.9.2 released 2026-09-12). Phase 25 and DESIGN "keeping a record on the network" confirm the 128-record LRU explanation and the 6 h republish. Phase 24 confirms "never within 180s", the `network restart` watchdog at 42–55 s, and "the node believes it is ready and can reach nobody".
- **APK:** the veilist 0.9.2 arm64 APK's central directory lists `lib/arm64-v8a/libveilid_flutter.so` at **11,761,776 bytes, stored uncompressed**. The researcher's 11.8 MB is confirmed.
- **Save Android** (branch `next`): `AppConfig.isDwebEnabled = false`, and `SnowbirdService` has `foregroundServiceType="specialUse"`.
- **save-dweb-backend:** `iroh = "0.24.0"`, `hickory-resolver = "=0.25.2"`, forked iroh crates, veilid tag v0.5.5.
- **save-rust:** "15 retries".
- **Save README:** "Due to the high latency of Veilid tunnels…". Confirmed.
- **iroh 1.2.0** (2026-09-09) has an `unstable-custom-transports` feature. The docs.rs page for `add_custom_transport` says it is "not covered by semantic versioning guarantees". The iroh CHANGELOG already shows a **breaking** change to this feature gate (#4228).
- **No terms of service.** veilid.com and veilid.org link only to a FAQ and a VeilidChat privacy policy.
- **No public latency or throughput figures.** veilid-vc (updated 2026-09-02) is a harness only and publishes no results.

### Corrections and challenges
1. **Licence claim is wrong.** The report says MPL-2.0 is "the same class we already accepted through Loro (D29)". It isn't accepted: DECISIONS.md D29 is still an open decision between (a) amending principle 9 and (b) replacing Loro. Under principle 9 as written ("MIT, Apache or BSD only"), `veilid-core` and `veilid-tools` (both MPL-2.0 only) are **not allowed**.
   - Adopting Veilid would force D29(a).
   - It would also make MPL unavoidable even if Loro were later replaced.
   - Direct dependencies I checked are permissive or dual-licensed: `keyring-manager`, `keyvaluedb*` (MPL OR MIT OR Apache), `capnp`, `maxminddb` (ISC), `webpki-roots` (CDLA-Permissive-2.0).
2. **Retention evidence is weaker than "measured, not theoretical".**
   - veilist's eviction is **one real-use incident with an inferred cause**. veilist itself says eviction "cannot be provoked from the harness (it needs the storage nodes to evict, which takes days and other people's traffic)". Storage nodes are ordinary user devices, so **node churn** is an equally plausible cause.
   - The conclusion (retention needs someone to rewrite) still stands. The numbers behind "days" do not exist yet.
   - veilist also found a Veilid bug: `make_room_for_record` counts only bytes, not the record-count limit. So the LRU path is under-exercised upstream.
3. **Missed: Veilid already has automatic rehydration.** "Local rehydration" arrived in 0.4.5 and is transaction-aware since 0.5.4 (`storage_manager/rehydrate.rs`, `tasks/rehydrate_records.rs`). When a member **opens** a record, the node re-pushes any local subkeys that lack online consensus.
   - So "republish" may reduce to "open the record inside a WorkManager job" rather than rewriting the data.
   - veilist says "nothing else refreshes a record", yet its devices still read empty after coming back. Why rehydration didn't save them is unexplained.
   - Worth testing in isolated-network experiment 2: evict, then reopen, then check reachability.
4. **"Phones as consumers" worsens free-riding on storage, not only on relays.** The report flags relay free-riding, but its C1 recommendation (disable `DHTV` on phones) means every phone record takes about 5 remote LRU slots and gives back none. In an Android-first app, nearly all DHT capacity would come from desktops and strangers. This is an internal tension in the recommendation.
5. **Android integration needs a JNI shim.** The report says "no Kotlin/JVM bindings, which we don't need".
   - On Android, `veilid-core` requires `veilid_core_setup_android(env, Context)` to be called through JNI before startup (`veilid_api/android`). It holds the JavaVM and the application Context for keystore access, and pulls in `jni 0.22`.
   - That's small, but it's an extra native entry point beside UniFFI. It also explains the ProtectedStore failures (#482).
6. **#499 is slightly mis-summarised.** Besides UnifiedPush, FCM and APNs, it lists periodic tasks, background fetch and the SimpleX no-server approach. None is implemented and the issue is still open, so the G3 REJECT stands.
   - Upstream is actively working in this area: a `main` commit on 2026-09-22 is titled "improve thaw speed for backgrounded veilid processes", and the Android setup code now handles "background engines".
   - Re-check at the next release. This could shorten attach time inside job windows.
7. **The network size can be observed.** `VeilidStateAttachment.estimated_network_size` (plus `reliable_peer_count` and `live_peer_count`) is exposed to every app. A single approved public attach, or a question to the maintainers, would give a size figure. It is still unpublished.
8. **The dependency conflict with iroh is probably moot now.** The Save conflict was iroh 0.24 against veilid 0.5.5 over hickory. iroh 1.2.0 has no direct hickory dependency (it uses `iroh-dns` and `simple-dns`), `veilid-tools` 0.5.7 needs `hickory-resolver ^0.25.2`, and `main` drops hickory entirely. Confirm with `cargo tree -d` in experiment 1.
9. **Principle 2 (two independent providers per role) isn't met for cold start** until `bootstrap_records` ships in a release. Today the Foundation's DNS/TXT bootstrap is the only provider. Unreleased `main` also adds direct B01T bootstrap and caching of records.

### Verdict checks
| Candidate | Researcher | Verifier | Note |
|---|---|---|---|
| C1 DHT slot mailbox | CONSIDER (med) | **CONSIDER, upheld (med on facts, low on fitness)** | Limits confirmed. The retention cause is inferred, not measured. Rehydration on open was missed. The licence needs D29(a). Phones-as-consumers worsens storage free-riding. |
| C2 private routes/relays | CONSIDER (low) | **Upheld** | Still no public latency data. Relay capacity unknown. |
| C3 discovery for iroh | CONSIDER (med) / REJECT-high for punching | **Upheld, with a different reason** | iroh 1.x NAT traversal runs **in-band inside an existing QUIC connection** (`noq` `n0_nat_traversal`; `remote_state.rs::trigger_holepunching`). So out-of-band Veilid messages can't coordinate it, whatever Veilid knows about NAT mappings. The viable route to hole punching is C4, not C3. |
| C4 iroh over Veilid custom transport | CONSIDER-lab (low) | **Upheld, but under-credited, so raise its priority** | Source reading, untested: iroh calls `trigger_holepunching()` after **every** new connection whatever the path type, and custom paths stay open beside IP paths. Once a connection exists over Veilid, iroh would swap NAT-traversal candidates over it and punch direct IP paths itself, keeping Veilid as the relayed fallback. That covers **G2 signalling and G4 relaying without n0**. The missing piece is **QAD** (the reflexive IPv4 address, which only iroh relays provide). So direct paths come only from global IPv6, UPnP/PCP port maps, LAN and `external_addr`. IPv4-NAT-to-IPv4-NAT pairs without UPnP stay relayed over Veilid. One unverified heuristic: feed Veilid's detected public IPv4 plus iroh's bound port through `external_addr` for port-preserving NATs. **This is the most gap-covering Veilid candidate, and it can be tested entirely locally** (isolated Veilid network plus two iroh endpoints). |
| C6 large blobs | REJECT (med) | **Upheld, confidence high** | `block_store/mod.rs` is an empty stub behind `unstable-blockstore`. The 32 KiB messages and 1 MiB records are confirmed. |

### Priority suggestion (per the user's "cover what we lack or doubt")
The gaps we most lack are G2 and G4 without n0. The best-leveraged Veilid experiment is **C4 in an isolated local network**:
- attach time;
- whether iroh's candidate exchange and hole punch actually run over a custom-transport path;
- QUIC behaviour over 1-hop routes;
- path failover when a route dies.

Run it before any C1 mailbox work. C1 adds a third mailbox beside Nostr and member devices, but its retention still depends on the same "someone must wake up" property that G3 leaves unsolved.
