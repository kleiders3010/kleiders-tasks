# Technical Design Principles

*Working name: Tasklist. Android package: `com.jurisgames.tasks` (permanent once published; the domain jurisgames.com is owned by the developer). Status: draft, 2026-09-24. Covers architecture and technical principles only, not UX or features.*

## 1. Mission and hard constraints

**Goal:** the most powerful task app in the world that is completely free to use, and the first of a family of "ultimate" free, open apps built on the same foundation.

Hard constraints. Every design decision must satisfy all of them:

| # | Constraint | Meaning |
|---|---|---|
| C1 | **Free for users** | No paid tier, no ads, no limits on lists, tasks, members or devices. |
| C2 | **Free for the developer, at any scale** | No infrastructure we host or pay for. Running costs must not grow with the number of users. |
| C3 | **No accounts** | No sign-up, email or password, for anyone, including people joining a shared list. |
| C4 | **Seamless** | Sync, sharing and backup work with default settings. The user never has to understand transports, relays or keys. |
| C5 | **Platforms** | Android first. Linux and Windows desktop. (macOS comes almost free with the chosen stack but is not a target.) |

## 2. Core principles

1. **Local-first.** Each device holds the complete data for every list it belongs to and works fully offline. The network only exchanges changes. There is no "source of truth" server.
2. **Zero hosted infrastructure.** We use only free public infrastructure run by others: DHTs, iroh relays, Nostr relays, STUN servers. For every role, at least two independent providers or mechanisms, so no single third party is a point of failure.
3. **Everything sent to third-party infrastructure is end-to-end encrypted.** Relays, DHTs and the like see only opaque data and unavoidable metadata (§9). **Storage the user owns** (their devices, their folders, their own cloud accounts) is the user's own trust domain. Data there follows the user's choices, like any other file they keep.
4. **Sync does not depend on the transport.** Every channel carries the same encrypted sync messages. Receiving a change twice, out of order, or from several channels at once is harmless (CRDT, §4). So channels run **in parallel, not as primary and backup**: adding a transport never requires changes elsewhere.
5. **Degrade gracefully.** Losing any transport or third-party service only makes sync slower, never impossible and never lossy.
6. **Respect the platform.** Work within Android's background rules (Doze, standby buckets, foreground-service limits). Never rely on loopholes Google Play could ban.
7. **Reusable core.** The sync, identity, storage and transport engine is a generic library with no task-specific code. Tasks are one domain on top of it. Future "ultimate" apps (notes, shopping, calendars…) reuse the engine unchanged.
8. **Always backward compatible, forever.**
   - Every message and stored file carries a format version.
   - **Nothing is ever removed or changes meaning.** Fields, message formats and transports are only *added*, and every new version keeps speaking every old format.
   - **Old clients keep working** with what they understand, and **preserve and pass on untouched** what they don't (Loro keeps unknown data natively). They never corrupt it.
   - The only loss: an old client can't *display* data it doesn't understand.
   - When a client receives data from a newer format version, it shows "A newer version is available to use certain features" with an **Update** button.
   - **The notice can't be abused.** Messages carry only a version *number*, never text or links. The notice text is built into the app, and **Update** opens only our own hard-coded store listing or release page. It appears only after the app confirms a newer version really exists (Play In-App Updates API on Android, GitHub Releases on desktop). So a fake high version number from a malicious member shows nothing.
   - **Required (security) updates are never triggered by peer data.** They come only from Play's in-app update priority, or from a release manifest signed with our developer key, whose public key is built into the app.
   - **All incoming data is untrusted, even from members:** size limits, strict validation, rejection without crashing, and fuzz testing of every parser.
   - Desktop update checks use GitHub Releases (static, free), plus winget where installed through it. Android updates come through Play.
   - **Sole exception:** a flaw in the cryptography itself may force retiring an old format. It's announced ahead through the update notice.
9. **Permissive dependencies only.** MIT, Apache or BSD. No GPL, so the decision about open-sourcing (§14) stays free either way.
10. **Defaults over settings.** Pick the right behavior instead of asking. A setting exists only if a normal user would plausibly want something different. Engineering knobs belong in a hidden diagnostics screen, never in settings. Problems are raised when they're detected (e.g. background sync being blocked), not pre-emptively through options.

### User-facing settings (complete list)

| Setting | Default | Platforms |
|---|---|---|
| Sync in the background (off = sync only while the app is open) | On | All |
| Notify me when others change a list | On | All |
| Backup: connect Google Drive / OneDrive, or choose a folder (desktop); export / import | Android's built-in backup is always on (§11). Cloud / folder backup is off, but prompted once on devices with no other peers. | All |
| Start with computer | On | Desktop |
| Help others sync (helper network, §7a; later phase). On/off; on Android only on Wi-Fi, never in battery saver / low battery / data saver. | Undecided | All |

Sub-options found only by looking under a parent setting: "Memory for helping others" and "Keep this PC awake while plugged in" (under Help others sync). See §7a.

Hidden diagnostics screen: per-layer connection status, relay list, sync log.

Principle 10 limits *behavioral* settings. **Appearance options** are a separate area: they can be as rich as needed, but live one level deeper and never affect the default experience (see [UX-PRINCIPLES.md](UX-PRINCIPLES.md)).

### Appearance (technical)

- **The active choice is per device, never synced.** Which theme a device uses lives in a local device-preferences store, outside all group documents. Nobody else's choice ever changes your app, and a user's own devices don't sync it either. It's included in that device's backups.
- **Themes are data, not code.** A theme is a versioned set of design tokens: color scheme, typography scale, density, shapes, list layout options. Built-in themes use the same format as custom ones.
- **Shared through the group.** "Share this look with [group]" adds the theme to the group document's theme collection, where it syncs like any other data. Members find shared themes under Appearance → "From your groups". There are no prompts and nothing is applied automatically.
- **List icon and color** are part of the list's own data, so every member sees the same identity. Their purpose is recognizing which list a task belongs to in mixed views (Today, widgets, search).
  - A list stores a **hue** (0–360°, optionally a vividness level) in the **HCT** color space, never a literal color value. **Each theme decides the tone**, tuned to its background, contrast and dark / light mode, so list colors can never clash with a theme. This uses Google's Material Color Utilities (Apache-2.0), the same system as Material You. Grey has no hue and is its own slot.
  - The picker shows a curated row of ~12–16 hues; "More colors…" opens a full hue wheel. Colors are unlimited, and none can clash.
  - Color is only a small accent (dot, icon tint), never a list background, and never the only signal: it always comes with the icon or name (colorblind users).
- **Background images** are part of a theme. Readability comes from:
  - content drawn on slightly see-through surfaces, never directly on the image;
  - scrims behind headers;
  - text color chosen from the image's brightness;
  - **the theme's whole palette (accents, list hues) generated from the image** with Material Color Utilities, as Material You does with wallpapers.

  **Background images never leave the device.** They're stored locally, not as blobs. Sharing a theme that uses one shares everything except the image, including the palette generated from it.
- Link / file theme sharing, for people outside your groups (e.g. publishing a theme online), is a low-priority extra.
- **The platform always wins where the user has already chosen:** system dark mode, font size and accessibility settings are respected by every theme. Material You dynamic color (Android 12+) is available as an option.

## 3. Architecture

```
┌──────────────────────────────────────────────────────────┐
│ UI: Kotlin + Compose Multiplatform                       │
│   Android (Jetpack Compose, Glance widgets, WorkManager) │
│   Desktop JVM (Windows, Linux): tray app                 │
├──────────────────────────────────────────────────────────┤
│ Domain: tasks (Rust)  ← app-specific, thin               │
├──────────────────────────────────────────────────────────┤
│ Engine (Rust, generic, reusable across apps)             │
│   documents (Loro) · groups & keys · storage             │
│   sync protocol · transport manager                      │
│   transports: iroh P2P · Nostr · LAN · (future layers)   │
└──────────────────────────────────────────────────────────┘
      bridged to Kotlin via UniFFI (Android: NDK .so, desktop: JNA)
```

- **Why a Rust core:** the best P2P (iroh) and CRDT (Loro) libraries are Rust. One core compiles for Android (arm64, armv7, x86_64) and desktop (Windows, Linux x86_64 and arm64), so there is a single implementation of all sync logic.
- **Why a Kotlin UI:** Jetpack Compose is Google's own toolkit, so it keeps up with Play requirements first. Glance widgets, WorkManager, Block Store and Auto Backup are all native Kotlin APIs. Compose Multiplatform reuses the UI on desktop.

## 4. Data model

- **CRDT library: [Loro](https://loro.dev/)** (Rust, 1.x, stable format). It provides maps, movable lists, movable trees (subtasks, reordering, moving between parents) and rich text, so any future feature is a new field or container, never a new sync mechanism.
- **Documents:**
  - One **group document**: members (device ID, display name), the list index, and group settings.
  - One **document per list.** Lists sync independently and a large list doesn't slow down the others.
- **Tasks** are tree nodes with open-ended properties. The domain layer defines known fields, and unknown fields survive round-trips.
- **Large files (blobs)**, i.e. images in tasks and task descriptions (not needed in v1), are kept out of the documents. They're stored content-addressed (by BLAKE3 hash), encrypted with the group key, and referenced from documents by hash.
  - Transfer, both by default, same principle as sync (all channels at once):
    - **P2P** via **iroh-blobs** (verified streaming, resumable).
    - **Blossom** (Nostr file-storage servers): the encrypted blob is uploaded to a few public servers for offline delivery.
  - **Delivery may fail, and that's accepted, silently.** Every image reference carries a **ThumbHash** (~25 bytes) in the document, so members immediately see a blurred preview with the right colors and shape. It sharpens once the full image arrives. There's no status text: at worst it looks like a slow load. Fetching retries automatically.
  - Blobs can be re-fetched from the group, so Auto Backup (25 MB cap) only includes compressed copies of what that device itself needs.
- **History is kept for free.** Loro keeps the operation log, which enables "who changed what", undo and restoring deleted items. Compaction (shallow snapshots) is available if history ever grows too large.

## 5. Identity, groups and sharing

- **Device identity:** each install generates an Ed25519 keypair. Its public key doubles as the iroh endpoint ID.
- **Personal group = a person without an account.** Every install creates a **personal group** that only the user's own devices join. It holds the user's display name, their device list, and the secrets of all their other groups.
  - Linking a new device = joining the personal group. Every group, including ones created later, then appears on all the user's devices automatically.
  - **Linking a device that already has its own data merges it:** the device's personal group contents move into the existing personal group. So "start fresh, link later" is lossless, and first launch never has to ask.
  - History and "who changed what" show **people**, not devices.
  - **Display names, not unique usernames:** global uniqueness needs a central registry (a server), and names only need to make sense within a group.
    - The name lives in the personal group and is published to each group's member list. Changing it updates everywhere.
    - It's asked for the first time the user shares or joins a list, never at first launch. Solo users never need one.
    - Every person gets a generated avatar (initials on an HCT hue derived from their identity key). An optional small profile photo can come later, stored as a tiny thumbnail in the group document, not as a blob.
    - Duplicate names in a group are told apart by avatar and, if needed, a short identity tag ("Alex · 4F2A").  - Recovery simplifies: Block Store only needs the personal group secret; everything else follows from it.
  - **Linking requires confirmation on an existing device** ("Link 'Pixel 8'? Only approve if it's yours"), and the link QR expires after 5 minutes, because linking grants access to everything. This is the only join flow with approval.
- **Key epochs (in the engine from day one).** Group secrets can be rotated: a new epoch key is encrypted to each remaining device's public key and distributed through the group. Removed devices keep what they already had but can't read anything new.
  - Used for **"Remove device"** (lost or stolen): removing a device rotates the personal group and every group the user belongs to, automatically. Other members' apps switch over silently.
  - **Also used for removing people from a group:** the same rotation excludes a member's devices. The removed person keeps what they already had but receives nothing new.
- **Group = a random 256-bit secret.** Every other value is derived from it with HKDF and separate labels:
  - group ID
  - document encryption key
  - Nostr topic tag
  - DHT discovery key
- **Membership:** knowing the secret makes you a full author. There are no roles or permissions (a deliberate product decision). **Leaving** is local. **Removing someone** rotates the group key (key epochs, below). A professional mode with roles and read-only access can come later without changing the engine.
- **Solo lists are groups with one member**, so there's one code path. Their encrypted copy on Nostr serves as an extra backup.
- **Ways to join:**
  - **Link:** the secret goes in the URL fragment (`#…`), so it never reaches a web server. It opens the app, or the future web client.
    - **When the app isn't installed yet:** the link's static page (GitHub Pages) has a "Get the app" button that copies the invite to the clipboard, then opens Play. On first launch the app checks the clipboard once for an invite (Android shows its "pasted" notice) and opens the join screen.
    - The Play Install Referrer isn't used, because it would pass the secret through Google.
    - Fallback: tapping the link again after installing.
  - **QR code:** the same payload as the link.
  - **Invite code** (e.g. `maple-orbit-guitar-seven-river-anchor`): 6 words from the EFF large word list (7,776 common English words, ~77 bits total), **valid for 1 day**, no approval step, and the inviter doesn't need to be online. Each "Invite with code" tap creates a new code, which can be used by several people within its day.
    - How it works: Argon2id(code) produces a lookup tag and an encryption key. The group secret is published to Nostr, encrypted with that key, under that tag, with an expiry tag (NIP-40) and an expiry timestamp inside the encrypted payload. The joiner derives the same values from the code, fetches the invite and decrypts it.
    - **Length is the real protection.** Invites on relays are public and can be copied and attacked offline indefinitely, so expiry can't stop brute force. An attacker can collect *every* invite ever posted and test each guess against all of them at once. With ~1M invites collected and ~100k Argon2id guesses/s of rented hardware, one random invite falls every few minutes at 4 words and every few days to years at 5. At 6 words from the 7,776-word list it takes tens of thousands of years. (A smaller 2,048-word list would give only years to decades, so the large list is required.)
    - Considered and rejected: shorter codes (`4821-maple-orbit`) protected by a PAKE handshake with a member device and an attempt limit. Rejected because a member device must be reachable to answer; 6-word codes work with nobody online.
    - **Expiry covers human leaks:** old chats, photos, screenshots. Relays drop expired invites, and the app refuses expired invites even if a relay still serves one.

## 6. Transport stack

All layers are active at once (§2, principle 4). Most internet P2P layers come from **[iroh](https://www.iroh.computer/)** (1.0 stable since June 2026, about 90% direct-connection success).

| Layer | Mechanism | Provided by | Covers |
|---|---|---|---|
| 1 | LAN discovery + direct connection | iroh local discovery (mDNS) | Same network, even with no internet |
| 2 | Direct IPv6 | iroh | Carriers with public IPv6 |
| 3 | UDP hole punching (QUIC NAT traversal) | iroh + public STUN | Most home and mobile NATs |
| 4 | Router port mapping (UPnP / NAT-PMP / PCP) | iroh portmapper | Desktops on home networks |
| 5 | Hard-NAT punching (birthday technique) | Evaluate; custom if needed | Strict carrier NATs |
| 6 | Relay through a reachable group member | Engine (our logic) | A and B can't meet but both reach C |
| 7 | Public relays | iroh public relays; evaluate libp2p circuit relays on IPFS nodes as a second provider | Everything else, while both online |
| 8 | Store-and-forward mailbox | **Nostr** relays (§7); later also desktop helpers (§7a) | The peer is offline |
| 9 | Nearby (Bluetooth / Wi-Fi Direct) | Android Nearby Connections (future, Play flavor only) | No network at all |

- **Discovery:** peers find each other through iroh discovery (DNS / pkarr records on the **BitTorrent Mainline DHT**) under a key derived from the group, plus Nostr as a second discovery channel.
- **Doorbell slot:** instant wake-ups for a sleeping phone need Google's push service (FCM), which needs a sending server. **Deliberately not built.** The transport manager has an interface for it so FCM or UnifiedPush can be added later without redesign.

## 7. Nostr mailbox

- **Mailbox = slots that get replaced, not a growing log.** Per group there's:
  - one **snapshot slot**: the full compacted state;
  - one **slot per member device**: that device's changes since the last snapshot, merged into a single Loro update.

  Each write *replaces* the slot's previous content, so a group's mailbox size is bounded by 1 + number of member devices, regardless of edit volume. When a device's bundle exceeds a size threshold, it writes a new snapshot and resets its bundle. The CRDT makes overlapping snapshot and bundle contents harmless. A device resets its bundle only after seeing a snapshot that includes its changes.
- Implemented with Nostr **addressable (replaceable) events** (NIP-01, kinds 30000–39999): relays keep only the latest event per (pubkey, kind, `d` tag). This keeps our footprint on public relays small, which also reduces acceptance risk.
- Events are encrypted and signed with a **per-device, per-group** throwaway Nostr key, so a device's groups can't be linked to each other.
- **Write rate:** members online at the same time sync live over P2P, so the mailbox is only for the absent.
  - Every change is **saved locally immediately**, never debounced.
  - While the app is in the foreground, a device rewrites its slot at most every few seconds.
  - **Leaving the app flushes immediately**: going to the background, turning the screen off, closing, or quitting the desktop app. The `shortService` (§8) finishes the upload.
  - If there's no network, a WorkManager job waiting for connectivity uploads as soon as possible, with periodic sync as a backstop.
  - Widget and notification actions flush immediately.
- **Default set of 5–8 public relays**, published to all of them. The relay list is shipped with the app and updated with app releases; it's visible only in the diagnostics screen.
- Relays may delete old events. To stay safe:
  1. Every device keeps the full data, so relays are a mailbox, never the only copy.
  2. Devices regularly republish a **compacted snapshot**, split into chunks under typical relay size limits.
  3. A device that finds a relay missing data re-uploads it.
- Per-message format: version, group ID hash, encrypted payload (XChaCha20-Poly1305 with the group key).
- **Relay health tracking:** at runtime the app records which relays accept, keep and return its events, prefers those that work, and drops those that don't. It supports proof of work (NIP-13) for relays that require it. The default list is refreshed with every release.

**How reliable Nostr is** (ACM measurement study, 2023, 712 relays):
- **Uptime is fine with redundancy.** Excluding dead relays, 90% were up >80% of the time, and most outages last <15 min. Publishing to 5–8 relays makes "all down at once" negligible.
- **Retention is weak but mostly irrelevant.** Relays are a short-lived mailbox, and snapshot republishing covers long absences.
- **Acceptance is the real long-term risk.** Relays choose what they accept (web-of-trust filtering, payment, proof of work, allowed event kinds), and free relays are donation-funded. At large scale, operators may rate-limit or block our traffic. The helper network (§7a) is the answer to that, and it grows with the user base.

## 7a. Helper network (later phase)

Devices can act as **helpers**: a user-run, Nostr-like mailbox for other groups, like torrent seeding. It's one more parallel mailbox (principle 4). Nothing else changes.

- **Desktops:** help whenever the app is running. They're the most reliable helpers.
- **Phones help only on Wi-Fi, and the app decides how much:**
  - **Charging on Wi-Fi: heavy help.** Doze and standby-bucket limits don't apply while charging, and most phones charge overnight on home Wi-Fi, so it costs no battery or mobile data. The phone runs as a chain of back-to-back WorkManager jobs, with no notification. A foreground service is only a fallback if that fails (it's capped at 6 h/day on Android 15 and may face Play review).
  - **On Wi-Fi, not charging: light help**, only while the app is open anyway or during its own sync runs, when the radio is already awake.
  - **Never:** on mobile data, in battery saver, below ~20–30% battery, at elevated thermal status, or with data saver on (all exposed by `PowerManager` / `ConnectivityManager`).
  - Phones are less reliable helpers: the process can be killed between jobs and lose its RAM contents. Clients weight them accordingly.
- **RAM only, never disk** (for other groups' data; a member device may keep its *own* groups' mailbox on disk, see Trust tiers). Stored messages disappear on restart. That's acceptable because helpers are never the only copy: every message goes to several helpers plus Nostr. Nothing is left on the user's drive, and no strangers' data (even encrypted) is ever persisted.
- **Caps:** memory scaled to the device (e.g. 16–32 MB on phones, 64–256 MB on desktops), bandwidth-capped, oldest messages evicted first. Only small messages shaped like the app's own sync messages are accepted, with an automatic lifetime, which keeps abuse (using helpers as free storage) impractical.
- **Discovery:** helpers announce themselves on the DHT. Clients spread each message over several helpers, so a bad helper that drops messages is harmless.
- **What a helper stores.** Helpers use the same slot model as Nostr (§7): only the latest version of each slot is kept, so volume is bounded per group, not per change.
  - **Assignment: chosen by reliability, recorded in the group.** Each group keeps its **helper set in the group document**. Writers choose it and every member reads it, so members agree on where the mailbox is without any ID-based rule.
    - **Bootstrap only:** a device that doesn't have the group document yet (first join) uses the *r* helpers closest to a daily-rotated hash of the group ID (XOR distance), plus Nostr. Groups also mirror to this bootstrap set.
    - Because the steady-state set isn't derived from the group ID, positioning fake helpers near a group's ID (an eclipse attack) buys an attacker almost nothing.
  - **Reliability records.** Each device records, per helper it uses:
    - reachability;
    - whether it returned what was stored;
    - whether it returned the *latest* version, which readers can check cheaply by comparing helpers;
    - latency.
  - Records are **shared only within the device's own groups**, whose members are trusted. There's no global reputation, which fake identities could manipulate.
  - **What lowers a record, by severity:**
    | Observation | Effect |
    |---|---|
    | Returns data failing signature / hash verification | Immediate exclusion (cryptographic proof; impossible for an honest helper) |
    | Loses data stored **during its current session** with no declared eviction | Heavy: impossible for an honest RAM helper without a restart |
    | Returns an older version of a slot it received **directly** from the writer in its current session | Heavy: proven withholding |
    | Returns an older version of a slot it received via replication | Light, only as a persistent pattern (replication lag is legitimate) |
    | Unreachable when contacted | Light, relative to its own normal uptime (50%-uptime PCs are judged against 50%) |
    | New session (restart, including unclean ones like power outages) | Light, counted like downtime. Frequent restarts naturally rank low |
    | Declared eviction for space | Very light |
  - **Sessions:** every helper reply includes its session start time. Data stored before the current session started is expected to be gone after a restart (RAM), so it counts as a restart, not a loss. This covers sudden power loss, where no restart could be announced. Faking restarts to excuse drops only lowers the helper's own uptime score.
  - **Why old versions matter:** signatures prove authorship, not freshness. An old slot version stays validly signed forever, so a helper can serve it to withhold newer data (a rollback). The damage is limited, since merging old CRDT data is harmless and newer data comes from other helpers and Nostr. The writer knows exactly which version it uploaded directly to which helper, which separates proven withholding from replication lag.
    | Slow | Ranking only |
  - **Records decay over time** (recent behavior dominates). Past good behavior doesn't shield a helper that turns bad, and bad luck isn't permanent. Only verification failures are permanent.
  - **Recovery:** unreachable or low-ranked helpers aren't banned. They're periodically re-probed with a small share of traffic and climb back as decay removes old failures. There's no global blacklist; a bad period only affects groups that observed it.
  - **Attacks:**
    - DoS makes a helper drop in rank while it's actually unusable, and it recovers afterward.
    - Smearing is not possible from outside: records come only from the group's own members, slot data is signed by member keys, and transport is encrypted, so no one can make a helper *appear* to misbehave.
    - Junk-filling (fake groups flooding a helper's RAM) is countered by proof-of-work on new slots that scales with helper load, per-group quotas, and priority for groups with the longest history with the helper, or that actually read their data.
  - **Exposure for new helpers:**
    - each group reserves one copy for a randomly chosen unproven helper, rotated (e.g. weekly). At 5k groups and 1k helpers, that's ~5 groups per helper per round;
    - the daily-rotated bootstrap set spreads usage across all helpers;
    - **trust spreads through people:** a device merges its observations from all groups it belongs to and shares them into each, so a helper proven in one group gets tried by overlapping groups, only via trusted members.
    - Trust stays per group by design.
  - Helpers can declare honest reasons (restart, eviction for space). These lower the ranking but aren't counted as misbehavior. Phones naturally rank below desktops.
  - **Selection:** weighted random among reliable helpers, weighted by reliability × advertised free capacity, so the best helpers aren't overloaded. A small share goes to unproven helpers on probation, so newcomers can build a record.
  - **Silent exclusion:** a helper that keeps failing drops out of the helper sets of every group that observed it. Its own groups are unaffected, and it isn't notified.
  - Residual risk: a helper that behaves well for a long time and then turns bad. Covered by replication and Nostr in parallel.
- **Trust tiers.** Trust comes from membership, never from self-claims:
  1. **Member devices** are always preferred as mailboxes for their own groups, automatically. They hold the key and full data, so they can't be adversaries to their own group. They may store their own groups' mailbox **on disk** (it's their own data), so nothing is lost on restart and the RAM cap doesn't apply.
  2. **Earned helpers:** everyone else, ranked by the reliability records above.
  - An always-on machine of your own (e.g. a home server) simply **joins as a member** and gets tier 1. A headless / command-line build of the engine for that is an optional, low-priority extra. (No separate "pinned helper" concept.)
- **Offering more capacity:** "Memory for helping others" (hidden under the helper switch: e.g. Standard / Generous / custom). Capacity attracts more groups, and trust follows from observed service, never from claims. Guardrails:
  - capacity claims count only after probation;
  - a helper's share of groups grows gradually with its observed record;
  - one copy per group per helper, so no machine ever holds most of a group's copies.
- **Reachability on desktop:** start with computer (default on), router port mapping (§6), and an optional hidden "keep this PC awake while plugged in" (OS sleep inhibition). It's never default because of the energy cost.
  - **Adaptive replication.** *r* is chosen from measured helper reachability *p* to hit ~99.9% availability: *r* ≥ ln(0.001) / ln(1 − p). That's ~10 at p = 50% (desktops), ~20 at 30%, ~31 at 20%. Expected range: **12–30 copies**.
  - **The sender uploads to only 2–3 helpers**, attaching the group's helper set (public IDs only). The helpers replicate to the rest of that set, so a phone never pays for 20+ uploads.
  - **Repair:** when a helper in a group's set disappears or gets excluded, the next writer picks a replacement by the same selection rules and records it in the group document, keeping *r* stable under churn.
  - **Rough estimate (to be measured):** 10k users → ~5k groups × ~50 KB × 12–30 copies ≈ 3–8 GB network-wide. With ~1,000 active helpers, that's a few MB per helper.
  - **Per-slot write-rate limit:** excess writes are ignored, since a newer version always follows.
  - **Eviction, in order:**
    1. per-group quota (e.g. 256 KB per helper), so no single group can crowd out others;
    2. slots not refreshed within ~2 weeks become **first to be evicted when space is needed**. This isn't a hard delete: they stay as long as there's room. Every background sync run by any member refreshes the group's slots, so the clock only runs while *all* of a group's devices are off, uninstalled or have background sync disabled;
    3. priority: assigned groups before extras, snapshots before bundles, recently requested before never-requested.
  - Nothing is lost if everything for a group expires: every member device holds full data and republishes when opened. **Once a change has reached any other member, it's permanent**: it lives on that device and in its next snapshot. The only possible loss is a change that reached nobody, whose author's device is permanently gone without a backup, while every other member stayed offline until the mailbox was evicted.
- **Privacy for helpers:** their IP address is visible to strangers' devices (as with torrents). That's the main reason the setting exists.
- **Setting:** "Help others sync", a single on/off switch on every platform. On Android the conditions above apply automatically. **Default undecided** (on, like torrent seeding, vs. off). Decide when it's built.
- **When:** not needed at launch. Build it before the app's scale makes public relays' acceptance a risk.

## 8. Android background behavior and lifecycle

| State | Behavior |
|---|---|
| App open | Live on every layer: instant. |
| Just left the app | All pending changes are flushed immediately on going to the background or screen off. A `shortService` foreground service (up to 3 min) finishes sending them; without a network, a connectivity-constrained WorkManager job takes over. Its notification is held back 10 s, so it's usually invisible. |
| Background | WorkManager periodic sync every **15 min** (Android's minimum). No interval options; the only control is the "Sync in the background" on/off switch. |
| "Stay connected" | **Not in v1.** A foreground service with a persistent notification, re-considered only if users report delays as a real problem. |
| App launch | The system splash screen is held until the first sync round or ~1.5 s, whichever is first. Later changes appear live. |
| Widgets | Jetpack Glance. Checkboxes (Android 12+), a refresh button and a "synced X ago" label. Every change redraws the widget. An active widget keeps the app in the *active* standby bucket, so periodic sync doesn't slow down. |

- **User-facing honesty:** the background-sync setting says updates arrive "usually within 15 minutes, longer if your phone has been idle for a while".
- **Blocked background sync:** if the app detects its periodic sync isn't running (common on aggressive OEM skins), it shows a one-time explanation with a button to the system battery settings. It never requests the exemption directly, which Play restricts.
- **Desktop has no such limits.** The tray app stays connected while running and starts with the computer by default, which makes desktops the natural always-reachable peers.

## 9. Security and privacy model

| Party | Sees |
|---|---|
| Group members | Everything in the group (by design) |
| Nostr relays / helpers | Encrypted blobs, timing, per-device-per-group pubkeys (so roughly how many devices a group has), the uploader's IP |
| iroh relays / STUN / DHT | IP addresses, endpoint IDs, a discovery key derived from the group, connection timing |
| Google (Auto Backup) | Nothing readable: the backup is encrypted with the screen lock |
| The user's own cloud (opt-in backup) | A sealed file: unreadable to automated systems, readable with deliberate effort. |
| Anyone else | Nothing |

- On the device, group secrets and device keys live in OS secure storage: Android Keystore-backed storage, Windows Credential Manager, Linux Secret Service (with an encrypted-file fallback).
- Group secrets leave the device only inside Block Store (E2E-encrypted), full backups and sealed backups (§11), and join links, QR codes and invite codes (§5). They're never written to logs or Markdown exports.
- **Accepted metadata risk:** infrastructure operators can see that certain IPs are in touch. Content is never exposed.
- **Play compliance:** export compliance is declared as standard encryption (exempt). The Data safety form declares no collected user data (E2E-encrypted data is exempt) apart from what's required. A privacy policy says that encrypted data passes through third-party relays.

## 10. Storage

- **Opaque by design.** Data is stored in Loro's binary format in app-private storage (Android internal storage; `%APPDATA%` / `$XDG_DATA_HOME` on desktop). It isn't editable by users, which protects it and allows any styling or content.
- **Not encrypted with device-held keys.** Android Keystore keys are not backed up, so encrypting with them would make restored backups useless. Protection comes from app sandboxing, the operating system's storage encryption and Auto Backup's own encryption.
- **Crash-safe writes:** write to a temporary file, then rename it into place in one step. Snapshot plus an append-only update log, compacted periodically.

### Size and storage budget

Users' space is a design constraint, not an afterthought.

- **App size targets (Android):** download ≤ 20 MB, installed ≤ 40 MB, hard ceiling 50 MB. Estimate: Compose UI ~5–8 MB + Rust core ~8–15 MB per architecture. **Measured in phase 1**, then enforced by a size check in the build that fails when the budget is exceeded.
  - Android App Bundle (per-architecture delivery), R8 + resource shrinking.
  - Rust release profile: `opt-level = "z"` or `"s"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`. Review regularly with `cargo bloat`.
  - No bundled fonts (system and downloadable fonts), no full icon packs.
- **Desktop:** Compose Desktop bundles a Java runtime and Skia, so installers are ~60–100 MB. That's accepted for desktop. If it becomes a problem, the desktop UI can be replaced without touching the engine.
- **Data:** text data with history is expected to stay in single-digit MB even for heavy long-term users. Automatic policy, no settings:
  - **History is budgeted by content amount, not by time.** Each document's history may use a multiple of its current state size, with a small minimum. Quiet lists keep years of history; heavily edited lists compact sooner.
  - **History thins with age instead of being cut off:**
    - *recent*: the full operation log (every change, with author, for undo and "who changed what");
    - *older*: state checkpoints at widening intervals (e.g. daily → weekly → monthly), with the operation log compacted away (Loro shallow snapshots);
    - *oldest* checkpoints are dropped only when the budget is exceeded.
  - Phones use a tight budget; desktops a much larger one, and act as the group's archive for "restore old version".
  - Budget ratios and checkpoint intervals are tuned from measurements, not fixed up front.
  - Deleted items stay in trash 30 days, then are purged at compaction.
  - **Long-offline devices:** a device returning with unsynced edits built on history the others have already compacted can't merge them normally. It takes the group's current state and re-applies its own unsynced changes as new edits. Nothing is lost. This path must be explicitly built and tested.
  - Blobs (images in tasks): compressed on attach (e.g. WebP, ≤ ~2048 px), fetched on view, removed from the cache when unused. The original author's device keeps a blob until at least one other member has a copy.
  - Storage use per group is shown in the diagnostics screen.

## 11. Backup and recovery

| Scenario | Recovery path |
|---|---|
| Shared lists | The **personal group secret** from **Block Store** (Android, Play Services, E2E-encrypted cloud backup) or from a cloud / folder backup. It unlocks all other group secrets; data then comes from members and Nostr, up to date |
| Solo lists, Android | **Auto Backup** (daily, ≤25 MB, E2E-encrypted), plus the Nostr copy, plus cloud backup if connected |
| Solo lists, desktop | **Cloud backup** (Google Drive / OneDrive sign-in) or an automatic **backup to a folder**. The app prompts once when a device has no other peers. |
| Phone + desktop in the same group | They back each other up automatically |

**Opt-in cloud backup to the user's own account** (all platforms, including Linux):

| Provider | Access requested | One-time setup (developer) |
|---|---|---|
| Google Drive | `drive.file`: only files the app created, shown as a visible backup folder | Free Cloud project + brand verification (name, logo, privacy policy, verified domain). No security audit needed. Unverified: warning screen, 100-user cap. |
| OneDrive | App folder only | Free Microsoft app registration |

- Sign-in uses OAuth with PKCE, the standard method for apps without a server: system browser on desktop, Credential Manager / Authorization API on Android. No server of ours is involved.
- **Every backup that leaves the device is sealed.** This applies to cloud and folder backups; readable Markdown exports stay plain. Backups include group secrets. Sealed means encrypted with a random key stored in the backup file itself. Automated, untargeted access (provider scanning, AI features and training, indexing, bulk leaks) sees only random bytes. It deliberately doesn't stop a *targeted* attacker who studies the app. In exchange, a sealed backup can always be restored as long as the file exists.
- **No backup password.** Offline brute force can't be rate-limited, so a password protects only users who choose excellent ones. That isn't worth a setting.
- **Personal mailbox bonus:** a connected cloud also syncs between the *same user's* devices that are never online at the same time. It's more durable than Nostr. Other group members can't use it.
- Google Drive first, then OneDrive. Candidates only if users ask: Dropbox (app folder, free registration, production approval before 500 users), WebDAV (one protocol covering Nextcloud, NAS devices, Koofr…), S3-compatible storage.

**Exports:**
- **Readable:** plain Markdown, compatible with Obsidian Tasks, lossy, one-way. Manual only.
- **Full backup:** a single `.tasklist` file with history. Re-importable.

Data is lost only if a user had no Android backup, no other devices in any group, no cloud backup and no backup folder, meaning no second copy ever existed.

## 12. Platforms and distribution

| Platform | Build | Channels |
|---|---|---|
| Android | Gradle, Rust via cargo-ndk. `minSdk` 26 (Android 8.0), `targetSdk` 36, then 37 as Play requires | Google Play (existing developer account), GitHub Releases APK; F-Droid later (`foss` flavor, below) |
| Windows | Compose Desktop + jpackage | MSI / MSIX, winget, GitHub Releases |
| Linux | Compose Desktop + jpackage | AppImage, .deb / .rpm on GitHub Releases (Flathub not pursued) |

- **Features on older Android versions:**
  | Feature | Needs | Fallback |
  |---|---|---|
  | Checkbox widgets | Android 12 | Tap opens the app |
  | Material You dynamic color | 12 | Theme colors |
  | `shortService` upload window | 14 | Expedited WorkManager job |
  | E2E-encrypted Auto Backup / Block Store | 9 | Backup contents are **sealed** (same level as cloud backups, §11) |
- **Permissions beyond the defaults** (each explained on a screen before the system prompt, and asked for only when first needed):
  | Permission | Purpose | When |
  |---|---|---|
  | `POST_NOTIFICATIONS` (13+) | Reminders, change notifications | First time either becomes relevant |
  | `SCHEDULE_EXACT_ALARM` (off by default on 14+) | On-time reminders | When a timed reminder is first set **or received** |
  | `ACCESS_LOCAL_NETWORK` (target 37+; `NEARBY_WIFI_DEVICES` on 36) | Direct same-Wi-Fi sync (LAN layer) | When a group member is detected on the same network. Without it, sync goes over the internet. |
  | Camera | QR scanning, `foss` flavor only | When scanning. The `play` flavor uses the Google code scanner, which needs no camera permission. |
  | Bluetooth / Nearby Wi-Fi (+ location on old versions) | Nearby layer (later phase) | When first used |
  - **Location: not planned.** If it's ever added, it will be an explicit, user-enabled feature.
  - Normal (auto-granted) only: internet, network state, Wi-Fi multicast, boot completed (re-arm alarms), foreground service types.
  - **Desktop:** Windows Firewall shows its own "Allow access" dialog the first time the app listens for connections. The app explains it just before.
- **Product flavors:** `play` uses Play Services for Block Store, Nearby and Google sign-in (Credential Manager). `foss` has none of the proprietary Play Services libraries. It keeps Auto Backup (an Android framework feature, which works with whatever backup service the device has) and does cloud sign-in through the browser (OAuth). The engine has no Play Services dependency.

## 12a. Engineering practices

- **Repository:** one private GitHub repo. A Rust workspace (`engine`, `tasks` domain, `ffi` bindings) plus a Gradle project (Android app, desktop app, shared Compose UI).
- **Simulated-network tests:** many engine instances run in one process over fake transports that drop, delay, duplicate and reorder messages, and go offline for long periods. Merges must converge. This is the main defense against sync bugs, since there are no server logs.
- **Diagnostics:** "Export log" in the hidden diagnostics screen, attached by users to bug reports. Logs never contain secrets or task content.
- **Crash reports:** Play Console's Android vitals (no crash SDK). On desktop, the local log.
- **Libraries:** audited RustCrypto crates (`chacha20poly1305`, `hkdf`, `argon2`), `nostr` / rust-nostr (MIT), iroh, Loro, UniFFI.
- **Invite-code word lists in other languages:** later. Every version recognizes every supported list, since a code is just indices.

## 13. Free-forever economics

Every cost is either paid by the user's own device, bandwidth and cloud account, or absorbed by free public infrastructure with alternatives. Nothing in the architecture gets more expensive as the number of users grows. That's what makes "free for everyone, forever" credible instead of a promise that depends on funding.

## 14. Open source (undecided)

It's kept possible by principle 9 (permissive dependencies only).
- **For:** an E2E and "no servers" claim is only verifiable with source, F-Droid requires it, the app outlives its maintainer, and community help.
- **Against:** easy clones, and it's harder to keep control of direction.
- A possible middle path is to open the **engine** first, since it's the reusable foundation for the app family, and keep apps closed until decided.

## 15. To verify during prototyping

- iroh 1.x: Android and desktop behavior through UniFFI, battery cost, and the public relay usage policy and long-term availability.
- Loro: bundle size and performance with large lists on low-end Android, shallow-snapshot compaction, and how importing updates that depend on compacted history fails (for the long-offline rebase path).
- Nostr: current retention and size limits of candidate default relays, and rate limits.
- Block Store: size limits per entry and in total, and how restore works on a new device.
- That an active Glance widget really keeps the app in the active standby bucket on current Android and on major OEM skins.
- Whether IPFS circuit relays are worth adding as a second relay provider.
- Phone helpers: whether back-to-back WorkManager jobs keep a charging phone reachable without a notification on Android 15/16 and major OEM skins, and how often the process is killed between jobs.
- Blossom: which free public servers accept encrypted (non-media) blobs, their size limits and retention.
- Google brand verification requirements for `drive.file`: whether a free domain (e.g. a GitHub Pages site) is enough for verification.

## 16. Build order

1. **Foundation:** Rust engine skeleton, Loro documents, storage, UniFFI bridge, and a minimal Compose UI on Android and desktop (local-only). **Measure app size with iroh and Loro linked in** and set the build's size budget.
2. **Groups and sharing:** keys and key epochs, secure storage, personal group and device linking, link and QR joining. Invite codes come after Nostr sync (phase 3), which they depend on.
3. **Nostr sync:** mailbox, snapshots, multi-relay.
4. **P2P with iroh:** LAN, hole punching, relays, discovery, member relaying.
5. **Android lifecycle:** WorkManager, short service, splash sync, widgets.
6. **Backup:** Auto Backup, Block Store, desktop backup folder, exports, opt-in cloud backup (Google Drive and OneDrive first).
7. **Extras:** hard-NAT punching, Nearby, web guest client, images in tasks (P2P + Blossom blobs), doorbell, **helper network (§7a)** before scale makes it necessary, headless engine build (own always-on server as a group member).
