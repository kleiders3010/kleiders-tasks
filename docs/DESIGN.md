# Technical Design Principles

*Working name: Tasklist. Android package: `com.jurisgames.tasks` (permanent once published; the developer owns jurisgames.com). Status: **draft v2, 2026-09-24**. Folds in the decisions from the research phase (`docs/research/DECIDED.md`); still-open items are marked **⏳ pending** with their ID in `docs/research/DECISIONS.md`. Covers architecture and technical principles only, not UX or features. A hobby project: no payments, no support obligations.*

## 1. Mission and hard constraints

**Goal:** the most powerful task app in the world that is completely free to use, and the first of a family of "ultimate" free, open apps built on the same foundation.

Hard constraints. Every design decision must satisfy all of them:

| # | Constraint | Meaning |
|---|---|---|
| C1 | **Free for users** | No paid tier, no ads, no limits on lists, tasks, members or devices. |
| C2 | **No obligatory infrastructure or maintenance** | The app works fully on P2P, public open networks and users' own devices, with zero developer upkeep and no running costs that grow with users. Developer-run or developer-registered services are allowed only as **optional bonuses** (added through the operator manifest, §7b) and must never become required. |
| C3 | **No accounts** | No sign-up, email or password, for anyone, including people joining a shared list. |
| C4 | **Seamless** | Sync, sharing and backup work with default settings. Nothing may require user setup, and nothing may break with scale. The user never has to understand transports, relays or keys. |
| C5 | **Platforms** | Android first. Linux and Windows desktop. No iOS (§12). macOS comes almost free with the stack but is not a target. |

## 2. Core principles

1. **Local-first.** Each device holds the complete data for every list it belongs to and works fully offline. The network only exchanges changes. There is no "source of truth" server.
2. **No required infrastructure; many independent layers.** We use open networks (Nostr, the BitTorrent DHT), free best-effort public services (n0's iroh relays, public STUN) and our own users' devices (helpers, §7a). **Best-effort services are non-critical parallel layers, never counted as a guarantee** (D50). The goal stays ≥ 2 independent providers per role; §6 lists where that isn't met yet.
3. **Everything sent to third-party infrastructure is end-to-end encrypted.** Relays, DHT nodes, helpers and the like see only opaque data and unavoidable metadata (§9). **Storage the user owns** (their devices, folders, cloud accounts) is the user's own trust domain.
4. **Sync does not depend on the transport.** Every channel carries the same encrypted, signed sync messages. Receiving a change twice, out of order, or from several channels at once is harmless (CRDT, §4). Channels run **in parallel, not as primary and backup**, and every function (sync, discovery, invites) works through all of them (D55).
5. **Degrade gracefully.** Losing any transport or third-party service only makes sync slower, never impossible and never lossy.
6. **Respect the platform.** Work within Android's background rules (Doze, standby buckets, network cuts, the app freezer). Never rely on loopholes Google Play could ban.
7. **Reusable core.** The sync, identity, storage and transport engine is a generic library with no task-specific code. Future "ultimate" apps reuse it unchanged.
8. **Always backward compatible, forever.**
   - Every message and stored file carries a format version.
   - **Nothing is ever removed or changes meaning.** Fields, message formats and transports are only *added*; every new version keeps speaking every old format.
   - **Old clients keep working** with what they understand and **preserve and pass on untouched** what they don't. They never corrupt it. A per-group **feature floor** keeps new container types unused until every device in the group supports them (an old client re-encoding unknown containers could lose them, D28).
   - The only loss: an old client can't *display* data it doesn't understand.
   - When a client receives data from a newer format version, it shows "A newer version is available to use certain features" with an **Update** button.
   - **The notice can't be abused.** Messages carry only a version *number*, never text or links. The notice text is built in, and **Update** opens only our own store listing or release page, and only after confirming a newer version exists (Play In-App Updates on Android, GitHub Releases on desktop).
   - **Required (security) updates are never triggered by peer data**, only by Play's in-app update priority or a release manifest signed with our key (§7b).
   - **Dependency update policy:** dependencies (iroh, iroh-blobs, Loro, nostr…) are pinned to exact versions. An update is accepted only when automated tests prove old and new app versions interoperate in both directions over the **golden corpus** of every format ever written. Otherwise keep the old version, or run old and new protocols side by side. **Exception: security fixes**, applied with the same proof or with both protocol versions supported.
   - **Sole exception:** a flaw in the cryptography itself may force retiring an old format, announced ahead through the update notice.
9. **Licenses: permissive + MPL-2.0.** MIT, Apache, BSD and MPL-2.0 (file-level copyleft; only modified MPL files would have to be published). **No GPL/AGPL.**
10. **Defaults over settings.** Pick the right behavior instead of asking. A setting exists only if a normal user would plausibly want something different. Engineering knobs belong in a hidden diagnostics screen. Problems are raised when detected, not pre-emptively through options.
11. **Security without friction.** Attacks are defeated with mitigations that cost normal users nothing: informative existing screens, distinct formats, undo and recovery, automatic key rotation. Security never shortens lifetimes or adds steps for everyone.
12. **All incoming data is untrusted, even from group members:** signature checks, size limits, strict validation, isolated imports (§9), fuzz testing of every parser.

### User-facing settings (complete list)

| Setting | Default | Platforms |
|---|---|---|
| Sync in the background (off = sync only while the app is open) | On | All |
| Check for updates: Normal / More often (uses more battery) | Normal. ⏳ offered only if its battery cost measures acceptable (§8) | Android |
| Notify me when others change a list | On | All |
| Help others sync: *Wi-Fi and charging (+ Wi-Fi while open)* / *Wi-Fi while open* / *Off* | Wi-Fi and charging. Turning it off is easy but explained (§7a) | All (desktop: On / Off) |
| Backup: connect Google Drive, or choose a folder (desktop); export / import | Android's built-in backup is always on (§11). Cloud / folder backup off, but prompted once on devices with no other peers | All |
| Start with computer | On | Desktop |

Sub-options found only under their parent setting: "Memory for helping others" and "Keep this PC awake while plugged in" (under Help others sync, §7a).

Hidden diagnostics screen: per-layer connection status, relay and helper lists (including developer-operated select nodes, §7b), storage per group, sync log, "Export log".

Principle 10 limits *behavioral* settings. **Appearance options** are a separate area: they can be as rich as needed, but live one level deeper and never affect the default experience (see [UX-PRINCIPLES.md](UX-PRINCIPLES.md)).

### Appearance (technical)

- **The active choice is per device, never synced.** Which theme a device uses lives in a local device-preferences store, outside all group documents, and is included in that device's backups.
- **Themes are data, not code:** a versioned set of design tokens (color scheme, typography scale, density, shapes, list layout options). Built-in themes use the same format as custom ones.
- **Shared through the group:** "Share this look with [group]" adds the theme to the group document's theme collection. Members find it under Appearance → "From your groups". Nothing is applied automatically.
- **List icon and color** are part of the list's data, so every member sees the same identity.
  - A list stores a **hue** (0–360°, optionally a vividness level) in the **HCT** color space, never a literal color. **Each theme decides the tone**, so list colors never clash with a theme (Material Color Utilities, Apache-2.0). Grey has no hue and is its own slot.
  - The picker shows ~12–16 curated hues; "More colors…" opens a full hue wheel.
  - Color is only a small accent (dot, icon tint), never the only signal: it always comes with the icon or name.
- **Background images** are part of a theme. Readability comes from see-through content surfaces, scrims behind headers, text color chosen from the image's brightness, and **the theme's whole palette generated from the image**. **Background images never leave the device**; sharing a theme shares everything except the image.
- Link / file theme sharing for people outside your groups is a low-priority extra.
- **The platform always wins where the user has already chosen:** system dark mode, font size and accessibility settings. Material You dynamic color (Android 12+) is an option.

## 3. Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ UI: Kotlin + Compose Multiplatform                           │
│   Android (Jetpack Compose, Glance widgets, WorkManager)     │
│   Desktop JVM (Windows, Linux): tray app                     │
├──────────────────────────────────────────────────────────────┤
│ Domain: tasks (Rust)  ← app-specific, thin                   │
├──────────────────────────────────────────────────────────────┤
│ Engine (Rust, generic, reusable across apps)                 │
│   documents (Loro) · groups, keys & epochs · storage         │
│   sync protocol (signed, per-device slots, document epochs)  │
│   import sandbox (separate process)                          │
│   transport manager + capability-based mailbox interface     │
│   transports: iroh P2P · Nostr · helpers · Mainline DHT · LAN│
│   operator manifest (signed optional config)                 │
└──────────────────────────────────────────────────────────────┘
      bridged to Kotlin via UniFFI (Android: NDK .so, desktop: JNA)
```

- **Why a Rust core:** the best P2P (iroh) and CRDT (Loro) libraries are Rust. One core compiles for Android (arm64, armv7, x86_64) and desktop (Windows, Linux x86_64 and arm64). The stack isn't dogmatic: another P2P stack could run alongside iroh if lab data favors it (⏳ D49).
- **Why a Kotlin UI:** Jetpack Compose is Google's own toolkit and keeps up with Play first. Glance widgets, WorkManager, Block Store and Auto Backup are native Kotlin APIs. Compose Multiplatform reuses the UI on desktop.
- **Capability-based mailbox interface:** each mailbox backend declares what it can do (replace, max size, time-to-live, renew, read authentication, append-only, push/subscribe, rate budget), so Nostr, helpers and any future mailbox (⏳ D48) plug in without assuming Nostr's semantics.

## 4. Data model

- **CRDT library: [Loro](https://loro.dev/)**, pinned (≥ 1.16.2). Maps, movable lists, movable trees and rich text, so any future feature is a new field or container, never a new sync mechanism.
- **Documents:**
  - One **group document**: members, their device keys and names, the helper set (§7a), the list index, group settings, shared themes, open invites (§5).
  - One **document per list.** Lists sync independently.
- **Tasks** are tree nodes with open-ended properties; unknown fields survive round-trips.
- **PeerIDs (D1):** a Loro PeerID is bound to a device key (a 64-bit hash of it). It's **never backed up** and never reused: a restored or second install gets a fresh identity. Two writers sharing a PeerID silently and permanently diverge (measured, E1/A5). The desktop app is single-instance.
- **Authorship (D2):** every update blob is signed by the device. A device may publish **only its own ops**: after importing into the isolated copy (§9), only the sender's PeerID may have advanced. There's **no shared snapshot slot**; its jobs are done by document epochs.
- **Document epochs (D24):** deleted items never leave a Loro document's state (measured: 114–171 KB after 2 years of churn vs 1.1 KB fresh). Periodically, a member proposes a **fresh document built from the live state**. Every member checks it equals its own state, then switches. Compaction and epochs happen only at versions **every device active in the last N days has acknowledged** (D26); devices that were away longer re-apply their unsynced edits on the new document (the **rebase path**, tested in E1). This also makes "trash purged after 30 days" real.
- **History:**
  - At least **30 days** of full history are always kept regardless of size (D27), so any destructive edit can be undone. "Undo changes by [person] since [time]" is built on it.
  - Beyond that, history is budgeted by content amount. ⏳ D25 (technical, to measure): full history within a window, plus small live-state snapshots stored separately at widening intervals (Loro can't keep sparse history inside one document).
  - Phones use a tight budget; desktops a larger one and act as the group's archive.
- **Soft size limits (D4):** large updates are split into pieces, never rejected. **Priority lanes:** small, recent edits first; heavy content in the background, preferring Wi-Fi and P2P. Text above ~100 KB becomes a compressed **text attachment** (a blob). Per-author quotas **throttle** instead of rejecting. A hard stop only at absurd sizes, with a clear message. Document size stays below the Android Auto Backup quota (25 MB).
- **Large files (blobs): images and text attachments.** **Images come after v1** (D52), once iroh-blobs is stable.
  - Stored content-addressed (BLAKE3), encrypted with the group key, referenced by hash.
  - Transfer through all channels at once: **P2P (iroh-blobs)**, **Blossom** servers that accept encrypted blobs (best-effort), and **helper and member desktops** holding blobs for their groups.
  - **Silent failure:** every image reference carries a **ThumbHash** (~25 bytes) for an immediate blurred preview that sharpens when the image arrives. No status text.
  - Blobs can be re-fetched from the group, so backups only include what that device needs.

## 5. Identity, groups and sharing

- **Device identity:** each install generates an Ed25519 keypair. It doubles as the iroh endpoint key. ⏳ A separate network identity per group is measured for cost and adopted only if seamlessness doesn't suffer (D23).
- **Personal group = a person without an account.** Every install creates a **personal group** that only the user's own devices join. It holds the display name, the device list and the secrets of all other groups. History shows **people**, not devices.
- **Display names, not unique usernames.** The name lives in the personal group and is published to each group. Asked the first time the user shares or joins a list, never at first launch. Generated avatars (initials on an HCT hue from the identity key). Duplicate names are told apart by avatar color and a short tag ("Alex · 4F2A"). No duplicate-device suggestions (D33 dropped).
- **Device linking (D6), works everywhere, no proximity requirement:**
  - Linking = joining the personal group. A device that already has lists merges them, **shown on the same confirmation screen**.
  - The single confirmation names what you're joining: *"This will join Mark's account: Pixel 7 · Mark's PC · 12 lists (Work, Groceries…)"*. A phishing victim sees a stranger's devices and lists.
  - Link codes look different from invites and are **refused on the join path** with a plain explanation.
  - Link codes are **single-use and valid for a few hours**.
  - **"New device linked · Undo"** appears on every device for a while. Undo removes the device and automatically rotates the keys of every list involved.
- **Groups and key epochs (D5):** a group is a random 256-bit secret; the working keys live in **epochs** that rotate. Rules, validated by **every receiver** (simulated in E8: 3,000/3,000 runs after rule 4):
  1. **Strong remove:** a removed device can never be re-added.
  2. **Concurrent rotations merge:** a deterministic merger creates one successor whose recipients are the *intersection* of the concurrent epochs.
  3. **Additions** are valid only when signed by a recipient of the current head epoch (no ghost devices from old or leaked secrets).
  4. **Every new epoch excludes every removal its creator knows about.**
  5. A removal carries the remover's version vector; the removed author's ops outside it are rejected.
  6. **All network-visible identifiers** (Nostr tags, DHT keys, slot keys, invite lookups) derive from the current epoch key and rotate daily. A stable internal group ID never appears on the wire.
  7. **Periodic rotation** about every 30 days and on every device add, which heals silent leaks.
  8. After the personal group rotates, Block Store is updated at once.
  - Also: version, group, epoch, slot and sender are bound as AEAD associated data; a hybrid post-quantum suite ID is reserved; every join notifies the group.
  - **Removing a device or a person** is a rotation. They keep what they had and get nothing new. Mutual removal (A removes B while B removes A) removes both.
- **Membership:** anyone in the group is a full author; no roles. **Leaving** is local. A professional mode with roles can come later without engine changes.
- **Solo lists are groups with one member**: one code path.
- **Invites: one mechanism, three wrappers (D7, D8, D55):**
  - An invite is **one 8-word code** from a **curated EFF short word list** (unpleasant words and first names removed). Every word is identified by its **first 3 letters** (autocomplete), and no single typo turns a word into another valid one. ~81+ bits.
  - The **link** (`jurisgames.com/join#words…`) and the **QR** carry the same code. The link/QR may also carry the inviter's current device addresses for a direct connection. The fragment never reaches a server.
  - Derivation: **Argon2id (m = 64 MiB, t = 3, p = 1 floor, versioned)** over the code, with a **date-based salt**, gives the lookup key and the encryption key. An attacker can only test guesses against invites from the same day (measured attacker rate: 783 guesses/s per RTX 3080, A1).
  - The invite record (~0.5 KB: current epoch key, group name, inviter name and addresses) is published and fetched through **every layer at once**:
    1. the inviter's device directly (via discovery records);
    2. same-Wi-Fi local discovery, even without internet;
    3. any group member device (open invites are stored in the group document);
    4. the BitTorrent DHT (encrypted, ≤ 1,000 B, re-posted hourly by member devices);
    5. helpers;
    6. Nostr;
    7. operator-manifest select nodes;
    8. Nearby (later).
  - **Multi-use:** any number of people until expiry. **Valid 30 days** (the key-rotation period), shown in the sent message ("valid until 24 Oct"). It ends early if the group's key rotates sooner. The inviter can cancel any time. Every join notifies the group.
  - **Install from an invite:** the link's page offers "Get the app"; on first launch the app checks the clipboard description first (no toast) for an invite, with "tap the link again" as the fallback. Clipboard content is marked sensitive where possible.
  - **iPhone users:** not supported; the invite page says so clearly (D45).

## 6. Transport stack

All layers run in parallel (principle 4). **Preference order when several work** (decided):
- **Live (both online):** same Wi-Fi → direct internet (IPv6, router-mapped port, public IP, hole punch) → relay through a member's device → helper relays → n0 relays.
- **Offline (mailbox):** member devices (incl. an own always-on machine) → helpers (incl. select nodes, §7b) → Nostr (and a second mailbox if ⏳ D48 picks one) → own Google Drive (own devices only).
- **No network at all:** Nearby (later).

| Layer | Mechanism | Depends on | Notes |
|---|---|---|---|
| Same Wi-Fi | iroh direct + mDNS (separate crate, pre-1.0) | Nothing external | **Rotating LAN identifiers** derived from group keys, never the stable device ID (D57). Android 17 needs the local-network permission (§12). |
| Direct internet | iroh over IPv6, router port mapping (UPnP/NAT-PMP/PCP), public IP | Networks that allow it | Addresses advertised through our own discovery (below). ⏳ D49 lab |
| Hole punching | iroh QUIC NAT traversal, **in-band**: it needs an existing relayed connection to coordinate | A relay (n0, helper or select relay) | iroh **no longer uses STUN**; it learns public addresses through its relays (QAD). ~70% realistic success (independent study); phone↔phone on carrier NAT usually needs a relay. |
| Relay through a member | Engine logic | A reachable member device | |
| Helper relays | Helper desktops running an iroh relay (Let's Encrypt IP-address or domain certificate) | Reachable helpers | ⏳ D49 (lab running) / D51 (strangers' live traffic) |
| Select relays | Developer-run relays via the operator manifest (§7b), e.g. `relay.jurisgames.com` on the developer's PC or a free VM | Optional bonus | Hosting evaluation queued after D49 |
| n0 public relays | iroh default relays | n0 ("development and hobby use only") | Best-effort only (D50). May drift from old iroh versions over years. |
| Mailboxes | Nostr (§7), helpers (§7a), member devices | See sections | |
| Nearby | Android Nearby Connections | Play Services | Later; `play` flavor only |

- **Discovery ("where am I" records) through three providers:** an encrypted per-device slot on Nostr, **Mainline DHT** records via pkarr (client mode, published directly, not via n0), and n0's DNS. Endpoint addresses from these are fed to iroh (`add_external_addr`, custom address lookup).
- **Signaling:** Nostr ephemeral events plus relays.
- **No doorbell (D47):** nothing that needs a push server or user setup. "Someone changed X" comes from the phone's own background sync (§8).

## 7. Nostr mailbox

- **Per-device slots only.** Per group, each member device has one **addressable slot** holding its own changes since the current document epoch, merged into one Loro update. Each write *replaces* the previous one, so mailbox size is bounded per device, not per edit. Epoch switches (§4) reset slots.
- **A dedicated event kind** (addressable range), **not NIP-78**: NIP-78 was amended on 2026-09-03 to serve kind 30078 only to its author. Our custom kind behaved correctly on 8/8 probed relays (E4).
- Events are encrypted (XChaCha20-Poly1305, epoch key; header bound as associated data) and signed with a per-device, per-group key that rotates with the epoch. Per-message format: version, rotating topic tag, encrypted payload.
- **Quiet-citizen write policy (D11):**
  - an engine-wide budget of **~6 events/min per relay, shared across all groups**, burst 5;
  - foreground slot rewrites at most every **30–60 s**, with jitter;
  - each group uses a **subset of relays** (e.g. 3 of 8, chosen from its epoch key);
  - stop at the first `rate-limited:` and back off exponentially with jitter;
  - `created_at` strictly increasing per slot, within relay clock-skew limits.
- **Sizes (D12):** ≤ 64 KiB of event JSON (strfry's default event limit; the advertised 128 KB–1 MB is the frame size). Chunks ≤ 40 KiB with generation/index/count/hash, switched atomically between two buffers. Document epochs keep most lists in one chunk.
- **Relay health by reading back:** "OK" doesn't mean stored. The app tracks which relays actually keep and return events. The default relay list ships with the app and is updated through the **signed operator manifest** (§7b).
- Every device keeps full data, so relays are a mailbox, never the only copy. Slots are refreshed by background sync.
- **Reality (R2, E4):** works today on the big free relays (fresh keys, custom kinds, opaque content accepted). Risks, accepted for launch (D13): per-IP rate limits (Damus bans 1 h after repeated rejections), operator hostility to "relays as storage", strfry monoculture, relay churn. The fallback is our own network (member devices, helpers, select nodes).

## 7a. Helper network (core layer, built right after Nostr)

Devices act as **helpers**: a user-run mailbox for other groups, like torrent seeding. **On by default on every device** (D20); off is easy but explained.

- **Desktops** help whenever the app runs. They're the most reliable helpers.
- **Phones** help **only on Wi-Fi**:
  - *Wi-Fi and charging (default):* back-to-back WorkManager jobs while charging (up to ~30 min each while charging), plus while the app is open. **Never a foreground service** for helping (Play policy; it can't even start from the background).
  - *Wi-Fi while open:* only while the app is in use.
  - Never on mobile data, in battery saver, at low battery, at elevated thermal status, or with data saver on.
  - Play-policy risk ("proxy services to third parties") judged low (encrypted sync data for the same app's users; BitTorrent precedent). A fallback Play build where phones help only their own groups is kept ready.
- **RAM only for strangers' data;** member devices may keep their own groups' mailbox on disk. **Survive sleep:** desktop helpers keep RAM across sleep (the process is suspended, not restarted). **Hand off before going offline:** on quit/shutdown, and before a charging job ends, a helper passes its copies to online set members.
- **Complementary schedules (E5b):** day desktops plus night-charging phones cover the clock *if their windows overlap*. With overlap, sleep-not-reboot and handoff, a small community (5 desktops + 50 phones) reached ~94%, and 10 + 200 ~97%. Selection prefers complementary schedules over raw uptime, and time-zone and IP/ASN diversity.
- **Slot model** as §7: only the latest version of each slot is kept.
- **Helper set per group, recorded in the group document**, chosen by reliability. Bootstrap for first joins uses helpers closest to a **secret-keyed, daily-rotated** point (HMAC(epoch key, date)), plus Nostr, so outsiders can't pre-position fake helpers.
- **Replication:** start around **r ≈ 20** (E5: RAM-only, phone-heavy helpers need 15–20 for ~99% without member refresh), adapting to measured reliability. Churn is correlated by time of day, so the independence formula is only a starting point. The sender uploads to 2–3 helpers; they replicate to the rest of the set. Repair picks replacements when helpers vanish.
- **Reliability records** per helper (reachability, returned what was stored, returned the latest version, latency), **shared only within one's own groups**, decaying over time:

  | Observation | Effect |
  |---|---|
  | Returns data failing signature / hash verification | Immediate exclusion |
  | Loses data stored during its current session, with no declared eviction | Heavy |
  | Returns an older version of a slot it received directly from the writer this session | Heavy (proven withholding) |
  | Returns an older version received via replication | Light, only as a persistent pattern |
  | Unreachable | Light, relative to its own normal uptime |
  | New session (restart, incl. power loss), reported via session start time | Light, counted like downtime |
  | Declared eviction for space | Very light |
  | Slow | Ranking only |

- **Selection:** weighted random by reliability × free capacity; a small probation share for new helpers; silent exclusion of failing helpers; periodic re-probing.
- **Attacks:** smearing impossible from outside; DoS only lowers rank temporarily; junk-filling countered by load-scaled proof of work on new slots, per-group quotas and priority for established, actually-read groups; **young groups get capped capacity** so new (real or fake) groups can't displace established ones quickly.
- **Trust tiers:** 1) member devices (own groups, on disk); 2) **select nodes** from the operator manifest (§7b); 3) earned helpers. An own always-on machine simply joins as a member.
- **Caps and eviction:** memory scaled to the device (e.g. 16–32 MB phones, 64–256 MB desktops, adjustable under "Memory for helping others"). Per-group quota; slots unrefreshed for ~2 weeks become first to evict when space is needed; background sync by any member refreshes them.
- **Relaying live connections for strangers:** ⏳ D51 (waits for lab data). Relay capability exists regardless; developer select relays are the early guarantee.
- **Exposure, disclosed in the privacy text:** a helper's IP is visible to the devices it serves; it sees encrypted data and who uses it, never content.

## 7b. Operator manifest (optional developer capacity, D56)

- An **optional, signed file** listing "select" capacity with priority weights: the developer's devices, any future servers or free-tier VMs, iroh relays, Nostr relay list updates, STUN servers.
- **Ships with a baseline copy.** Updates are fetched from several sources (jurisgames.com, a public GitHub repo, a Nostr event, the DHT), so no single host matters.
- **Verified with a separate offline signing key.** A compromised website or GitHub account can't forge it.
- **Hard rules:** it can only *add* candidates, never disable P2P, Nostr or helpers. Select nodes run the normal helper protocol (ciphertext only, same reliability checks). The last good copy is kept if all sources fail.
- Diagnostics and the privacy text disclose developer-operated nodes.
- ⏳ Evaluating free hosting for select entries (Oracle Cloud Always Free, Google Cloud e2-micro, Cloudflare Workers feasibility) is queued after the D49 lab.

## 8. Android background behavior and lifecycle

| State | Behavior |
|---|---|
| App open | Live on every layer. |
| Leaving the app | Every change is already saved locally. An **expedited WorkManager job**, enqueued while the app is still visible (on pause and after commits), sends pending changes: no notification, and it bypasses Doze network limits. `shortService` is only an optional fallback (its notification deferral is rate-limited). |
| Background | WorkManager periodic sync (15 min minimum). **On idle phones Doze stretches this to hours** (maintenance windows about +1, +3, +7, +13 h), and a widget doesn't prevent it. Android 15+ cuts background network seconds after leaving, and the freezer (14+) kills sockets 10 s after caching. Android 16 adds job quotas even for active apps. |
| "More often" (opt-in) | ⏳ Silent `setAndAllowWhileIdle` checks a few times per hour in Doze, offered only if measured battery cost is acceptable. Not an alarm clock: it never overrides notification settings or Do Not Disturb. |
| App launch | The splash screen is held until the first sync round or ~1.5 s. Later merges never move a row under the user's finger. |
| Widgets | Jetpack Glance (checkboxes are backported below Android 12). Every change redraws the widget. Shows an **absolute** time ("Updated 14:05"), since relative labels freeze between redraws. |

- **Notifications about others' changes** come only from background sync, built from merged state (batched, showing their age, dropping stale ones).
- **Honest wording:** no "15 minutes" promise. Something like "updates can take a few hours when your phone is idle", plus per-member freshness.
- **Blocked background sync** (aggressive OEM skins) is detected and explained once, with a button to battery settings. The exemption is never requested directly.
- **iroh on Android:** initialize the JNI context before creating an endpoint; call `network_change()` from a `ConnectivityManager` callback; shut the endpoint down in the background; take a `MulticastLock` for mDNS.
- **Desktop has none of these limits.** The tray app stays connected and starts with the computer.

## 9. Security and privacy model

| Party | Sees |
|---|---|
| Group members | Everything in the group (by design) |
| Nostr relays | Encrypted blobs, timing, sizes, uploader IP. **Can tell which groups one device belongs to** (IP, connection, timing) |
| Helpers (incl. select nodes) | Encrypted slots, IPs of devices using them, timing |
| n0 relays and DNS, DHT nodes | IP addresses, endpoint IDs, rotating discovery keys, who connects to whom and when |
| Google (Android backup) | Nothing readable: E2E-encrypted with the screen lock, or **sealed** when there's no lock (D30) |
| The user's own cloud (opt-in) | A sealed file |
| Anyone else | Nothing |

- **Privacy text states metadata plainly** (D23/D54): "operators of the networks we use can see which devices sync with each other and when, never what."
- **Import sandbox (D3):** untrusted blobs are imported first into a copy of the live document in a **disposable process** (Android: a service in its own process; desktop: a child process). Fuzzing Loro 1.16.2 aborted the process within seconds (4 crash sites, an 85-byte minimum), and a poisoned lock turns the panic into an abort, so in-process catching can't work (A6). Prototype: the crash stays in the child, the parent survives (~18 ms per validation on desktop), and impersonation is caught by the same check. Plus a **crash-loop breaker** (a quarantine journal) and `panic = "unwind"` builds.
- **Hardening (D57–D60):**
  - rotating LAN identifiers;
  - **signed releases**, verified by any self-updater;
  - no local control interface, or localhost-only with a key and cross-site-request protection;
  - **no path injection, ever:** file names are always generated by the app, peer-provided names are never paths, links and special names are rejected, and all of it is fuzz-tested.
- **Group secrets and device keys** live in OS secure storage (Android Keystore-backed, Windows Credential Manager, Linux Secret Service). They leave the device only in Block Store, sealed backups, full backups and invite records. Never in logs or Markdown exports.
- **Play compliance:** standard encryption declared (exempt); Data safety with no collected user data beyond what's required; the privacy policy discloses relays, helpers and developer-operated nodes.

## 10. Storage

- **Opaque by design:** Loro binary format in app-private storage (Android internal; `%APPDATA%` / `$XDG_DATA_HOME`). Not encrypted with device-held keys, which would make restored backups useless.
- **Crash-safe writes:** temporary file, then an atomic rename. Snapshot plus an append-only update log. Raw received blobs are kept until nothing pending references them.
- **Measured sizes (E6, E9):** the whole Rust core (iroh + Loro + nostr + crypto) is **7.07 MB installed / 4.33 MB compressed** on arm64 with the size profile, and is 16 KB page-aligned (NDK 29). Estimated app ≈ 15–20 MB installed, ≈ 10 MB download. Targets: ≤ 20 MB download, ≤ 40 MB installed, 50 MB hard ceiling, enforced by a size check in the build.
  - Release profile: `opt-level = "z"`, LTO, 1 codegen unit, **`panic = "unwind"`** (D3), stripped.
- **Data sizes (E1):** 5 years of daily shared-list use ≈ 0.35 MB; 10k tasks load in ~28 ms on desktop.
- **Desktop installers** (Compose + a Java runtime) ~60–100 MB, accepted.

## 11. Backup and recovery

| Scenario | Recovery path |
|---|---|
| New phone, restored during setup | **Block Store** (≤ 1 KB entry: the personal group secret) + Android backup; a fresh device identity is created (D1). Data comes from members, helpers and Nostr |
| New phone, restore skipped | **First-launch prompt:** link from another device (the default recovery) or restore a backup file (D31) |
| Solo lists, Android | Android backup (daily, ≤ 25 MB), plus mailbox copies, plus cloud backup if connected |
| Solo lists, desktop | Cloud backup (Google Drive) or an automatic backup folder, prompted once when a device has no other peers |
| Phone + desktop in the same group | They back each other up automatically |

- **Android backup:** E2E-encrypted when a screen lock exists; **sealed** otherwise (D30). Never turned off. Device identity files are excluded.
- **Every backup that leaves the device is sealed** (a random key inside the file): unreadable to automated scanning, readable with deliberate effort. No backup password. Backups contain live group secrets, so periodic key rotation (§5) limits how long a stolen backup is useful.
- **Opt-in cloud backup:** **Google Drive** with `drive.file`, a non-sensitive scope that needs **no verification** (brand verification is optional); publish the OAuth app "In production". Sign-in via OAuth + PKCE. Google sign-in in the `foss` flavor is likely impossible (custom URI schemes are unsupported for Android OAuth clients). **OneDrive is postponed** (D42). A connected cloud also syncs the *same user's* devices.
- **Exports:** readable Markdown (Obsidian Tasks compatible, one-way) and a full `.tasklist` backup file (with history, re-importable).
- Data is lost only if no second copy ever existed.

## 12. Platforms and distribution

| Platform | Build | Channels |
|---|---|---|
| Android | Gradle, Rust via cargo-ndk (NDK 29). `minSdk` 26, `targetSdk` 36, then 37 as Play requires | **Google Play** (package registration in progress), GitHub Releases APK. F-Droid later (`foss` flavor; needs open source, §14) |
| Windows | Compose Desktop | GitHub Releases. ⏳ D39 at the Windows release: unsigned installers get SmartScreen warnings; the Microsoft Store is free for individuals |
| Linux | Compose Desktop | AppImage, .deb / .rpm on GitHub Releases (Flathub not pursued) |
| iOS | – | **Not supported** (D45) |

- **Signing:** one app-signing key (`com.jurisgames.tasks`, RSA 4096, valid to 2054) for both Play (Play App Signing, "use my own key") and GitHub APKs, so installs from either source update each other. Backed up to a private repo (encrypted); password in Credential Manager and the developer's password manager.
- **Public repo** (separate from the private code repo) for the join page on jurisgames.com, releases, the static update manifest and the operator manifest (D41). Update checks read the static manifest, not the rate-limited GitHub API.
- **Older Android versions:** Material You → theme colors below 12; E2E backup → sealed below 9 or without a lock.
- **Permissions beyond the defaults** (explained first, asked only when needed):

  | Permission | Purpose | When |
  |---|---|---|
  | `POST_NOTIFICATIONS` (13+) | Reminders, change notifications | First time relevant |
  | `SCHEDULE_EXACT_ALARM` | On-time reminders (task apps aren't eligible for `USE_EXACT_ALARM`) | When a timed reminder is first set or received; **re-requested after a restore** (it's denied on restored 14+ devices), alarms rescheduled on grant (D43) |
  | `ACCESS_LOCAL_NETWORK` (only when targeting 37+) | Same-Wi-Fi sync; covers LAN traffic, mDNS, CGNAT ranges and same-LAN IPv6 | When a group member is detected on the same network. Nothing needed on SDK 36 |
  | Camera | QR scanning in the `foss` flavor only | When scanning (`play` uses the Google code scanner, which needs no camera permission) |
  | Bluetooth / Nearby Wi-Fi | Nearby (later) | When first used |

  - **Location: not planned.** Desktop: Windows Firewall shows its own prompt the first time the app accepts connections, explained just before.
- **Product flavors:** `play` (Block Store, Nearby, Google sign-in, code scanner) and `foss` (no proprietary Play libraries). The engine has no Play Services dependency.

## 12a. Engineering practices

- **Repository:** a private GitHub repo with a Rust workspace (`engine`, `tasks`, `ffi`) plus a Gradle project (Android, desktop, shared Compose UI).
- **Simulated-network tests:** many engine instances over fake transports that drop, delay, duplicate and reorder, including long offline periods. Merges must converge.
- **Property tests:** key-epoch rules (E8 as a CI test), capability-based mailbox backends, document-epoch switching and rebase.
- **Fuzzing in CI:** every parser and message format, Loro import through the sandbox, file-name and path handling (D60).
- **Golden corpus:** every format ever written, with old↔new interoperability tests gating every dependency update (principle 8).
- **Diagnostics:** "Export log" (never secrets or task content). Crash reports from Play Console vitals; local log on desktop.
- **Libraries:** RustCrypto (`chacha20poly1305`, `hkdf`, `argon2`), rust-nostr, iroh, Loro, UniFFI, pkarr. Measured alternatives for later (E9): libp2p 3.45 MB, str0m 2.12 MB, webrtc-rs 2.34 MB, veilid-core 6.79 MB on arm64.
- **Research record:** `docs/research/` holds all reports, experiments, attacks and decisions behind this document.

## 13. Economics

Every obligatory cost is paid by users' own devices and bandwidth or absorbed by open networks and free best-effort services. Nothing required gets more expensive as the number of users grows. The developer may add optional capacity (§7b) to improve early stability, but the app never depends on it. That's what makes "free for everyone, forever" credible.

## 14. Open source (undecided)

- **License rule:** permissive + MPL-2.0 (principle 9), so every option stays open.
- **For:** "E2E, no servers" becomes verifiable, F-Droid eligibility, longevity if the maintainer steps away (the Syncthing-Android lesson), and the mission's "open apps".
- **Against:** easy clones on stores (trust comes from the official listing and the signing key), unsolicited contributions (ignorable for a hobby project).
- **If opened later:** the app license decides whether forks must stay open (GPL-style) or not (MIT-style). GPL wouldn't unlock major capabilities; it would mainly make Session / Veilid client code reusable.

## 15. To verify

- **D49 lab** (running): P2P and relaying without n0 across NAT types; helper relay resource use.
- **D48:** second mailbox (mock interface, then local Session storage server and an isolated Veilid network).
- **D51:** how many helpers are directly reachable; ISP reactions to residential relays.
- Relay hosting for select nodes (free-tier terms and limits).
- Android: the import sandbox's cost on low-end phones; the battery cost of "More often" checks; a per-group network identity's cost; phone helper reachability during charging jobs; Argon2 timing on a low-end phone.
- DHT lookup latency from phones (invites, discovery).
- Loro: behavior with MB-sized text on low-end phones (sets the text-attachment threshold); history checkpoint sizes (D25).
- The developer's own connection: can it accept incoming connections (public IPv4 / port mapping) for a select relay?

## 16. Build order

1. **Foundation:** Rust engine skeleton (pinned deps, `panic = "unwind"`), Loro documents with signed per-device updates, the import sandbox, storage, UniFFI bridge, minimal Compose UI on Android and desktop (local-only). Size budget in the build.
2. **Identity and groups:** device keys, personal group, key epochs (E8 rules), device linking (D6), secure storage.
3. **Nostr sync:** per-device slots, write budget, chunking, relay health, the operator manifest (baseline + signed updates).
4. **Invites:** one code, link and QR; published and fetched through every available layer (grows with phases 3–6).
5. **P2P with iroh:** LAN (rotating IDs), direct internet, discovery via Nostr + DHT + n0, member relaying, helper/select relays per the D49 outcome.
6. **Helper network:** storage helping on desktops and phones, reliability records, handoff, selection.
7. **Android lifecycle:** expedited-job flush, periodic sync, splash sync, widgets, notifications from merged state.
8. **Backup:** Android backup (sealed without a lock), Block Store, first-launch restore prompt, desktop backup folder, exports, Google Drive.
9. **Document epochs and history policy** (needed before lists grow large).
10. **Later:** images (iroh-blobs + Blossom + helpers), "More often" checks, Nearby, a second mailbox (D48), headless engine build, select-node hosting.
