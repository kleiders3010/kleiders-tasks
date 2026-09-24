# R1: iroh — does it hold up as Kleiders' P2P layer?

Research date: 2026-09-24. Scope: DESIGN.md §3 (architecture), §4 (blobs), §5 (device identity), §6 (transport stack), §8 (Android lifecycle), §9 (privacy), §12 (size budget), §15–16 (open items, build order).
Method: I read iroh's official docs (the `.md` sources on docs.iroh.computer), n0's blog, the iroh, net-tools, iroh-address-lookups, iroh-blobs and iroh-ffi source, and GitHub issues. I also checked crates.io, Maven Central and PyPI metadata and inspected the published Android AAR's ELF files. Third-party sources: an arXiv measurement study, Android developer docs, and the Delta Chat, Lobsters and HN discussions. I sent no traffic to any relay, DNS server or DHT.

Current versions (from crates.io, 2026-09-24): `iroh` **1.2.0** (2026-09-09), `iroh-relay` / `iroh-dns-server` 1.2.0, `iroh-blobs` **0.103.0** (2026-06-15), `iroh-gossip` 0.101.0, `iroh-mdns-address-lookup` **0.5.0**, `iroh-mainline-address-lookup` **0.5.0**, `portmapper` / `netwatch` 0.19.3, `iroh-ffi` 1.1.0 (Maven `computer.iroh:iroh` / `computer.iroh:iroh-android` 1.1.0, 2026-07-17).

---

## Summary

1. **n0's public relays are not meant for production. This is the biggest problem.** n0's own docs say the public relays are for "**development and hobby use only**" with "no SLA", "only the latest stable release... officially supported", rate limits that "get tuned as needed" and have been enforced "more strictly" as usage grew, and a "right to block offending IP addresses or users". I found **no independent public iroh relay provider**. Delta Chat's chatmail servers run iroh relays, but they are on 0.35, which is wire-incompatible with 1.x. §2 principle 2 ("≥2 independent providers per role") therefore **fails** for three iroh roles: relaying, hole-punch coordination and address discovery.
2. **Layer 3 is mislabeled: iroh no longer uses STUN.** STUN was removed in 0.90 (June 2025). iroh learns its public address through **QUIC Address Discovery (QAD) on its own relays** (UDP 7842), and hole-punch coordination also travels over the relay path. Without a reachable relay there is no hole punching and no address discovery. Layers 2, 3 and 7 all share **one provider: n0**.
3. **Default discovery runs on n0 infrastructure, and the design's group-keyed DHT discovery isn't an iroh feature.** The `N0` preset publishes to and resolves from n0's `dns.iroh.link` (DNS + HTTPS pkarr). mDNS and Mainline-DHT lookup are **off by default** and live in separate **pre-1.0 (0.5)** crates. The DHT crate publishes **only the relay URL** by default, so it still depends on relays. iroh resolves *endpoint IDs*. Publishing "under a key derived from the group" (§6) would be custom pkarr/BEP44 code.
4. **iroh 1.x is real and stable as an API and wire format, and the Rust-core + own-UniFFI plan works.** 1.0 shipped 2026-06-15 with a semver and wire-compatibility policy. Official Kotlin bindings (UniFFI, JVM + Android) came back in June 2026. The published Kotlin artifacts are immature, though: Android libraries were missing until 1.1.0, the AAR still ships **4 KB-aligned `.so` files that fail Play's 16 KB rule** (iroh-ffi #279/#285, still unreleased on 2026-09-18), JDK 21 is required, and mDNS and blobs aren't exposed. Kleiders should wrap iroh inside its own Rust engine, as §3 already plans, and should not depend on iroh-ffi.
5. **Android works, with sharp edges the design doesn't mention yet.**
   - The JNI context must be initialized before creating an endpoint. Otherwise iroh panics in release builds, or with `panic="abort"`, or **silently falls back to Google DNS 8.8.8.8**.
   - iroh's Android network monitor is a **no-op** ("Very sad monitor. Android doesn't allow us to do this"). The app must call `Endpoint::network_change()` from a Kotlin `ConnectivityManager` callback.
   - n0 itself says to **shut the endpoint down in the background**.
   - A bound endpoint wakes the radio often: path heartbeats every 5 s, QAD re-probes every 20–26 s, pkarr republish and net-report every 5 min.
   - mDNS needs a `MulticastLock`, which iroh doesn't take.
   - **Android 17 (targetSdk 37) blocks all LAN traffic** (mDNS, LAN unicast, gateway port mapping) unless the user grants the `ACCESS_LOCAL_NETWORK` runtime permission.
6. **The "~90% direct" figure comes from the vendor.** n0 says "roughly 9 out of 10 network configurations" and "95–99% direct *data* rate". A large independent study of a comparable decentralized technique (libp2p DCUtR, 4.4 M attempts) measured **70% ± 7.1%**. iroh added hard-NAT↔easy-NAT punching in 1.0-rc.1, but **hard↔hard (for example both phones behind carrier-grade NAT, CGNAT) still falls back to the relay**. The phone↔phone-on-cellular case matters most for this app and is likely well below 90%.
7. **iroh-blobs isn't production-ready on iroh 1.x.** Its README says: "this version of iroh-blobs is not yet considered production quality... use iroh-blobs 0.35". But 0.35 needs iroh 0.35, which doesn't interoperate with 1.x, and n0's public relays for 0.35 end on 2026-12-31. Between 0.98 and 0.103 it had a breaking release every 3–6 weeks.
8. **Binary size (measured from the published AAR):** the stripped `libiroh_ffi.so` for iroh 1.1 is **14.0 MB for arm64-v8a** (6.0 MB deflated), 9.8 MB for armeabi-v7a and 15.6 MB for x86_64. iroh alone therefore fills most of the §10 estimate of "Rust core ~8–15 MB per architecture". A core with Loro and nostr on top will probably land at **~18–28 MB uncompressed per ABI** unless it's size-tuned. Native libraries are stored uncompressed in the APK by default, which puts the "installed ≤ 40 MB" target at risk.
9. **WASM/browser: confirmed relay-only.** It needs `default-features = false`. The example bundle is 2.6 MB (1.1 MB gzipped). A web guest would lean 100% on relays, whose free tier is dev-only. Nostr works natively in browsers and fits the web guest better.
10. **Privacy gaps §9 doesn't list:**
    - The stable device key is also the iroh endpoint ID. Relays, n0's DNS server, the user's DNS resolver and everyone on the same Wi-Fi (via mDNS) see the same identifier, so it links all of a device's groups. That undoes the per-group Nostr key separation.
    - DNS lookups of `_iroh.<id>.dns.iroh.link` show who is looking up whom.
    - Restoring or migrating a device can put two devices on one endpoint ID.

---

## Claim-by-claim table

| # | Claim (DESIGN.md) | Verdict | Severity if wrong | One-line evidence | Fix |
|---|---|---|---|---|---|
| C1 | iroh 1.x is stable, suitable as the P2P layer, and usable from Kotlin via UniFFI on Android and JVM desktop | **Partial** | Medium | 1.0 on 2026-06-15, now 1.2.0, with a wire/semver policy. Official UniFFI Kotlin bindings exist (JVM + Android), but the published Android artifact has missing libs (fixed in 1.1.0) and 4 KB alignment (unfixed). Runtime bugs and security fixes are still landing. | Keep the own-UniFFI wrapper. Don't depend on iroh-ffi. Copy its Android JNI-init pattern. Build with NDK r28+ (16 KB). Plan to update iroh at least every quarter. |
| C1b | iroh-ffi deprecated? | **Refuted** (not deprecated) | — | Paused in Feb 2025, officially supported again since 2026-06-18. It covers only the 1.0 core. Blobs, gossip, docs and mDNS are out of scope. | Not needed. The engine is Rust. |
| C2a | LAN discovery (mDNS) via iroh | **Partial** | Medium | Separate crate `iroh-mdns-address-lookup` 0.5 (pre-1.0), off by default. On Android it needs a `MulticastLock` and, from targetSdk 37, `ACCESS_LOCAL_NETWORK`. It broadcasts the endpoint ID. | Add the crate. Take the MulticastLock only while in the foreground. Plan the permission UX. Rotate the transport key (see NR2). |
| C2b | Direct IPv6 | **Partial** | Low | Supported, but peers exchange addresses through the relay unless direct addresses are published (off by default, for privacy). Mobile IPv6 firewalls still need simultaneous open. | Treat IPv6 as a path iroh picks, not as a layer independent of relays. Optionally publish IPv6 addresses inside the E2E group doc. |
| C2c | UDP hole punching with "iroh + public STUN", ~90% direct | **Partial / Refuted ("public STUN")** | High | STUN removed in 0.90. QAD runs on iroh relays and punching is coordinated over the relay path. The 90% is a vendor figure; an independent libp2p study got 70% ± 7%. Hard↔hard NAT isn't supported. | Rewrite layer 3 as "iroh (needs an iroh relay)". Drop "public STUN". Budget for a relay-dependent 60–80% direct rate on mobile and measure it (E3b). |
| C2d | Router port mapping (UPnP / NAT-PMP / PCP) | **Confirmed** (with caveats) | Low | `portmapper` is a default feature and on at runtime (`PortmapperConfig::Enabled`). It is pre-1.0 (0.19). On Android 17 it needs LNP. It can trigger firewall prompts. | Keep it on for desktop. Turn it off on Android (little value on mobile, costs a permission and battery). |
| C2e | Discovery via DNS/pkarr on the BitTorrent Mainline DHT under a group-derived key | **Refuted as stated** | High | By default DNS/pkarr goes to **n0's** `dns.iroh.link`. The DHT is an opt-in 0.5 crate that publishes the relay URL only. iroh looks up endpoint IDs, not group keys. | Put member endpoint IDs in invites and the group doc so peers can be dialed directly. If group-keyed discovery is still wanted, build it with the `pkarr` / `n0-mainline` crates (1000-byte limit, one writer per key/salt). |
| C2f | Which parts need n0 infrastructure / optional crates | **Answered** | — | n0: relays (4 regions), DNS/pkarr server. Optional crates: mDNS, DHT, blobs. Default feature: portmapper. | See details. |
| C3 | n0 public relays usable free by production apps long-term; ≥2 providers | **Refuted** | High (breaks a §1/§2 hard constraint) | "Development and hobby use only". "Do not rely on it for production workloads". Rate limits unpublished and tightening. Latest-version-only. No independent 1.x-compatible public relay found. | See C3 details: ask n0 for written permission, add a community relay list, member-hosted relays, or Nostr-only fallback. At minimum, reword principle 2 honestly. |
| C3b | "0.35 relays supported until Dec 31 2026" | **Confirmed** | — | 1.0 post table: v0.35.x → 2026-12-31. v0.9x and 1.0.0-rc → 2026-09-30. Public relays are run "for the latest major version". | Target 1.x only. Expect a similar sunset when 2.0 ships (majors come at most every 6 months). |
| C4 | iroh works on Android (background, network changes, multicast lock, battery, Doze; production apps) | **Partial** | Medium–High | Works and has Android CI since 2026-04. But JNI init is mandatory (else panic or Google DNS fallback), there is no native network-change detection, the endpoint must shut down in the background, and there are frequent timers. Production users: Delta Chat (0.35) and Holochain-based apps (1.1). I found no large 1.x Android app. | See the Android checklist in C4 details and experiment E3c/E3d. |
| C5 | iroh-blobs suits P2P image transfer | **Partial (not production quality on 1.x)** | Medium (phase 7 feature) | README: "not yet considered production quality". The recommended 0.35 is incompatible with 1.x. Frequent breaking releases. | For images ≤ ~1 MB, use a small custom ALPN (request by hash, BLAKE3 check). Re-evaluate iroh-blobs when it reaches 1.0. |
| C6 | Binary size fits "Rust core ~8–15 MB per arch", installed ≤ 40 MB | **Partial (likely exceeded)** | Medium | Measured: iroh-ffi 1.1 arm64 `.so` is 14.0 MB stripped (6.0 MB deflated). Adding Loro and nostr likely gives 18–28 MB. `.so` files are stored uncompressed in the APK by default. | Run E6 early with opt-level z/s, fat LTO, codegen-units=1, strip, and no metrics/portmapper on Android. Update the §10 budget with the measurement. |
| C7 | iroh runs in the browser (relay-only) for a web guest | **Confirmed** (with caveats) | Low | Official docs say all browser connections go through a relay. Needs `default-features=false`. Example is 2.6 MB wasm / 1.1 MB gzipped. | Make Nostr the web guest's primary transport. Use iroh in the browser only if a relay with a suitable policy exists. |

---

## Details per claim

### C1 — iroh 1.x stability and Kotlin/UniFFI usability

**Evidence for:**
- iroh 1.0 shipped on 2026-06-15 ([v1 post](https://www.iroh.computer/blog/v1)). Patch releases 1.0.1–1.0.3 followed, then 1.1.0 (crates.io 2026-08-25) and 1.2.0 (2026-09-09) ([CHANGELOG](https://github.com/n0-computer/iroh/blob/main/CHANGELOG.md)).
- Wire guarantee: "an iroh v1 endpoint will be able to communicate with another iroh v1 endpoint, regardless of minor version or language."
- [Release policy](https://docs.iroh.computer/about/release-policy): majors ≥ 6 months apart, minors ≥ 4 weeks apart. A new major must stay wire-compatible with the previous major. Full support lasts 1 year for a major and **3 months for a minor**.
- The public API deliberately exposes only 1.0 crates (`noq`, `iroh-base`, `iroh-relay`, `iroh-dns`...). rc.0 moved mDNS and DHT out of the crate ([rc.0 post](https://www.iroh.computer/blog/iroh-1-0-0-rc-0)).
- Official bindings: "Swift, Kotlin, Python, and JavaScript bindings with first-party support" ([language support, 2026-06-18](https://www.iroh.computer/blog/iroh-language-support)). Kotlin is generated by uniffi-rs 0.31, published as `computer.iroh:iroh`, and covers "JVM + Android" ([Kotlin docs](https://docs.iroh.computer/languages/kotlin), [languages](https://docs.iroh.computer/languages/index), [iroh-ffi Cargo.toml](https://github.com/n0-computer/iroh-ffi)). This shows that UniFFI → Kotlin over iroh works on both targets Kleiders needs.
- iroh-ffi is **not deprecated**. It was paused in Feb 2025 ("We are pausing FFI releases until we figure out a better solution", [ffi-updates](https://www.iroh.computer/blog/ffi-updates)) and revived for 1.0.

**Evidence against / caveats:**
- **Published Kotlin artifacts are immature on Android:**
  - The Maven artifact shipped **no Android native library** through 1.0.0: apps "crash at runtime with `UnsatisfiedLinkError`" ([iroh-ffi #246](https://github.com/n0-computer/iroh-ffi/pull/246), fixed in 1.1.0).
  - The 1.1.0 AAR's `libiroh_ffi.so` is **4 KB ELF-aligned, failing Google Play's 16 KB requirement** ([#279](https://github.com/n0-computer/iroh-ffi/issues/279)). A fix is committed, but on 2026-09-18 users were still asking for the release ([#285](https://github.com/n0-computer/iroh-ffi/issues/285)). I confirmed `LOAD align 0x1000` on arm64-v8a in the published AAR myself.
  - The Kotlin docs say the published artifact is single-platform and that Android "requires building from source". The hello-iroh-ffi demo says the opposite, so the docs disagree with each other.
  - Requirements: JDK 21+ and Kotlin 2.2 metadata.
  - Not exposed: mDNS and Bluetooth ("Rust-only features"). Blobs, gossip and docs are "out of scope".
  - There is no Kotlin Multiplatform (Kotlin/Native) support ([#272](https://github.com/n0-computer/iroh-ffi/issues/272)). A community wrapper exists (`annexflow/iroh4k`). This doesn't matter for Kleiders, which only targets Android + JVM.
- **Runtime maturity (3 months after 1.0):**
  - 1.0.2 fixed a pre-auth DoS in the relay server ([1.0.2](https://www.iroh.computer/blog/iroh-1-0-2)).
  - 1.1.0 fixed a **client-side** bug where "a specially-crafted malicious relay packet could pin a cpu core to 100%", plus a panic on crafted `EndpointAddr` and NAT probes directed at unrelated IPs ([1.1.0](https://www.iroh.computer/blog/iroh-1-1-0)).
  - 1.0.3 fixed noq panics.
  - Open question: #4549, "Anonymous memory grows ~6 GB in ~30 min" on 1.1/1.2 server workloads, was closed as not planned. A related unbounded `pending_open_paths` bug is #4548 ([#4549](https://github.com/n0-computer/iroh/issues/4549)).
- **Upgrade treadmill:** minors get only 3 months of full support, and public relays officially support "only the latest stable release". Kleiders therefore has to ship iroh updates roughly quarterly. F-Droid and APK users will lag behind.

**Assessment for the design:** the §3 plan (Rust engine → own UniFFI → Kotlin through the NDK `.so` on Android and JNA on desktop) is sound. Kleiders never needs iroh-ffi, but should copy two things from it: the Android JNI-context init (`IrohAndroid.installAndroidContext`, [#237](https://github.com/n0-computer/iroh-ffi/pull/237)) and the "JNA from the `@aar` variant" packaging fix ([hello-iroh-ffi Android README](https://github.com/n0-computer/hello-iroh-ffi/tree/main/kotlin-android)).

**Fix:**
- Keep the own-UniFFI wrapper.
- Build Android `.so` files with NDK r28+ or `-Wl,-z,max-page-size=16384` ([Android 16 KB](https://developer.android.com/guide/practices/page-sizes)).
- Pin iroh with `~1.x` and add a quarterly "bump iroh" task to §12a.
- Subscribe to iroh security advisories.

### C2 — What iroh actually provides (defaults, infrastructure, optional crates)

| Layer (§6) | What iroh 1.2 really does | Default on? | Needs n0 (or other) infra? | Crate / stability |
|---|---|---|---|---|
| 1 LAN discovery (mDNS) | `MdnsAddressLookup` advertises `<endpoint-id>._irohv1._udp.local` via `swarm-discovery`. Has a passive (listen-only) mode. | **No** ("mDNS address lookup is not enabled by default") | No | `iroh-mdns-address-lookup` **0.5.0**, pre-1.0 ([docs](https://docs.iroh.computer/connecting/local-address-lookup), [repo](https://github.com/n0-computer/iroh-address-lookups)) |
| 2 Direct IPv6 | Binds v4 + v6, and multipath QUIC picks the best path | Yes | Address exchange normally goes through the relay, because pkarr publishes only the relay URL by default ("By default the `PkarrPublisher` publishes only the home relay URL, not direct IP addresses") | core |
| 3 Hole punching | QUIC-NAT-traversal draft on noq multipath. Reflexive address via **QAD on relays** (UDP 7842). Coordination frames travel over the relay path. **No STUN since 0.90** ("iroh-relays no longer support STUN", [0.90](https://www.iroh.computer/blog/iroh-0-90-the-canary-series); [QAD](https://www.iroh.computer/blog/qad); [enterprise networks](https://docs.iroh.computer/configuring-networks): UDP 7842 "is the job STUN does in a VoIP system") | Yes | **Yes: an iroh relay** | core |
| 4 Port mapping | UPnP, PCP, NAT-PMP. `PortmapperConfig::Enabled` by default ([endpoint.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint.rs)). Cargo default feature `portmapper` ([Cargo.toml](https://github.com/n0-computer/iroh/blob/main/iroh/Cargo.toml)) | Yes | No (talks to the local gateway) | `portmapper` 0.19, pre-1.0 |
| 7 Public relays | `N0` preset: 4 hardcoded n0 relays `use1-1`, `usw1-1`, `euc1-1`, `aps1-1.relay.n0.iroh.link` ([defaults.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/defaults.rs)) | Yes (N0 preset) | **Yes: n0** | `iroh-relay` 1.2 (self-hostable) |
| Discovery (DNS/pkarr) | Publishes a signed pkarr packet to n0's `iroh-dns-server` and resolves via DNS `_iroh.<z32-id>.dns.iroh.link TXT` plus HTTPS pkarr (added to the N0 preset in 1.0.3) ([address lookup](https://docs.iroh.computer/concepts/address-lookup), [presets.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint/presets.rs), [1.0.3](https://www.iroh.computer/blog/iroh-1-0-3)) | Yes | **Yes: n0** `dns.iroh.link` | core + `iroh-dns-server` 1.2 (self-hostable) |
| Discovery (Mainline DHT) | `DhtAddressLookup` publishes the same pkarr record to BEP44. Default `AddrFilter::relay_only()` (only the relay URL). Republishes every 1 h ([lib.rs](https://github.com/n0-computer/iroh-address-lookups)) | **No** | DHT bootstrap nodes. The record points at a relay, so a relay is still needed | `iroh-mainline-address-lookup` **0.5.0** on `n0-mainline` 0.6 |

**Key problems in the design text:**

- **"iroh + public STUN" (layer 3) is wrong.** There is no STUN client left in iroh, so public STUN servers can't act as a second provider. The only address-discovery and hole-punch coordination provider is whatever iroh relay the endpoint uses.
- **The ~90% figure.**
  - n0's claims: "roughly 9 out of 10 network configurations allow a direct connection... Where it fails (some corporate firewalls or cellular networks) iroh automatically falls back to the relay" ([NAT traversal](https://docs.iroh.computer/concepts/nat-traversal)). Also "Expect a 95–99% direct connection rate", but that figure is a *data rate* ([language support](https://www.iroh.computer/blog/iroh-language-support)).
  - A data rate weights heavy transfers on good networks. It isn't a per-pair success rate.
  - Independent baseline for a comparable decentralized, relay-coordinated technique: **70% ± 7.1%** hole-punch success over 4.4 M attempts from 85k networks in 167 countries ([arXiv 2510.27500](https://arxiv.org/abs/2510.27500)).
  - iroh punched only in the "hard client ↔ easy server" direction until 1.0-rc.1, which added "the asymmetric case where the server is behind a hard NAT" ([rc.1](https://www.iroh.computer/blog/iroh-1-0-0-rc-1)). **Hard↔hard is not covered.** That is the typical phone-on-CGNAT ↔ phone-on-CGNAT case.
  - 0.96–0.97 had punching regressions ("Connections that used to punch through would sometimes sit on the relay", [0.98](https://www.iroh.computer/blog/iroh-0-98-0-getting-back-to-traversing-nats)). Punching quality can regress between releases.
- **Layer 5 (birthday punching).** iroh has no birthday or port-prediction punching. Adding it would mean patching noq or using the *unstable* custom-transport API (`unstable-custom-transports`, excluded from 1.0 guarantees).
- **Discovery "under a key derived from the group" (§6, §5).** iroh's address lookup maps **EndpointId → addresses**. The DHT crate publishes under the *endpoint's* key. A group-derived key would need custom BEP44/pkarr code:
  - The value is limited to 1000 bytes.
  - It is mutable and single-signer. Every holder of the group secret can overwrite it. Use `salt` per device, or accept last-writer-wins.
  - It must be republished (the iroh crate uses 1 h), which a sleeping phone can't do.
  - The iroh FAQ also warns that DHT lookup is off by default "partly so that mobile apps don't look like BitTorrent clients and get flagged by the OS" ([FAQ](https://docs.iroh.computer/about/faq)).
  - Simpler approach: invites (link/QR/code payload) carry the inviter's endpoint ID(s), and the group doc carries every member's endpoint ID. Then plain `connect(endpoint_id)` works with any address lookup.

**Fix:**
- Rewrite §6 rows 2–4 and 7 to show the real dependency: layers 2, 3 and 7 all depend on reaching an iroh relay, and the design has one provider for it.
- Remove "public STUN".
- Mark mDNS and DHT as optional pre-1.0 crates.
- Replace group-keyed DHT discovery with endpoint IDs carried in invites and the group doc (Nostr stays as the second discovery channel). If DHT discovery is kept, use the iroh DHT crate for *endpoint* records only.

### C3 — n0 public relays: policy, limits, capacity, independent providers

**n0's own words:**
- "Public relays are suitable for **development and hobby use only**. For production, use managed relays." ([public relays](https://docs.iroh.computer/iroh-services/relays/public))
- "No SLA or uptime guarantee · Only the latest stable release of iroh is officially supported · ... n0.computer reserves the right to remove support for older iroh versions at any time · Traffic is rate-limited to prevent abuse". Also: "We monitor public relays for abuse... we reserve the right to block offending IP addresses or users."
- "The exact rate limits aren't published today, as they get tuned as needed... public infrastructure is meant for development and hobby use. **Do not rely on it for production workloads**" ([rate limiting](https://docs.iroh.computer/relays/rate-limiting)).
- "As iroh has gotten more popular, we have had to enforce these rate limits more strictly" ([shared relays, 2026-09-08](https://www.iroh.computer/blog/shared-relays)). The same post answers "Are the public relays going away? No." Pricing: Community $0 with a variable rate limit and "Best for: Development". Pro $19/mo: shared relays, 5 MB/s, 100 GB egress, 10k concurrent connections. Dedicated $199 per region.
- "Number 0 runs public relays for the **latest major version** of iroh" ([release policy](https://docs.iroh.computer/about/release-policy)).
- Support table in the 1.0 post: v0.35.x until **2026-12-31**, v0.9x and 1.0.0-rcX until **2026-09-30**, v1.0 "until end of life" ([v1](https://www.iroh.computer/blog/v1)). **The "0.35 until Dec 31 2026" note is confirmed.**
- Inconsistency: for the **DNS/pkarr** servers n0 is more permissive: "You're more than welcome to run production systems using the public relays if you find performance acceptable. The public servers do rate-limit traffic, there is no guaranteed uptime" ([DNS docs](https://docs.iroh.computer/connecting/dns-address-lookup)).

**Capacity and incidents:**
- "More than 200 million endpoints created, *in the last 30 days alone*" ([v1](https://www.iroh.computer/blog/v1)).
- On 2024-11-05 a global relay failure lasted ~12 h. New hole-punched connections failed, existing ones survived. It "handled a peak of 100,000 concurrent connections before failing", and capacity was quadrupled afterwards ([post-mortem](https://www.iroh.computer/blog/relay-down-a-post-mortem)).
- Authenticated managed relays have been the default since June 2026 ([authenticated relays](https://www.iroh.computer/blog/authenticated-relays)).

**Independent public providers:** I found none.
- Delta Chat's chatmail servers deploy `iroh-relay` (cmdeploy) for their own clients ([chatmail/core #5591](https://github.com/chatmail/core/issues/5591), [Delta Chat realtime](https://delta.chat/en/2024-11-20-webxdc-realtime)). But Delta Chat is on **iroh 0.35** ([Delta Chat forum](https://support.delta.chat/t/reducing-the-number-of-dependencies-for-packaging-iroh-openssl/4481)), and 0.91 made "our final relay wire-level breaking change... this version of iroh can't connect to older relays" ([0.91](https://www.iroh.computer/blog/iroh-0-91-0-the-last-relay-break)). Those relays can't serve 1.x clients, and using them without their operators' consent would be free-riding anyway.
- **libp2p circuit relays** (the §6 "second provider" idea) speak a different protocol. iroh can't use them without the unstable custom-transport API. Circuit-relay v2 is also *limited* by design: go-libp2p defaults to **2 minutes and 128 KiB per relayed connection** ([resources.go](https://github.com/libp2p/go-libp2p/blob/master/p2p/protocol/circuitv2/relay/resources.go), [spec](https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md)). They are meant for hole-punch coordination only, so they aren't a drop-in second relay.
- Self-hosting needs "a server with a public IP and a DNS name", with ACME TLS ([FAQ](https://docs.iroh.computer/about/faq), [self-hosted](https://docs.iroh.computer/iroh-services/relays/self-hosted)). That conflicts with "zero hosted infrastructure".

**Why this is high and not critical:** data safety doesn't depend on iroh relays. The Nostr mailbox and LAN still work. What breaks is live internet P2P (layers 2, 3 and 7) and the §1/§2 hard constraint. The app would be using a service against its operator's stated policy.

**Fix options, from cheapest to cleanest:**
1. **Ask n0 in writing** whether a free, non-commercial, low-traffic app may use the Community relays and DNS in production. Traffic here is tiny (CRDT deltas) and direct paths carry the bulk. Record the answer in DESIGN.md. Their DNS docs already allow production use.
2. **Make relays configurable and multi-provider from day one.** Ship a relay list with the app (like the Nostr list). Accept community-run 1.x relays with an explicit public-use policy if any appear. Record per-relay health, as §7 already plans for Nostr.
3. **Member-hosted relays.** A desktop member (tray app) with a public IPv4/IPv6 address or a successful port mapping could embed `iroh-relay` (the `server` feature) for *its own groups only*. Trust would come from the group doc instead of web PKI, using custom TLS roots or cert pinning (iroh has "allow to customize TLS trust roots", #3973). Group-only access is enforced with the relay's `AccessControl` trait ([rc.1](https://www.iroh.computer/blog/iroh-1-0-0-rc-1)). This fits the helper network (§7a) and gives an honest second provider. Feasibility is unverified; see E3e.
4. **Degrade gracefully.** If no relay is reachable, P2P is LAN-only and sync goes through Nostr. Say so in §6.
5. Paying for Iroh Pro ($19/mo) breaks the "free/no hosted infrastructure" constraints. Authentication would also require shipping the project API key inside the app, and the key is "a project-wide root credential". Not recommended.

### C4 — iroh on Android

**Works:**
- Android is officially supported for aarch64 and armv7 ([languages](https://docs.iroh.computer/languages/index)).
- The iroh test suite has run in the Android emulator in CI since 2026-04 ([#4183](https://github.com/n0-computer/iroh/pull/4183)).
- Production users: Delta Chat, which ships iroh **0.35** on all platforms (webxdc realtime, multi-device setup). Holochain apps through `kitsune2_transport_iroh` on iroh **1.1** ([#4502](https://github.com/n0-computer/iroh/issues/4502)). n0's own demos.
- n0 claims "running in production on hundreds of thousands of devices". I found no named large 1.x Android app, and nobody in the 1.0 HN thread reported mobile or battery experience ([HN](https://news.ycombinator.com/item?id=48542480), [Lobsters](https://lobste.rs/s/cslljn/iroh_1_0_dial_keys_not_ips)).

**Android issues to design for:**

1. **JNI context is mandatory.**
   - What happens without it: iroh's resolver reads system DNS through JNI and `ndk_context`. Without a context it panics. Since 1.0.x the panic is caught and iroh falls back to **Google DNS (8.8.8.8)**. "If your compilation profile sets `panic = "abort"`, this can't work, and thus your app will panic" ([Endpoint docs, "Usage on Android"](https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint.rs); [#4371](https://github.com/n0-computer/iroh/pull/4371)).
   - A 1.1 user reported the silent Google fallback as "a privacy leak" and got "wrong answers on split-horizon / VPN" ([#4502](https://github.com/n0-computer/iroh/issues/4502)). The fallback became configurable in 1.2.0 (#4419).
   - Tethering to an iPhone can hand out link-local DNS servers, which time out ([#4210](https://github.com/n0-computer/iroh/pull/4210), open).
   - **Fix:** initialize `ndk_context` from Kotlin before the first bind. Better still, pass explicit nameservers read from `LinkProperties` and set fallback mode to "none/fail", refreshing both on network change. Don't use `panic = "abort"` unless the JNI path is guaranteed.
2. **No native network-change detection.**
   - netwatch's Android route monitor is a stub: "Very sad monitor. Android doesn't allow us to do this" ([netmon/android.rs](https://github.com/n0-computer/net-tools/blob/main/netwatch/src/netmon/android.rs)). Background polling is set to 1 h "to effectively disable it on mobile" ([actor.rs](https://github.com/n0-computer/net-tools/blob/main/netwatch/src/netmon/actor.rs)).
   - The Endpoint docs agree: "some systems like android do not expose this functionality to native code... This function allows for notifying iroh".
   - Default-route detection shells out to `/system/bin/ip route` ([interfaces/linux.rs](https://github.com/n0-computer/net-tools/blob/main/netwatch/src/interfaces/linux.rs)). Netlink is blocked for apps (old [#2324](https://github.com/n0-computer/iroh/issues/2324)).
   - **Fix:** in Kotlin, register a `ConnectivityManager.NetworkCallback` that calls `engine.network_change()` (→ `Endpoint::network_change()`) on every `onAvailable` / `onLost` / `onLinkPropertiesChanged`.
3. **Background, Doze and battery.**
   - n0's Kotlin guide: "Going to background: call `ep.shutdown()`... Staying live while backgrounded: run iroh inside an Android foreground service... Without one, Android will eventually kill the network sockets" ([Kotlin docs](https://docs.iroh.computer/languages/kotlin)). Doze "Suspends network access" ([Doze](https://developer.android.com/training/monitoring-device-state/doze-standby)).
   - Timers while an endpoint is bound:
     - path `HEARTBEAT_INTERVAL` = 5 s and `PATH_MAX_IDLE_TIMEOUT` = 15 s ([socket.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/socket.rs))
     - QAD re-probe every 20–26 s
     - full net report every 5 min ([net_report.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/net_report.rs))
     - pkarr republish every 5 min ([pkarr.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/address_lookup/pkarr.rs))
     - n0 describes hole-punched connections that "just ping the relay path every ~5s" ([shared relays](https://www.iroh.computer/blog/shared-relays)).
   - A bound endpoint keeps the cellular radio awake, so it is a foreground-only or short-job-only object on phones.
   - **Fix:** bind at foreground or job start and close on background (matching §8's `shortService`). Persist only the key. Expect a cold start of ~1–5 s: relay connection, net report, publish (measure in E3d).
   - Two phones syncing in 15-minute WorkManager windows will rarely be online together. **Phone↔phone P2P is essentially a foreground feature.** Background jobs should only try iroh with peers marked "usually reachable" (desktops) and otherwise go straight to Nostr.
4. **mDNS on Android.**
   - Wi-Fi drivers filter multicast unless the app holds a `WifiManager.MulticastLock`: "Normally the Wifi stack filters out packets not explicitly addressed to this device... Processing these extra packets can cause a noticeable battery drain" ([MulticastLock](https://developer.android.com/reference/android/net/wifi/WifiManager.MulticastLock)).
   - iroh and swarm-discovery don't acquire it. Neither iroh doc mentions it. n0's own Android demo says "no multicast permissions are needed since the demo does no mDNS discovery".
   - **Fix:** acquire the lock (needs the normal permission `CHANGE_WIFI_MULTICAST_STATE`) only while in the foreground on Wi-Fi.
5. **Android 17 Local Network Protection** (new, not in DESIGN.md):
   - From targetSdk 37, "local network is blocked by default". Apps must declare and request the runtime permission `ACCESS_LOCAL_NETWORK` (NEARBY_DEVICES group).
   - The block covers TCP in and out, "Sending UDP unicast, multicast, broadcast", receiving the same, and ".local" DNS resolution. UDP failures return EPERM ([local network permission](https://developer.android.com/privacy-and-security/local-network-permission)).
   - Android 16 has it as an opt-in (`adb shell am compat enable RESTRICT_LOCAL_NETWORK <pkg>`).
   - This hits layer 1 (mDNS plus LAN direct paths) and layer 4 (gateway UPnP/NAT-PMP/PCP). DESIGN §12 plans "targetSdk 36, then 37 as Play requires", so it will land within about a year.
   - It also adds a permission prompt, which conflicts with UX principle C4 ("never has to understand transports").
   - n0's Android demo README still says "no ... local-network permission exists on Android", which is outdated.
   - **Fix:** ask for the permission in context, for example when the user shares a list with someone nearby or links a device on the same Wi-Fi. Without it, fall back to relay and Nostr. Test now with the Android 16 opt-in.
6. **16 KB pages.** Play requires 16 KB-aligned native libraries for apps targeting Android 15+. NDK r28+ does this by default ([page sizes](https://developer.android.com/guide/practices/page-sizes)). This caught iroh-ffi's own AAR ([#279](https://github.com/n0-computer/iroh-ffi/issues/279)).
7. **Private DNS (DoT) bypass (plausible, unverified).** iroh's resolver sends plain UDP DNS to `LinkProperties` servers. That probably bypasses the user's Android "Private DNS" setting. Check in E-privacy.

**Verdict: partial.** iroh runs on Android, but "it just works" isn't true for background use, network switches, LAN discovery or future targetSdk.

### C5 — iroh-blobs for images

- Design fit: BLAKE3 verified streaming, range requests, resume, `MemStore` / `FsStore` (redb), `Downloader` ([blobs docs](https://docs.iroh.computer/protocols/blobs)).
- **Status:**
  - The README opens with "**NOTE: this version of iroh-blobs is not yet considered production quality. For now, if you need production quality, use iroh-blobs 0.35**" ([iroh-blobs README](https://github.com/n0-computer/iroh-blobs)).
  - 0.35 is tied to iroh 0.35, which is wire-incompatible with 1.x ([0.91](https://www.iroh.computer/blog/iroh-0-91-0-the-last-relay-break)), and its public relays end on 2026-12-31. **No production-quality blobs version exists for iroh 1.x.**
  - Release cadence: 0.98 (2026-01-28) → 0.103 (2026-06-15), each one breaking, with nothing since. The repo had 80 open issues and PRs.
  - It is not in the FFI (irrelevant here). It builds on iroh 1.0, so it is API-compatible with 1.2.
- Encryption interplay: Kleiders hashes the *ciphertext*. That is fine as long as each image is encrypted once by its author and everyone fetches the same ciphertext. Don't re-encrypt per upload.
- **Fix:** images are small (≤ ~2048 px WebP, usually < 1 MB), so write a ~200-line custom protocol: ALPN `kleiders/blob/1`, request by BLAKE3 hash, stream the bytes, verify the hash at the end, retry on failure. That avoids a pre-1.0 dependency and its binary size. Switch to iroh-blobs if it reaches 1.0 before phase 7. Blossom stays the offline path.

### C6 — Binary size

**Measured** by inspecting the published `computer.iroh:iroh-android:1.1.0` AAR (a normal file download, [Maven](https://repo1.maven.org/maven2/computer/iroh/iroh-android/1.1.0/)). The build is iroh 1.1 + iroh-services + tickets + UniFFI scaffolding, `lto = true`:

| ABI | `libiroh_ffi.so` uncompressed | deflated in AAR | stripped? |
|---|---|---|---|
| arm64-v8a | **14.00 MB** (`.text` 10.2 MB) | 6.02 MB | yes (no `.symtab`, no `.debug_*`) |
| armeabi-v7a | 9.76 MB | 5.56 MB | yes |
| x86_64 | 15.58 MB | 6.51 MB | — |
| x86 | 15.39 MB | 6.46 MB | — |

Desktop, from the `computer.iroh:iroh:1.1.0` jar: Windows x64 DLL 16.24 MB, Linux x64 19.76 MB, Linux arm64 19.21 MB. PyPI wheels are 6.3–7.5 MB ([PyPI](https://pypi.org/project/iroh/)). I found no n0 statement on binary size ([smaller-is-better](https://www.iroh.computer/blog/smaller-is-better) is about API scope). n0 does say ESP32 builds fit 4 MiB flash with `opt-level="z"` and a "reduced-dependency build" ([compatibility](https://docs.iroh.computer/compatibility)), so aggressive tuning can shrink it a lot.

**Implications:**
- The §10 estimate is "Rust core ~8–15 MB per architecture" for iroh + Loro + rust-nostr + crypto. iroh alone, default-tuned, is ~14 MB on arm64.
- Native libraries are stored **uncompressed** in the APK when `extractNativeLibs` is false (`useLegacyPackaging=false`), the modern default. Installed size therefore counts the full `.so` ([manifest docs](https://developer.android.com/guide/topics/manifest/application-element#extractNativeLibs)).
- Download size stays fine, because Play compresses the `.so` about 2.3×. **Installed ≤ 40 MB is at risk.**
- **Verdict: partial, probably exceeded without tuning.**

**Fix:** run E6 in phase 1 as planned, but compare these configurations:
- (a) `iroh` with `default-features=false, features=["tls-ring"]`, no metrics, no portmapper on Android;
- (b) `opt-level="z"` vs `"s"` vs 3;
- (c) `lto="fat"`, `codegen-units=1`, `strip=true`;
- (d) `panic="unwind"` (needed by iroh's JNI catch; see C4.1) vs `"abort"`;
- (e) `cargo bloat --crates` to find the big dependencies (rustls/ring, hickory, reqwest, noq).

Set the §10 budget from those numbers.

### C7 — Browser/WASM (web guest client)

- **Confirmed:**
  - "**All connections from browsers to somewhere else need to flow via a relay server**... browsers don't support sending UDP packets."
  - Build with `iroh = { version = "1", default-features = false }`. There is no npm package ([WASM docs](https://docs.iroh.computer/languages/wasm-browser)).
  - In the browser the N0 preset resolves through HTTPS pkarr only ([presets.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint/presets.rs)).
  - iroh-gossip builds for browsers from 0.33.
- Size: the official browser-echo example ships `browser_echo_bg.wasm` at **2,608,331 bytes raw, 1,105,576 bytes gzipped** (HTTP HEAD on [the example](https://n0-computer.github.io/iroh-examples/main/browser-echo/)).
- Caveats:
  - A web guest would depend entirely on relays, and the free ones are "development and hobby use only" (C3).
  - n0's FAQ says "If you need browser support, WebRTC remains the default choice; iroh doesn't run in the browser", which contradicts its own WASM page ([FAQ](https://docs.iroh.computer/about/faq)).
  - Browser WASM gets no mDNS, DHT, portmapper or hole punching.
- **Fix:** make **Nostr (WebSocket) the web guest's transport** and add iroh-in-browser only if a relay with a suitable policy exists. Budget ~1 MB gzipped for iroh, plus Loro and crypto.

---

## New risks (not in DESIGN.md)

| # | Risk | Severity | Evidence | Fix |
|---|---|---|---|---|
| NR1 | **One provider for three roles.** Hole punching (QAD + coordination), relaying and default discovery all run on n0 infrastructure. §2 principle 2 is violated. The "public STUN" backup doesn't exist in iroh 1.x. | High | C2, C3 | See C3 fixes (permission, configurable list, member-hosted relays, honest degradation). |
| NR2 | **The stable device key is also the iroh endpoint ID, which links groups and allows tracking.** The same ID is seen by n0 relays ("they theoretically know that Endpoint ID X talks to Endpoint ID Y", [FAQ](https://docs.iroh.computer/about/faq)), n0's DNS server (republish every 5 min), the user's DNS resolver (`_iroh.<id>.dns.iroh.link` lookups, or 8.8.8.8 on fallback) and every device on the same Wi-Fi (mDNS `<id>._irohv1._udp.local`). This undoes §7's per-device, per-group Nostr key separation and makes phones trackable across public Wi-Fi. | Medium | C2, [mDNS source](https://github.com/n0-computer/iroh-address-lookups), [DNS docs](https://docs.iroh.computer/connecting/dns-address-lookup) | Separate the **transport key** from the **device identity key**. Rotate the transport key (for example weekly), certify it with the identity key, and publish it in the group doc and on Nostr. Use mDNS passive mode (`advertise(false)`) on networks the user hasn't marked as home. Add n0 DNS, system DNS resolvers and LAN observers to the §9 table. |
| NR3 | **Key reuse across protocols.** Using the Ed25519 device identity key directly as the iroh TLS key means the same key signs app-level data and TLS handshakes. That is cross-protocol reuse, which is generally discouraged. | Low–Medium | §5 "Its public key doubles as the iroh endpoint ID" | Derive or certify a separate iroh key (same fix as NR2). |
| NR4 | **Duplicate endpoint IDs after restore or migration.** iroh: "When using a persisted key make sure no other endpoint is using the same key at the same time" ([creating an endpoint](https://docs.iroh.computer/connecting/creating-endpoint)). Android device-to-device transfer or Auto Backup could clone the key file onto a new phone while the old one still runs. | Medium | iroh docs, §11 | Exclude the transport key from both `cloud-backup` and `device-transfer` in `data_extraction_rules.xml`, and regenerate it on restore. The device identity still comes from the personal group. |
| NR5 | **Android 17 Local Network Protection** blocks mDNS, LAN direct paths and gateway port mapping without a runtime "Nearby devices" permission once targetSdk ≥ 37. | Medium (lands ~2027) | [LNP docs](https://developer.android.com/privacy-and-security/local-network-permission) | Contextual permission request, graceful fallback, test with the Android 16 opt-in now. Also relevant to R4. |
| NR6 | **Silent Google-DNS fallback on Android** when the JNI context is missing or the DNS read fails. That leaks lookups to Google and gives wrong answers on VPN or split-horizon networks. `panic="abort"` turns it into a crash. | Medium | [#4502](https://github.com/n0-computer/iroh/issues/4502), [#4371](https://github.com/n0-computer/iroh/pull/4371) | Initialize the JNI context, pass explicit nameservers from Kotlin, disable the fallback (configurable since 1.2.0), keep `panic="unwind"`. |
| NR7 | **Battery and data cost of a bound endpoint** (5 s path pings, 20–26 s QAD, 5 min republish). Holding it in the background also needs a foreground service, which §8 rules out for v1. | Medium | [socket.rs](https://github.com/n0-computer/iroh/blob/main/iroh/src/socket.rs), [Kotlin docs](https://docs.iroh.computer/languages/kotlin) | Bind only in the foreground or inside jobs. In background jobs, try iroh only with peers marked always-on. Measure (E3d). |
| NR8 | **Upgrade treadmill and security patches.** Minors get 3 months of full support and public relays support only the latest release. 1.0.2 and 1.1.0 already fixed security bugs, one of them client-side (CPU pinning by a malicious relay packet). | Medium | [release policy](https://docs.iroh.computer/about/release-policy), [1.1.0](https://www.iroh.computer/blog/iroh-1-1-0) | Quarterly iroh bump in §12a, dependency-audit CI (cargo-audit/deny), and a forced-update or "update available" notice for stale builds. |
| NR9 | **Pre-1.0 satellite crates.** mDNS 0.5, DHT 0.5, portmapper and netwatch 0.19, and iroh-blobs 0.103 can break on every release, outside the 1.0 promise. | Low–Medium | crates.io | Pin exact versions. Keep engine-side adapters thin. |
| NR10 | **The Windows firewall prompt may break the "seamless" desktop experience.** iroh binds UDP listening sockets and probes the gateway. Windows Defender Firewall usually prompts on the first listening socket, and iroh's docs mention gateway probing that "triggers firewall prompts". If the user clicks Cancel, inbound LAN connections break. | Low–Medium (unverified) | [endpoint.rs portmapper doc](https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint.rs) | Have the installer add a firewall rule. Verify in E-windows. |
| NR11 | **Mainline DHT on phones may look like BitTorrent** to OS, network or enterprise filters (n0's own reason for keeping it off by default). Records must be republished hourly, which sleeping phones can't do. | Low | [FAQ](https://docs.iroh.computer/about/faq) | Use the DHT on desktops only, or drop it in favor of IDs in invites and the group doc plus Nostr. |
| NR12 | **Relay rate limits under CGNAT.** Many users behind one carrier IP share limits "tuned as needed", and n0 can "block offending IP addresses". Kleiders' traffic is tiny, but the rules aren't published. | Low | [rate limiting](https://docs.iroh.computer/relays/rate-limiting) | Relay health tracking and multiple relays. Handle the `RateLimited` status (1.1+). |
| NR13 | **Memory growth in long-running endpoints** (desktop tray app, future helpers). #4549 reports ~6 GB in 30 min from path accumulation on 1.1/1.2, closed as not planned. #4548 (unbounded `pending_open_paths`) is related. | Low–Medium (desktop/helpers) | [#4549](https://github.com/n0-computer/iroh/issues/4549) | 24 h soak test of the desktop engine with churned connections (E3f). Watchdog restart of the endpoint. |

---

## Suggested experiments (all local; no public relays, DNS or DHT)

- **E3a — Local stack baseline.**
  - Setup: two endpoints on iroh 1.2, `presets::Minimal` plus a **local `iroh-relay`** (rate limit off) and a **local `iroh-dns-server`**.
  - Measure: time to first byte, path events (relay → direct), reconnect after the relay restarts.
  - Confirms the engine can run entirely on a configurable relay/DNS list.
- **E3b — NAT matrix.** Use iroh's own `patchbay` netsim (Linux dev-dependency) or network namespaces with iptables NAT to emulate EIM/EDM mappings, CGNAT and IPv6 with a stateful firewall. Measure the direct-path success rate for easy↔easy, hard↔easy, easy↔hard, hard↔hard and v6-firewalled pairs. This replaces the vendor "~90%" with our own matrix for §6.
- **E3c — Android behavior** (headless emulator, plus one physical phone for radio behavior):
  - (1) bind without and with the JNI context; check for the `read system DNS via Android JNI` log and that no 8.8.8.8 traffic appears (local pcap);
  - (2) toggle Wi-Fi/cellular with and without calling `network_change()`, and measure recovery time;
  - (3) mDNS discovery with and without `MulticastLock`;
  - (4) `adb shell am compat enable RESTRICT_LOCAL_NETWORK` to preview Android 17 LNP effects on mDNS, LAN paths and portmapper;
  - (5) `/system/bin/ip route` availability to the app.
- **E3d — Battery and cold start.** Use `dumpsys batterystats` or Battery Historian to compare:
  - (a) an idle bound endpoint for 30 min with 0, 1 and 3 connections (local relay);
  - (b) a bind → sync → close cycle every 15 min.
  - Record radio wakeups, bytes and cold-start latency, which decide the §8 background strategy.
- **E3e — Member-hosted relay feasibility.** Embed `iroh-relay` (server feature) in a desktop build with a self-signed certificate. Trust it through iroh's custom TLS roots and gate it with an `AccessControl` that admits only group endpoint IDs. Check whether a phone endpoint can use it as home relay and for hole-punch coordination.
- **E3f — Soak test.** Run a desktop endpoint for 24 h with synthetic connection churn against the local relay. Track RSS (#4549) and file descriptors.
- **E6 — Size** (planned). Build a cdylib for `aarch64-linux-android` in these configurations: iroh-only minimal; plus portmapper and metrics; plus mDNS and DHT crates; plus iroh-blobs; plus Loro and rust-nostr. Use opt-level z/s/3 and fat LTO, and compare with and without `panic=abort`. Check `llvm-readelf -l` for 16 KB alignment and run `cargo bloat --crates`. Compare against the 14.0 MB iroh-ffi reference.
- **E-wasm.** Build the engine's transport subset for `wasm32-unknown-unknown` and measure the gzipped size alongside Loro. Decide between iroh and Nostr for the web guest.
- **E-privacy.** Run a local pcap and DNS log with the N0 preset and with custom settings. List every host contacted, what the mDNS advertisement shows, and whether Android Private DNS is bypassed.
- **E-windows.** Fresh Windows 11 VM: does binding the endpoint trigger a Defender Firewall prompt? What breaks if the user cancels it (LAN direct, punching)?
- **E-group-discovery.** On a **local** `n0-mainline` testnet, prototype a pkarr/BEP44 record under a group-derived key: size limits, salt per device, overwrite and seq behavior, republish needs. Compare with endpoint IDs carried in invites and the group doc.
- **E-blobs.** iroh-blobs 0.103 with FsStore on Android against a minimal custom ALPN plus BLAKE3 check. Transfer 300 KB–5 MB images with a network switch in the middle, and record the size delta and code complexity.

---

## Sources

**iroh official (docs, blog, code)**
- iroh 1.0 announcement (support table, 0.35 → 2026-12-31): https://www.iroh.computer/blog/v1
- Language support (Kotlin, 95–99% direct data rate, mDNS not in FFI): https://www.iroh.computer/blog/iroh-language-support
- FFI pause (Feb 2025): https://www.iroh.computer/blog/ffi-updates
- 1.0.0-rc.0 (mDNS and DHT moved out): https://www.iroh.computer/blog/iroh-1-0-0-rc-0
- 1.0.0-rc.1 (hard-NAT asymmetric punching, AccessControl): https://www.iroh.computer/blog/iroh-1-0-0-rc-1
- 1.0.2 relay security fix: https://www.iroh.computer/blog/iroh-1-0-2
- 1.0.3 (HTTPS pkarr resolver in N0 preset): https://www.iroh.computer/blog/iroh-1-0-3
- 1.1.0 security fixes: https://www.iroh.computer/blog/iroh-1-1-0
- Shared relays (pricing, stricter rate limits, ~5 s pings): https://www.iroh.computer/blog/shared-relays
- Authenticated relays: https://www.iroh.computer/blog/authenticated-relays
- Relay outage post-mortem (2024-11-05): https://www.iroh.computer/blog/relay-down-a-post-mortem
- 0.90 canary series (STUN removed): https://www.iroh.computer/blog/iroh-0-90-the-canary-series
- 0.91 last relay break: https://www.iroh.computer/blog/iroh-0-91-0-the-last-relay-break
- 0.98 NAT traversal regressions and fixes: https://www.iroh.computer/blog/iroh-0-98-0-getting-back-to-traversing-nats
- QAD: https://www.iroh.computer/blog/qad
- 0.92 mDNS passive mode: https://www.iroh.computer/blog/iroh-0-92-0-mdns-improvements
- iroh-blobs 0.95 features: https://www.iroh.computer/blog/iroh-blobs-0-95-new-features
- Healing connections: https://www.iroh.computer/blog/healing-connections
- Public relays policy: https://docs.iroh.computer/iroh-services/relays/public
- Rate limiting: https://docs.iroh.computer/relays/rate-limiting
- Relays concept: https://docs.iroh.computer/concepts/relays
- Shared relays: https://docs.iroh.computer/iroh-services/relays/shared
- Self-hosted relay: https://docs.iroh.computer/iroh-services/relays/self-hosted
- Release and support policy: https://docs.iroh.computer/about/release-policy
- FAQ: https://docs.iroh.computer/about/faq
- Address lookup: https://docs.iroh.computer/concepts/address-lookup
- DNS lookup: https://docs.iroh.computer/connecting/dns-address-lookup
- DHT lookup: https://docs.iroh.computer/connecting/dht-address-lookup
- mDNS lookup: https://docs.iroh.computer/connecting/local-address-lookup
- NAT traversal: https://docs.iroh.computer/concepts/nat-traversal
- Enterprise networks (ports, UDP 7842 = STUN role): https://docs.iroh.computer/configuring-networks
- Creating an endpoint (unique key warning): https://docs.iroh.computer/connecting/creating-endpoint
- Languages / platform matrix: https://docs.iroh.computer/languages/index
- Kotlin (foreground/background guidance): https://docs.iroh.computer/languages/kotlin
- WASM/browser: https://docs.iroh.computer/languages/wasm-browser
- Compatibility (ESP32 size notes): https://docs.iroh.computer/compatibility
- Blobs: https://docs.iroh.computer/protocols/blobs
- CHANGELOG: https://github.com/n0-computer/iroh/blob/main/CHANGELOG.md
- iroh Cargo.toml (default features): https://github.com/n0-computer/iroh/blob/main/iroh/Cargo.toml
- Default relays: https://github.com/n0-computer/iroh/blob/main/iroh/src/defaults.rs
- Endpoint (portmapper default, Android usage, network_change): https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint.rs
- Presets: https://github.com/n0-computer/iroh/blob/main/iroh/src/endpoint/presets.rs
- Timers: https://github.com/n0-computer/iroh/blob/main/iroh/src/socket.rs · https://github.com/n0-computer/iroh/blob/main/iroh/src/net_report.rs · https://github.com/n0-computer/iroh/blob/main/iroh/src/address_lookup/pkarr.rs
- netwatch Android monitor stub: https://github.com/n0-computer/net-tools/blob/main/netwatch/src/netmon/android.rs
- netwatch poll interval: https://github.com/n0-computer/net-tools/blob/main/netwatch/src/netmon/actor.rs
- netwatch default route on Android: https://github.com/n0-computer/net-tools/blob/main/netwatch/src/interfaces/linux.rs
- Address-lookup crates (mDNS, DHT): https://github.com/n0-computer/iroh-address-lookups
- iroh-blobs README ("not yet considered production quality"): https://github.com/n0-computer/iroh-blobs
- iroh-ffi: https://github.com/n0-computer/iroh-ffi
- hello-iroh-ffi Android demo: https://github.com/n0-computer/hello-iroh-ffi/tree/main/kotlin-android
- Browser echo example (wasm size): https://n0-computer.github.io/iroh-examples/main/browser-echo/

**Issues and PRs**
- iroh #4502 (Android DNS → Google fallback): https://github.com/n0-computer/iroh/issues/4502
- iroh #4371 (fallback when no JNI context): https://github.com/n0-computer/iroh/pull/4371
- iroh #4183 (Android docs and emulator CI): https://github.com/n0-computer/iroh/pull/4183
- iroh #4249 (APK JNI smoke test): https://github.com/n0-computer/iroh/pull/4249
- iroh #4210 (link-local DNS when tethered): https://github.com/n0-computer/iroh/pull/4210
- iroh #4549 (memory growth): https://github.com/n0-computer/iroh/issues/4549
- iroh #2324 (netlink permission denied on Android): https://github.com/n0-computer/iroh/issues/2324
- iroh #3961 (netmon unsupported fallback): https://github.com/n0-computer/iroh/pull/3961
- iroh-ffi #246 (Android libs missing from Maven): https://github.com/n0-computer/iroh-ffi/pull/246
- iroh-ffi #279 / #285 (16 KB alignment): https://github.com/n0-computer/iroh-ffi/issues/279 · https://github.com/n0-computer/iroh-ffi/issues/285
- iroh-ffi #272 (KMP): https://github.com/n0-computer/iroh-ffi/issues/272
- iroh-ffi #237 (Android JNI init): https://github.com/n0-computer/iroh-ffi/pull/237

**Registries (metadata and artifact inspection)**
- Maven iroh-android 1.1.0: https://repo1.maven.org/maven2/computer/iroh/iroh-android/1.1.0/
- Maven iroh 1.1.0: https://repo1.maven.org/maven2/computer/iroh/iroh/1.1.0/
- PyPI iroh: https://pypi.org/project/iroh/
- crates.io: https://crates.io/crates/iroh · https://crates.io/crates/iroh-blobs · https://crates.io/crates/iroh-mdns-address-lookup · https://crates.io/crates/iroh-mainline-address-lookup

**Third-party**
- NAT traversal measurement (70% ± 7.1%): https://arxiv.org/abs/2510.27500
- Android local network permission: https://developer.android.com/privacy-and-security/local-network-permission
- Android MulticastLock: https://developer.android.com/reference/android/net/wifi/WifiManager.MulticastLock
- Android Doze: https://developer.android.com/training/monitoring-device-state/doze-standby
- Android 16 KB page sizes: https://developer.android.com/guide/practices/page-sizes
- Android extractNativeLibs: https://developer.android.com/guide/topics/manifest/application-element#extractNativeLibs
- libp2p circuit relay v2 limits: https://github.com/libp2p/go-libp2p/blob/master/p2p/protocol/circuitv2/relay/resources.go · https://github.com/libp2p/specs/blob/master/relay/circuit-v2.md
- Delta Chat iroh 0.35 dependency: https://support.delta.chat/t/reducing-the-number-of-dependencies-for-packaging-iroh-openssl/4481
- Delta Chat realtime / iroh: https://delta.chat/en/2024-11-20-webxdc-realtime
- chatmail default relays issue: https://github.com/chatmail/core/issues/5591
- Lobsters 1.0 discussion: https://lobste.rs/s/cslljn/iroh_1_0_dial_keys_not_ips
- HN 1.0 discussion: https://news.ycombinator.com/item?id=48542480

---

## Independent verification

Verifier pass, 2026-09-24. I tried to refute the report's key conclusions using different evidence from the researcher's where possible:
- the **released crate sources** from static.crates.io, not GitHub `main`: iroh 1.2.0, iroh-relay 1.2.0, iroh-dns 1.3.0, n0-dns-resolver 0.1.0, iroh-mdns-address-lookup 0.5.0, iroh-mainline-address-lookup 0.5.0, iroh-blobs 0.103.0 and netwatch 0.19.3;
- crates.io and Maven metadata APIs, and the GitHub REST API (iroh-ffi releases and issues, iroh issues);
- my own download and ELF parse of `computer.iroh:iroh-android:1.1.0`;
- `chatmail/core` `Cargo.toml` and the chatmail relay config;
- RFC 6888, and the Markdown sources of the iroh FAQ, Kotlin, WASM and release-policy docs.

I sent no traffic to any relay, DNS server or DHT.

### Verdicts

| Claim | Report verdict | Verifier verdict | Independent evidence |
|---|---|---|---|
| iroh 1.x stable; Kotlin via UniFFI; iroh-ffi deprecated? | Partial | **Upheld (partial)** | crates.io: iroh 1.0.0 was published 2026-06-15 and 1.2.0 on 2026-09-09. The iroh-ffi README says the bindings "mirror the stabilized iroh 1.0 surface" and that blobs, docs and gossip are "out of scope". It is not deprecated. Maven's `iroh-android` metadata lists only 1.1.0 (2026-07-17), and the GitHub releases stop at v1.1.0. #285 is still open. A maintainer wrote "will try and get a release out this week" on 2026-08-17, and nothing had shipped by 2026-09-24. The release policy `.md` confirms 3 months of full support for minor versions. |
| mDNS LAN discovery | Partial | **Upheld (partial)** | iroh-mdns-address-lookup 0.5.0 (2026-08-18) is a separate crate. `advertise` defaults to `true`, the service is `_irohv1._udp.local`, and `advertise(false)` gives a passive mode. The crate has no Android or MulticastLock code. Android's LNP page confirms that targetSdk 37 on Android 17 blocks UDP multicast and mDNS without `ACCESS_LOCAL_NETWORK` (NEARBY_DEVICES), and that NsdManager is only exempt through `FLAG_SHOW_PICKER`. **Addition:** by default the mDNS record carries *all* addresses, including QAD-learned public IPs and the relay URL, so LAN observers learn more than the endpoint ID. |
| Direct IPv6 | Partial | **Upheld (partial)** | In 1.2.0, `PkarrPublisherBuilder` defaults to `AddrFilter::relay_only()`. The FAQ says iroh "binds two UDP ports, one for IPv4 and one for IPv6". **Addition:** the `N0DisableRelay` preset's doc comment says it "publishes IP addresses rather than relay urls", but its code is `N0.apply(builder).relay_mode(RelayMode::Disabled)`, so the publisher keeps `relay_only` and publishes an empty address set. A relay-less fallback mode has to set `AddrFilter::ip_only()` or `unfiltered()` explicitly. |
| Hole punching via "iroh + public STUN", ~90% | Refuted (the table says "Partial / Refuted") | **Relabel: partial.** The mechanism is confirmed, "public STUN" is refuted and "~90%" is unverified. Severity stays **high**. | iroh 1.2.0 and iroh-relay 1.2.0 contain no STUN client or server, only legacy `re_stun` names that now trigger QAD and net-report runs. The FAQ confirms relays are "the introduction, not just the fallback... how two devices first find each other and coordinate the simultaneous connection attempt". The source has no port prediction. The only hard-NAT candidate is `Qad4LocalPort` (QAD IP plus the local fixed port, which assumes a manual port forward). So hard↔hard is unsupported. **Corrections:** (1) iroh *does* do the hole punching; only the provider column and the independence claim are wrong, so "refuted" overstates the case. (2) The report equates CGNAT with hard NAT. RFC 6888 REQ-1 requires CGNs to meet RFC 4787's UDP requirements, which include Endpoint-Independent Mapping, so a conforming CGNAT can be punched. The risk comes from *non-conforming* carrier NATs. (3) The 70% ± 7.1% baseline comes from IPFS/libp2p DCUtR nodes over TCP and QUIC, mostly desktops and servers, so it is no better a mobile baseline than n0's 90%. The "plan for 60–80%" figure is a guess. Only E3b plus field telemetry can settle it. |
| Discovery via pkarr on Mainline DHT under a group-derived key | Refuted | **Upheld (refuted as stated)** | In 1.2.0 `presets::N0` = `PkarrPublisher::n0_dns()` + `PkarrResolver::n0_dns()` + `DnsAddressLookup::n0_dns()` (outside browsers), all pointing at n0's `iroh.link`. The DHT crate 0.5.0 uses `AddrFilter::relay_only()` and `REPUBLISH_DELAY` = 1 h, and publishes under the endpoint key. I confirmed the FAQ's BitTorrent quote in its source. **The fix is incomplete** (see Missed item 1). |
| n0 public relays usable free in production; ≥2 providers | Refuted | **Upheld** | The FAQ source says the relays "are free to use for development and testing... For production workloads, use Shared Relays, a dedicated relay, or self-host your own". The pricing page says "No uptime guarantees" and gives a "Variable" rate limit for the free tier. The rate-limiting page: "Do not rely on it for production workloads". **Delta Chat is still on iroh 0.35:** `chatmail/core` main `Cargo.toml` has `iroh = "0.35"`, and the latest release is v2.62.0 (2026-09-22). The chatmail relay config is an iroh-relay 0.35 config. I found no independent 1.x provider. **Refinements to the fix:** the member-hosted relay API exists. iroh-relay 1.2 has an `AccessControl` trait and an `Allowlist(Vec<EndpointId>)` config, and iroh has `Builder::ca_tls_config` for custom roots. But the relay needs an inbound, publicly reachable TCP/HTTPS port, plus UDP 7842 if it also serves QAD. Behind ISP IPv4 CGNAT, UPnP maps only the home gateway, so many desktops won't qualify, and the idea overlaps with layer 6. |
| iroh on Android | Partial | **Upheld (partial), with two corrections** | Confirmed in source: netwatch 0.19.3 `android.rs` ("Very sad monitor"); `Endpoint::network_change` docs; `HEARTBEAT_INTERVAL` 5 s; re-probe timer random 20–26 s; `FULL_REPORT_INTERVAL` 5 min; pkarr republish 5 min; the Kotlin doc's background `shutdown()` guidance. **Corrections:** (1) In 1.2 (iroh-dns 1.3 → n0-dns-resolver 0.1) the fallback tier is **Cloudflare, Google and Quad9**, not only 8.8.8.8, and the default mode is `Deferred`. `disable_fallback()` / `FallbackMode::Never` exist. (2) The report's "Private DNS bypass (plausible, unverified)" is **confirmed at source level**. The Android reader calls `LinkProperties.getDnsServers()` and builds `DnsProtocol::Udp` nameservers, with no Private DNS lookup. iroh-dns supports `Tls`/`Https` nameservers, so the engine can configure DoT or DoH itself. #4502's reporter says reads failed even *with* the context installed, because iroh's runtime threads weren't JVM-attached. The maintainer attributed it to a missing context init. Either way, the report's fix (pass explicit nameservers from Kotlin) is the right one. |
| iroh-blobs for images | Partial | **Upheld** | The 0.103.0 crate README opens with "not yet considered production quality... use iroh-blobs 0.35". The crate depends on `iroh = "1.0.0"` with default features off. crates.io shows 0.98–0.103 between 2026-01-28 and 2026-06-15 and nothing since. |
| Binary size | Partial | **Upheld (partial)** | My own parse of the AAR: arm64-v8a 13,996,024 B (6,022,339 compressed, `.text` 10,215,448, no `.symtab`), armeabi-v7a 9,763,396, x86_64 15,580,712, x86 15,392,024. **Every ABI has LOAD align 0x1000**, which confirms the 16 KB failure. **Nuance:** iroh-ffi's release profile is only `lto = true` (opt-level 3, default codegen-units). Its build also includes iroh-services, iroh-tickets, iroh-metrics, default features (metrics, portmapper) and a large UniFFI surface. So 14 MB is an untuned upper reference for "iroh alone", not a floor. The budget is at risk, not shown to be exceeded. Play splits by ABI, so only one `.so` is installed. |
| Portmapper (spot check) | Confirmed | **Upheld** | 1.2.0 `Cargo.toml` default features = metrics, fast-apple-datapath, portmapper and tls-ring. The `endpoint.rs` doc says "Defaults to `PortmapperConfig::Enabled`". |
| 0.35 relays until 2026-12-31 (spot check) | Confirmed | **Upheld** | The v1 post's table: "v0.35x Dec 31, 2026 · v0.9x & v1.0.0-rcX Sept 30, 2026 · v1.0 until End of Life". The release-policy source: "Number 0 runs public relays for the **latest major version**". |
| Browser/WASM relay-only (spot check) | Confirmed | **Upheld** | The WASM doc source: "All connections from browsers to somewhere else need to flow via a relay server", with `default-features = false` and no npm package. `browser_echo_bg.wasm` HEAD gives Content-Length 2,608,331. The FAQ contradicts itself in its own source: line 242 says "Both WebRTC and iroh work in browsers" and line 246 says "iroh doesn't run in the browser". |

### What the report missed or got slightly wrong

1. **An endpoint ID alone can't be dialed without address lookup.** The fix for C2e ("put member endpoint IDs in invites and the group doc, then dial them directly") still needs something that maps the ID to the peer's *current* home-relay URL and addresses. With the N0 preset, that is n0's DNS. To actually remove the n0 DNS dependency and the `_iroh.<id>.dns.iroh.link` lookup leak (NR2), the engine should implement iroh's public `AddressLookup` trait, or feed `MemoryLookup`, from E2E-encrypted, per-group Nostr events and the group doc. Each device publishes its `EndpointAddr` there (home relay URL plus optional IPs) and refreshes it when the home relay changes. This also takes the place of group-keyed DHT discovery.
2. **`N0DisableRelay` doc/code mismatch.** With relays disabled, the publisher still uses `relay_only`, so the record is empty. Any relay-less or LAN-only mode must set the address filter explicitly.
3. **mDNS leaks more than the ID.** The default filter publishes public (QAD) addresses and the relay URL to the LAN. Use `advertise(false)` or an `ip_only` filter limited to private and link-local addresses on untrusted networks.
4. **The Private DNS bypass is real (upgrade NR-list item C4.7 from "unverified").** Fix: set DoH/DoT nameservers in the iroh-dns builder, or read `LinkProperties.getPrivateDnsServerName()` from Kotlin, and disable the public fallback tier (Cloudflare, Google, Quad9).
5. **Don't equate CGNAT with hard NAT.** RFC 6888 requires CGNs to follow RFC 4787, which mandates EIM. Treat the direct-rate figure as unknown until E3b and in-app telemetry (direct vs relayed, per network type) measure it.
6. **The member-hosted relay depends on reachability.** A desktop behind ISP CGNAT can't host a relay that phones can reach. Add a reachability test to E3e, and consider that layer 6 (engine-level forwarding over existing iroh connections) may cover most of the same need without any relay server.
7. **Minor:** the relay `RateLimited` notice goes only to endpoints "at iroh version 1.0.4 or higher". No 1.0.4 was ever published (1.0.3 was followed by 1.1.0), so in practice this means ≥ 1.1.0.
8. **The iroh-ffi release cadence is itself a signal.** The 16 KB fix has been committed since July, and a maintainer promised a release on 2026-08-17 that still hasn't shipped five weeks later. That supports the report's advice not to depend on the published bindings.
