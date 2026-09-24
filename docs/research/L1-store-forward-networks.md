# L1: Permissionless store-and-forward networks as a second mailbox

*2026-09-24. Research dimension L1 (gap **G1**: offline store-and-forward that doesn't depend on Nostr). The question: could any of these networks carry our encrypted per-device slots (DESIGN §7) alongside Nostr, within C1–C4: $0 for users and for us, nothing we host, no accounts, seamless?*

*Method: official specs and source code (fetched with `gh api` from the upstream repos), operators' own terms and info pages, crates.io metadata, and project issue trackers. The WebSearch budget was exhausted, so every fact below comes from a fetched primary source (URLs at the end). I sent no traffic to any relay, snode, mail server or peer network, and I installed and built nothing. Anything I could not verify is marked **unverified**.*

---

## Summary

- **No candidate can be adopted outright.** Every network that can technically hold our slots is run for one flagship app (Session, Status/Logos, Delta Chat, SimpleX). We would be a second tenant nobody invited, which is the same situation as n0 (D14) and Nostr (D13). Adding one of them buys an **independent failure domain**, not a guarantee.
- **Best technical fit: Session's storage swarms (CONSIDER, medium confidence).** The storage server has a **"public outbox" namespace**: storing a message replaces the previous one, only the owner key can write, anyone with the key can read without auth, and it lives up to **30 days**. That is Nostr's addressable event almost exactly, with **75 KiB per message** (76,800 B) and a far looser rate limit (300 requests/s per IP token bucket). The network has 1,500+ staked nodes. Caveats:
  - The Session ToS covers only the Session app and **neither permits nor forbids** other apps.
  - The network is funded by a crypto token (SESH).
  - The official client libraries are GPL-3.0, so we'd write our own client.
  - Session's seed domains now appear in **malware IOC lists** (Mini Shai-Hulud worm, May 2026).
- **Second-best: chatmail relays (CONSIDER, low confidence).** Everything is standard IMAP/SMTP with mature permissive Rust crates. There's no signup (an account is created on first login), 20-day retention, 500 MB per address, 30 MB per message, 60 messages/min. But there are only **~22 small volunteer relays**, "optimized for... DeltaChat". Mapping our slot model onto them needs a shared-credential group mailbox, which operators might reasonably read as abuse.
- **REJECT the Waku Network** (high confidence) for G1. Store nodes **"SHOULD only store messages with a valid rate_limit_proof"**, and an RLN membership needs an Ethereum L2 transaction (testnet today; the mainnet design uses a DAI deposit). Retention is only **≥12 h** (spec) / **2 days** (nwaku default). The Rust bindings on crates.io haven't been published since 0.6.0 (Feb 2024).
- **REJECT SimpleX preset servers** (high confidence). Their Conditions of Use allow third-party apps only if the app "provides user-to-user messaging only", "displays the notice that it is the App for using SimpleX network", and makes users accept SimpleX's conditions. On top of that, SMP queues delete a message once it's delivered, and the library is Haskell under AGPL-3.0.
- **REJECT Freenet (new, Rust) for now** (high confidence), but keep watching. Its contract model (commutative-monoid state with summary/delta sync) matches our CRDT slots conceptually. Today it is **alpha**, desktop-only ("A mobile app isn't available yet"), needs a local node, sends telemetry, and "older versions stop working over time".
- **Briar Mailbox is not a network** (REJECT as a candidate). It confirms tier 1 of the design (a member-owned always-on device), and it's AGPL, reached over Tor, last released July 2024, with Briar itself in maintenance mode since 2026-07-09.
- **None of the six covers G2, G3 or G4.** The push services attached to these networks (Session's push server, `notifications.delta.chat`, SimpleX's iOS push) are run centrally by their projects, for their own app IDs.

---

## Baseline we compare against (Nostr, from D11/D12/D13)

| Property | Nostr today |
|---|---|
| Slot semantics | Addressable events (replace by pubkey+kind+d) |
| Max size | ≤ 64 KiB event (~47 KiB payload) |
| Rate limits | ~8 writes/min/IP (Damus), 1 h bans; khatru default 2 per 3 min |
| Retention | Operator's choice, often undocumented |
| Operators | Hundreds, but ~75% run strfry; donation-funded |
| Acceptance risk | Operators hostile to "relays as storage" |

A second mailbox is worth adding only if it (a) fails independently of Nostr, (b) has comparable or better slot semantics, and (c) is at least as legitimate for us to use.

---

## 1. The Waku Network (Logos Messaging, formerly Waku, Status)

**What it is:** a libp2p gossipsub network (8 shards, cluster 1) with request/response services: Store (history), Filter, Lightpush, and Peer Exchange. Spam protection is RLN (zero-knowledge rate-limit proofs tied to an on-chain membership). In Nov 2025 it was renamed "Logos Messaging", and the repos are now `logos-messaging/logos-delivery*`.

**Gaps covered:** G1 weakly (short retention, needs RLN); G5 in principle (pub/sub presence, small messages). Not G2, G3, G4 or G6 (150 KB max, 4 KB average recommended).

**Evidence:**
- **Rate limit and cost.** The 64/WAKU2-NETWORK spec (draft, updated 2026-05-11):
  - `rlnRelayUserMessageLimit=100` messages per `rlnEpochSizeSec=600`.
  - The contract lives on Sepolia (chain 11155111), with "`staked_fund=0`... Registering a membership just requires to pay gas". The current operator docs say to use a **Linea Sepolia** RPC.
  - The raw RLN-contract spec's "RECOMMENDED values for the initial mainnet deployment": membership price **0.05 USD per message-per-epoch per 180 days**, paid in **DAI**, minimum rate 20 per epoch, and a global cap `R_max=160000` per epoch *for the whole network*.
  - Blog, Oct/Nov 2025: work on "service incentivisation" and a "payment protocol"; RLN is still on a testnet.
- **Messages without proofs.** "individual relayers MAY choose to freely route messages without RLN proofs up to a discretionary bandwidth limit" (1 Mbps per shard). But "Store service nodes SHOULD only store messages with a valid `rate_limit_proof`", and "at times of high network utilization these messages may not be relayed at all." **For G1 this means no RLN, no mailbox.**
- **Lightpush** (for phones): "Add support attaching RLN proof for the message requested to be relayed" is still listed as *future work*. A light client either needs its own membership or relies on a service node that attaches its own.
- **Retention.** The spec says "The store SHOULD be configured to retain at least `12` hours". The nwaku default is `time:` 2 days (`cli_args.nim`). The FAQ: "Waku does not guarantee the message's availability", and Waku is "not a long-term data storage network".
- **Slot semantics.** Store is an **append log** queried by content topic and time range, with pagination. Nothing gets replaced, so each rewrite is a new message. Readers must page through a time window, and a writer absent longer than the retention leaves nothing behind.
- **Message rules:** 150 KB absolute max after protobuf serialization; timestamps must be within **20 s** of the relay's clock.
- **Operators.** "Status currently maintains two fleets for `nwaku` nodes, the `waku.test` fleet and the `waku.sandbox` (sandbox) fleet... each fleet consists of three waku nodes." The `test` fleet redeploys on every commit to master. Anyone can run nwaku (2 GB RAM recommended, needs a Linea Sepolia RPC). I found **no count of independent operators (unverified)**. Known production users: Status, and Safe's Harbour (Aug 2025).
- **Terms.** The docs.waku.org Terms of Use cover only the *website* (Logos Collective Association, Zug). There are no network ToS, so the protocol is permissionless. The "Rules of Engagement" page invites projects to talk to the team.
- **Privacy.** Relay broadcasts every message to all subscribers of its shard. The content topic is visible to every relay and store node, so group linkage depends on our topic design. Lightpush and store service nodes see the client's IP. RLN proofs don't reveal the member's identity.
- **Library maturity:**
  - `nwaku` (Nim) is `logos-delivery`, MIT OR Apache-2.0, v0.38.1 (2026-05), v0.39.0-rc.2 (2026-09-07).
  - The Rust wrapper `waku-bindings` is MIT OR Apache-2.0. **crates.io max version is 0.6.0 (2024-02-19)**; the repo's Cargo.toml says 1.0.0, unpublished, last commit 2026-02.
  - It wraps the Nim library through FFI, so the build needs the Nim toolchain.
  - `go-waku` (now `logos-delivery-go`, Apache-2.0) was last pushed 2026-08.
  - An Android example and `build_rln_android.sh` exist in `logos-delivery`.
  - **Binary size unknown (unverified).** It adds a second full libp2p stack and zerokit next to iroh.
- **Android:** gossipsub relay isn't viable in the background. Light-client mode (lightpush + store queries) fits WorkManager runs, but its RLN story isn't finished.
- **Abuse resistance:** strong by design (RLN), which is exactly what makes it cost money or need a wallet.
- **Failure modes:** RLN registration friction or cost; retention shorter than a Doze night plus weekend; testnet contracts being reset; dependence on Status fleets for bootstrap (`enrtree://...@sandbox.waku.nodes.status.im`).

**Verdict: REJECT for G1. Confidence: high.** RLN on an Ethereum L2 breaks C1/C3/C4 (a wallet or deposit for each user), or C2 if we paid for memberships. Retention of 12 h to 2 days is too short for an absent-member mailbox. If Logos ever runs free, proof-less store nodes for third parties, revisit it as a G5 signaling channel, not as a mailbox.

---

## 2. Session network (Session Technology Foundation; Session Nodes, formerly Oxen service nodes)

**What it is:** ~1,500 staked nodes ("1,500+ Session Nodes in over 50 countries"). Each account's messages are replicated across a *swarm* of nodes chosen by the network. Clients call `/storage_rpc/v1` on a swarm node directly over HTTPS, or wrap it in an onion request (`/onion_req/v2`).

**Gaps covered:** G1 (strong technical fit). G5 partial: polling only, since the push notification server is STF-run and tied to Session's own app. G6 partial: 75 KiB per message; the STF file server isn't evaluated here and is centralized anyway. Not G2, G3 or G4.

**Evidence (from `session-storage-server` source, MIT, pushed 2026-09-21):**
- **Store limits.** `MAX_MESSAGE_BODY = 76'800` bytes. `TTL_MAXIMUM = 14 * 24h` for public-inbox namespaces, `TTL_MAXIMUM_PRIVATE = 30 * 24h` for all others, `TTL_MINIMUM = 10s`. Retrieval returns up to 7.8 MB per request. The per-node database cap is `SIZE_LIMIT = 10 GiB`, and I found **no per-account quota** in the code.
- **Namespaces map onto our slots directly:**
  - "namespaces divisible by 10... allow unauthenticated submission: that is, anyone may deposit messages into them... Authentication is required for retrieval."
  - "non-divisible-by-10 namespaces require authentication for all operations, including storage."
  - **Public outbox** (any negative namespace −(20n+1): −1, −21, −41…): "can store only one single message at a time; storing a new message automatically removes any existing messages", it is "publicly retrievable: anyone, without authentication, can retrieve messages", and it requires "the owner's authentication to store to". It has existed since 2023-11 (commit "Add 'outbox' & unrevocable-retrieval namespaces").
  - That gives about 1,638 public-outbox slots per key, each up to 75 KiB, each replaced atomically, each living up to 30 days, and **renewable with the `expire` call without re-uploading**.
- **Subaccounts:** a 36-byte token with permission bits (read/write/delete), signed by the main key. Group members could get write tokens for a shared slot key.
- **Rate limit:** a token bucket per client IP, `BUCKET_SIZE = 600` and `TOKEN_RATE = 300` requests/s. That's far above Nostr's ~8/min. Timestamps must be close to the node's clock.
- **Terms of service** (getsession.org, updated 2024-10-15): they govern "the Session Website" and "the Session App". They say **nothing about third-party clients or other apps using the network**. Acceptable use prohibits "bulk messaging, auto-messaging, and auto-dialing" and accounts created "through unauthorized or automated means". Session "Account IDs" aren't accounts in the server sense, but an operator could read our per-device keys as automated account creation. **This is a grey zone, not a permission.**
- **Operators and sustainability:**
  - Each node stakes **25,000 SESH**; an operator must put in at least 25% (6,250 SESH), with up to 10 contributors.
  - Rewards come from a staking reward pool on Arbitrum One. "Bad behavior (such as failing to route traffic or store a message) results in lost rewards."
  - STF sells **Session Pro** ("Increased file transfer limits"...), because it "would not be sustainable for the network to offer these features for free". That shows STF thinks about capacity and pricing: heavy non-Session usage could lead to fees or limits.
  - Sustainability rides on the SESH token's value. Operator concentration: **unverified**.
- **Bootstrap:** seed nodes `seed1/2/3.getsession.org` (STF). Session Android bundles a snode pool at build time (`LocalSnodePoolPlugin.kt`), and the pool can later be refreshed from any snode, which reduces the seed dependency.
- **Precedent for third-party use:** `session.js` (MIT, last pushed 2026-01) is a "JavaScript library for programmatic usage of Session messenger" used for bots and custom clients. I found **no** precedent for a non-messenger app storing its own data in swarms.
- **Libraries:** `libsession-util` (C++) and `libsession-util-android` are **GPL-3.0**, and so are session-android and session-desktop, so linking them would make our app GPL. The server (MIT) documents the RPC in its headers. A small Rust client (HTTPS JSON with Ed25519/X25519 signatures, plus optional onion requests) is feasible but must be written and maintained by us. **No Rust crate exists (checked on GitHub).**
- **Privacy:** swarm nodes see the storing and retrieving key (use per-device-per-group throwaway keys, as in §7), timing and sizes. They see the IP too unless we use onion requests, which cost 3 hops over TCP.
- **Android:** a plain HTTPS request per WorkManager run, no persistent connection. Session's own Android app polls in "slow mode" (R5).
- **Abuse resistance:** per-IP rate limits and staking costs for operators. The public inbox accepts anonymous deposits (spam vector), but public outboxes are owner-signed.
- **New risk found:** the Mini Shai-Hulud npm/PyPI worm (May 2026, OSV `MAL-2026-3608`) uses **`seed1.getsession.org` and `filev2.getsession.org` as C2/exfiltration domains**. Detection tools now list them as IOCs (`shai-hulud-detect` CHANGELOG). Corporate firewalls, EDR and possibly Play Protect heuristics may block or flag an app contacting them. Mitigation: bundle a snode pool and talk to snode IPs directly, never to the seed domains or the file server.

**Verdict: CONSIDER (medium confidence) as the second mailbox for G1.** It is the only candidate whose semantics match our slot model (replace-on-write, owner-signed, publicly readable, 30-day TTL, renewable). Its limits (75 KiB, 300 req/s per IP) are more generous than Nostr's, and it has a large incentivized operator set that doesn't run strfry. Before building:
1. Decide whether to ask STF (outward contact, your call). The ToS is silent.
2. Accept a token-economy dependency.
3. Write our own client instead of using the GPL libraries.
4. Avoid the seed domains flagged as malware IOCs.

---

## 3. SimpleX SMP preset servers (SimpleX Chat Ltd and other preset operators)

**Gaps covered:** G1 technically (queues). Not G2–G6 for us.

**Evidence:**
- **Conditions of Use** (updated 2026-04-18), "3rd party client applications": "You may use a 3rd party application (App) to access preset operators' Infrastructure or systems, provided that this App: is compatible with the protocol specifications not older than 1 year, **provides user-to-user messaging only** or enables automated chat bots..., implements the same limits, rules and restrictions as Software, **requires that the users accept the same Conditions of use** of preset operators' Infrastructure as in Software prior to providing access to this Infrastructure, **displays the notice that it is the App for using SimpleX network**, provides its source code under open-source license... In case the App uses the source code of Software, the App's source code must be provided under AGPLv3 license".
- **Retention:** "irreversibly removed from the preset relay servers **as soon as they are delivered** or after 21 days" (server default `defMsgExpirationDays = 21`). Messages are "padded to a fixed size of 16kb". The file servers keep files for 48 h.
- "SimpleX network operators are not communication service provider, and provide public relays 'as is', as experimental, without any guarantees of availability or data retention."
- **Semantics:** one-way queues, each with one recipient, and messages deleted once delivered. A slot read by several members would need one queue per reader pair, and a snapshot couldn't be re-read. Poor fit.
- **Library:** `simplexmq` is Haskell, **AGPL-3.0** (v7.1.0-beta.3, 2026-09-19). I found no Rust implementation.
- **Operators:** SimpleX Chat Ltd plus other preset operators. The names of the others weren't verified in this session.

**Verdict: REJECT. Confidence: high.** The Conditions exclude a task-sync engine ("user-to-user messaging only") and require SimpleX-branded notices and acceptance screens (C4). Delete-on-delivery semantics don't fit multi-reader slots.

---

## 4. Delta Chat chatmail relays (volunteer-run email servers)

**What it is:** minimal Postfix/Dovecot servers that accept only end-to-end encrypted mail and create an address on first login. The `chatmail/relay` software is MIT; filtermail is MIT (Rust).

**Gaps covered:**
- G1: yes (20-day mailbox).
- G6: partial (messages up to 30 MB, but those over 200 KB are deleted after 7 days).
- G5: partial, on desktop only (IMAP IDLE).
- Not G2 or G4. The relays run `iroh-relay` 0.35, which is wire-incompatible with iroh 1.x (R1), and it's meant for their own users.
- Not G3. Relays push through `https://notifications.delta.chat/notify`, a central proxy for Delta Chat's own app.

**Evidence:**
- **Limits (nine.testrun.org, the default relay):**
  - "You may send up to 60 messages per minute."
  - "You can store up to 700M messages on the server" (reads as 700 MB; the software default is `max_mailbox_size = 500M`, "Oldest messages will be removed automatically").
  - "Messages are unconditionally removed latest 20 days after arriving on the server."
  - Software defaults: `max_message_size = 31457280`, `delete_large_after = 7` (days, for >200 KB), `delete_inactive_users_after = 90`, `username_min/max_length = 9`, `password_min_length = 9`.
  - New account creation is refused under load (`max_load_1m`, memory, disk), or entirely if `/etc/chatmail-nocreate` exists.
- **No signup:** `doveauth.py` "Create[s] chatmail addresses on first login". Any 9-character local part with a ≥9-character password works if the address is free. A group could derive a mailbox address and password from its group key.
- **Encryption enforcement:** filtermail "Rejects messages that do not meet at least one of the following criteria: PGP encrypted, securejoin message, mailer-daemon message, all recipients allow cleartext". We can meet this with genuine OpenPGP messages (`pgp` crate).
- **Terms and positioning:**
  - "provides a low-maintenance, resource efficient and interoperable e-mail service for everyone... optimized for the usage in chats, especially DeltaChat".
  - Operator: "a small voluntary group of devs and sysadmins" (privacy policy: testrun UG i.G., Freiburg).
  - The privacy policy cites "rate limits", "storage limits" and "message size limits" for abuse prevention. **It doesn't restrict which client is used** (it mentions "standard IMAP client tools").
- **Operators:** the chatmail.at list shows **~22–24 public relays** in about 15 countries, mostly individuals or small groups. There's no funding model beyond volunteers.
- **Precedent:** Delta Chat itself syncs app state over chatmail: webxdc apps and multi-device "sync messages". That's app-data sync over these servers, but inside the flagship client.
- **Slot mapping:**
  - Option (a): **one shared mailbox per group** (credentials derived from the group key). Each device writes its slot as a new encrypted message, and the older version is deleted over IMAP. Readers log in and fetch. Operators would see many IPs using one account.
  - Option (b): **one address per device** that sends PGP mail to other members' addresses. That's legitimate "messaging", but it's queue semantics with no replacement.
- **Libraries:** `async-imap` 0.11.3 (MIT/Apache, 2026-07), `lettre` 0.11.23 (MIT, 2026-08), `pgp` 0.20.0 (MIT/Apache, 2026-06). Chatmail core (the Delta Chat engine, Rust) is **MPL-2.0**, v2.62.0 (2026-09-22); we wouldn't need it.
- **Android:** IMAP/SMTP per WorkManager run is fine. IMAP IDLE in the background isn't (no FGS).
- **Privacy:** the relay sees the account, IPs, sizes and timing. Content is PGP-encrypted. Logs are kept "usually 7 days".
- **Abuse resistance:** per-user send limits, quotas, and the ability to stop account creation.
- **Failure modes:** a small operator pool; volunteers can block us with one line (`nocreate`, or blocking the account pattern); the shared-account pattern looks unusual; 20-day hard deletion (below Session's 30).

**Verdict: CONSIDER (low confidence) as a tertiary mailbox, or as a fallback if Session is ruled out.** Technically this is the easiest candidate to build, with standard protocols and permissive crates. But the operator base is tiny and volunteer-run, and it exists for Delta Chat users. Delta Chat's own history (R5: it had to leave free third-party mail providers) shows how this ends when a new app's load arrives. Only viable with a strict per-relay budget, and preferably with operator consent (outward contact, your call).

---

## 5. Freenet (new Rust implementation, "Locutus" lineage)

**What it is:** a small-world P2P network of WASM **contracts** whose state is a commutative monoid, synchronized by summary/delta. Delegates hold private state. Apps talk to a local node over HTTP/WebSocket.

**Gaps covered:** G1 and G5 in principle (contract state plus subscriptions). None in practice on Android.

**Evidence:**
- **Status:** "Freenet is still alpha software and has bugs". "Do not use alpha builds for anything sensitive yet". "Auto-updates: your peer updates as the network evolves; **older versions stop working over time**". "Telemetry: your peer reports diagnostic data". Releases ship every few days (v0.2.137, 2026-09-24).
- **Platforms:** "Freenet runs on a desktop computer... **A mobile app isn't available yet.**" There is an open RFC for a public web proxy (#4381), which admits a public peer "is a trusted, centralized intermediary that can read everything".
- **Limits:** `MAX_STATE_SIZE` is 50 MiB (#5050). Hosting is capped at 1 GiB and "RAM-derived", with state bytes driving eviction (#5414). Persistence depends on peers choosing to host the contract.
- **Bootstrap:** gateway servers run by the project ("nova, vega", #3666).
- **License:** `freenet-core` is AGPL-3.0. Apps link `freenet-stdlib` (LGPL-3.0-only, v0.12.0), or talk to an unmodified node over the network without AGPL obligations (LICENSE.md). Both options carry copyleft considerations.
- **Precedent:** River (group chat) is Freenet's own app.

**Verdict: REJECT for now. Confidence: high.** It's alpha with forced upgrades, has no mobile node, needs a local node, and sends telemetry. **Watch it:** its contract model (mergeable state, summary/delta sync, signed updates) fits a CRDT sync engine better than any mailbox protocol, and a desktop-only integration could become a helper-like layer once it stabilizes.

---

## 6. Briar Mailbox (self-hosted, for comparison)

- A spare Android device "connected to power and Wi-Fi", one per user, reached over **Tor onion services**. It's a REST API with bearer tokens.
- AGPL-3.0-or-later. Last tagged release 1.0.9 (2024-07-09). Briar entered maintenance mode on 2026-07-09, citing "high battery usage and unreliable background operation on Android" (R6).
- **Gaps covered:** G1 only for the owner's own contacts, and only if the user dedicates a device. It isn't a network and has no operators.
- **Verdict: REJECT as a candidate (high confidence).** It confirms DESIGN §7a tier 1 (a member-owned always-on device, "old phone on a charger", R5-P8), which we already have. Using Tor onion services to make a member's home device reachable is a separate question **I did not evaluate** here (it belongs to G2/G4).

---

## Comparison against the gaps

| Candidate | G1 store-and-forward | G2 P2P w/o n0 | G3 wake phone | G4 free relaying | G5 signaling | G6 blobs | Permitted for us? | Cost | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| Waku Network | Weak: needs RLN; 12 h–2 d retention; append log | No | No | No | Possible (pub/sub), RLN-gated | No (150 KB) | Protocol open; Status fleets | Gas/DAI membership | **REJECT** (high) |
| Session swarms | **Yes**: replaceable public outbox, 75 KiB, 30 d, renewable | No | No (STF push) | No | Polling only | Partial (75 KiB/msg) | ToS silent (app-only scope) | $0 (operators paid in SESH) | **CONSIDER** (medium) |
| SimpleX SMP | Queue, deleted on delivery, 16 KB, 21 d | No | No | No | No | 48 h file relays | **No** ("user-to-user messaging only", branding) | $0 | **REJECT** (high) |
| chatmail relays | Yes: 20 d, 500 MB/address | No (iroh 0.35) | No (Delta Chat proxy) | No | IMAP IDLE (desktop) | Partial (≤30 MB, 7 d if >200 KB) | Not forbidden; "optimized for... DeltaChat" | $0 (volunteers) | **CONSIDER** (low) |
| Freenet (Rust) | In principle (contracts) | No | No | No | Subscriptions | 50 MiB states | Yes (open network) | $0 | **REJECT now** (high), watch |
| Briar Mailbox | Own device only | No | No | No | No | No | N/A (self-hosted) | A spare device | **REJECT** (high) |

Slot-semantics fit, from best to worst: Session public outbox ≈ Nostr addressable > chatmail shared mailbox (emulated replace) > Waku store (append log, time queries) > SimpleX (single-reader, delete-on-delivery).

---

## New risks found

1. **The seed domains of the best candidate are malware IOCs.** Since May 2026, `seed1.getsession.org` and `filev2.getsession.org` appear in Mini Shai-Hulud detection lists. An app contacting them may be flagged by enterprise security tools and possibly by store scanners (the Play angle is **unverified**). Mitigation: a bundled snode pool and direct snode IPs.
2. **"Free" networks drift toward paid services.** Waku's RLN mainnet design prices memberships in DAI, and Logos is building "service incentivisation". Session sells Pro because free usage is "not sustainable". Any free third-party layer we pick may start charging or gating. Principle 2 (≥2 providers) must assume layers disappear.
3. **Every candidate is a flagship-app network.** Its operators can protect their app from us with a small change: a namespace restriction, a client-version check, `nocreate`, an RLN requirement. Diversifying across Nostr, Session and chatmail helps only because the three are run by unrelated groups.
4. **License traps in the official client libraries:** libsession-util (GPL-3.0), simplexmq (AGPL-3.0), freenet-core (AGPL-3.0) / freenet-stdlib (LGPL-3.0), Briar Mailbox (AGPL-3.0), chatmail core (MPL-2.0). Any adoption means writing our own client against the server's (MIT) protocol docs.
5. **The mailbox interface must handle different semantics.** Replaceable slots (Nostr, Session), mailbox-with-delete (IMAP) and append logs (Waku) need different write, read and GC strategies. The transport-manager interface in §6 should model "slot store" capabilities (replace, max size, TTL, renew, read-auth) instead of assuming Nostr's.
6. **Shared-credential mailboxes (chatmail) are a new abuse-looking pattern.** Many IPs logging into one account could trip operator heuristics or be treated as account sharing.
7. **Token-economy exposure.** Session's operator incentive depends on SESH's market value. A token crash could shrink the network, as happened with other staked networks (general observation; **no Session-specific data verified**).

---

## Suggested safe local experiments (no public network traffic)

| ID | Experiment | What it answers | How (local only) |
|---|---|---|---|
| L1-X1 | **Session storage-server in a local VM/testnet** | Public-outbox replace semantics, `expire` renewal, subaccount write tokens, TTL enforcement, error codes; the size of a minimal Rust client | Build `session-storage-server` (MIT) in an isolated VM/container with a local stagenet or mocked `oxend`. Point a prototype Rust client (reqwest + ed25519-dalek) at it. Needs your approval to build/install. |
| L1-X2 | **Rust client size budget** | Added APK size of a Session-style HTTPS+signature client vs. the current 7.07 MB core | Compile a stub client into the existing E6 size harness (offline) and compare `.so` sizes |
| L1-X3 | **chatmail relay in a local container** | Whether `pgp`-crate PGP/MIME passes filtermail; IMAP APPEND vs SMTP-to-self; replace-by-delete cost; quota and `delete_large_after` behavior; shared-credential login from several clients | `cmdeploy` against a local VM with a fake domain, or run filtermail's black-box tests locally |
| L1-X4 | **Waku without RLN on a private cluster** | Store append-log read cost for slot emulation; libwaku arm64 size; light-client battery on a real phone against a LAN node | Two local nwaku nodes in Docker, a local Anvil chain for RLN; phone on LAN only |
| L1-X5 | **Freenet slot contract in `fdev` simulation** | Whether a per-group contract with per-device slot deltas maps cleanly; state-size and eviction behavior | Freenet's local network simulator; no public gateways |
| L1-X6 | **Mailbox capability abstraction (paper + unit tests)** | Can one engine interface serve Nostr, Session outbox and IMAP without leaking semantics? | Mock backends in the engine test suite; property tests for GC/replace/renew across heterogeneous stores |

Decisions only you can make (outward contact): asking STF whether a non-Session app may use public-outbox namespaces at scale, and asking the chatmail operators (nine.testrun.org first) about a task-sync client.

---

## Sources

**Waku / Logos Messaging**
- 64/WAKU2-NETWORK spec: https://github.com/logos-co/logos-lips/blob/master/docs/messaging/core/draft/64/network.md
- RLN contract spec (mainnet parameters): https://github.com/logos-co/logos-lips/blob/master/docs/messaging/core/raw/rln-contract.md
- Lightpush spec: https://github.com/logos-co/logos-lips/blob/master/docs/messaging/core/raw/lightpush.md
- Store spec: https://github.com/logos-co/logos-lips/blob/master/docs/messaging/core/draft/13/store.md
- nwaku default retention (`store-message-retention-policy`): https://github.com/logos-messaging/logos-delivery/blob/master/tools/confutils/cli_args.nim
- Fleets: https://github.com/logos-messaging/logos-delivery/blob/master/docs/contributors/waku-fleets.md
- Releases: https://github.com/logos-messaging/logos-delivery/releases
- Docs FAQ / run-node / terms / rules of engagement: https://github.com/logos-messaging/docs.waku.org/tree/main/docs
- Rust bindings: https://github.com/logos-messaging/logos-delivery-rust-bindings ; crates.io: https://crates.io/crates/waku-bindings
- Blog: https://blog.waku.org/logos-messaging-monthly-update-november-2025/ , https://blog.waku.org/waku-monthly-update-october-2025/ , https://blog.waku.org/waku-monthly-update-august-2025/

**Session**
- Storage server (store/retrieve RPC docs, limits): https://github.com/session-foundation/session-storage-server/blob/dev/oxenss/rpc/client_rpc_endpoints.h
- Namespaces (public outbox): https://github.com/session-foundation/session-storage-server/blob/dev/oxenss/common/namespace.h
- TTL constants: https://github.com/session-foundation/session-storage-server/blob/dev/oxenss/rpc/request_handler.h ; handler: .../oxenss/rpc/request_handler.cpp
- Rate limiter: .../oxenss/rpc/rate_limiter.h ; DB size: .../oxenss/storage/database.hpp
- Terms of Service: https://getsession.org/terms-of-service
- Network docs: https://docs.getsession.org/session-network.md , https://docs.getsession.org/session-network/session-nodes/staking-and-collateralization.md , https://docs.getsession.org/session-network/session-nodes/swarms.md , https://docs.getsession.org/session-network/session-nodes/incentivization.md , https://docs.getsession.org/session-network/session-protocol/onion-requests-and-message-routing.md , https://docs.getsession.org/readme/advanced-features/session-pro.md
- Snode pool bundling: https://github.com/session-foundation/session-android/blob/dev/build-logic/src/main/kotlin/LocalSnodePoolPlugin.kt
- Licenses: https://github.com/session-foundation/libsession-util , https://github.com/session-foundation/libsession-util-android
- Third-party client: https://github.com/sessionjs/client
- Malware IOC: https://github.com/Cobenian/shai-hulud-detect/blob/main/CHANGELOG.md ; https://github.com/ossf/malicious-packages/blob/main/osv/malicious/pypi/mistralai/MAL-2026-3608.json

**SimpleX**
- Privacy Policy and Conditions of Use (updated 2026-04-18): https://simplex.chat/privacy/
- Server defaults (`defMsgExpirationDays = 21`): https://github.com/simplex-chat/simplexmq/blob/stable/src/Simplex/Messaging/Server/Env/STM.hs
- simplexmq (AGPL-3.0): https://github.com/simplex-chat/simplexmq

**chatmail**
- Relay info and privacy: https://nine.testrun.org/info.html , https://nine.testrun.org/privacy.html
- Relay list: https://chatmail.at/relays
- Default config: https://github.com/chatmail/relay/blob/main/chatmaild/src/chatmaild/ini/chatmail.ini.f
- Account creation: https://github.com/chatmail/relay/blob/main/chatmaild/src/chatmaild/doveauth.py , .../newemail.py
- Push proxy: https://github.com/chatmail/relay/blob/main/chatmaild/src/chatmaild/notifier.py
- filtermail: https://github.com/chatmail/filtermail
- chatmail core (MPL-2.0): https://github.com/chatmail/core
- Crates: https://crates.io/crates/async-imap , https://crates.io/crates/lettre , https://crates.io/crates/pgp

**Freenet**
- Repo, releases, licensing: https://github.com/freenet/freenet-core , https://github.com/freenet/freenet-core/blob/main/LICENSE.md
- Quickstart (alpha, desktop-only, telemetry): https://freenet.org/quickstart/ ; FAQ: https://freenet.org/about/faq/
- Issues: https://github.com/freenet/freenet-core/issues/5050 , /5414 , /4381 , /3666
- freenet-stdlib: https://crates.io/crates/freenet-stdlib

**Briar Mailbox**
- https://code.briarproject.org/briar/briar-mailbox (README, API.md, LICENSE → AGPL-3.0-or-later; tag release-1.0.9, 2024-07-09)
- Briar maintenance mode: https://briarproject.org/news/2026-maintenance-mode/ (via R6)

**Internal:** DESIGN.md §6–8; DECISIONS.md D10–D23; R1 (iroh 0.35 on chatmail), R5 (prior art: Delta Chat, Briar Mailbox, Session push), R6 (Briar maintenance mode).

---

## Independent verification

*2026-09-24. A second pass by a separate verifier, trying to refute the verdicts above. Sources: upstream source code fetched with `gh api` (including the unreleased `dev` branch and the latest release tag), crates.io API, operator pages via WebFetch. No traffic to any snode, relay, mail server or peer network.*

### Verdict checks

| Candidate | Original | Result | Corrected |
|---|---|---|---|
| Session storage swarms | CONSIDER (medium) | **Upheld as CONSIDER, confidence lowered** | CONSIDER (**low**) |
| chatmail relays | CONSIDER (low) | Upheld | CONSIDER (low) |
| Waku Network | REJECT (high) | Upheld | REJECT (high) |
| SimpleX preset servers | REJECT (high) | Upheld | REJECT (high) |
| Freenet (Rust) | REJECT now (high) | Upheld | REJECT now (high), watch |

**Session: what holds.** I confirmed every number from the source myself:
- `MAX_MESSAGE_BODY` is 76,800 bytes.
- `TTL_MAXIMUM` is 14 days for public inboxes (namespaces divisible by 10). `TTL_MAXIMUM_PRIVATE` is 30 days for all other namespaces, including the public outbox.
- The rate limiter's `BUCKET_SIZE` is 600 and `TOKEN_RATE` is 300 requests/s per client IP.
- `SIZE_LIMIT` is 10 GiB per node. When the database is full, stores fail with `StoreResult::Full`. Old data is not evicted.
- The public outbox is namespace −(20n+1). Only the owner can write to it, and anyone can read it without auth.
- `user_pubkey::load` accepts **any** network-prefix byte followed by a 32-byte key. Signatures are checked with the raw Ed25519 key for every prefix except `05`. So throwaway Ed25519 keys with a non-Session prefix work.
- Staking is 25,000 SESH per node, the operator's minimum is 6,250, and "1,500+ Session Nodes in over 50 countries" is confirmed on docs.getsession.org.
- The ToS is dated 15/10/2024. It covers "services made available by the Session Technology Foundation... via the Session website... and in connection with the Session Private Messenger applications". It has no clause on third-party clients or on other software using the network.
- The IOC is real: shai-hulud-detect CHANGELOG [3.0.9] (2026-05-12) lists `filev2.getsession.org` and `seed1.getsession.org` as C2 domains.

**Session: what the report missed. These lower confidence to low.**
1. **The public outbox is a test-only feature on the live network today.** The latest *release* is v2.11.2 (2025-12-11). All recent work is on unreleased `dev`. Commit `5fb99ba` (2026-09-18) says: "Older versions applied that rule [one message, newest wins] only to a direct store, **not to messages pushed by peers, so an outbox can hold several rows**". It also says the upgrade used to delete every public-outbox message "on the assertion that the namespaces were **only used for testing -- true today**". So:
   - On deployed nodes, replace-on-write is not guaranteed across a swarm.
   - Readers can get several versions back and must pick the newest themselves.
   - No production client exercises the path yet.

   The semantics we'd rely on arrive only in a release that hasn't shipped.
2. **We don't need the public outbox.** Ordinary authenticated namespaces (not divisible by 10, positive, above 30; −30…30 is reserved for Session) have the same 30-day TTL, use the store/delete code path that Session itself uses in production, and support explicit `delete`. If every group member holds the slot key (derived from the group secret), or a read subaccount token, they can read those slots. This drops the dependency on the test-only outbox but costs replace atomicity: write the new version, then delete the old one.
3. **The subaccount detail is wrong in the report.** Storing to a public outbox needs a subaccount token with **Write and Delete** bits, because a store implicitly deletes (namespace.h and `request_handler.cpp`). A plain write token is rejected.
4. **The rate limit may tighten.** The source comment on `TOKEN_RATE = 300` reads `// Too much for a client??`. Treat 300 req/s as an upper bound that could be cut, not as a budget.
5. **Clock and TLS requirements.**
   - Stores are rejected if the timestamp is more than 10 s in the future (`STORE_TOLERANCE`).
   - Signed requests must be within 60 s of the node's clock (`SIGNATURE_TOLERANCE`).
   - Phones with a skewed clock must first read the node's time from the `info` endpoint.
   - Snodes serve self-generated certificates (recent commits: "Use an ECDSA certificate for the https listener", generated with gnutls). A Rust client can't use WebPKI validation. It must pin by snode key or accept unverified TLS. Payloads are signed and encrypted, but a MITM could hide data by answering "empty".
6. **The IOC risk is wider than two domains.** The malware used the Session *network* as an exfiltration channel. Security vendors could start flagging snode traffic in general, not only the seed domains, and STF could add restrictions for non-Session keys in response. Neither is verified. Bundling a snode pool avoids the domains but not these two risks.
7. **The software is moving.** `dev` is mid-migration to QUIC between nodes, removed AES-CBC onion encryption (2026-09-17) and swapped HTTPS backends. Session Router (formerly Lokinet, GPL-3.0) is becoming the privacy transport. A hand-written client must follow protocol changes that STF makes for its own app, and nobody promises other clients stability.

**chatmail: upheld.** Confirmed against the source:
- The `chatmail.ini.f` defaults are `max_user_send_per_minute = 60`, `max_mailbox_size = 500M`, `max_message_size = 31457280`, `delete_mails_after = 20`, `delete_large_after = 7`, `delete_inactive_users_after = 90`, and a username of **exactly** 9 characters (min = max = 9).
- There is also a **burst limit of 10** (`max_user_send_burst_size`). A single IP may use at most a fifth of `max_smtp_connections` (1000).
- The `doveauth` docstring reads "Create chatmail addresses on first login". The nocreate file and load, memory and disk thresholds refuse **new** addresses only; existing addresses keep working.
- nine.testrun.org says "up to 700M", "removed latest 20 days after arriving... **Earlier, if storage may exceed otherwise**", and is run by "a small voluntary group".
- chatmail.at/relays lists **24** relays.
- The crate versions and licenses match the report: async-imap 0.11.3 (MIT/Apache), lettre 0.11.23 (MIT), pgp 0.20.0 (MIT/Apache).

One addition: the Dovecot config I read shows no restriction on IMAP APPEND into one's own mailbox, and filtermail filters SMTP only. So a shared group mailbox could be written with APPEND alone, without a PGP/MIME envelope. That is simpler to build, but it goes further from how operators expect the relay to be used. **Unverified in a live deployment.**

**Waku, SimpleX, Freenet: upheld.** Waku's 64/WAKU2-NETWORK spec has both "The store SHOULD be configured to retain at least `12` hours" and "Store service nodes SHOULD only store messages with a valid `rate_limit_proof`". Both are SHOULD, not MUST, but the Status fleets follow them. waku-bindings is 0.6.0 on crates.io (2024-02-19). SimpleX's Conditions (updated 2026-04-18) include "provides user-to-user messaging only or enables automated chat bots" and "displays the notice that it is the App for using SimpleX network". Freenet's quickstart confirms "A mobile app isn't available yet", the telemetry and "older versions stop working over time". freenet-stdlib is 0.12.0, LGPL-3.0-only (2026-09-23).

### Candidates the report did not evaluate

- **XMPP PEP/PubSub on public servers with in-band registration.** This is the closest semantic match to our slots after the Session outbox:
  - A persistent PubSub/PEP node with `max_items=1` is a replaceable slot, and an `open` access model lets anyone read it.
  - providers.xmpp.net lists about 15 free "Category A" servers that allow registration from the app.
  - Blockers:
    - Registration is automated account creation, the same C3 grey zone as chatmail.
    - The Rust stack (`tokio-xmpp` 6.0.0, `xmpp-parsers` 0.23.0) is **MPL-2.0**, which violates principle 9 as written.
    - Per-server PEP quotas, persistence settings and bot policies are unverified.
  - Worth a short follow-up; my estimate is CONSIDER (low) at best.
- **Reticulum / LXMF propagation nodes.** They are store-and-forward by design. REJECT (medium confidence):
  - The "Reticulum License" is MIT-like but adds field-of-use restrictions, including a ban on use in AI/ML training datasets. That makes it non-OSI and incompatible with principle 9.
  - The only Rust crate, `reticulum`, is 0.1.0 (2025-10).
  - The public propagation-node pool is hobbyist and small (unverified count).
- **Bitmessage.** It is a permissionless store-everything network with proof-of-work and TTLs of days to weeks. PyBitmessage is still maintained (pushed 2026-05), but the network is small and has no Rust or mobile implementation. REJECT (low confidence, not deeply checked).
- ntfy.sh, Mainline DHT BEP44, Hyperswarm, Tor and Veilid are covered in L4, L6 and L7, so I didn't repeat them.

### Net effect on the recommendation

Session is still the best-fitting second mailbox, but it's a weaker bet than the report says. The feature that made it look like "Nostr addressable events" is test-only on the live network. The fallback, authenticated private namespaces with group-shared keys, works with released code but loses atomic replace. Experiment L1-X1 should run **both** v2.11.2 (released) and `dev` storage servers, and test the replicated-outbox bug directly. Nothing here changes the finding that none of these networks covers G2–G4.
