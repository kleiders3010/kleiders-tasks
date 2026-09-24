# R2: Nostr relays (and Blossom) as the offline mailbox

Date: 2026-09-24. Scope: DESIGN.md §5 (invite codes), §7 (Nostr mailbox), §4 (Blossom blobs), §9 (metadata), §15 (open questions).

Method: read the current NIP texts and their git history, read relay source code (strfry, nostr-rs-relay, khatru, noteguard, blossom-server), fetched NIP-11 info documents from 51 relay hosts (40 returned a parseable document) (one HTTPS GET each with `Accept: application/nostr+json`; no websockets opened, no events sent), and read issue trackers and server policy pages. No software installed and nothing published. The session's web-search budget ran out partway through. After that I used direct fetches of known URLs, so a few relay-specific facts (production configs of damus/primal/nos.lol, Primal's Blossom limits) remain **unverified** and are listed as experiments.

---

## Summary

1. **The mailbox basically works on today's big free relays, but not for the reasons the design gives, and under much tighter rate limits than it assumes.** relay.damus.io, nos.lol and relay.primal.net are open strfry relays with no pubkey, payment, PoW or WoT gate on writes. White Noise (Marmot) ships with exactly these three as defaults and publishes opaque kind-445 events signed by a **fresh key per event**. That precedent strongly suggests fresh keys and opaque content are accepted today.
2. **Rate limits are the problem nobody in the design has budgeted for (HIGH).** Damus's own filter plugin (noteguard) ships with `posts_per_minute = 8` per **IP**, and after 10 consecutive rejections it bans that IP for **1 hour**, optionally at Cloudflare's edge, which blocks reads too. relay.damus.io is observed returning exactly this ban message to an app that republished a kind-30078 slot on every change (Sept 2026). khatru's "sane defaults" allow **2 events per 3 min per IP (burst 10)**. The design's "rewrite the slot every few seconds", "flush all groups on backgrounding" and "chunked snapshots" can each exceed these limits by themselves. On mobile carrier NAT (CGNAT) the per-IP budget is shared with strangers.
3. **Do not use NIP-78 (kind 30078/78). This changed three weeks ago (HIGH).** On 2026-09-03 NIP-78 was amended: relays **SHOULD require NIP-42 AUTH and serve 30078/78 only to the author's own pubkey**. The stated motivation was apps "using kind 30078 incorrectly as a generic interchange format… Adding AUTH would rightfully bite them." Ditto relay (relay.ditto.pub, relay.mostr.pub) already enforces it. A group mailbox read by *other* members' keys would break on any relay that follows the change. Use a dedicated kind.
4. **Addressable replacement is reliable on strfry and khatru, not uniform elsewhere.** strfry and khatru implement NIP-01 exactly: newest `created_at` wins, and on a tie the lowest id wins. nostr-rs-relay keeps the **first-seen** event on a tie. Future-timestamp limits range from **30 s** (relay.nostrcheck.me) through 300 s (nostr.wine) and 900 s (strfry default) to 1800 s (nostr-rs-relay). A device whose clock ran fast and was then corrected is **locked out of its own slot** ("replaced: have newer event") until real time catches up.
5. **Size: plan for ≤ 64 KiB of event JSON, i.e. ~47 KiB of binary per event after base64.** That is strfry's default `maxEventSize = 65536`. relay.nostr.build states "64 KB, matches what most relays accept". nostr.land allows 65,535; relay.nostrcheck.me allows only 8,196 content chars. **strfry's NIP-11 `max_message_length` is the websocket frame size, not the event size**, so damus's and primal's "1,000,000" does not mean 1 MB events. Size must be probed.
6. **NIP-40 expiry is honored by strfry and nostr-rs-relay. It is not honored everywhere.** strfry rejects already-expired events and purges expired ones every ~9 s. relay.snort.social is now an **in-memory** relay (memlay) that doesn't advertise NIP-40. The spec itself says relays "MAY persist them indefinitely." **NIP-09:** strfry and khatru delete by `e` and `a` and block re-publication of older versions. nostr-rs-relay only *hides* `e`-referenced events and ignores `a`.
7. **Retention: there is no published data, and the cited study doesn't measure it.** The ACM paper (Wei & Tyson, arXiv 2402.05709, CoNEXT'25 / PACMNET 2025; data Jul–Dec 2023, 712 relays) measures *availability* only. Its numbers: 132 relays (18.5%) dead within 6 months, ~20% of relays down >40% of the time, and "90% up >80%" **only after excluding dead relays**. NIP-11's retention field was **deleted from the spec in Feb 2026**, and none of the 40 NIP-11 documents I parsed publishes one, apart from a single paid inbox relay. Churn is real: relay.nostr.band, once a major relay, refused connections on 2026-09-24.
8. **Per-group throwaway keys don't hide anything from the relay operator.** A device's groups are linkable through its IP, its single websocket (NIP-01 says to use one per relay), combined REQ filters, multi-pubkey NIP-42 AUTH and simultaneous flush timing. They also *invite* the anti-spam response the spec now recommends for random-key events: AUTH with a real identity (NIP-59/NIP-17). §9's privacy wording should be corrected.
9. **Blossom for encrypted blobs: weak.** The largest free server (nostr.build, also serving blossom.band) takes **media types only** on the free tier (20 MiB). 24242.io takes anything up to 100 MiB but deletes blobs not accessed for 60 days, "for now". The reference server's default rules keep non-image/video blobs **1 week after last access**. Every upload needs a BUD-11 signed token. Fine for v2 images as a best-effort extra; not a storage layer.
10. **Relay operators oppose "relay as generic storage", openly and in shipped code.** Damus's noteguard config blocks file kinds with `"blocked: files on nostr is dumb"`. NIP-95 (files on relays) was closed with "better offloaded to blossom". NIP-37 sends private data to AUTH'd private relays. strfry began AUTH-gating kinds 4/1059 by default in Aug 2026. The app's traffic is identifiable by its kind number and blockable with one config line.

---

## Claim-by-claim table

| # | Claim (design) | Verdict | Severity if wrong | One-line evidence |
|---|---|---|---|---|
| 1 | Big free relays accept new pubkeys, custom kinds, addressable kinds, opaque content | **Partial** | High | Open strfry on damus/nos.lol/primal; White Noise sends fresh-key opaque events there. But per-IP rate limits with bans, kind deny-lists, AUTH trend, and many paid/WoT/AUTH relays |
| 2 | Addressable events reliably replaced per (pubkey, kind, d) | **Partial** | Medium | strfry and khatru follow NIP-01 incl. tie-break; nostr-rs-relay first-seen on ties; future-skew limits 30–1800 s; lockout after clock correction |
| 3 | Snapshot "split into chunks under typical relay size limits"; typical limits | **Partial** (limits confirmed; chunking cost underestimated) | High | 64 KiB JSON default (~47 KiB binary); rate limits turn a 20-chunk snapshot into a ban risk |
| 4 | NIP-40 honored ("relays drop expired invites"); NIP-09 behavior | **Partial** | Low | strfry and nostr-rs-relay purge; memlay/others don't; spec allows indefinite keep; NIP-09 `a` unsupported in nostr-rs-relay |
| 5 | Reliability numbers from "ACM study, 2023, 712 relays"; retention "mostly irrelevant" | **Partial** | Medium | Uptime numbers quoted correctly but only after excluding 18.5% dead relays; the paper doesn't measure retention; no relay publishes retention |
| 6 | Relays tolerate app data / encrypted non-social storage (implicit) | **Partial → trending refuted** | High | NIP-78 now AUTH/owner-only; damus blocks file kinds; app banned for 30078 republish spam; precedents that work use dedicated kinds |
| 7 | Blossom public servers accept encrypted blobs for offline delivery | **Partial** | Low (v1) / Medium (when images ship) | nostr.build free = media only; 24242.io 60-day idle expiry; reference default 1 week idle; auth required |
| 8 | Per-device-per-group keys: groups can't be linked; won't trigger spam filters | **Refuted** (linkability vs. relay) / **Partial** (spam) | Medium | IP/connection/REQ/AUTH/timing link groups; NIP-59/17 steer relays to AUTH-with-real-identity for random keys; WoT relays reject |

---

## Details per claim

### Claim 1: Acceptance of new pubkeys, custom and addressable kinds, opaque content

**What supports it**
- relay.damus.io (strfry 1.1.0-158), nos.lol (strfry 1.1.3) and relay.primal.net (strfry 1.0.3) declare no `auth_required`, `payment_required`, `restricted_writes` or `min_pow_difficulty` in NIP-11. The strfry default config has no pubkey or kind restrictions and applies `allowedKinds = ""` (all).
- **Strongest precedent:** White Noise (Marmot protocol) defaults to exactly `wss://relay.damus.io`, `wss://relay.primal.net` and `wss://nos.lol` in release builds (`src/whitenoise/relays.rs`). Marmot's Nostr transport requires "The event `pubkey` MUST be a fresh ephemeral Nostr public key generated for that event", with base64 AEAD ciphertext as content (kind 445). A shipping messenger depends on these relays accepting never-seen keys with opaque content.
- NIP-59 gift wraps (kind 1059, random one-time keys) and NIP-60 Cashu wallet events (encrypted `kind:7375`) are more precedents for opaque content from keys with no social graph.

**What limits it (relays that restrict, from NIP-11 fetched 2026-09-24 and source)**

| Restriction | Relays (evidence) |
|---|---|
| Payment | nostr.wine, nostr.land / eden.nostr.land (NFDB), relay.azzamo.net, nostr21.com, theforest.nostr1.com, relay.nostrplebs.com, filter.nostr.wine, inbox.nostr.wine, relay.nostr.build ("Publishing stops when your subscription expires") |
| NIP-42 AUTH required for everything | filter.nostr.wine, inbox.nostr.wine, auth.nostr1.com (free DM inbox) |
| AUTH for specific kinds | relay.damus.io: reads of kinds 4/1059 refused, and AUTH can't succeed because `serviceUrl` is empty (divine-mobile #9467, 2026-09-23). Ditto relay (relay.ditto.pub, relay.mostr.pub): AUTH for 4, 1059, 78, 30078 (NIPs #2452/#2458). strfry default since Aug 2026: `restrictedReadKinds = "4, 1059"` |
| Web of trust | wot.utxo.one (bitvora wot-relay, `MINIMUM_FOLLOWERS=3`), filter.nostr.wine (follows-based filter, "Purgatory" for new users), haven-based personal relays (relay.utxo.one) |
| Kind deny-lists | noteguard (damus-io) reference config blocks 30065/1064 ("blocked: files on nostr is dumb") and 34550/4550. nostr-rs-relay has `event_kind_allowlist`/`blacklist` options |
| Special purpose (effectively restricted) | purplepag.es (profiles and relay lists), relay.getalby.com (NWC), relay.fountain.fm (podcast NIPs only), relay.nostr.watch ("Stores nostr.watch events"), nostr-01.yakihonne.com (long-form) |
| Content/spam filters | nos.lol ("Generally accepts notes, except spammy ones"), nostr.mom ("Spam and nudity filtering. Stricter and more experimental than nos.lol"). The public classifier plugin nspam-strfry only gates `kind:1`, and other kinds bypass it |
| Tiny limits | relay.nostrcheck.me: `max_content_length` 8,196, `created_at_upper_limit` 30 s, `max_subscriptions` 5 |
| No persistence | relay.snort.social: "High-performance in-memory Nostr relay" (memlay), NIPs 1/9/11 only |
| PoW | **None** of the 40 parsed NIP-11 documents declares `min_pow_difficulty` > 0 |
| Down or unreachable on 2026-09-24 | relay.nostr.band (ECONNREFUSED, twice), relay.nostr.bg, relay.0xchat.com, nostr.fmt.wiz.biz, relay.current.fyi, nostr.lu.ke, relay.exit.pub, nostr.polonkai.eu (timeout); relay.nostr.net (HTTP 500); relay.lumina.rocks, relay.bitcoinpark.com (502) |

**Rate limits (the real acceptance constraint)**
- noteguard (`damus-io/noteguard`, `noteguard.toml`): `posts_per_minute = 8` per IP. `ban_after = 10` consecutive rejections leads to `ban_duration_secs = 3600` with `ban_message = "banned: too many rate-limit violations, try again later"`. An optional Cloudflare WAF edge ban drops the IP before it reaches the relay, which also kills reads. Content-filter violations are **stealth shadow-rejected**.
- Production evidence: `ssotomayor/discordia#198` (2026-09-22). relay.damus.io answered a kind-30078 "XP ledger" republished on every change with `"rejected event … (banned: too many rate-limit violations, try again later)"`, the exact noteguard string. It affected all events from that client, including DMs and kind 0. (The 8/min value is the repo's reference config; damus's production value is unverified.)
- khatru `ApplySaneDefaults`: `EventIPRateLimiter(2, 3*time.Minute, 10)` gives 10-event burst, then one event per 90 s per IP. `ConnectionRateLimiter(1, 5*time.Minute, 100)`.
- nostr-rs-relay: `messages_per_sec` (operator-set).

**Verdict: partial.** New keys, custom kinds and opaque content are accepted on the big open strfry relays. Write rate per IP is tightly limited and punished with bans. A growing set of relays gate by payment, AUTH or WoT.

**Fix**
- Put a hard **per-relay client-side write budget** in the engine, shared across *all* groups on the device. Suggested starting point: ≤ 6 events/min sustained and a burst of 5 per relay. Tune after E4.
- On the first `rate-limited:` reply, stop writing to that relay and back off exponentially (≥ 60 s, doubling). Never retry into a rate limit, because ~10 consecutive rejections triggers a 1-hour IP ban.
- Treat `banned:`/`blocked:` as "relay unavailable for ≥ 1 h" in health tracking.

### Claim 2: Addressable replacement per (pubkey, kind, d)

**Spec (NIP-01, current):** "only the latest event MUST be stored by relays, older versions MAY be discarded… In case of replaceable events with the same timestamp, the event with the lowest id (first in lexical order) should be retained." NIP-01 sets no `created_at` bounds.

**Implementations**
- **strfry** (`src/events.cpp`): `isEventABeforeEventB` = older `created_at`, or equal and *greater* id. That is NIP-01 exact. An older event is refused with `OK false "replaced: have newer event"`. It also refuses re-publication of an address after a NIP-09 `a`-tag deletion with a newer timestamp. Defaults: reject `created_at` > now+900 s (`invalid: created_at too late`) and < now−3 years.
- **khatru** (`utils.go`): `isOlder` = same NIP-01 rule. It blocks re-publication after an `a` deletion ("blocked: this event has been deleted").
- **nostr-rs-relay** (`src/repo/sqlite.rs`): skips the insert if any stored event for the address has `created_at >= new`, so **first-seen wins on ties** (deviates from NIP-01). Default `reject_future_seconds = 1800`.
- Observed `created_at_upper_limit`: relay.nostrcheck.me **30 s**, nostr.wine / inbox.nostr.wine 300 s, nostr21 / theforest / auth.nostr1.com 900 s.
- "Older versions MAY be discarded": archival relays (e.g. relay.nostr.build archives rather than deletes) may keep and return old versions.

**Failure scenarios**
- **Same-second writes:** the design flushes on backgrounding right after a foreground write. Two versions with equal `created_at` resolve to *lowest id* on strfry/khatru (random: the older content can win) and to *first-seen* on nostr-rs-relay. Relays then **disagree** about which version the slot holds.
- **Fast clock, then corrected:** a phone 10 min fast writes its slot at T+600. After NTP correction, each write with `created_at` < T+600 is refused as "replaced: have newer event" for 10 minutes. Beyond +30 s the write is rejected outright on nostrcheck, and beyond +15 min on strfry relays.
- **Design inconsistency:** §7 says "one snapshot slot" per group, but also "signed with a per-device, per-group throwaway key". Addressable events are keyed by pubkey, so there are *N* snapshot slots (one per device key), not one. The bound "1 + number of devices" is really about 2N plus chunks.

**Verdict: partial.** Replacement works on the dominant software, but only if the client guarantees strictly increasing timestamps per slot. The skew tolerance differs per relay.

**Fix**
- Per slot, persist `last_created_at` and publish with `created_at = max(now, last_created_at + 1)`. Never reuse a second.
- Never write a slot more than once per second, and delay the backgrounding flush by up to 1 s if needed.
- Treat `replaced:` as a clock problem: log it and show it in diagnostics. Don't retry.
- On read, always pick the highest (`created_at`, then lowest id) among everything returned. Never assume a relay returns one version.
- Rewrite §7 so snapshot slots are explicitly per writer key (or see the shared-key option under New risk R7).

### Claim 3: Typical limits, and what one event can carry

| Source | Event/message size | Other |
|---|---|---|
| strfry default | `maxEventSize = 65536` (normalized JSON); `maxWebsocketPayloadSize = 131072` | `maxNumTags 2000`, `maxTagValSize 1024`, `maxFilterLimit 500`, `maxSubsPerConnection 200`, `maxTagsPerFilter 3` |
| strfry NIP-11 output | **reports `max_message_length` = websocket payload size**, not event size (`RelayWebsocket.cpp`) | so damus/primal "1,000,000" and yabu.me "1,310,720" say nothing about max event size |
| nostr-rs-relay | `max_event_bytes` 131072 (default, commented) | `reject_future_seconds 1800` |
| relay.nostr.build | "A single event is capped at 64 KB… matches what most relays accept" | paid |
| nostr.land / eden | `max_message_length` 65,535 | paid |
| relay.nostrcheck.me | `max_content_length` 8,196 chars | 5 subs, +30 s |
| nostr21 / theforest / auth.nostr1.com | content 262,140 | |
| nostr.wine | message 524,288 | paid |
| Ditto (relay.ditto.pub, relay.mostr.pub) | 4,000,000 | |
| `max_subscriptions` | **20** on nos.lol, relay.primal.net, offchain.pub, nostr.bitcoiner.social, relay.angor.io; 16 on wirednet; 5 on nostrcheck | per connection |

**Payload math:** content must be a JSON string, so binary is base64 (+33%). 65,536 − ~500 bytes of id/pubkey/sig/tags overhead ≈ 65,000 chars ≈ **47.6 KiB binary per event** on strfry defaults. With margin: **≤ 40 KiB binary per chunk**, or ≤ 6 KiB to be accepted everywhere including nostrcheck-type relays.

**Snapshot volume (from E1-loro.md):** full snapshots are 70 KB (1 year), 211 KB (3 years) and 353 KB (5 years) per busy list, and 445 KB for 10,000 tasks. A group with 5 years of history across several lists is 10–40 chunks. At noteguard's 8/min that is 2–5 minutes of one IP's whole budget *per relay*. At khatru defaults (10 burst, then 1 per 90 s), a 30-chunk snapshot takes ~30 minutes. The Android `shortService` window (§8) is ~3 minutes, so a snapshot republish **cannot finish in one background window** on rate-limited relays. If the client fires it as a burst, it risks the 1-hour ban.

**Verdict: partial.** The limits are real and now documented above. The design's chunking plan ignores that the *count* of events matters more than their size.

**Fix**
- Chunk at ≤ 40 KiB binary.
- Make snapshots rare: only when a bundle exceeds the threshold, never on a timer.
- Rewrite only the chunks that changed: split per list document, and use content-defined chunking so unchanged chunks keep their ids and relays answer `duplicate:`.
- Spread chunk uploads over time within the write budget.
- Put chunk *generation* metadata in every chunk (see New risk R5).
- Cap subscriptions: one REQ with many filters (strfry `maxReqFilterSize 200`) rather than one REQ per group, and ≤ 15 open subs per relay.

### Claim 4: NIP-40 expiration and NIP-09 deletion

- **NIP-40 text:** relays "SHOULD" delete, but "MAY NOT delete expired messages immediately… and MAY persist them indefinitely." "Clients SHOULD NOT send expiration events to relays that do not support this NIP." The warning section says third parties can download them.
- **strfry:** parses `expiration`, rejects already-expired events at ingest (`invalid: event expired`), and a cron job deletes expired events every 9 s (`RelayCron.cpp`). damus, nos.lol and primal advertise NIP-40.
- **nostr-rs-relay:** stores `expires_at`, filters expired from queries, and runs periodic `DELETE … WHERE expires_at <= ?`.
- **Not supported:** relay.snort.social (memlay, NIPs 1/9/11). Archivers and any mirror that ignores NIP-40 keep invites indefinitely.
- **NIP-09:** "Relays SHOULD delete or stop publishing any referenced events", and deletion requests themselves are kept "indefinitely". strfry deletes for `e` and `a` (all versions up to the request's `created_at`) and then blocks older re-publication. khatru blocks re-publication after `a` deletion. nostr-rs-relay only sets `hidden=TRUE` for `e` tags and has **no `a`-tag handling**.

**Verdict: partial.** "Relays drop expired invites" holds for the strfry majority. The design already refuses expired invites client-side and relies on code length rather than expiry, which is correct.

**Fix**
- Keep the in-payload expiry (already designed).
- Send invites only to relays whose NIP-11 lists 40.
- Add NIP-40 `expiration` to *mailbox slots* too (e.g. now + 60–90 days, refreshed on every rewrite), so abandoned groups disappear by themselves. That is a cheap goodwill signal to operators.
- Don't rely on NIP-09. If you use it, remember strfry/khatru then refuse older re-publication of that address.

### Claim 5: Reliability and retention ("ACM measurement study, 2023, 712 relays")

- **Paper:** Yiluo Wei & Gareth Tyson, *An Empirical Analysis of the Nostr Social Network: Decentralization, Availability, and Replication Overhead*. arXiv 2402.05709 v2 (2025-09-29), CoNEXT'25 / Proceedings of the ACM on Networking (DOI 10.1145/3768994). Data: 2023-07-01 to 2023-12-31, 911 relays listed, 712 with data.
- **Quoted numbers:**
  - "Approximately 20% of the relays experience downtime for more than 40% of the time."
  - "132 dead relays", i.e. 18.5% of 712 in 6 months.
  - "90% of the relays being operational for more than 80% of the time" **after excluding dead relays**.
  - "50% … available for more than 99%".
  - "The majority of outages (56%) last for less than 15 minutes"; "10% of the outages lasting for more than 100 minutes".
  - "95% of the free-to-use relays cannot cover their operational cost"; "64% of relays do not receive any zaps".
  - "on average, a post is replicated across 34.6 relays".
- **The paper does not measure retention.** Both v1 and v2 lack any retention, pruning or data-loss analysis. The design's "Retention is weak" is therefore not from this source.
- **No published retention:** NIP-11's "Event Retention" section was removed on 2026-02-15 (PR #1946, "partially remove bloat…"). Of 40 parsed NIP-11 documents, only inbox.nostr.wine (paid) publishes one. Free relays give no retention promise at all.
- **Churn since the paper:** relay.nostr.band is down (ECONNREFUSED). relay.snort.social became in-memory. Several other formerly listed relays are unreachable (table above).

**Verdict: partial.** The uptime figures are quoted correctly but framed optimistically. The design's *architecture* (full local copies, republish, re-upload on miss) is the right answer to unknown retention.

**Fix**
- Correct the §7 citation: 2025 publication with 2023 data; "excluding the 18.5% of relays that died within 6 months".
- Stop citing it for retention.
- Add **runtime relay discovery and replacement** (e.g. NIP-66 relay-monitor events, or a signed relay list the app fetches from a GitHub Pages file) so a dead default relay doesn't wait for an app release.
- Pick defaults for **operator diversity**. nos.lol and nostr.mom appear to be run by the same operator (each description references the other), so they fail together.

### Claim 6: Operator stance on non-social / generic encrypted storage

**Against (dated, primary sources)**
- **NIP-78 amended 2026-09-03 (PR #2458, closes #2452 by Alex Gleason):** "relays SHOULD require clients to perform the NIP-42 AUTH flow before accepting or serving kind 78 and kind 30078 events, and SHOULD only serve these events to the authenticated owner". Also: "These kinds are not meant to be used as a generic interchange format." Issue text: "Some people are using kind 30078 incorrectly as a generic interchange format when they are too lazy to pick a kind, and I have noticed that LLMs even steer people towards this… Adding AUTH would rightfully bite them." fiatjaf: "Good point, let's add this to the NIP." Implemented in Ditto relay (both kinds) and announced for another relay engine (iqbqioza).
- **strfry Aug 2026:** new config `restrictedReadKinds` defaults to `"4, 1059"` with `restrictReadToInvolvedPubkey = true` (commits 2026-08-03 and 2026-08-12). relay.damus.io runs it. The same mechanism can take any kind list.
- **damus noteguard:** kind blocks with `"blocked: files on nostr is dumb"`.
- **NIP-95** (files on relays) closed May 2025, noting it was "better offloaded to blossom".
- **NIP-37:** private drafts go to "Private Storage relays [that] SHOULD be NIP-42-authed and only allow downloads of events signed by the authed user."
- **Precedent of punishment:** discordia's kind-30078 republish-per-change was banned by damus (above).

**For (works today)**
- White Noise / Marmot (kind 445, fresh keys) on damus/primal/nos.lol.
- NIP-60 encrypted wallet events.
- NIP-17/59 gift wraps. NIP-78 still endorses "Personal private data… allow users to use Nostr relays as their personal database", but on relays the user chooses, read by the owner only.

**Verdict: partial, trending toward refuted** for a multi-reader encrypted mailbox on *default public* relays. The design's §7 already names acceptance as "the real long-term risk". The evidence says the squeeze has **already started** at the spec and software-default level, not only "at large scale".

**Fix**
- **Never use 30078/78.** Pick one dedicated kind in 30000–39999 and document it publicly (a short NIP or at least an entry in the NIPs kind list), so operators can recognize it as legitimate low-volume traffic.
- Keep volume tiny (write budget, rare snapshots, NIP-40 on slots).
- Publish an operator-facing page (what the kind is, its expected volume, a contact).
- Treat "bring your own relay" and paid relays as optional user settings, and pull the helper network (§7a) earlier rather than "before scale makes it necessary".
- Don't disguise traffic as another kind. It's bad faith, and 1059/4 reads are already AUTH-gated.

### Claim 7: Blossom servers for encrypted non-media blobs

- **Spec:** BUD-02 `PUT /upload`: "The server MUST NOT modify the blob in any way", which is good for ciphertext. 415 "Unsupported Media Type", 402 payment and 429 rate-limit responses are standardized. BUD-11: every upload carries a kind-24242 token (`t=upload`, `x=<sha256>`, NIP-40 expiration). Blobs are addressed by **SHA-256 of the uploaded bytes**. BUD-05 `/media` *does* transcode, so never use it for ciphertext.
- **Reference server (hzrd149/blossom-server `config.example.yml`):**
  - rules `image/* 1 month`, `video/* 1 week`, `* 1 week` ("How long a blob may go unaccessed before being pruned");
  - `requireAuth: true`, `maxSize` 2 GB;
  - "when this list is non-empty, blobs whose MIME type matches no rule are rejected with 415".
- **nostr.build / blossom.band:** free tier is 20 MiB with "Images, audio and video" only; paid adds ".pdf, .svg and .zip"; "no expiry"; upload auth required; GPS-bearing media rejected (it parses media). Encrypted `application/octet-stream` is not in the free list.
- **24242.io:** "Free and unrestricted… with no warranties… for now", 100 MiB max, "Files are deleted if not accessed for 60 days."
- **blossom.primal.net:** implements BUD-01/02/04/05. Policies aren't published on its page (**unverified**). Primal's app uploads via `/media` (a 2025-07-25 Primal note).
- **Precedent:** NIP-17 kind-15 file messages upload AES-GCM-encrypted files to media servers. I could not verify which servers accept them (search budget exhausted).

**Verdict: partial.** A few servers accept arbitrary blobs with idle-expiry. The biggest free one does not.

**Fix**
- Keep Blossom strictly best-effort (the design already accepts silent failure plus ThumbHash).
- Upload as `application/octet-stream` via `/upload` only.
- Store the **SHA-256 of the ciphertext** in the document next to the BLAKE3 reference (Blossom can't look up by BLAKE3).
- Probe with `HEAD /upload` (BUD-06) before `PUT`, and keep a runtime list of servers that answered 2xx.
- Don't count on it for snapshots.

### Claim 8: Per-group throwaway keys are unlinkable and won't trigger spam filters

**Linkability (refuted as a guarantee against the relay operator)**
- The relay sees the uploader's IP (§9 already says so). All of a device's groups come from one IP.
- NIP-01: "Clients SHOULD open a single websocket connection to each relay and use it for all their subscriptions". Following it puts all groups on one connection. Not following it multiplies connections and runs into per-IP connection limiters (khatru 1 per 5 min, burst 100).
- A REQ that lists several groups' topic tags links them directly. So does multi-pubkey AUTH on one connection (NIP-42: "Clients MAY provide signed events from multiple pubkeys… Relays MUST treat all pubkeys as authenticated").
- "Leaving the app flushes immediately" writes every group's slot in the same second. Anyone scraping relays can correlate groups by recurring identical `created_at` clusters, even without IPs.
- Per-device keys *add* a leak: the number of member devices per group (the §9 table admits this).

**Spam filters (partial)**
- Key-reputation filters can't score fresh keys, so operators fall back to per-IP limits (noteguard, khatru) or AUTH. NIP-59: "Gift wrap events are signed by random one-time keys, so relays cannot rely on pubkey-based anti-spam… a relay may enforce a NIP-42 auth-required policy… clients SHOULD support authenticating to such relays using the user's real identity." NIP-17 repeats this.
- WoT relays reject unknown keys outright.
- The public ML spam plugin (nspam-strfry) only gates `kind:1`. Custom kinds bypass it.

**Verdict:** refuted for linkability against relays; partial for spam.

**Fix**
- Reword §5/§7/§9: "per-group keys stop *other relay users and readers* from linking a device's groups by key. Relay operators can link them by IP, connection and timing."
- Add random per-group jitter (0–30 s) to background flushes and avoid multi-group REQs if that matters. Otherwise accept the risk explicitly.
- Consider the **group-shared Nostr key** option (New risk R7), which removes the device-count leak and makes owner-only-AUTH relays usable.

---

## New risks the design hasn't considered

| ID | Risk | Severity | Fix |
|---|---|---|---|
| R1 | **Per-IP rate limits with bans, amplified by CGNAT.** One active editor at "every few seconds", or one snapshot burst (≥ ~18 events/min to damus), can earn a 1-hour IP ban, possibly edge-wide via Cloudflare. That kills reads and writes for every group and for other users on the same carrier NAT IP | High | Engine-wide per-relay write budget (≤ 6/min, burst 5); slot rewrite ≥ 30–60 s in foreground (live peers use P2P anyway); coalesce and jitter the background flush; stop on the first `rate-limited:` |
| R2 | **NIP-78 AUTH/owner-only rule** (2026-09-03), and the ecosystem moving private kinds behind AUTH (strfry defaults, Ditto, NIP-37/17/59). If the app used 30078, members couldn't read each other's slots; a future default could also cover other "private-looking" kinds | High | Dedicated documented kind; monitor NIPs repo and strfry defaults; see R7 for AUTH-compatible design |
| R3 | **OK=true is not proof of storage.** strfry write-policy `shadowReject` returns `OK true` without storing (`RelayWriter.cpp`), and noteguard content bans are "silently shadow-rejected". Health tracking based on OK counts is fooled | Medium | Health = read-back verification on a later sync (different connection), not OK messages |
| R4 | **NIP-11 lies about event size on strfry** (`max_message_length` = frame size). Adaptive sizing from NIP-11 over-sizes events | Medium | Fixed ≤ 40 KiB binary chunks; on `invalid: event too large`, halve per relay and remember it |
| R5 | **Chunked snapshots are not atomic.** Readers can mix chunk *k* of a new generation with chunk *j* of an old one, and relays may accept some chunks and reject others (rate limits). A Loro snapshot split across generations doesn't decode | Medium-High | Each chunk carries `(generation id, index, count, hash of whole)`; double-buffer d-tags (A/B generations) and only switch the pointer in the bundle slot once every chunk of the new generation has been read back |
| R6 | **Same-second tie divergence and fast-clock lockout** (Claim 2) | Medium | Monotonic per-slot `created_at`; never 2 writes per second |
| R7 | **Design choice not considered: one group-derived Nostr key (per epoch) instead of per-device keys.** Pros: truly one snapshot slot; hides device count; lets the app AUTH as the author on owner-only relays (NIP-78-style, NIP-37 private relays) and use NIP-70 protected events against re-broadcast. Cons: per-pubkey rate limits aggregate the whole group; shared slots race on `created_at` (use per-device d-tags); needs working AUTH (damus's AUTH is currently broken) | Medium (design option) | Prototype both in E4; decide on read-back success rates |
| R8 | **Kind fingerprint and scale.** One kind number identifies all app traffic; one config line (`[filters.kinds]`) blocks it network-wide on noteguard relays. Rough ingress per relay ≈ DAU × writes/day × bytes/write × (relays written ÷ relays in list). E.g. 100k DAU × 30 writes × 20 KB ≈ 60 GB/day per relay. That would make the app a top write source on donation-funded relays long before "large scale" | High (long-term) | Budget writes; NIP-40 on slots; operator outreach; paid/BYO relay options; helper network sooner |
| R9 | **NIP-13 PoW cost scales with event size.** Content follows the nonce tag in the id hash, so every attempt rehashes the whole event. Estimate: at ~1 GB/s SHA-256 on a phone, difficulty 20 costs ~1 min per 48 KB chunk (≈1 s for a 1 KB event). No relay I checked requires PoW today | Low-Medium | Use PoW only for small events; avoid PoW relays for chunks; measure (E5) |
| R10 | **Relay churn outpaces release cadence** (relay.nostr.band dead; snort in-memory; 8+ listed relays unreachable today). Correlated *policy* failure: most defaults run strfry, so one default change (like Aug 2026 restricted kinds) hits all of them at once | Medium | Runtime relay list (signed static file and/or NIP-66); mix software (strfry, khatru, nostr-rs-relay, Ditto) and operators in defaults |
| R11 | **Subscription caps** (20 per connection on nos.lol/primal/offchain; 5 on nostrcheck) versus one REQ per group; combining groups into one REQ links them (Claim 8) | Low-Medium | One REQ with up to N filters; for privacy-sensitive setups rotate connections |
| R12 | **Archival and mirroring.** Events are copied by aggregators and strfry mesh/negentropy sync to relays the app never chose. Invites and slot histories persist beyond NIP-40/NIP-09 (NIP-40 warns about this) | Low (content encrypted) | Already covered by 6-word invites. Optionally NIP-70 protected events where AUTH works |
| R13 | **damus AUTH is non-functional** (`serviceUrl` empty): any flow requiring AUTH (NIP-70 publish, restricted reads) fails there today | Low-Medium | Don't depend on AUTH on damus. Detect `auth-required:` and fall back |
| R14 | **Blossom hash mismatch:** the design references blobs by BLAKE3 (iroh-blobs); Blossom needs SHA-256 of the exact ciphertext | Low | Store both hashes per blob reference |

---

## Suggested experiments (local unless marked)

1. **E4a: local strfry with damus-like noteguard.** Run strfry with the default config plus noteguard `noteguard.toml` (8/min, ban after 10), locally.
   - Measure the max binary per event through base64 and the exact `invalid: event too large` threshold.
   - Test same-second tie behavior, fast-clock lockout (`replaced:`), NIP-40 purge latency, and NIP-09 `a` deletion plus re-publication blocking.
   - Trigger `shadowReject` and confirm OK=true without storage.
   - Replay a realistic day of app writes (N groups, flush-on-background, snapshot chunks) and count rejections and bans.
2. **E4b: local nostr-rs-relay and khatru (`ApplySaneDefaults`).** Same matrix. Confirm first-seen-on-tie in nostr-rs-relay, missing `a`-tag deletion, and khatru's 2/3 min limiter against the app's write budget.
3. **E4c: CGNAT simulation.** Run K engine instances behind one source IP against local noteguard. Find the K at which bans start, for candidate write budgets.
4. **E4d: chunk and generation protocol.** Using E1's Loro snapshots (70–445 KB), measure chunk counts at 40 KiB. Verify the A/B generation switch never yields a torn read under random chunk rejection.
5. **E5: PoW on phones.** Mine difficulty 16/20/24 for 1 KB and 48 KB events with rust-nostr on a low-end arm64 device or emulator. Record time and battery.
6. **E4e: group-shared key vs per-device keys.** Implement both in the simulated-network tests. With a local relay configured for owner-only AUTH reads (NIP-78-style), check which design still syncs.
7. **E6b: local Blossom reference server.** Upload `application/octet-stream` ciphertext, confirm acceptance with `*` rules, 415 with media-only rules, and idle-expiry pruning.
8. **Polite public probe (needs explicit approval; STATE.md allows "a handful of small events per relay").** For each candidate relay: one throwaway key, one ~1 KB and one ~40 KiB event of the chosen kind with `expiration` = +2 h; read back from a second connection; record OK messages. Repeat reads at +1 h and after expiry. Also probe with `+300 s` and `+1000 s` `created_at` to learn skew limits. This answers the open unknowns: damus/primal/nos.lol max event size, kind acceptance, NIP-40 purge.
9. **Clock-skew survey:** log NTP offset on test phones for a week to size the monotonic-`created_at` guard.

---

## Sources

Specs (nostr-protocol/nips, master as of 2026-09-24):
- NIP-01: https://github.com/nostr-protocol/nips/blob/master/01.md
- NIP-09: https://github.com/nostr-protocol/nips/blob/master/09.md
- NIP-11: https://github.com/nostr-protocol/nips/blob/master/11.md; retention removal PR #1946 (merged 2026-02-15): https://github.com/nostr-protocol/nips/pull/1946
- NIP-13: https://github.com/nostr-protocol/nips/blob/master/13.md
- NIP-17: https://github.com/nostr-protocol/nips/blob/master/17.md
- NIP-37: https://github.com/nostr-protocol/nips/blob/master/37.md
- NIP-40: https://github.com/nostr-protocol/nips/blob/master/40.md
- NIP-42: https://github.com/nostr-protocol/nips/blob/master/42.md
- NIP-59: https://github.com/nostr-protocol/nips/blob/master/59.md
- NIP-60: https://github.com/nostr-protocol/nips/blob/master/60.md
- NIP-78: https://github.com/nostr-protocol/nips/blob/master/78.md; issue #2452: https://github.com/nostr-protocol/nips/issues/2452; PR #2458 (merged 2026-09-03): https://github.com/nostr-protocol/nips/pull/2458; PR #2292 (kind 78): https://github.com/nostr-protocol/nips/pull/2292
- NIP-95 PR #345 (closed 2025-05-22): https://github.com/nostr-protocol/nips/pull/345

Relay software:
- strfry config: https://github.com/hoytech/strfry/blob/master/strfry.conf
- strfry `src/events.cpp`: https://github.com/hoytech/strfry/blob/master/src/events.cpp
- strfry `src/apps/relay/RelayWriter.cpp`, `RelayIngester.cpp`, `RelayCron.cpp`, `RelayWebsocket.cpp`: https://github.com/hoytech/strfry/tree/master/src/apps/relay
- strfry plugins doc: https://github.com/hoytech/strfry/blob/master/docs/plugins.md
- noteguard: https://github.com/damus-io/noteguard; config: https://github.com/damus-io/noteguard/blob/master/noteguard.toml
- nostr-rs-relay config: https://github.com/scsibug/nostr-rs-relay/blob/master/config.toml
- nostr-rs-relay `sqlite.rs`: https://github.com/scsibug/nostr-rs-relay/blob/master/src/repo/sqlite.rs
- khatru sane defaults: https://github.com/fiatjaf/khatru/blob/master/policies/sane_defaults.go
- khatru `ratelimits.go`: https://github.com/fiatjaf/khatru/blob/master/policies/ratelimits.go
- khatru `adding.go`, `utils.go`: https://github.com/fiatjaf/khatru
- wot-relay: https://github.com/bitvora/wot-relay
- nspam-strfry: https://github.com/barrydeen/nspam-strfry
- spamblaster: https://github.com/relaytools/spamblaster
- filter.nostr.wine: https://docs.nostr.wine/filter/readme

Field evidence:
- discordia #198 (damus ban for 30078 republish, 2026-09-22): https://github.com/ssotomayor/discordia/issues/198
- divine-mobile #9467 (damus restricted reads, broken AUTH, 2026-09-23): https://github.com/divinevideo/divine-mobile/issues/9467
- White Noise default relays: https://github.com/marmot-protocol/whitenoise-rs/blob/master/src/whitenoise/relays.rs
- Marmot Nostr transport (fresh key per kind-445 event): https://github.com/marmot-protocol/marmot/blob/master/transports/nostr.md
- relay.nostr.build policy: https://relay.nostr.build/
- NIP-11 documents fetched 2026-09-24 from `https://<host>` with `Accept: application/nostr+json`:
  - relay.damus.io, nos.lol, relay.primal.net, relay.snort.social, nostr.mom, offchain.pub, nostr.oxtr.dev, nostr.bitcoiner.social
  - nostr-pub.wellorder.net, relay.wellorder.net, nostr.wine, filter.nostr.wine, inbox.nostr.wine, purplepag.es, relay.ditto.pub, relay.mostr.pub
  - nostr.land, eden.nostr.land, yabu.me, nostr21.com, relay.nos.social, relay.coinos.io, relay.fountain.fm, relay.angor.io
  - theforest.nostr1.com, auth.nostr1.com, relay.azzamo.net, wot.utxo.one, relay.utxo.one, relay.nostrcheck.me, nostr-01.yakihonne.com, relay.olas.app
  - nostr.einundzwanzig.space, relay.nostr.wirednet.jp, nostr.data.haus, relay.nostrplebs.com, relay.getalby.com, relay.nostr.watch, nostr.sathoarder.com, relay.nostr.info
  - failures as listed above

Measurement:
- Wei & Tyson, arXiv 2402.05709: https://arxiv.org/abs/2402.05709; v2 HTML: https://arxiv.org/html/2402.05709v2; v1: https://arxiv.org/html/2402.05709v1; ACM: https://dl.acm.org/doi/10.1145/3768994

Blossom:
- Specs: https://github.com/hzrd149/blossom; BUD-02: https://github.com/hzrd149/blossom/blob/master/buds/02.md; BUD-06: https://github.com/hzrd149/blossom/blob/master/buds/06.md; BUD-11: https://github.com/hzrd149/blossom/blob/master/buds/11.md
- Reference server config: https://github.com/hzrd149/blossom-server/blob/master/config.example.yml
- Public servers: https://blossom.nostr.build/, https://blossom.band/, https://24242.io/, https://blossom.primal.net/

---

## Independent verification

Date: 2026-09-24. This was a separate skeptical pass that tried to refute R2's key conclusions. Method:
- Re-read the primary code and specs at `master`: strfry (`strfry.conf`, `events.cpp`, `RelayWebsocket.cpp`), khatru (`adding.go`, `helpers.go`, `policies/*.go`), nostr-rs-relay (`sqlite.rs`, `config.rs`, `db.rs`), noteguard, NIPs 01/09/11/17/40/42/59/78, BUD-02 and BUD-11, the blossom-server config, and the Marmot transport.
- Fetched 13 NIP-11 documents (HTTPS GET only).
- Read the full text of Wei & Tyson v2.
- **Independent sources:** GitHub issue reports from other Nostr projects (May–Sep 2026) and this project's own measured probe data (`research/experiments/e4/probe_results.jsonl`).

No websockets were opened and nothing was published.

### Verdicts

| # | Claim | R2 verdict | Upheld? | Verifier's verdict |
|---|---|---|---|---|
| 1 | Acceptance of new keys, custom and addressable kinds, opaque content | Partial | **Yes** | Partial. More evidence on both sides (below). R2 leans on NIP-11 silence too much |
| 2 | Addressable replacement and clock skew | Partial | **Yes** | Partial. R2's `replaced:` fix only works on strfry (below) |
| 3 | Size limits and chunking | Partial | **Yes** | Partial. E4's raw data supports R2's ~47 KiB ceiling. "10–40 chunks" is an upper bound |
| 4 | NIP-40 and NIP-09 | Partial | **Yes** | Partial |
| 5 | The "ACM study" numbers and retention | Partial | **Yes, but R2's own correction needs correcting** | Partial. The availability window was 3 months, not 6 |
| 6 | Operator stance on generic encrypted storage | Partial ("trending toward refuted") | **Yes (partial)** | Partial. "Trending toward refuted" goes beyond the evidence for a *dedicated* kind |
| 7 | Blossom accepts encrypted non-media blobs | Partial | **Yes, with a correction** | Partial. blossom.primal.net is verified to *reject* ciphertext (415) |
| 8 | Per-group keys are unlinkable and won't trip spam filters | Refuted | **Yes** | Refuted |

### Claim-by-claim notes

**1. Acceptance (upheld: partial)**
- **For:**
  - This project's E4 probe (same day, raw data read) had 8 of 8 relays accept a fresh key, kind 30078 and custom kind 31234: damus, nos.lol, primal, nostr.mom, offchain.pub, nostr.oxtr.dev, wellorder and snort.
  - The payload was 45 KiB of random bytes as base64, about 61 KB of JSON. It was read back on the same connection.
  - `protolayer-io/choke#130` (2026-07-22): nos.lol accepted 25 of 25 and primal 12 of 12 fresh-key addressable kind-31415 events with `expiration` set.
- **Against:**
  - The noteguard reference config is as R2 says: 8 per minute, ban after 10 rejections, 1 h, optional Cloudflare edge ban.
  - Independently, `jared-logan/nostr-station#54` (2026-05-14) reports relay.damus.io as "rate-limited every publish" with `rate-limited: you are noting too much`. That wording differs from the reference config's `you note too much`. So damus runs a *customized* noteguard, and its production numbers are still unknown.
  - relay.mostro.network answers `rate-limited: … max 5 events per minute per IP` (choke#130). `citadel-foss/openswap-ffi#114` reports rate limiting from nos.lol and damus in CI loops.
  - **offchain.pub rejected ephemeral keys** with "Policy violated and pubkey is not in our web of trust" in two independent reports: nostr-station#54 (May 2026) and `hongyime/theprawnsplit#4` (Aug 2026). Its NIP-11 declares no restriction, and E4 got through today. **NIP-11 silence is not evidence of open writes.** R2's summary point 1 ("declare no … restricted_writes") should not be read as proof.
- Minor: wot-relay's `.env.example` has `MINIMUM_FOLLOWERS=5` (R2 says 3) and `MAX_AGE_DAYS=365`, which deletes old notes.

**2. Replacement (upheld: partial)**
- Tie-break code verified: strfry `isEventABeforeEventB`, khatru `isOlder` and nostr-rs-relay's `created_at >= ?` skip.
- **Correction:** only strfry answers an older write with `OK false "replaced: …"`.
  - khatru's default replace path sets `shouldStore = false` and returns no error, so it sends **OK true and stores nothing**.
  - nostr-rs-relay returns `Ok(0)`, which becomes **OK true `duplicate:`**.
  - So the fast-clock lockout is **silent** on khatru and nostr-rs-relay. R2's "treat `replaced:` as a clock problem" catches strfry only. Detection elsewhere needs read-back.
- **Correction:** nostr-rs-relay's code defaults are `reject_future_seconds = None` (the 1800 comes from the sample `config.toml`) and `max_event_bytes = 2 << 17` = **262,144** (the code comment says "128K"; the sample config has 131072 commented out).
- Independent support for R2's fix: choke#130 already uses a monotonic `last + 1` `created_at` for addressable events to avoid the lowest-id tie problem.

**3. Size (upheld: partial)**
- Verified: strfry `maxEventSize = 65536` (normalized JSON), and its NIP-11 `max_message_length` is `relay__maxWebsocketPayloadSize` (`RelayWebsocket.cpp:98`).
- E4 sent 45 KiB of binary (about 61 KB of JSON), and all 8 relays accepted it. That fits R2's ~47 KiB ceiling. **E4's written conclusion ("128 KB per message, ~95 KB payload") is not supported by E4's own test**, which never sent more than 64 KiB, and it contradicts the strfry source.
- **Chunk count:** E1 also gives state-only sizes: 31, 92 and 155 KB. At 40 KiB chunks, a 5-year busy list is 4 chunks (state-only) to 9 (full history). "10–40" fits only multi-list groups with full history. The rate-limit concern still holds, since 9 chunks already exceed noteguard's 8 per minute.
- **Subscriptions:**
  - relay.damus.io now reports `max_subscriptions` 200; nos.lol and primal report 20 (confirmed).
  - nos.lol answers bursts with `NOTICE "ERROR: too many concurrent REQs"` (`MostroP2P/app#560`).
  - strfry 1.1 and later reject subscription ids longer than 64 characters (`divinevideo/divine-mobile#9018`).
  - Both refusals arrive as NOTICE or CLOSED messages that the client cannot tie to a subscription, so the subscription is **lost silently**.

**4. NIP-40 and NIP-09 (upheld: partial)**
- Verbatim text verified: "MAY persist them indefinitely", "Clients SHOULD NOT send expiration events to relays that do not support this NIP", and deletion requests are kept "indefinitely".
- nostr-rs-relay hides only `e`-referenced events (verified). khatru also runs an expiration manager.
- NIP-40 is listed by damus, nos.lol, primal, offchain, oxtr and nos.social. memlay (snort) does not list it.
- E4's NIP-09 deletions got `OK true`, but deletion was never read back, so it is **not empirically verified**.

**5. Measurement paper (upheld: partial; R2's correction corrected)**
- **Availability window:** availability was measured **only 2023-10-01 to 2023-12-31 (3 months)**, from nostr.watch snapshots taken every 15 minutes. "Dead" means offline for more than a week and still offline at the end. The correct wording is "132 of 712 (18.5%) dead within a 3-month window", not "within 6 months".
- **Outage resolution:** "56% of outages < 15 min" means "seen down in one 15-minute snapshot". The data doesn't resolve durations under 15 minutes.
- **Survivorship bias:**
  - 911 relays were listed and 199 were excluded for having no data.
  - Of those 199, 41% "remain online for only a short period before going constantly offline". That is about 80 more short-lived relays that are not in the 712.
  - Real churn is therefore higher than 18.5%.
- **Citation:** arXiv lists v2 (2025-09-29) as "Accepted to CoNEXT '25". The DOI 10.1145/3768994 resolves to the ACM DL, but the page returned 403, so the title wasn't confirmed there.
- **NIP-11 retention:** PR #1946 was merged 2026-02-15 (confirmed), and the current NIP-11 text has no retention section.

**6. Operator stance (upheld: partial)**
- **Verified timeline:**
  - #2452 was opened by alexgleason on 2026-08-30. fiatjaf replied "Good point, let's add this to the NIP." on 08-31.
  - #2458 was merged 2026-09-03.
  - Ditto enforced 30078 first, then kind 78 (09-02). A second engine (iqbqioza) followed on 09-06.
  - NIP-95 was closed by staab on 2025-05-22: "This is better offloaded to blossom".
  - The strfry `restrictedReadKinds` commits are dated 2026-08-03 and 2026-08-12.
- **Nuance:** the owner-only AUTH rule is scoped to kinds 78 and 30078. No spec text or software default extends it to other custom kinds, and E4's kind 31234 was accepted everywhere today. For a **dedicated** kind the evidence reads "tolerated today; the direction is against generic app data", not "trending toward refuted".
- **Conflict inside this project:** `E4-nostr.md` ("Use NIP-78 kind 30078") and the STATE.md findings log recommend the opposite of R2. The current NIP-78 text makes that recommendation wrong for a mailbox read by *other* members' keys. E4's 30078 read-back only worked because the author's own key read it, on relays that haven't adopted the rule. **R2 is right; E4's recommendation should be withdrawn.**

**7. Blossom (upheld: partial, with a correction)**
- **blossom.primal.net rejects ciphertext:** R2 lists it as unverified, but `marmot-protocol/mdk#851` (2026-07-13) shows the rejection:
  - `PUT /upload` of `application/octet-stream` ciphertext returned **HTTP 415** "unsupported media type application/octet-stream".
  - Relabeling the upload as `image/jpeg` also failed, because the server inspects the bytes.
- **Servers that accept it:** mdk#852 moved Marmot's defaults to `blossom.divine.video`, `blossom.ditto.pub` and `cdn.hzrd149.com`. Each was verified in July 2026 to accept random 1 KiB `application/octet-stream` payloads.
- **BUD-11:** tokens are *optional* in the spec ("servers MAY require them"). "Every upload needs a token" is true of the servers named and of the reference config (`requireAuth: true`), not of the spec.
- Re-verified: the blossom.band free tier is media-only at 20 MiB with no expiry. 24242.io allows 100 MiB, deletes blobs after 60 days without access, and is free "for now".

**8. Linkability and spam (upheld: refuted)**
- NIP-01 (single websocket), NIP-42 (multi-pubkey AUTH), NIP-59/17 (spam guidance) and nspam (kind 1 only) were all verified verbatim.
- **Spam, independent evidence:** WoT rejections of ephemeral keys at offchain.pub (two reports), against acceptance at nos.lol and primal (choke#130, E4).
- **Caveat on the Marmot precedent:** kind 445 uses a fresh key *per event*, so per-pubkey limits never build up. This design's per-device-per-group keys are stable, so any per-pubkey limit does build up. Kind 445 is also a small regular event, not a repeatedly rewritten large addressable one, so it is a weaker precedent for slots and snapshots than R2 implies. E4 covers addressable acceptance directly.

### Missed by R2

1. **relay.damus.io's operator announced a shutdown for the end of July 2026**, reportedly because it was "operating at a loss" (`dmnyc/sidecar#43`, 2026-07-02; `zapcooking/frontend#500`).
   - Several apps removed it from their defaults.
   - It is still running: `zapcooking/zapcooking_ios#66` probed it on 2026-09-07, and this verification fetched its NIP-11 on 2026-09-24.
   - Marmot retired it, then restored its eligibility on 2026-09-20 (`marmot-protocol/mdk#1955`).
   - One of the "big three" defaults is at risk, which is a concrete case of the donation-funding fragility the paper describes.
2. **Operator overlap is wider than R2 says.** offchain.pub and nostr.bitcoiner.social publish the same operator pubkey (`ece3317b…`), just as nos.lol and nostr.mom do (`c5fadeb5…`).
3. **NIP-11 doesn't reveal WoT gating** (offchain.pub). Acceptance must be probed live and re-probed over time. The health tracker must also classify free-text reasons like "not in our web of trust" as a permanent block for fresh keys.
4. **OK semantics differ by software.** strfry answers `replaced:`, khatru silently returns OK true, and nostr-rs-relay returns OK true `duplicate:`. noteguard's `banned:` is not a NIP-01 prefix (`damus-io/noteguard#20`), so clients that key off NIP-01 prefixes misclassify it.
5. **Silent subscription loss:**
   - NOTICE-based refusals that name no subscription.
   - The 64-character limit on subscription ids.
   - "Connected but not delivering" sockets (`MostroP2P/app#572`).

   Relay health must come from read-back and liveness checks, not from connection state or OK counts.
6. **Blossom:** Primal rejects ciphertext with 415, and a verified list of servers that accept it exists (claim 7 above).
7. **Weaknesses in the paper's method:** a 3-month window, 15-minute resolution and survivorship bias (claim 5 above).
8. **Conflicts with E4-nostr.md and STATE.md**, which say to use kind 30078 and claim ~95 KB per event. Both conflict with R2 and with the primary sources. Also, E4 did not test rate limits, reads by a *different* member's key, reads from a separate connection, or deletion read-back.

Independent sources used (beyond R2's): `protolayer-io/choke#130`, `jared-logan/nostr-station#54`, `hongyime/theprawnsplit#4`, `citadel-foss/openswap-ffi#114`, `damus-io/noteguard#20`, `MostroP2P/app#560` and `#572`, `divinevideo/divine-mobile#9018`, `marmot-protocol/mdk#851`, `#852` and `#1955`, `dmnyc/sidecar#43`, `zapcooking/frontend#500`, `zapcooking/zapcooking_ios#66`, khatru `adding.go` and `helpers.go`, nostr-rs-relay `config.rs` and `db.rs`, strfry `RelayWebsocket.cpp` and the `strfry.conf` commit history, the full text of arXiv 2402.05709v2, NIP-11 documents fetched 2026-09-24, and `research/experiments/e4/probe_results.jsonl` with `e4_nostr_probe.py`.
