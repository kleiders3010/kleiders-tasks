# E3 + E6: iroh connectivity and Android app size (measured)

Date: 2026-09-24. iroh 1.2.0, Loro 1.16.2, nostr 0.45.5, Rust 1.98.1, NDK 29.0.14206865.
Code: `research/experiments/rust/e3_iroh`, `research/experiments/rust/e6_size`.

## E6: Android arm64-v8a release `.so`
Size-optimized profile: `opt-level="z"`, LTO, 1 codegen unit, `panic=abort`, stripped. `minSdk` 26.

| Contents | Installed (.so) | Compressed (≈ download) |
|---|---|---|
| Crypto only (Argon2, XChaCha20-Poly1305, HKDF) | 0.30 MB | 0.16 MB |
| + Loro | 1.74 MB | 0.94 MB |
| + iroh | 4.58 MB | 2.45 MB |
| + nostr | 1.50 MB | 1.32 MB |
| **All together** | **7.07 MB** | **4.33 MB** |

Adding the Kotlin/Compose UI (~5–8 MB estimated), UniFFI glue and iroh-blobs gives an estimated **≈15–20 MB installed and ≈10 MB download**.

**Verdict: the size targets hold with room to spare** (≤20 MB download / ≤40 MB installed / 50 MB ceiling). iroh is the biggest single piece (~65% of the native code).

## E3: iroh connectivity (desktop, two endpoints on one machine)
| Scenario | Result |
|---|---|
| (a) Local only: relays disabled, no discovery, direct addresses | 3 direct addresses; connect **2.4 ms**; 1 MB echo 8.3 ms |
| (b) n0 preset, dial by endpoint ID only | "online" (home relay chosen) after 2.6 s; connect **669 ms**; 10 KB echo 308 ms (via n0 relay path before direct upgrade) |

## Finding: iroh's defaults depend on one company's infrastructure
From the iroh 1.2 source (`endpoint/presets.rs`, `defaults.rs`):
- The `N0` preset uses **n0's relays** (4 regions, `*.relay.n0.iroh.link`) **and n0's DNS/pkarr discovery server** (`iroh.link`) for dial-by-ID.
- Core iroh ships only DNS, pkarr and in-memory address lookup. **mDNS and Mainline-DHT discovery are not in the core crate**; they need extra crates or our own implementation.
- `RelayMode::Custom(RelayMap)` accepts other relays, but we host none, and no independent public iroh relay operators are confirmed yet (see R1).

This conflicts with principle 2 (≥ 2 independent providers per role). The design's §6 table implied discovery via the DHT and mDNS comes "from iroh", which is only partly true.

**Fix:**
- **Discovery:** publish and resolve endpoint addresses ourselves through channels we already have: **Nostr** (a replaceable "where am I" slot per device, encrypted to the group) and **Mainline DHT via pkarr** (the `pkarr` crate can publish straight to the DHT without n0). Use n0's DNS as one of several.
- **LAN:** add mDNS explicitly (iroh's mDNS lookup crate or `mdns-sd`), subject to Android 17's local-network permission.
- **Relays:** n0's relays are the only iroh relay provider. Nostr remains the independent fallback for delivery, and helpers (§7a) later add capacity. Watch for independent iroh relay operators.
