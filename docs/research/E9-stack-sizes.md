# E9: Android size of candidate network stacks (measured)

Date: 2026-09-24. Android arm64-v8a release `.so`, same size profile as E6 (`opt-level="z"`, LTO, 1 codegen unit, `panic=abort`, stripped), NDK 29, `minSdk` 26. Code: `research/experiments/rust/e9_*`. Each probe exercises the stack's main path so the linker can't drop it.

| Stack (what the probe uses) | Installed (.so) | Compressed | License |
|---|---|---|---|
| **libp2p 0.57**: QUIC + TCP/noise/yamux, relay client (hole-punch coordination), DCUtR, identify, Kademlia | 3.45 MB | 1.84 MB | MIT |
| **str0m 0.23**: sans-IO WebRTC, data channel, offer/answer | 2.12 MB | 1.07 MB | MIT |
| **webrtc-rs 0.21**: async WebRTC, data channel, offer | 2.34 MB | 1.26 MB | MIT/Apache-2.0 |
| **veilid-core 0.5.7**: startup, attach, DHT record create/set | 6.79 MB | 3.61 MB | **MPL-2.0** |
| *for reference (E6):* iroh 1.2 | 4.58 MB | 2.45 MB | MIT/Apache-2.0 |
| *for reference (E6):* whole current core (iroh + Loro + Nostr + crypto) | 7.07 MB | 4.33 MB | |

**Notes:**
- Each figure includes its own copy of shared dependencies (tokio, rustls/ring, etc.). Combined builds share them, so adding a second stack to our core costs **less** than its standalone number. That's not measured yet.
- All four build cleanly for Android with the NDK 29 toolchain. Veilid needed no extra tools (its pre-generated Cap'n Proto code was used).
- **Budget impact:** even the largest addition (Veilid, ≤ 6.8 MB) keeps the estimated app well under the 40 MB installed target (E6 estimate ~15–20 MB before this).
- Veilid's MPL-2.0 is file-level copyleft, the same category as the Loro dependencies in D29.
