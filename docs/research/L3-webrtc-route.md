# L3: The WebRTC route (public STUN, free TURN, Rust WebRTC, Nostr signaling)

*2026-09-24. Research only. No code was built, nothing was installed, and no packets were sent to any STUN, TURN, Nostr or iroh server. The only network use was documentation and API fetches (web pages, GitHub API, crates.io API) and DNS lookups of the STUN/TURN hostnames. DESIGN.md and DECISIONS.md are unchanged. Note: the task mentioned a "D46 discussion", but DECISIONS.md ends at D45 and has no D46.*

Gaps this layer was asked to cover: **G2** internet P2P connectivity without n0 (address discovery plus hole-punch coordination) and **G4** free relaying when hole punching fails (~30% of attempts). Where a candidate also touches G1, G3, G5 or G6, that is noted.

---

## 1. Summary

1. **Public STUN solves address discovery (half of G2) at no cost, with many independent operators.** Cloudflare says its STUN is "free and unlimited" and Twilio lists STUN at "no cost". Google's `stun.l.google.com:19302` has no published terms but is the default in almost every WebRTC library and app. An hourly-checked public list counted **264 working host:port entries (182 IPv4, 21 IPv6) out of 635 candidates** on 2026-09-24. Syncthing, Amethyst (Android) and Trystero ship with public STUN hard-coded. **Verdict: ADOPT as an address-discovery provider, high confidence.** It only helps if the engine sends the STUN request **from the same socket that carries the traffic**, and iroh 1.2 has no STUN client and doesn't hand out its socket.
2. **The other half of G2, hole-punch coordination, can run over Nostr.** Precedents: Trystero (a JS library whose *default* signaling is Nostr, using ephemeral kinds), Amethyst, 0xChat, Noscall and nospeak (WebRTC calls signaled over Nostr), and two open NIP drafts (NIP-100, NIP-AC). Two signed, encrypted **ephemeral** events per connection attempt (offer and answer, not trickle) fit inside the D11 relay budget, and relays don't store them. **Verdict: ADOPT Nostr as the signaling/rendezvous channel (also covers G5), medium-high confidence.**
3. **No TURN relay fits the constraints, so G4 is not solved by this route.**
   - Cloudflare TURN requires a **server-side** key: "You should keep your TURN key on the server side (don't share it with the browser/app)." That means developer infrastructure, and billing goes to the developer's account after 1,000 GB. **REJECT.**
   - ExpressTURN's free plan gives static per-account credentials, which would have to be embedded in the app for all users. **REJECT.**
   - Twilio, Xirsys and Metered's paid plans cost money. **REJECT.**
   - The only embeddable option is **Metered's "Open Relay"**. It publishes a shared static-auth secret meant for public use, and Amethyst embeds older Open Relay credentials. Its own terms say "THE TURN SERVER CAN AND WILL GO DOWN WITHOUT NOTICE", and it gives 20 GB/month with unclear scope. **CONSIDER only as an opportunistic, health-tracked last resort that never counts as a provider. Low confidence.**
   - **The ~30% of connections that can't be punched stay dependent on n0 relays, member relaying (layer 6) and the mailbox.**
4. **WebRTC doesn't raise the hole-punch success rate.** ICE uses the same UDP punching as iroh, on the same NATs. It helps only because it removes n0 from address discovery and coordination. Independent baselines: 82% of NATs were UDP-punchable (Ford et al. 2005, 380 data points), and 70% ± 7.1% of punches succeeded in libp2p's 4.4 M-attempt study. Tailscale estimates "over 90%" with its own STUN and relays. No independent WebRTC-specific figure was verified.
5. **Best-fitting architecture (CONSIDER, medium-low confidence):** don't add a second full WebRTC stack. Instead, add an **ICE-only path**:
   - str0m's standalone Sans-IO ICE agent crate **`is` 0.11** (MIT OR Apache-2.0, a ~70 KB crate);
   - plus public STUN from several operators;
   - plus Nostr signaling;
   - with the punched UDP path handed to iroh as an **iroh custom transport** (`unstable-custom-transports`).

   This is Firezone's production architecture (str0m's `is` ICE agent plus WireGuard, including on Android). It reuses iroh's QUIC/TLS identity and adds no DTLS or SCTP. The risk is that iroh's custom-transport API is explicitly unstable: a send-path bug was closed only on 2026-09-22.
6. **A full Rust WebRTC stack is viable as a second provider, but its main payoff is browser interop, not G2/G4.** webrtc-rs 0.21.0 (2026-09-19, MIT/Apache-2.0, includes a TURN client, Android bugs fixed in Aug 2026) is the only full Rust stack with a TURN client. Its real payoff would be a **browser/PWA client**, which is the only realistic path for iPhone invitees (D45), since iroh in the browser is relay-only. **CONSIDER, medium confidence.**
   - str0m (SFU-focused, no TURN, Android "compiled, not tested"): **REJECT as the full stack.**
   - libdatachannel/datachannel-rs (MPL-2.0, C++ toolchain): **REJECT.**
   - Google libwebrtc for Android (20.6 MB AAR, Kotlin only, not in the shared Rust engine): **REJECT.**
7. **"Just extend iroh" (add_external_addr + public STUN + Nostr) is weaker than it looks.** Code-level facts (iroh 1.2):
   - `add_external_addr` is advertised as a NAT-traversal candidate (`DirectAddrType::Config`);
   - hole punching needs **an existing connection** ("not holepunching: no connections"; maintainer: "**Without a relay server you will not be able to holepunch**");
   - a STUN result from a *different* socket is only right on port-preserving NATs, which is iroh's own `Qad4LocalPort` heuristic.

   Without a first path, this covers public IPv6, port-mapped desktops and port-preserving NATs, and would need an unverified "simultaneous QUIC connect" trick for the rest. **CONSIDER as a cheap partial layer, medium confidence**, and test it first (experiment X2).
8. **WebRTC doesn't help G1, G3 or G6.**
   - Every route here needs both peers online and foreground at the same moment.
   - On Android, sockets die ~5–10 s after backgrounding (D16; webrtc-rs issue #868 shows exactly this).
   - Android 17's local-network permission (D21) also gates ICE checks to LAN and CGNAT-range host candidates.

---

## 2. What STUN, ICE and TURN can and cannot do (the physics)

- **STUN** tells a device its public `ip:port` *for the socket it sent from*. Tailscale: "To discover your public `ip:port`, you have to send and receive STUN packets from the socket that you intend to use for communication." On NATs with endpoint-dependent mapping ("symmetric"/hard NATs), the port differs per destination, so the STUN result is wrong for the peer. That's true for every stack.
- **ICE** = gather candidates (host, server-reflexive via STUN, relayed via TURN), exchange them over a signaling channel, then run paired connectivity checks. The *signaling channel* is exactly the "hole-punch coordination" role that n0's relays play for iroh.
- **Hole punching success is a property of the NATs, not the library.**
  - UDP punching worked on 82% of NATs in 2005 ([Ford, Srisuresh, Kegel](https://bford.info/pub/net/p2pnat/): "Out of the 380 reported data points for UDP, in 310 cases (82%)...").
  - A 2025 libp2p study measured 70% ± 7.1% over 4.4 M attempts, "statistically indistinguishable" for TCP and QUIC ([arXiv 2510.27500](https://arxiv.org/abs/2510.27500)).
  - Tailscale's estimate: "I'd estimate you could get a direct connection over 90% of the time" ([Tailscale](https://tailscale.com/blog/how-nat-traversal-works)).
  - Hard↔hard NAT: "To hit a 99.9% chance of success, we need each side to send 170,000 probes... 28 minutes." None of the stacks evaluated here does port prediction.
  - I could not verify an independent, recent figure for the share of WebRTC sessions that need TURN. I'm not quoting the commonly repeated 10–20% because I couldn't trace it to a primary source.
- **Carrier NAT caveat that applies equally to WebRTC:** iroh issue [#4424](https://github.com/n0-computer/iroh/issues/4424) (iOS on T-Mobile CGNAT). Punching "succeeds" but "direct path drops back to relay every ~50–75s", consistent with an absolute UDP mapping timeout. Any ICE implementation needs periodic re-punching and a fallback path on such carriers.
- **So:** STUN + ICE + Nostr can replace n0 for *discovery and coordination*. They can't replace a *relay*. Only a TURN server (or any publicly reachable relay) serves the ~10–30% that can't be punched.

---

## 3. Candidate-by-candidate evaluation

### C1. Public STUN servers (Google, Cloudflare, Twilio, Nextcloud, FreeSWITCH, Linphone, SignalWire, many VoIP operators)

| Aspect | Evidence |
|---|---|
| Gaps | **G2 (address discovery only).** Useless without an ICE/rendezvous layer that uses the same socket. |
| Cost | $0 for users and developer; no account. Traffic is tiny (a binding request is ~20 bytes of payload plus headers). |
| Terms | **Cloudflare:** "Cloudflare's STUN service at `stun.cloudflare.com` is free and unlimited." ([TURN FAQ](https://developers.cloudflare.com/realtime/turn/faq/)); port 3478/udp ([overview](https://developers.cloudflare.com/realtime/turn/)). **Twilio:** pricing page lists STUN as "Globally available in all regions" at no cost ([pricing](https://www.twilio.com/en-us/stun-turn/pricing)); `global.stun.twilio.com:3478`. **Google** (`stun{,1..4}.l.google.com:19302`): **no published terms or SLA found.** It's the de facto default: webrtc-rs's README example, Trystero's defaults and Amethyst's defaults all use it. **Metered:** "Unlimited STUN usage included free" ([Metered](https://www.metered.ca/stun-turn)). VoIP operators' servers (Syncthing's fallback list) have no stated policy. |
| Operators | Many and independent. The [always-online-stun](https://github.com/pradt2/always-online-stun) list (MIT, checked hourly, last push 2026-09-24) had **264 valid host entries, 182 IPv4, 21 IPv6, 58 "NAT-testing" capable, out of 635 candidates**. Includes Google (5 entries), Nextcloud (`:3478` and `:443`), FreeSWITCH, Linphone, SignalWire, and an address in Meta's `2a03:2880` range. **Caveat:** dozens of `stun.voip*.com` names look like one VoIP reseller family, so count *operators*, not hostnames. `stun.syncthing.net` and `stun.services.mozilla.com` are candidates that currently **fail** the check, which shows churn. |
| Reliability | Cloudflare is anycast. Google has been stable for years (no documented number). The list's 264/635 ratio shows that ~60% of "known" public STUN hosts are dead at any time: ship several operators and health-track them. |
| Privacy | The STUN operator sees the device's public IP, port and timing, not who it talks to. STUN is **unencrypted**, so on-path observers see it (iroh cited this as one reason for QAD: "While in theory you can encrypt STUN requests using DTLS it's not something that is done much", [iroh QAD](https://www.iroh.computer/blog/qad)). Omit the `SOFTWARE` attribute to avoid fingerprinting. Rotate operators so no single one gets a presence log. |
| Android | Plain UDP, INTERNET permission only. Works while the process has network (foreground, expedited job). |
| Libraries | `stun_codec` 0.4.0 (MIT, 5.1 M downloads, used by Firezone), `stun-rs` 0.1.11 (MIT/Apache), `rtc-stun` 0.21 (MIT/Apache). `is` 0.11 includes STUN message handling. |
| Abuse | Binding requests have tiny amplification. Low risk to us. A hostile operator can return a wrong address, so use ≥ 2 operators and a majority vote. |
| Precedent | **Syncthing** falls back to nine third-party VoIP STUN servers (`DefaultFallbackStunServers` in [config.go](https://github.com/syncthing/syncthing/blob/main/lib/config/config.go)). **Amethyst** (Android, Play) ships Google + Cloudflare STUN. **Trystero** ships `stun.l.google.com` ×3 + `stun.cloudflare.com`. |
| Failure modes | Hard NAT (the result is wrong for the peer); UDP-blocked networks; operator removal. The result is only valid for the socket that queried. |
| **Verdict** | **ADOPT** (as the address-discovery provider for any ICE/rendezvous path). **Confidence: high** on availability and cost. Its value depends entirely on C8 or C9 existing. |

### C2. Cloudflare Realtime TURN (free 1,000 GB/month)

| Aspect | Evidence |
|---|---|
| Gaps | G4 (relaying). |
| Cost | "The first 1,000 GB each month is free. SFU and TURN share this allowance" then "$0.05 per GB of egress" ([pricing](https://developers.cloudflare.com/realtime/pricing/)). It sits on the **developer's** Cloudflare account, so overage and abuse bill the developer. |
| Terms / embedding | **Credentials must be minted by a backend:** "You should have a back-end service that uses your TURN Token ID and API token to generate credentials", and "You should keep your TURN key on the server side (don't share it with the browser/app)" ([generate credentials](https://developers.cloudflare.com/realtime/turn/generate-credentials/)). Credentials last ≤ 48 h ([FAQ](https://developers.cloudflare.com/realtime/turn/faq/)). |
| Operators | One (Cloudflare), anycast, global except China. |
| Limits | Per allocation: ">5 new IP/sec", ">5-10 kpps", ">50-100 Mbps"; "Hitting these limits will result in packet drops" ([overview](https://developers.cloudflare.com/realtime/turn/)). |
| Why it fails | Any compliant use needs a developer-run credential service (even a free Worker is developer-hosted infrastructure). Embedding the key violates Cloudflare's instructions and exposes the developer's account to unlimited billed abuse. |
| **Verdict** | **REJECT** (violates "no developer infrastructure" and "free at any scale"). **Confidence: high.** |

### C3. Metered "Open Relay" (free public TURN)

| Aspect | Evidence |
|---|---|
| Gaps | G4. |
| Cost | "The TURN Server provides 20 GB of free TURN Usage every month" ([Open Relay](https://www.metered.ca/tools/openrelay/)). It's unclear whether that's per account or pooled for the static-auth service. The main path now says "Signup for free account" to get an API key. Metered's separate TURN product has a 500 MB free plan ([Metered](https://www.metered.ca/stun-turn)). |
| Terms / embedding | It publishes a **shared static-auth secret for public use**: "use the static auth url which is staticauth.openrelay.metered.ca secret: openrelayprojectsecret" (meant for Nextcloud Talk / Matrix). A client can compute time-limited TURN REST credentials locally from it, so no backend is needed. **Terms:** "THE TURN SERVER CAN AND WILL GO DOWN WITHOUT NOTICE AND WE DO NOT PROVIDE ANY GUARANTEES OF ANY KIND" ([T&C](https://www.metered.ca/tools/openrelay/terms-and-conditions/)). The terms have no explicit permitted-use or prohibited-use clauses. |
| Operators | One company (Metered Video). Ports 80/443, TURNS. |
| Precedent | **Amethyst** hard-codes `turn:openrelay.metered.ca:80/443` with the older username/password `openrelayproject` as its default TURN ([IceServerConfig.kt](https://github.com/vitorpamplona/amethyst/blob/main/amethyst/src/main/java/com/vitorpamplona/amethyst/service/call/IceServerConfig.kt)). Those credentials are no longer on the current Open Relay page. **Not verified whether they still work** (testing needs a TURN allocation on a public server, which is outside this task's rules). |
| Privacy | The TURN operator sees both peers' IPs and traffic volume: a who-talks-to-whom log (worse than STUN). Payload stays end-to-end encrypted (QUIC/TLS or DTLS). |
| Abuse | A published secret means every app on earth shares the quota. Heavy users (video callers) will exhaust or trigger throttling. Our sync traffic is tiny, but we'd be collateral damage. |
| Library | TURN client: `rtc-turn` / `turn` 0.17 (MIT/Apache) from webrtc-rs; Firezone's snownet has its own (Apache-2.0, not published). To carry iroh traffic it must be wrapped as an iroh custom transport (unstable). |
| Failure modes | Service closure or quota exhaustion without notice; secret rotation (breaks every shipped app until the next release unless the secret comes from a signed runtime list). |
| **Verdict** | **CONSIDER, only as an opportunistic last resort:** off the critical path, health-tracked, delivered via a signed runtime config, and **never counted as one of the "≥ 2 providers"**. Explicitly permitted-looking, but the least dependable. **Confidence: low.** |

### C4. ExpressTURN free plan

| Aspect | Evidence |
|---|---|
| Gaps | G4. |
| Cost / terms | "Free plan... is limited to 1000 GB of traffic per month." Free accounts get a static username/password. Temporary (secret-key) credentials and "all server locations" need Premium ($9/mo) ([ExpressTURN](https://www.expressturn.com/)). Terms: not to be used "for any unauthorized, abusive, or illegal activities", and "The Company reserves the right to terminate your use of the Service at any time, with or without notice" ([terms](https://www.expressturn.com/terms)). |
| Why it fails | The only way to use it without a backend is to embed **one developer account's static credentials** in every copy of the app. They get extracted and abused, and the account gets banned for everyone. |
| **Verdict** | **REJECT.** **Confidence: medium-high.** |

### C5. Paid / account TURN (Twilio NTS, Xirsys, Metered paid, self-hosted coturn)
- Twilio TURN: $0.40–0.80/GB ([pricing](https://www.twilio.com/en-us/stun-turn/pricing)), and tokens need "an API key SID and secret" ([docs](https://www.twilio.com/docs/stun-turn)).
- Metered paid tiers: $0.10–0.40/GB overage.
- Xirsys: the pricing page didn't render; **not verified**.
- coturn: developer-hosted.
- **Verdict: REJECT** (paid or developer infrastructure). **Confidence: high.**

### C6. webrtc-rs (`webrtc` / `rtc` 0.21.0): full Rust WebRTC stack

| Aspect | Evidence |
|---|---|
| Gaps | G2 (with C1 + C10), **G4 client side** (TURN client included, but no free TURN server exists, C2–C5), G6 over a direct data channel. **Bonus: browser interop** (D45 iPhone invitees via a PWA). |
| Maturity | `webrtc` 0.21.0 released **2026-09-19**. Pre-1.0: "While the version is 0.x, a minor bump may carry breaking changes". It's a new async layer over the Sans-I/O `rtc` core ("95%+ W3C API compliance"). 5.1k stars, 8 open issues, 6.9 M downloads. Sponsors include Recall.ai and Stream. |
| License | MIT / Apache-2.0. Dependencies: rustls, ring or aws-lc-rs (crypto per connection), rcgen, x509-parser, quinn-udp. All permissive. **iroh already pulls rustls + ring**, so some code is shared. |
| Features | ICE (RFC 8445), trickle, ICE restart, ICE-TCP, mDNS candidates, STUN (RFC 8489), TURN (RFC 8656), DTLS, SCTP data channels ([rtc README](https://github.com/webrtc-rs/rtc)). |
| Android | Real Android users filed and got fixes in Aug 2026: [#868](https://github.com/webrtc-rs/webrtc/issues/868) "Android ICE restart stalls after ~10s background; rebinding UDP sockets restores recovery" (closed: "both v0.20.x and master branch have the fix") and [#874](https://github.com/webrtc-rs/webrtc/issues/874) (stale addresses after network handover). Older interface-listing bugs on Android 11+ (#272) are closed. |
| Binary size | **Not measured.** The full stack (DTLS + SCTP + SRTP + interceptors) is a second transport security stack next to iroh's QUIC/TLS. Measure before deciding (X4). |
| Privacy | DTLS fingerprints and ICE credentials must be carried in **signed, encrypted** signaling, so the peer is bound to a device key (D2). Host candidates reveal private IPs to the peer, so send them only to group members. |
| Failure modes | A second connection state machine and keepalive regime (battery); API churn before 1.0; the same NAT limits as iroh. |
| **Verdict** | **CONSIDER**, mainly for a future browser/PWA peer and as a second provider on desktops. It isn't needed for G2 if C9 works. **Confidence: medium.** |

### C7. str0m 0.23 (full stack)

| Aspect | Evidence |
|---|---|
| Status | "We use str0m for a specific use case: str0m as a server SFU (as opposed to peer-2-peer)... it also works for peer-2-peer, though that aspect has received less testing." Platform table: `aarch64-linux-android` **compiled ✅, tested ❌**. Feature table: **TURN ❌**, network interface enumeration ❌ ([README](https://github.com/algesten/str0m)). |
| License | MIT OR Apache-2.0. Crypto backends: aws-lc-rs (default), RustCrypto, OpenSSL, Apple, Windows CNG. |
| **Verdict** | **REJECT as the full P2P stack** (SFU focus, no TURN, Android untested). Its ICE crate is a separate candidate (C9). **Confidence: medium.** |

### C8. libdatachannel (C++) / datachannel-rs (Rust bindings); Google libwebrtc (Kotlin)
- **libdatachannel** v0.24.5 (2026-06-12), 2.7k stars.
  - Supports Android explicitly ("GNU/Linux, Android, FreeBSD, Apple macOS, iOS, and Microsoft Windows").
  - ICE via libjuice with STUN + TURN, plus usrsctp and a TLS library (GnuTLS, Mbed TLS or OpenSSL).
  - **MPL-2.0** ([README](https://github.com/paullouisageneau/libdatachannel)).
  - `datachannel` crate 0.16.1 is MPL-2.0 with 151 stars and low recent downloads (17.7k).
  - It adds a C/C++ cross-compile chain (CMake, a TLS library) to a Rust/NDK build.
  - **REJECT**: MPL (weak copyleft, against the permissive preference already strained by D29), toolchain cost, a small Rust binding community. **Confidence: medium.**
- **Google libwebrtc via `stream-webrtc-android`** 1.3.10 (2025-09-11), Apache-2.0 wrapper over BSD WebRTC.
  - The AAR is **20,609,071 bytes** (Maven Central HEAD).
  - Kotlin/Android only, so no shared engine for Linux/Windows. Media-oriented.
  - **REJECT.** **Confidence: high.**

### C9. ICE-only: str0m's `is` crate + public STUN + Nostr signaling → iroh custom transport

| Aspect | Evidence |
|---|---|
| What it is | `is` 0.11.0 (2026-08-08): "ICE (Interactive Connectivity Establishment) in Sans-IO style", from the str0m repo. MIT OR Apache-2.0. **~70 KB crate**, dependencies: crc, serde, str0m-proto, subtle, tracing, optional hmac/sha1. 1.1 M downloads. |
| Gaps | **G2 fully without n0**: public STUN (C1) for server-reflexive candidates, Nostr (C10) for coordination, `is` for paired checks and nomination. **G4 only if a TURN socket is added** (none free, C2–C5). G5 via C10. |
| Precedent | **Firezone** (9.1k stars, Apache-2.0) runs this pattern in production. Its `connlib/snownet` uses `is::IceAgent` + `stun_codec` + WireGuard (boringtun), with its own TURN allocations ([snownet Cargo.toml](https://github.com/firezone/firezone/blob/main/rust/libs/connlib/snownet/Cargo.toml), [agent.rs](https://github.com/firezone/firezone/blob/main/rust/libs/connlib/snownet/src/agent.rs)). The same Rust connlib runs in its Android client (`kotlin/android` + `rust/client-ffi`). That's the strongest evidence that a Sans-IO Rust ICE agent works on phones. |
| Integration with iroh | iroh 1.2 has `Builder::add_custom_transport` behind feature `unstable-custom-transports`: "This API is unstable and gated behind the `unstable-custom-transport` feature" ([docs.rs](https://docs.rs/iroh/1.2.0/iroh/endpoint/struct.Builder.html)). The traits `CustomTransport` / `CustomEndpoint` / `CustomSender` ([custom.rs](https://github.com/n0-computer/iroh/blob/v1.2.0/iroh/src/socket/transports/custom.rs)) let us **own the UDP socket**. So STUN, ICE checks and QUIC share one 5-tuple, demultiplexed by first byte (STUN 0–3, QUIC ≥ 64, per RFC 7983/9443). In iroh's remote-state code, "Relay and custom paths are kept open", and a custom-path connection also becomes the *first path* from which iroh's own IP hole punching can proceed. **Risk:** [#4544](https://github.com/n0-computer/iroh/issues/4544) "Custom transport send path drops Poll::Pending and reports the transmit as sent" was closed only 2026-09-22. The API has live bugs and may change without notice. |
| Android | Same limits as any UDP: foreground or expedited-job windows only (D16). The Android 17 local-network permission (D21) covers **outgoing checks to LAN and CGNAT-range host candidates**, so without it, only send checks to server-reflexive or public candidates. |
| Size | **Not measured.** Likely small (no DTLS/SCTP; reuses iroh QUIC). Measure (X4). |
| Failure modes | Unstable iroh API; hard NATs; CGNAT mapping rebinding (#4424 pattern); both peers must be online and foreground within the same Nostr signaling window. |
| **Verdict** | **CONSIDER, the best-fitting way to get an n0-independent G2.** **Confidence: medium-low** (architecture proven by Firezone; the iroh custom-transport bridge is unproven). |

### C10. Nostr as WebRTC/ICE signaling (ephemeral events)

| Aspect | Evidence |
|---|---|
| Gaps | **G2 coordination, G5** (presence: "I'm online, here are my candidates"). |
| Precedent | **Trystero** (MIT, 2.8k stars): "By default Trystero uses the Nostr network which is highly decentralized with hundreds of active relays running". It uses **ephemeral kinds** (`strToNum(topic, 10_000) + 20_000`, i.e. 20000–29999), redundancy 5 relays, a 60 s steady heartbeat and exponential backoff to 15 min ([nostr strategy](https://github.com/dmotz/trystero/blob/main/packages/nostr/src/index.ts)). **Amethyst** (Android), **0xChat** (NIP-100), **Noscall** and **nospeak** implement WebRTC calls over Nostr (NIP-AC drafts [#2301](https://github.com/nostr-protocol/nips/pull/2301), generic P2P bootstrap [#2461](https://github.com/nostr-protocol/nips/pull/2461), older [NIP-100 #363](https://github.com/nostr-protocol/nips/pull/363)). All are still **unmerged drafts**. |
| Relay acceptance | Ephemeral events aren't stored, so they avoid the "relays as storage" objection (D13), but they still count against per-IP rate limits (D11). Non-trickle signaling = 2 events per attempt. Not probed in E4 (which tested addressable kinds): **ephemeral-kind acceptance is unverified for our relay set.** |
| Timing | NIP-AC implementers report "ICE gathering take 10–15s on mobile behind challenging NATs" and asked for a 30–60 s window (#2301 comment). Ephemeral delivery needs the receiver to be **subscribed right now**. |
| Privacy | Encrypt candidates (they contain IPs) with the group/epoch key. Use rotating tags (D23). NIP-59 gift-wrap is optional. |
| Library | `nostr-sdk` 0.45.4 (MIT), already in the stack. |
| **Verdict** | **ADOPT** as the rendezvous channel for C9/C6/C11. **Confidence: medium-high** (precedents are strong; ephemeral acceptance on our specific relays needs one polite probe, your call). |

### C11. Extending iroh without a custom transport: `add_external_addr` + public STUN + Nostr coordination

| Aspect | Evidence |
|---|---|
| Mechanism | `Endpoint::add_external_addr`: "Adds an external address on which this endpoint is directly reachable. This address will be advertised to peers together with any discovered external addresses and will be used in NAT traversal" ([docs.rs](https://docs.rs/iroh/1.2.0/iroh/endpoint/struct.Endpoint.html)). In source, it becomes `DirectAddrType::Config` in `update_direct_addresses` ([socket.rs](https://github.com/n0-computer/iroh/blob/v1.2.0/iroh/src/socket.rs)). A custom `AddressLookup` fed from Nostr (R1 "missed item 1") gives peers each other's candidates. |
| Blocker 1: which socket | iroh owns its sockets and has no STUN client. A STUN reply from **another** socket is right only if the NAT preserves ports. With `bind_addr` on a fixed port, "public IP from STUN + local port" is exactly iroh's own `Qad4LocalPort` heuristic ("Worst case it's an invalid candidate mapping"). Many CGNATs don't preserve ports. |
| Blocker 2: no first path | Hole punching starts only from an existing connection ("not holepunching: no connections" in [remote_state.rs](https://github.com/n0-computer/iroh/blob/v1.2.0/iroh/src/socket/remote_map/remote_state.rs)). The maintainer said in [#2567](https://github.com/n0-computer/iroh/issues/2567): "**Without a relay server you will not be able to holepunch.** The `RelayMode::Disabled` is for when you can establish a direct connection without holepunching." Without n0, a first path exists only if one side is directly reachable (public IPv6, port-mapped desktop, member relay) **or** a simultaneous QUIC `connect` from both sides, timed via Nostr, opens both NATs. The latter is **unverified** and not a documented iroh feature. |
| Gaps | G2 partially: IPv6, UPnP/PCP desktops, port-preserving home NATs. Not CGNAT phone↔phone. |
| **Verdict** | **CONSIDER** as a cheap first step (no unstable API, no new stack) that shrinks n0 dependence, **only after X2 shows the simultaneous-connect trick works on port-restricted NAT**. **Confidence: medium** on the facts, low on the trick. |

---

## 4. Comparison vs gaps

| Candidate | G1 offline S&F | G2 n0-free P2P | G3 wake phone | G4 free relay | G5 presence | G6 blobs | Verdict (confidence) |
|---|---|---|---|---|---|---|---|
| C1 Public STUN | – | discovery half | – | – | – | – | **ADOPT** (high) |
| C2 Cloudflare TURN | – | – | – | yes, but needs a backend and bills the dev | – | – | **REJECT** (high) |
| C3 Open Relay TURN | – | – | – | opportunistic, no guarantee | – | – | **CONSIDER** last resort (low) |
| C4 ExpressTURN free | – | – | – | shared account creds | – | – | **REJECT** (med-high) |
| C5 Paid TURN / coturn | – | – | – | paid / dev infra | – | – | **REJECT** (high) |
| C6 webrtc-rs full stack | – | yes (with C1, C10) | – | client only | – | direct channel | **CONSIDER** (medium), browser interop |
| C7 str0m full | – | yes | – | no TURN | – | – | **REJECT** (medium) |
| C8 libdatachannel / libwebrtc | – | yes | – | client | – | – | **REJECT** (med / high) |
| C9 `is` ICE + STUN + Nostr → iroh custom transport | – | **yes** | – | only with a TURN socket | via C10 | via iroh | **CONSIDER**, best fit (med-low) |
| C10 Nostr ephemeral signaling | – | coordination half | – | – | **yes** | – | **ADOPT** (med-high) |
| C11 iroh add_external_addr + STUN + Nostr | – | partial | – | – | via C10 | via iroh | **CONSIDER** after X2 (medium) |

**Uncovered by this whole dimension:** G1, G3, G6-when-offline, and **G4 at any scale**. The ~30% of pairs that can't be punched still rely on n0 relays (D14 option a), member relaying (layer 6) and the mailbox.

---

## 5. Could WebRTC run alongside iroh as a second internet P2P provider?

**Yes, technically. They're independent sockets, and the sync protocol only needs a reliable authenticated byte or message channel.** But what it buys has to be stated precisely:
- It **does** remove n0 from address discovery (public STUN from ≥ 3 operators) and coordination (Nostr). That satisfies principle 2 for those two roles.
- It does **not** add a second *relay* provider. No TURN offering is both free and embeddable, except Open Relay's no-guarantee service.
- It does **not** raise punch success: same NATs, same physics.
- A full WebRTC stack doubles the transport security surface (DTLS + SCTP next to QUIC/TLS), adds a second keepalive regime, and adds unmeasured size.

**Recommended shape if you pursue it:** C9 (ICE-only into iroh's custom transport) on the engine, C1 + C10 as its providers, C3 only as an opportunistic extra. Keep C6 in reserve for the day a browser/PWA client is wanted (D45). If the custom-transport API proves too unstable, fall back to C6 on desktops only, where battery and size matter less.

---

## 6. New risks found

1. **iroh's custom-transport API is unstable and buggy right now** (#4544 closed 2026-09-22; the feature is documented "may change without notice"). Any n0-independent G2 via iroh depends on it, or on the unverified simultaneous-connect trick (C11).
2. **iroh can't use public STUN correctly without owning the socket.** STUN results from a side socket are wrong on non-port-preserving NATs, which describes most CGNATs: exactly the phone↔phone case.
3. **Android 17 local-network permission applies to ICE** (D21 covers outgoing traffic to LAN and CGNAT ranges). ICE host-candidate checks will silently fail without it. Gate host-candidate checks on the permission, or only pair server-reflexive and public candidates.
4. **Ephemeral Nostr events are untested on our relay set.** E4 tested addressable kinds only. strfry/Damus limits (D11) still apply, and a D10-style policy shift could AUTH-gate ephemeral kinds too.
5. **Open Relay credential churn.** Amethyst's embedded `openrelayproject` credentials no longer appear on Metered's page. A secret baked into shipped APKs can go dead overnight, so it would need a signed runtime config.
6. **Public STUN list churn is high** (~60% of known hosts dead at any time), and many "different" hostnames belong to one VoIP family. Health-track by operator, not by hostname.
7. **Metadata:** TURN operators see both peers' IPs (a social graph). STUN operators see presence timing. Unencrypted STUN is visible on-path. All three belong in the §9 table if adopted.
8. **Carrier NAT mapping rebinding** (~50–75 s on T-Mobile, iroh #4424) hits any UDP punching. A C9 path must re-punch periodically and keep a fallback, which costs battery on phones.
9. **NIP drafts for WebRTC signaling are unmerged and competing** (NIP-100, NIP-AC ×2). Use our own encrypted event format rather than pinning to a draft.

## 7. Suggested SAFE local experiments (no public-infrastructure traffic unless you approve it)

- **X1: `is` + iroh custom transport, loopback/LAN.** Two processes. A custom transport owns one UDP socket and demuxes STUN vs QUIC. Run `is` ICE with host candidates, signaling via a **local** strfry, and hand the nominated pair to iroh. Measure time-to-connected and whether iroh then opens its own IP path.
- **X2: NAT lab (Linux network namespaces + iptables MASQUERADE, or two VMs).** Build three NAT types: port-preserving full-cone, port-restricted, and symmetric (`--random`). Run a **local STUN server** (e.g. the `turn-server` crate or `stunserver`). Test:
  - (a) C11: `add_external_addr` + simultaneous `connect()` timed via local Nostr;
  - (b) C9;
  - (c) webrtc-rs data channel.

  Record success per NAT pair. This is the only way to get a real number for our own stack, not a vendor's.
- **X3: Local TURN (coturn or `turn-server`, MIT) with static-auth secret.** Verify a TURN socket can be wrapped as an iroh custom transport, and measure the overhead. Tells us what C3 would cost to integrate before depending on any public TURN.
- **X4: Size.** Release-build arm64 `.so` with (i) iroh only (baseline 7.07 MB from E6), (ii) + `is` + `stun_codec`, (iii) + `webrtc` 0.21 (ring, tokio). Report the size deltas.
- **X5: Android behavior (headless emulator).** Background the app with an established ICE/iroh path. Confirm the ~10 s socket death, then recovery with socket rebind (the webrtc-rs #868 pattern) and with the Android 17 local-network permission denied.
- **X6 (needs your approval, outward traffic):** one polite ephemeral-kind round-trip on 3–4 public relays, and a single STUN binding to each default STUN operator. Both are tiny and consistent with STATE.md's probing rules. A single Open Relay TURN allocation test only if you decide C3 is worth keeping.

## 8. Sources

**STUN / TURN providers**
- Cloudflare TURN overview (ports, limits): https://developers.cloudflare.com/realtime/turn/
- Cloudflare TURN FAQ ("free and unlimited" STUN, 48 h credentials): https://developers.cloudflare.com/realtime/turn/faq/
- Cloudflare credential generation (server-side key): https://developers.cloudflare.com/realtime/turn/generate-credentials/
- Cloudflare Realtime pricing (1,000 GB shared, $0.05/GB): https://developers.cloudflare.com/realtime/pricing/
- Cloudflare anycast TURN blog: https://blog.cloudflare.com/webrtc-turn-using-anycast/
- Metered Open Relay: https://www.metered.ca/tools/openrelay/
- Open Relay terms: https://www.metered.ca/tools/openrelay/terms-and-conditions/
- Metered TURN pricing: https://www.metered.ca/stun-turn
- ExpressTURN: https://www.expressturn.com/ and terms https://www.expressturn.com/terms
- Twilio NTS docs: https://www.twilio.com/docs/stun-turn ; pricing https://www.twilio.com/en-us/stun-turn/pricing
- Public STUN list (hourly): https://github.com/pradt2/always-online-stun (valid_hosts.txt, valid_ipv4s.txt, valid_ipv6s.txt, valid_nat_testing_hosts.txt, candidates.txt fetched 2026-09-24)
- Syncthing STUN defaults: https://github.com/syncthing/syncthing/blob/main/lib/config/config.go ; docs https://docs.syncthing.net/users/config.html
- Not reachable during research: https://freestun.net/ (HTTP 520), Xirsys pricing (no content)

**Success rates / NAT behavior**
- Ford, Srisuresh, Kegel, P2P across NATs: https://bford.info/pub/net/p2pnat/
- libp2p DCUtR measurement: https://arxiv.org/abs/2510.27500
- Tailscale, How NAT traversal works: https://tailscale.com/blog/how-nat-traversal-works
- iroh CGNAT rebinding issue: https://github.com/n0-computer/iroh/issues/4424

**iroh**
- Endpoint (add_external_addr): https://docs.rs/iroh/1.2.0/iroh/endpoint/struct.Endpoint.html
- Builder (add_custom_transport, bind_addr): https://docs.rs/iroh/1.2.0/iroh/endpoint/struct.Builder.html
- Custom transport traits: https://github.com/n0-computer/iroh/blob/v1.2.0/iroh/src/socket/transports/custom.rs
- Direct address assembly / Qad4LocalPort: https://github.com/n0-computer/iroh/blob/v1.2.0/iroh/src/socket.rs
- Holepunch trigger logic: https://github.com/n0-computer/iroh/blob/v1.2.0/iroh/src/socket/remote_map/remote_state.rs
- Features (unstable-custom-transports): https://github.com/n0-computer/iroh/blob/v1.2.0/iroh/Cargo.toml
- "Without a relay server you will not be able to holepunch": https://github.com/n0-computer/iroh/issues/2567
- Custom transport send bug: https://github.com/n0-computer/iroh/issues/4544
- QAD rationale: https://www.iroh.computer/blog/qad

**Rust / native WebRTC stacks**
- webrtc-rs: https://github.com/webrtc-rs/webrtc ; rtc core https://github.com/webrtc-rs/rtc ; crates https://crates.io/crates/webrtc , https://crates.io/crates/rtc
- webrtc-rs Android issues: https://github.com/webrtc-rs/webrtc/issues/868 , https://github.com/webrtc-rs/webrtc/issues/874
- str0m: https://github.com/algesten/str0m ; `is` crate https://crates.io/crates/is
- Firezone snownet (str0m ICE in production): https://github.com/firezone/firezone/tree/main/rust/libs/connlib/snownet
- libdatachannel: https://github.com/paullouisageneau/libdatachannel ; datachannel-rs https://github.com/lerouxrgd/datachannel-rs
- stream-webrtc-android: https://github.com/GetStream/webrtc-android ; AAR https://repo1.maven.org/maven2/io/getstream/stream-webrtc-android/1.3.10/stream-webrtc-android-1.3.10.aar
- crates.io API (versions, licenses, dependencies) queried 2026-09-24 for: iroh, webrtc, rtc, str0m, is, datachannel, libdatachannel, stun-rs, stun_codec, turn, webrtc-ice, rtc-turn, rtc-ice, rtc-stun, dimpl, sctp-proto, turn-server, turn-rs (GPL-2.0-or-later, avoid), nostr-sdk

**Nostr signaling precedents**
- Trystero: https://github.com/dmotz/trystero ; Nostr strategy https://github.com/dmotz/trystero/blob/main/packages/nostr/src/index.ts ; default ICE servers https://github.com/dmotz/trystero/blob/main/packages/core/src/peer.ts
- Amethyst ICE config: https://github.com/vitorpamplona/amethyst/blob/main/amethyst/src/main/java/com/vitorpamplona/amethyst/service/call/IceServerConfig.kt
- NIP-AC (calls): https://github.com/nostr-protocol/nips/pull/2301 ; NIP-AC (generic P2P): https://github.com/nostr-protocol/nips/pull/2461 ; NIP-100: https://github.com/nostr-protocol/nips/pull/363

**Internal**
- `docs/DESIGN.md` §6–§8; `docs/research/DECISIONS.md` D11–D23; `docs/research/R1-iroh.md`; `docs/research/REPORT.md`

---

## Independent verification

*2026-09-24, skeptical second pass. I used only documentation, source and registry fetches (raw GitHub, the GitHub API, the crates.io API, WebFetch) plus DNS lookups of STUN hostnames. No packets went to any STUN, TURN, Nostr or iroh server. Wherever I could, I checked primary sources rather than the researcher's quotes.*

### Verdict checks

| # | Candidate | Original | Result | Corrected verdict |
|---|---|---|---|---|
| C1 | Public STUN | ADOPT, high | **Upheld, with a caveat on permission** | ADOPT. High confidence on availability, medium on explicit permission |
| C3 | Metered Open Relay | CONSIDER, low | **Upheld, leaning towards REJECT** | CONSIDER, low; opportunistic only |
| C4 | ExpressTURN free | REJECT | **Upheld** | REJECT, medium-high |
| C6 | webrtc-rs 0.21 | CONSIDER, medium | **Upheld, confidence lowered** | CONSIDER, medium-low |
| C7 | str0m full stack | REJECT, medium | **Upheld, one reason corrected** | REJECT, medium |
| C8 | libdatachannel / libwebrtc | REJECT | Upheld (not re-verified in depth) | REJECT |
| C9 | `is` ICE + STUN + Nostr -> iroh custom transport | CONSIDER, medium-low | **Upheld, but the precedent is weaker than claimed** | CONSIDER, **low** |
| C10 | Nostr ephemeral signaling | ADOPT, medium / medium-high | **Upheld, with corrections** | ADOPT, **medium** (until the X6 probe) |
| C11 | iroh add_external_addr + STUN + Nostr | CONSIDER, medium | **Upheld; an unexamined lever makes it stronger** | CONSIDER, medium; gate on X2 plus an IPv6-share measurement |

**C1, public STUN.**
- **Confirmed:**
  - Cloudflare's FAQ says, verbatim, "free and unlimited".
  - always-online-stun (MIT, pushed 2026-09-24 14:50 UTC) has exactly 264 / 182 / 21 / 635 entries.
  - `stun.syncthing.net` and `stun.services.mozilla.com` appear only in `candidates.txt`, not in the valid list.
  - Amethyst ships Google x2 + Cloudflare.
- **Operator diversity is real:**
  - The 181 distinct IPv4 addresses fall into about 170 distinct /24s, and the valid hosts span 254 registrable domains.
  - The "VoIP family" caveat is also real. There are 23 `stun.voip*` hosts, and 14 of the first 40 resolved names map to 77.72.169.210-213, one operator.
- **Corrections:**
  1. **Cloudflare and Twilio aren't in the always-online-stun list at all**, so the 264 count doesn't include the two operators with explicit free statements.
  2. **Twilio's permission is weaker than stated.** The pricing page lists STUN as "Free". But Twilio's NTS docs only present `stun:global.stun.twilio.com:3478` inside a token response obtained with an account ("request a token and pass it to your RTCPeerConnection"). Nothing grants account-less third-party use. Treat Twilio like Google: de facto, not permitted.
  3. The only operators with a **written** free statement are Cloudflare and Metered.
- ADOPT stands, because the fallback set is large and independent. Default to Cloudflare, Google and Nextcloud, and use the rest only as health-tracked fallbacks.

**C3, Open Relay.**
- **Confirmed:** the static-auth secret `openrelayprojectsecret` at `staticauth.openrelay.metered.ca`, the T&C line "CAN AND WILL GO DOWN WITHOUT NOTICE", and no explicit prohibited-use clause (the T&C also carries an indemnification clause). Amethyst still hard-codes `openrelayproject/openrelayproject` on `openrelay.metered.ca:80/443` (fetched today).
- **New context:**
  - The page now makes **sign-up plus a per-account API key** the main path: "To Connect to the Open Relay TURN Server, you need to sign-up for a free account and obtain your API Key". The "20 GB / month" is tied to that account flow.
  - The static secret is framed narrowly: "Services like Nextcloud Talk or Matrix+Synapse+Riot uses static auth", meaning an admin configuring one server.
  - Baking that secret into a mass-distributed app goes beyond the described use, and it has no stated quota at all.
- Keep it strictly opportunistic and off by default, and never count it as a provider. It is no stronger than low confidence.

**C4, ExpressTURN.**
- Confirmed: "limited to 1000 GB of traffic per month", static credentials, sign-up required, one location on the free tier, Premium $9/mo.
- REJECT stands. The summary said "medium" and the body said "medium-high"; use medium-high.

**C6, webrtc-rs.**
- **Confirmed:** v0.21.0 released 2026-09-19 15:33Z; MIT/Apache-2.0; 5,152 stars, 8 open issues; #868 closed 2026-08-13 by the maintainer ("both v0.20.x and master branch have the fix... Feel free to re-open").
- **Caveats not in the report:**
  1. The async `webrtc` crate is "a clean... rewrite on top of a Sans-I/O core" (`rtc`). The 6.9 M downloads mostly predate that architecture, so the current code is young.
  2. **Bus factor.** The top contributor has 818 commits; the next has 198.
  3. The #868 recovery depends on **rebinding UDP sockets** after backgrounding, which the app has to drive.
- Confidence lowered to medium-low. The main value is still a browser/PWA peer (D45).

**C7, str0m.**
- The SFU focus, "peer-2-peer... has received less testing" and "aarch64-linux-android: compiled yes, tested no" are all confirmed.
- **Correction:** "no TURN" is a deliberate Sans-IO design choice, not a missing feature. The README says "We consider TURN similar to enumerating local network interfaces - it's a way of obtaining sockets", and the user adds relay candidates. The same is true of `is` (C9). The REJECT still stands on the other grounds.

**C9, `is` + iroh custom transport. This is the most important correction.**
- **Confirmed:**
  - `is` 0.11.0 (2026-08-08, MIT OR Apache-2.0, 69,625-byte crate);
  - Firezone's workspace pins `is = "0.11.0"` and snownet depends on `is` + `stun_codec` + `str0m-proto` + boringtun;
  - #4544 closed 2026-09-22 09:22Z;
  - iroh `Cargo.toml` says "Unstable: Custom transport API (may change without notice)";
  - `remote_state.rs` has "Relay and custom paths are kept open".
- **But the Firezone precedent doesn't carry over to our constraints:**
  1. **Firezone's Android client runs as a `VpnService` with `foregroundServiceType="systemExempted"`** (AndroidManifest.xml). It never faces the ~10 s background socket death that our app faces.
  2. **Firezone always has its own TURN relays.** `node.rs` calls `sample_relay()` and seeds the agent's candidates from its relay allocations, with "portal-issued credentials". Its reflexive candidates and its signaling come from Firezone's own relays and portal, not from public STUN or a public rendezvous.

  So Firezone proves that the ICE agent works on Android. It does not prove relay-less, public-STUN, Nostr-signaled, backgrounded operation.
- **Crate maturity:**
  - The name `is` belonged to an unrelated 2019 crate (0.1-0.6). algesten's line starts at 0.7.0 on 2026-03-24, with five breaking minor releases in six months.
  - On crates.io, str0m is its **only** reverse dependency, so the ~1.1 M downloads are str0m's.
  - The ICE code itself is older (it was str0m's ice module), but the standalone API is new.
- **Mechanics:**
  - An iroh custom transport carries `CustomAddr` paths, which are separate from iroh's IP sockets.
  - After the custom path is up, iroh's own QNT punching still runs on its IP sockets. Without n0 QAD it has only host, port-mapped and configured candidates, which is harmless but wastes probes.
  - `trigger_holepunching` also needs a **client-side** connection ("not holepunching: no client connection"), not just any connection.
- Confidence lowered to **low**.

**C10, Nostr ephemeral signaling.**
- **Confirmed:**
  - Trystero's kind formula is `strToNum(topic, 10_000) + 20_000` with `defaultRedundancy = 5`.
  - Trystero's README now reads "The default Trystero package runs on the Nostr network", and it tells developers to "Sign up for a TURN service" (no bundled TURN).
  - NIP PRs #2301, #2461 and #363 are all **open**; #2301 and #2461 were updated today.
- **New evidence from strfry's default `strfry.conf`** (strfry runs Damus and many others):
  - `rejectEphemeralEventsOlderThanSeconds = 60`, so a device clock skewed by more than about a minute gets its signaling rejected.
  - `ephemeralEventsLifetimeSeconds = 300`, so strfry **stores ephemeral events for 5 minutes**. This softens "the receiver must be subscribed right now", since a fresh REQ within about 5 minutes may still see the event on strfry relays (behavior may vary by relay). It also weakens the privacy claim that "relays don't store them".
- Use medium, not medium-high, until the X6 probe.

**C11, extend iroh.**
- **Confirmed in v1.2.0 source:**
  - `trigger_holepunching` returns early with "not holepunching: no connections";
  - `Qad4LocalPort` carries the comment "Worst case it's an invalid candidate mapping";
  - configured addresses become `DirectAddrType::Config`.
- **Caveat:** the "Without a relay server you will not be able to holepunch" quote is from July 2024 (iroh-net era, maintainer flub), before iroh's QUIC-NAT-traversal rewrite. The current code independently supports the same conclusion, so it stands, but cite the code rather than the issue.
- **Missed lever:** iroh 1.2 exposes `QuicTransportConfig::send_observed_address_reports` / `receive_observed_address_reports` (QUIC address discovery) on **peer** connections. The docs say: "This will aid peers in inferring their reachable address, which in most NATd networks will not be easily available to them".
  - A reachable member (a desktop helper or an IPv6 host) could report a phone's reflexive address **for iroh's own socket**. That removes the "wrong socket" blocker without side-socket STUN.
  - Caveat: `remote_state.rs` currently ignores `ObservedAddr` events ("Nothing to do for these events"), and whether the public API surfaces them is unverified.

### Cross-cutting corrections

- **Success-rate baseline.** The 70% +/- 7.1% figure (arXiv 2510.27500, confirmed) comes from libp2p/IPFS DCUtR over "85,000+ networks", a mostly non-mobile population. Don't transfer it to phone<->phone carrier CGNAT, where the rate is likely lower. There is still no independent phone-specific figure.
- **IPv6 is the dominant unmeasured parameter.** On IPv6 there is no address translation. A phone's global address comes from interface enumeration (no STUN needed), and simultaneous open reduces to ordinary stateful-firewall punching. Both C9 and C11 get much better as the share of phone pairs with IPv6 on both ends grows. The report mentions IPv6 but never treats it as the main variable. X2 should include an IPv6 stateful-firewall case, and the beta should log address families.
- **Other Rust ICE agents** (not evaluated in the report):
  - `webrtc-ice` 0.17.2: async, MIT/Apache, 6.4 M downloads, with built-in interface enumeration, mDNS and TURN relay candidates;
  - `rice-proto` / `librice` 0.4.3 plus `turn-client-proto` 0.7.2: Sans-IO ICE and TURN, MIT OR Apache-2.0, low adoption (about 2.8k and 32k downloads).

  Both are alternatives to `is` if its API churn becomes a problem.
- **Internal inconsistencies to fix in the report:** ExpressTURN confidence (medium vs medium-high) and Nostr confidence (medium vs medium-high) differ between the summary and the body.

### Bottom line

The dimension's overall conclusion survives. Public STUN plus Nostr can take n0 out of discovery and coordination, nothing free and embeddable solves G4, and WebRTC doesn't raise punch success.

Two things change:
- The Firezone evidence for C9 is weaker than presented: foreground VPN service, its own relays, its own signaling.
- C11's prospects hinge on two things the report under-weights: the IPv6 share of phone pairs, and QUIC observed-address reports from reachable members.

Prioritize X2 with IPv6 and observed-address variants before any code on C9.
