# L4: Waking sleeping Android phones without our own server (gap G3)

*Layer research for the Kleiders Tasks transport stack. Researched 2026-09-24 against DESIGN.md §6 ("Doorbell slot"), §7, §7a, §8, DECISIONS D16/D17 and FINDINGS #9 / R5-16 / R6-3. Sources: UnifiedPush docs, specs and source code (Codeberg), Firebase and Android docs, ntfy docs and its live public tier listing, the Google APIs Terms, GitHub code and issue search, and crates.io. Research only: nothing was installed or built, and no push message was sent to any service. The one live endpoint read was ntfy.sh's public `/v1/tiers` pricing JSON.*

---

## Summary

- **A content-free doorbell with $0 cost and no server run by us is possible for most Android users today.** D17 assumed that FCM needs a sending server. That is only true for Firebase's own HTTP v1 API. Google's FCM servers also accept **standard Web Push requests (RFC 8030 + RFC 8291 encryption + RFC 8292 VAPID)** at `https://fcm.googleapis.com/fcm/send/<token>`. A native Android app can get such an endpoint from Play Services **without a Firebase project** by registering with a VAPID public key it generated itself. After that, **anyone who holds the matching VAPID private key can push**, with no Google credentials, no account and no server. In our case that means any member device of the group. UnifiedPush documents this ("Google FCM servers can handle webpush requests out of the box, but they require a VAPID authorization") and ships it as the library `org.unifiedpush.android:embedded-fcm-distributor` 3.x (3.0.0, 2025-01-31: "Use FCM Webpush endpoints with VAPID, without a gateway… The library is now fully FOSS"). It's in production in DAVx5 (Push on by default since 2026-04), Nextcloud Talk, Fedilab, SchildiChat Next and two Nostr clients (0xchat, YakiHonne).
- **The catch is that Google doesn't document it.** The client side calls Play Services' private C2DM / Messenger IPC with a `wp:` subtype, which is how Chrome registers. UnifiedPush's own blog calls it "not (well?) documented by Google". The Google APIs Terms say "You will only access (or attempt to access) an API by the means described in the documentation of that API." That's a real, non-zero policy and breakage risk. Google could close it at any time, and the library already had to switch IPC mechanisms once (3.1.0, 2026-06). **So it must stay an optional accelerator layer (principle 4), never something sync depends on.** When it fails, we're back to today's behavior.
- **Two things are still unverified and decide how much it's worth:**
  1. Whether a Web Push `Urgency: high` header becomes an Android **high-priority** delivery that wakes a phone in deep Doze.
  2. How FCM's rule applies: "If FCM detects a pattern in which messages don't result in user-facing notifications, your messages may be deprioritized to normal priority" (7-day window).

  Normal priority is "Deferred to doze maintenance window", which is about what WorkManager already gives us. Both can be measured on one phone (experiments E1 and E2).
- **UnifiedPush with external distributors (ntfy, Sunup, NextPush, Conversations) works but isn't seamless.** The user has to install a second app, grant it a battery-optimization exemption and keep a permanent foreground-service notification (ntfy registers UnifiedPush topics with `instant = true, // No Firebase, always instant!`, even in its Play build). It's the right answer for phones **without Play Services** (de-Googled or Huawei) as a user choice, and a poor default for everyone else. The UnifiedPush connector library (Apache-2.0) lets one integration serve both paths.
- **Rejected:**
  - Sending FCM HTTP v1 from devices with an embedded service-account key (Firebase: "Securely store the JSON file containing the key"; server environments must be "Able to securely store server authorization credentials"). One extraction lets anyone send arbitrary visible notifications to every user.
  - Asking for `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` (a to-do app isn't in Android's acceptable-use table).
  - Our app opening its own socket to Mozilla's autopush (needs a persistent connection, and Mozilla publishes no third-party permission).
  - The redundant gCompat-UP app.
- **A small extra:** while-idle alarms (`setAndAllowWhileIdle`) can poll during Doze more often than WorkManager (docs: "about every minute" normally, "such as 15 minutes" in idle; a 7/hour cap in Doze; standby-bucket caps of 10/2/1 per hour). Each alarm puts the app on the "temporary power exemption list for approximately 10 seconds". It's a cheap, fully local accelerator, not a doorbell, but it can help phones without Play Services. Whether network access works inside that window on Android 15+ is unverified (E6).

---

## How the recommended doorbell would work (for orientation)

1. **Registration (receiver phone).** For each group it belongs to, the device generates a P-256 **VAPID keypair**. It registers a Web Push subscription with the embedded FCM distributor (UnifiedPush connector API: `UnifiedPush.register(context, instance, message, vapidPublicKey)`) and gets back an endpoint `https://fcm.googleapis.com/fcm/send/<token>`. RFC 8291 also gives it a `p256dh` public key and an `auth` secret.
2. **Publication.** The device writes `{endpoint, p256dh, auth, VAPID private key, created}` into its member record in the encrypted group document. Only group members can read it.
3. **Ringing (any member device, including desktops).** After a change is flushed (the expedited job from D16) and the target device wasn't reached live over P2P, the sender builds a Web Push request:
   - an RFC 8291 `aes128gcm` body of a few bytes (e.g. "sync group slot X", itself also sealed with the group key);
   - a `Topic` header so FCM collapses repeated rings;
   - `Urgency: high`;
   - a moderate `TTL`;
   - `Authorization: vapid t=<JWT signed with the shared private key>, k=<public key>`.

   It POSTs this directly to the endpoint. Rust: `web-push-native` 0.5.0 (MIT OR Apache-2.0, 2026-07-26) plus any HTTP client. Kotlin needs no extra library if the Rust core sends.
4. **Wake (receiver).** Play Services delivers the raw bytes to the app's receiver. The app gets "several seconds" (more for high priority). It enqueues an expedited sync job that fetches the group's Nostr slots or connects over iroh, then shows a notification from **merged state** (FINDINGS #9).
5. **Membership changes.** When a member is removed (key epoch, E8), every device re-registers with a fresh VAPID key and unregisters the old subscription. Old keys then only reach dead endpoints.

No component here is run by us. The only infrastructure is Google's FCM, which already runs on every Play-certified phone.

---

## Candidate-by-candidate evaluation

### C1. Web Push protocol (RFC 8030 + RFC 8291 + RFC 8292 VAPID) as the sender-side protocol

- **Gaps:** G3 (primary). Partial G5: payloads carry up to 4096 bytes encrypted, about 3993 bytes of cleartext per the UnifiedPush spec. Partial G1 for tiny messages only: push services store messages until the TTL expires (FCM: up to 28 days).
- **Can ANY sender push with only a VAPID keypair?** **Yes.** A subscription is a capability URL, optionally bound to a VAPID public key at subscribe time. The push service checks only that the JWT is signed by the matching private key. UnifiedPush intro: "VAPID is a mechanism to authenticate requests to a push server, using asymmetric keys… The mobile application fetches the public key and uses it to register to the distributor". Autopush docs: "When the VAPID key is provided, autopush will return an endpoint that can only be used by the application server that provided the key." FCM requires VAPID: "this authorization is known as VAPID and is optional with most distributors, but required by FCM."
- **Which push services accept it:**
  - FCM (`fcm.googleapis.com/fcm/send/…`, VAPID required);
  - Mozilla autopush (VAPID optional; "the VAPID asserion block must contain a sub field");
  - ntfy (UnifiedPush topics `up*`, anonymous write);
  - NextPush;
  - Conversations' rewrite proxy.

  The standard is identical across all of them, so one sender implementation reaches every distributor a user might pick.
- **Cost:** $0 to users and to us. The sender is a user device.
- **ToS:** Web Push is an open IETF standard. Terms attach to the receiving push service (see C2 and C4).
- **Privacy:** the push service sees the sender IP, the target endpoint, size and timing. The content is end-to-end encrypted (RFC 8291). We add a second layer with the group key and pad to a fixed size.
- **Libraries:**
  - `web-push-native` 0.5.0 (2026-07-26, MIT OR Apache-2.0, ~94k recent downloads). Sans-IO, so it fits our own HTTP stack.
  - `web-push` 0.11.0 (Apache-2.0), last release 2025-02-22. Staler.
  - `ece` 2.4.2 (MPL-2.0, Mozilla). Avoid, or accept MPL.
  - `jwt-simple` (ISC) for VAPID JWTs if needed.
- **Abuse resistance:** only holders of the VAPID private key can push. For us that's group members. Removed members lose it through rotation.
- **Precedents:** Mastodon servers, WebDAV-Push servers (DAVx5 ecosystem) and Nextcloud all push Web Push directly to user-chosen endpoints.
- **Failure modes:**
  - old libraries implementing the `aesgcm` draft ("check the content-encoding header: it must be aes128gcm");
  - 404/410 on dead subscriptions, which means re-registering;
  - SSRF advice ("reject push endpoints that resolve to non-global IP addresses"). This matters for us because endpoints come from group members. **Only send to an allow-list of known push hosts or to https with a public IP.**
- **Verdict: ADOPT** (as the doorbell wire protocol). **Confidence: high.**

### C2. FCM Web Push endpoints for a native app, through Play Services (UnifiedPush `embedded-fcm-distributor` 3.x or our own re-implementation)

- **Gaps:** G3 (primary, for devices with Google Play Services). Partial G5 and a tiny G1 (4 KB messages, stored by FCM up to TTL, max 28 days).
- **How it works (source-verified):**
  - `C2mRequests.kt` sends Play Services a registration bundle with `scope=GCM`, `sender`/`subscription` set to **the app's own VAPID public key**, and `subtype = "wp:<channelId>"`. It does this through Messenger IPC to `com.google.android.gms`.
  - `Constants.kt` builds the endpoint as `https://fcm.googleapis.com/fcm/send/%s`.
  - The receiver is exported with `android:permission="com.google.android.c2dm.permission.SEND"` and reads the raw POST body from the `rawData` extra.
  - The method comes from microG developer Marvin Wißfeld's (mar-v-in) public gist ("WebPush… Sending messages: Regular WebPush server request to `https://fcm.googleapis.com/fcm/send/<token>`").
  - The library is **843 lines of Kotlin** with a single dependency (kotlinx-coroutines). It contains no Google binaries ("It doesn't contain any Google proprietary blobs").
- **Cost:** $0. No Firebase project, no `google-services.json`, no gateway ("If your application supports VAPID, you have nothing special to do"). The `DefaultGateway` at `fcm.distributor.unifiedpush.org` is only needed for senders that *can't* do VAPID (Matrix, for example). We control the sender, so we never need it.
- **ToS / permission:**
  - **Not documented by Google.** UnifiedPush: "What is less known, and not (well?) documented by Google, is that you can directly send webpush requests to FCM servers"; and "being able to register a VAPID pubkey retrieved during execution is not possible with the firebase-messaging library."
  - Google APIs Terms §3.c: "You will only access (or attempt to access) an API by the means described in the documentation of that API". §4.a also prohibits "Reverse engineer…" and requires that you not "Interfere with or disrupt the APIs".
  - The *sending* side is standard Web Push, which FCM advertises for web apps. The *registration* side imitates Chrome through private IPC. I found no Google statement that permits or forbids this for native apps.
  - Signals that it's tolerated so far: DAVx5 (a paid Play Store app) has shipped it with Push on by default since 2026-04; Nextcloud Talk, Fedilab, SchildiChat Next, 0xchat and YakiHonne depend on it (GitHub code search: `embedded-fcm-distributor` in their build files).
  - **Risk rating: medium.** The likely outcome is that it stops working, not that the app gets banned.
- **Operators:** one, Google. Very sustainable, but a single point of policy control.
- **Reliability / limits (FCM docs; they apply to the FCM backend, and whether they apply identically to `wp:` registrations is inferred):**
  - "up to 240 messages per minute and 5,000 messages per hour to a single device";
  - collapsible messages "a burst of 20 messages per app per device, with a refill of 1 message every 3 minutes";
  - "The FCM server can simultaneously store four different collapsible messages per device";
  - non-collapsible: "a limit of 100 messages that can be stored without collapsing. If the limit is reached, all stored messages are discarded";
  - TTL "from 0 to 2,419,200 seconds (28 days)";
  - "FCM doesn't apply collapsible message throttling to messages with a time to live value of 0 seconds".
- **Priority / Doze (the key unknown):**
  - Android docs: in Doze, FCM "High priority: No execution limits / Normal priority: Deferred to doze maintenance window".
  - Firebase: "High priority… FCM attempts to deliver high priority messages immediately even if the device is in Doze mode. High priority messages are for time-sensitive, user visible content."
  - Deprioritization: "If FCM detects a pattern in which messages don't result in user-facing notifications, your messages may be deprioritized to normal priority… 7 days of message behavior". Android 13+: "App Standby Buckets no longer determine how many high priority FCMs an app can use."
  - Firebase's own example maps the web `Urgency: high` header to web priority. **Whether `Urgency: high` sent to a native `wp:` registration arrives as Android high priority is not documented.** Measure it (E1): Play Services puts `google.delivered_priority` / `google.original_priority` extras on deliveries.
- **Privacy / metadata:**
  - Google learns (sender IP) → (recipient Play Services instance, and so the Google account) with timestamps. That's a social-graph edge between group members.
  - Content is invisible to Google (RFC 8291 plus our group AEAD).
  - UnifiedPush's own analysis: "If you use FCM, you are constantly connected to Google servers… Government agencies have been asking Apple and Google for metadata related to push notifications" (quoting Sen. Wyden).
  - This is a new exposure compared with the current design. It needs to be disclosed, and a hidden "use Google to wake this phone" switch would be reasonable.
- **Android feasibility:**
  - Needs no permissions of our own beyond the manifest receiver.
  - Play Services keeps its own MCS connection, so there's **no battery or FGS cost to us**.
  - Works on Play-certified phones only; no Huawei (HMS) or de-Googled ROMs without sandboxed Play. The library has a check (`Add tip to check if the Play Services are installed`; DAVx5 issue #2200).
  - The UnifiedPush UX guide says the embedded distributor "should not be used by default if the user has another distributor installed".
- **Library maturity:**
  - v3.1.0 (2026-06-23), 3.0.0 (2025-01-31), minSdk 16, NLnet/NGI-funded, single maintainer (s1m).
  - **License: LGPL-2.1.** That brings replacement/relinking obligations in an APK, which interacts awkwardly with R8 minification. It's small enough to **re-implement cleanly** from the gist's documented facts (about 500 lines) under our own license.
  - Connector `org.unifiedpush.android:connector` 3.3.4 (2026-08-20), **Apache-2.0**.
- **Abuse resistance:**
  - Only VAPID-key holders can push. A malicious *member* could spam-wake a phone (battery DoS). FCM caps it at 240/min, and the app must also coalesce (at most one sync per N seconds per group; ignore rings while a sync is running).
  - Anyone who steals an endpoint *without* the VAPID key can't push (the endpoint is VAPID-bound).
- **Failure modes:**
  - Google changes the IPC or the backend (it happened once: the 3.1.0 switch to "Messenger based IPC").
  - Token rotation (must re-register at every app start, as the spec recommends: "From time to time, like every time the application starts, C register to D").
  - Deprioritization to normal priority.
  - OEM task killers that block broadcast delivery to force-stopped apps.
  - No GMS.
- **Verdict: ADOPT, as an optional best-effort doorbell layer**, gated on E1–E3. **Confidence: medium.** It's the only candidate that meets $0, no developer infrastructure and zero user setup for mainstream users. Its failure mode degrades gracefully to today's behavior.

### C3. UnifiedPush connector with a user-chosen external distributor (general)

- **Gaps:** G3 for phones without Play Services, or for users who don't want Google.
- **Cost:** $0 to us. The push server is run by the distributor's operator or by the user.
- **Setup burden:** high for non-technical users. They install a second app, open it, "Grant the necessary permissions (e.g., notifications and battery optimization exemptions)", then pick it in our app. The spec says selecting a default distributor "must always be done from a user view".
- **Android feasibility:** the distributor holds a persistent WebSocket in a foreground service. The battery cost is the distributor's, shared across all apps that use it.
- **Library:** the same Apache-2.0 connector as C2, so supporting both costs almost nothing extra. The UX guide covers "Default: Onboarding screen" logic: if only one option is available, register silently.
- **Precedents:** Molly, Tusky, Element-family clients, DAVx5, FluffyChat.
- **Verdict: CONSIDER** (support it through the same connector, never prompt for it; show it only in diagnostics or settings for users who already have a distributor installed). **Confidence: medium-high.**

### C4. ntfy app + public ntfy.sh server (as a UnifiedPush distributor)

- **Gaps:** G3 (and it *could* serve G5 or small G1 as a generic pub/sub mailbox, but see ToS and limits).
- **Cost:** free tier: "You may use the Service without creating an account or subscribing to a paid plan. Free usage is subject to rate limits". Paid tiers fund it: Supporter $6/mo, Pro $12/mo, Business $25/mo (from `ntfy.sh/v1/tiers`).
- **Terms (ntfy.sh ToS, updated 2026-01-26):**
  - Prohibited: "Circumvent rate limits or other technical restrictions"; "Use the Service in a manner that could reasonably be expected to impose an unreasonable or disproportionately large load on our infrastructure".
  - "The Service is provided on a 'best effort' basis. We do not guarantee any specific uptime". "Messages will be delivered successfully or in a timely manner" isn't warranted.
  - There's no clause against third-party apps. UnifiedPush use is the advertised default.
- **Operator:** one company, ntfy LLC (Philipp Heckel), in Connecticut, USA. Funded by subscriptions and sponsors. Self-hostable (Apache-2.0 / GPLv2 dual license).
- **Limits (free, per publisher IP):**
  - `"messages":250` per day;
  - message cache `"messages_expiry_duration":43200` s (12 h);
  - "Each message can be up to 4,096 bytes long";
  - request bucket "60 requests… refilled at a rate of one request every 5s";
  - IPv6 limited per /64.
  - Subscriber-based limiting for `up*` topics exists (`visitor-subscriber-rate-limiting`); whether ntfy.sh enables it is not documented.
- **Play version and FCM:** "The ntfy Android app uses Firebase only for the main host ntfy.sh, and only in the Google Play flavor". Without instant delivery, messages "may arrive with a significant delay (sometimes many minutes, or even hours later)". **UnifiedPush registrations are always instant (a foreground-service WebSocket), even in the Play flavor.** The source says `instant = true, // No Firebase, always instant!` (`up/BroadcastReceiver.kt`), and the manifest declares `foregroundServiceType="specialUse"` plus `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`. So the Play build does **not** relay UnifiedPush through FCM.
- **Privacy:** ntfy.sh sees topic, IP and timing. Topic names act as passwords. Encrypted Web Push bodies hide content.
- **Android:** a permanent notification, a battery exemption, a second app.
- **Failure modes:** a single operator; rate limits; the 12 h cache means a phone offline for more than 12 h misses rings (harmless, since the ring is only an accelerator).
- **Verdict: CONSIDER** only as one user-chosen distributor under C3. **REJECT** as a default, and **REJECT** as a mailbox (G1): a single operator, and "unreasonable load" terms. **Confidence: medium-high.**

### C5. Sunup + Mozilla autopush (`push.services.mozilla.com`)

- **Gaps:** G3 for users who install it.
- **Facts:**
  - Apache-2.0 app, 1.3.3 (2026-09-08); "Technology: WebSockets".
  - The default server is Mozilla's production autopush (`DEFAULT_API_URL = "https://push.services.mozilla.com"`).
  - Distributed via F-Droid, IzzyOnDroid and Accrescent. **Not on Google Play** (issue #15 open since 2025-02).
  - Users must "grant background usage without restrictions permission".
- **Terms:** I found **no published Mozilla policy that allows non-Firefox clients** on Mozilla's push servers. Sunup's privacy text had to borrow Firefox's policy (issue #62). The service is hosted on Google Cloud (issue #62), and it's IPv4-only (autopush-rs issue #1090, open).
- **Libraries:** autopush-rs is MPL-2.0 (server; irrelevant unless someone self-hosts).
- **Verdict: CONSIDER** only as a user-chosen distributor (C3). **REJECT** as anything we'd default to or point users at. **Confidence: medium.**

### C6. gCompat-UP (standalone UnifiedPush-to-Play-Services distributor app)

- Same mechanism as C2, packaged as a separate app ("mainly designed for testing purposes"). It's on Google Play, Apache-2.0, 2.1.2 (2026-09-08).
- **Verdict: REJECT.** It's redundant with the embedded C2 and adds an install step. **Confidence: high.**

### C7. Mozilla autopush directly from our app (our own WebSocket, no Sunup)

- A phone would need to hold a WebSocket open. Android 15+ cuts background network about 5 s after leaving the app (D16), so this needs a foreground service. That's just "Stay connected" (§8, not in v1) with a different server, and there's no evidence Mozilla allows it. The autopush FCM *bridge* only works for app IDs configured by Mozilla (`/v1/{type}/{app_id}/registration`), and FCM bridging limits data to "only 2744 bytes".
- **Verdict: REJECT.** **Confidence: high.**

### C8. FCM HTTP v1 sent directly from devices (service-account key embedded in the app)

- **Cost:** a free Firebase project, but in practice the developer's credential becomes public.
- **Terms and docs:**
  - "Firebase projects support Google service accounts, which you can use to call Firebase server APIs from your app server or trusted environment… Securely store the JSON file containing the key."
  - The server environment must be "Able to securely store server authorization credentials and client registration tokens."
  - UnifiedPush summarises: "these keys are considered sensitive; you must ensure they are kept secret. So you must not share them."
- **Failure modes:**
  - Anyone who extracts the key from the APK can send **visible notification messages with arbitrary text** to every token they learn, and can use whatever other IAM roles the account has.
  - Google may disable the project. That kills push for every user at once, and we're the accountable party.
  - Rotating the key requires an app update.
  - The legacy server-key API (which also had this problem) was retired by Google in 2024. That's from memory and wasn't re-fetched here.
- **Verdict: REJECT.** **Confidence: high.**

### C9. A developer-owned Firebase project with a static project VAPID key (Web Push to `firebase-messaging` tokens)

- UnifiedPush notes "it is also possible to upload your VAPID keys on the Firebase Console, if you use a centralized application and want to push with Webpush". The private key would then have to ship inside the app, so anyone could push to any leaked token. It's undocumented whether native `firebase-messaging` tokens accept Web Push at all. It's strictly worse than C2, which uses per-device keys and needs no project.
- **Verdict: REJECT.** **Confidence: low** (not tested; rejected on design grounds).

### C10. Other free wake mechanisms on Android

- **High-priority FCM semantics** (the mechanism C2 relies on):
  - "In Doze or App Standby mode, the system delivers the message and gives the app temporary access to network services and partial wakelocks."
  - The handler gets "Several seconds… with slightly more time allocated for high priority messages".
  - Our design should start the expedited sync job from the handler and never do network work inline.
- **Battery-optimization exemption (`REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`):**
  - Android's acceptable-use table allows it for chat apps that *can't* use FCM, safety apps, task-automation apps and companion apps. Chat apps that can use FCM are "Not Acceptable". Play: "prohibit apps from requesting direct exemption… unless the core function of the app is adversely affected".
  - Even when exempt, "Exempted apps cannot: Run jobs and syncs… Fire regular AlarmManager alarms" in Doze. The exemption alone doesn't wake anything; it only helps a persistent connection.
  - **Verdict: REJECT** (policy). **Confidence: high.** Keep the existing §8 approach of linking to settings only on hard evidence of blocked sync.
- **While-idle alarms as a Doze-time poll (`setAndAllowWhileIdle`, no permission needed):**
  - "When the alarm is dispatched, the app will also be added to the system's temporary power exemption list for approximately 10 seconds"; "it will not dispatch these alarms more than about every minute… when in low-power idle modes this duration may be significantly longer, such as 15 minutes".
  - Power table: Doze "While-idle alarms: Limited to 7 per hour"; buckets Working set 10/h, Frequent 2/h, Rare 1/h, Restricted 1/day.
  - Docs warn the alarm "must only be used for situations where it is actually required… cause significant battery blame".
  - It's a poll, not a doorbell. Each run costs a radio wake and hits Nostr relays (their rate limits apply).
  - **Unverified:** whether network access is actually allowed during that ~10 s window on Android 15+ (E6).
  - **Verdict: CONSIDER** as a low-rate accelerator (for example, one poll per 30–60 min while there are shared groups with recent activity), mainly for phones without GMS. **Confidence: low-medium.**
- **Persistent connection in a foreground service ("Stay connected"):** already evaluated in D16/§8. It works technically but has Play FGS-type issues and a permanent notification. It remains opt-in or later and isn't re-evaluated here.

---

## Comparison table vs gaps

G1 offline store-and-forward, G2 internet P2P without n0, G3 wake phones, G4 free relay, G5 signaling, G6 blobs. "—" means not covered.

| Candidate | G1 | G2 | G3 | G4 | G5 | G6 | $0 / no dev infra | User setup | Verdict | Confidence |
|---|---|---|---|---|---|---|---|---|---|---|
| C1 Web Push protocol (sender side) | tiny (≤4 KB, TTL) | — | **yes** (needs an endpoint source) | — | partial | — | yes | none | **ADOPT** | high |
| C2 FCM Web Push via embedded distributor | tiny (FCM stores ≤28 d; 4 collapse keys) | — | **yes, GMS phones (~most)** | — | partial | — | yes (Google infra) | **none** | **ADOPT** (optional layer) | medium |
| C3 UnifiedPush external distributor (generic) | — | — | yes, if installed | — | partial | — | yes | high | CONSIDER | medium-high |
| C4 ntfy + ntfy.sh | 12 h cache, 250 msg/day/IP | — | yes (FGS WebSocket) | — | partial | — (2 MB attachments; not suitable) | yes (third party) | high | CONSIDER as user choice / REJECT as default or mailbox | medium-high |
| C5 Sunup + Mozilla autopush | — | — | yes (FGS WebSocket) | — | — | — | yes (unclear permission) | high; not on Play | CONSIDER as user choice only | medium |
| C6 gCompat-UP | — | — | yes | — | — | — | yes | medium | REJECT (redundant) | high |
| C7 Own socket to autopush | — | — | only with FGS | — | — | — | unclear ToS | n/a | REJECT | high |
| C8 FCM v1 + embedded service account | — | — | yes | — | — | — | credential leak | none | REJECT | high |
| C9 Project VAPID + firebase-messaging | — | — | unverified | — | — | — | shared secret in APK | none | REJECT | low |
| C10a Battery-opt exemption | — | — | no (alone) | — | — | — | yes | medium | REJECT (Play policy) | high |
| C10b While-idle alarm polling | — | — | partial (poll ≤7/h Doze, bucket-capped) | — | — | — | yes | none | CONSIDER | low-medium |

---

## New risks introduced by adopting C1 + C2

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| NR-L4-1 | **Undocumented Google interface.** Registration uses Play Services private IPC (`wp:` subtype). Google APIs Terms require access "by the means described in the documentation". Google can break or forbid it; it changed once already (3.1.0 IPC switch). | Medium | Keep the doorbell strictly optional (principle 4). Detect failures (registration fail, 404/410, no ring received in self-test) and fall back silently. Track the UnifiedPush repo. Keep the Web Push sender generic so another endpoint source can be swapped in. |
| NR-L4-2 | **Metadata to Google.** The sender IP → recipient Play Services instance (Google account) edge, with timing, reveals who shares lists with whom. This is new compared with Nostr-only mailboxes. | Medium (privacy posture) | Disclose it in the privacy text. Add a hidden or diagnostics switch. Pad bodies to a fixed size. Optionally delay or batch rings (UnifiedPush recommends it: "fixed-length push notifications, or certain batches"). A member desktop can ring on a phone's behalf, which hides the phone's IP. |
| NR-L4-3 | **Deprioritization.** Rings that don't lead to a visible notification may be downgraded to normal priority (7-day pattern), which is no better than WorkManager in Doze. | Medium | Ring only for changes the recipient will be notified about (other members' edits to shared lists), and show a notification built from merged state. Measure (E2). |
| NR-L4-4 | **Battery DoS by a malicious or buggy member** holding the VAPID key. | Low-Medium | Coalesce on the receiver (at most one sync per ~60 s per group). Ignore rings during a sync. Use a `Topic` header on the sender. Rotate VAPID keys at every membership epoch (E8 key epochs). Relevant to A5 (malicious member). |
| NR-L4-5 | **SSRF / amplification from endpoints supplied by members.** A hostile member could publish an "endpoint" pointing at a LAN host, and other members' devices would POST to it. | Medium | Send only to https hosts on an allow-list of known push services (fcm.googleapis.com, push.services.mozilla.com, ntfy.sh, …). For custom UnifiedPush servers, require a public, non-private IP after DNS resolution and never follow redirects (the UnifiedPush server-security guidance). |
| NR-L4-6 | **LGPL-2.1** for `embedded-fcm-distributor`. | Low | Re-implement the ~500-line core under our own license, or accept LGPL with a relinkable module. Decide alongside P9 (MPL decisions). |
| NR-L4-7 | **VAPID private key inside the group document.** A leaked group doc allows spam-waking (not reading) every member phone. | Low | Per-group, per-device keys; rotate on epoch; the receiver ignores payloads that don't decrypt under the group key. |
| NR-L4-8 | **Expectation creep.** Instant rings on GMS phones but not on de-Googled or Huawei phones makes latency uneven between members. | Low | Keep the D18 honest status ("Saved → Sent → On Alex's phone"). Per-member freshness already covers this. |

---

## Suggested SAFE local experiments

All use only the user's own devices and accounts. Each sends at most a few dozen messages to Google's FCM, which is normal client traffic. No public relays are involved.

| # | Question | Method | Pass criterion |
|---|---|---|---|
| E1 | Does `Urgency: high` to a `wp:` registration arrive as Android high priority and wake a phone in deep Doze? | Test APK with a C2-style registration and a receiver that logs `google.delivered_priority`, `google.original_priority` and the arrival time. Send from a desktop with `web-push-native`. Force Doze with `adb shell dumpsys deviceidle force-idle`, and also test with a still phone, screen off, for 30+ min. Compare `Urgency: high`, `normal` and none. | High-priority deliveries arrive within seconds in forced and natural deep Doze. The extras show `high`. |
| E2 | Deprioritization. | Over 8–10 days, send ~10 rings per day to two test installs. Install A always posts a notification; install B never does. | Record when B's `delivered_priority` drops to `normal`. Decide the "notify on ring" policy. |
| E3 | Can the receiver actually sync on a ring in Doze on Android 15/16/17? | On the ring, enqueue an expedited WorkManager job that fetches one Nostr slot (use a private test relay on the LAN) and posts a notification. | The job starts and the network works inside the allowlist window; the sync completes in under 10 s. |
| E4 | Collapsing and throttling. | Send bursts of 30 rings with and without a `Topic` header and with TTL 0 vs 3600, some while the phone is offline. | Confirm the "burst 20 / 1 per 3 min" behavior and that Topic collapses to one stored message. Choose Topic plus a TTL. |
| E5 | Registration robustness. | Re-register at every app start; update Play Services; clear app data; reinstall; restore from backup; run on GrapheneOS sandboxed Play and on a no-GMS emulator. | The endpoint changes are detected, and old endpoints return 404/410 so senders drop them. There's a clean failure without GMS. |
| E6 | While-idle alarm polling (C10b). | `setAndAllowWhileIdle` every 15 min. Log whether a socket to a LAN test relay opens inside the ~10 s exemption on Android 15+, and the actual cadence under forced Doze for each standby bucket (`adb shell am set-standby-bucket`). | The network works during the exemption, and the cadence matches the docs (≤7/h in Doze). Otherwise drop C10b. |
| E7 | UnifiedPush external-distributor path. | Install ntfy (F-Droid build) pointed at a self-hosted ntfy on the LAN. Select it; send Web Push from the desktop. | Delivery works with the same sender code as E1, which proves one code path serves both. |
| E8 | Membership-change rotation. | Remove a member, then have that member's old device keep sending rings with the old VAPID key. | After re-registration, FCM rejects the old endpoint and the receiver sees nothing. |

---

## Suggested design text (for whoever edits DESIGN.md later; not applied here)

- §6 "Doorbell slot": replace "needs Google's push service (FCM), which needs a sending server. Deliberately not built." with: "A content-free doorbell over **standard Web Push (RFC 8030/8291/8292)**. Each device registers a VAPID-bound subscription (on Play-services phones, FCM Web Push through Play Services; elsewhere, any UnifiedPush distributor the user already has) and shares the endpoint and VAPID key inside the encrypted group. Any member device rings it directly. No server of ours. It's an optional accelerator; sync never depends on it."
- D17: option (b) no longer needs "a free stateless worker". The objection that led to declining it (developer infrastructure) doesn't apply to this variant. **It's still the user's decision.**

---

## Sources

- UnifiedPush intro (Web Push, RFC 8291, VAPID, server security): https://unifiedpush.org/developers/intro/
- UnifiedPush Android spec AND_3.1.0 (4096-byte message, VAPID, endpoint rules): https://unifiedpush.org/developers/spec/android/
- UnifiedPush implementations / embedded distributors: https://unifiedpush.org/developers/implementations/
- Embedded FCM distributor docs: https://unifiedpush.org/kdoc/embedded_fcm_distributor/
- Embedded FCM distributor source, releases and license (LGPL-2.1): https://codeberg.org/UnifiedPush/android-embedded_fcm_distributor (files `c2m/C2mRequests.kt`, `Constants.kt`, `impl/FirebaseReceiver.kt`, `DefaultGateway.kt`, `AndroidManifest.xml`)
- mar-v-in gist "Various methods to use Google Cloud Messaging" (WebPush registration and sending): https://gist.github.com/mar-v-in/2a054e3a4c0a508656549fc7d0aaeb74
- UnifiedPush blog "Push notifications for decentralized services" (2025-01-31): https://codeberg.org/UnifiedPush/documentation/src/branch/main/content/news/20250131_push_for_decentralized.md
- UnifiedPush blog on push security and privacy (2025-05-13): https://codeberg.org/UnifiedPush/documentation/src/branch/main/content/news/20250513_push_security_privacy.md
- UnifiedPush UX guide: https://codeberg.org/UnifiedPush/documentation/src/branch/main/content/developers/ux.md
- UnifiedPush distributors list and Sunup/ntfy/gCompat pages: https://unifiedpush.org/users/distributors/ , https://unifiedpush.org/users/distributors/sunup/ , https://unifiedpush.org/users/distributors/ntfy/
- UnifiedPush gateway page: https://unifiedpush.org/users/gateway/
- Common-Proxies README (FCM rewrite proxy only for non-VAPID senders): https://codeberg.org/UnifiedPush/common-proxies
- android-connector (Apache-2.0): https://codeberg.org/UnifiedPush/android-connector
- Sunup README and issues #15, #59, #62, #87: https://codeberg.org/Sunup/android
- autopush-rs README (MPL-2.0) and HTTP/bridge docs: https://github.com/mozilla-services/autopush-rs , https://mozilla-services.github.io/autopush-rs/http.html ; IPv6 issue: https://github.com/mozilla-services/autopush-rs/issues/1090
- ntfy Terms of Service (2026-01-26): https://docs.ntfy.sh/terms/
- ntfy phone docs (Play vs F-Droid, FCM, instant delivery): https://docs.ntfy.sh/subscribe/phone/
- ntfy publish limitations (250/day, 4,096 bytes): https://docs.ntfy.sh/publish/
- ntfy config (rate limiting, subscriber-based limiting for `up*`): https://docs.ntfy.sh/config/
- ntfy public tiers JSON: https://ntfy.sh/v1/tiers
- ntfy-android `up/BroadcastReceiver.kt` (`instant = true`) and manifest: https://github.com/binwiederhier/ntfy-android
- Firebase message priority (Android): https://firebase.google.com/docs/cloud-messaging/android/message-priority
- Firebase set message priority: https://firebase.google.com/docs/cloud-messaging/customize-messages/setting-message-priority
- Firebase message lifespan (28 days): https://firebase.google.com/docs/cloud-messaging/customize-messages/setting-message-lifespan
- Firebase collapsible messages (4 keys, 100 stored): https://firebase.google.com/docs/cloud-messaging/customize-messages/collapsible-message-types
- Firebase throttling and quotas: https://firebase.google.com/docs/cloud-messaging/throttling-and-quotas
- Firebase auth-server (service accounts): https://firebase.google.com/docs/cloud-messaging/auth-server
- Firebase server environment requirements: https://firebase.google.com/docs/cloud-messaging/server-environment
- Google APIs Terms of Service: https://developers.google.com/terms
- Android power details (FCM in Doze, alarm caps, bucket table): https://developer.android.com/topic/performance/power/power-details
- Android Doze and standby (FCM, exemption table, Play policy): https://developer.android.com/training/monitoring-device-state/doze-standby
- AlarmManager `setAndAllowWhileIdle`: https://developer.android.com/reference/android/app/AlarmManager
- web.dev Web Push protocol (Urgency, TTL, Topic, 4096 bytes): https://web.dev/articles/push-notifications-web-push-protocol
- Precedents: DAVx5 Push on by default https://github.com/bitfireAT/davx5-ose/issues/2151 and its `libs.versions.toml` (`unifiedpush-fcm = "3.1.0"`); WebDAV-Push spec https://github.com/bitfireAT/webdav-push ; Charm issue (why non-VAPID senders need a gateway) https://github.com/CloudHub-Social/Charm/issues/48 ; GitHub code search for `embedded-fcm-distributor` (Nextcloud Talk, Fedilab, SchildiChat Next, 0xchat, YakiHonne)
- crates.io: https://crates.io/crates/web-push-native , https://crates.io/crates/web-push , https://crates.io/crates/ece , https://crates.io/crates/unifiedpush

---

## Independent verification

*Skeptical second pass, 2026-09-24. Re-checked from primary sources where possible: Codeberg API and raw source for the library, Maven Central metadata and POM, GitHub code search on adopters' build files, the Chromium source (nwjs mirror), microG source, the live Google Play policy page, the Google APIs Terms, Firebase and Android docs, crates.io API and the ntfy tiers JSON. Nothing was installed and no push message was sent.*

### Verdict checks

| Candidate | Original | Verdict after check | Why |
|---|---|---|---|
| C1 Web Push (RFC 8030/8291/8292) as the wire protocol | ADOPT, high | **Upheld** | Library `Module.md`: FCM "can handle webpush requests out of the box, but they require a VAPID authorization". The mar-v-in gist says sending is a "Regular WebPush server request". `web-push-native` 0.5.0 (2026-07-26, MIT OR Apache-2.0, 93,617 recent downloads) confirmed on crates.io. C1 is only worth anything with an endpoint source (C2 or C3). |
| C2 FCM Web Push via Play Services (embedded distributor) | ADOPT (optional), medium | **Downgrade: CONSIDER, becomes ADOPT only if E1 and E2 pass. Confidence low-medium.** | The mechanism is real (source read). The *risk* is understated, and the *benefit* is unmeasured. Details below. |
| C3 UnifiedPush connector + user-chosen distributor | CONSIDER, high | **Upheld** | UX guide confirmed. One small fix: the connector is at **3.3.5** (Maven, 2026-08-22), not 3.3.4. DAVx5 already uses 3.3.5. |
| C4 ntfy + ntfy.sh | CONSIDER (user choice), high | **Upheld** | Live `/v1/tiers`: free tier `basis: ip`, `messages: 250`, `messages_expiry_duration: 43200`. ntfy-android `up/BroadcastReceiver.kt` line 85: `instant = true, // No Firebase, always instant!` (last commit 2026-07-09). Nuance: ntfy can switch `up*` topics to **subscriber-based** rate limiting (`visitor-subscriber-rate-limiting`, off by default). If ntfy.sh turns it on, the cap counts against the receiving phone, not the sender's IP. It makes no difference for a doorbell. |
| C5 Sunup + Mozilla autopush | CONSIDER (user choice), medium | **Upheld** | README: default `https://push.services.mozilla.com`, and the user must "grant background usage without restrictions permission". Issue #15 (Play Store) is still open (2025-02-27). Issue #62 (privacy policy, "apparently hosted by Google") is closed. autopush-rs #1090 (IPv6) is still open. Latest release 1.3.3, 2026-09-08. |
| C9 Developer Firebase project + static VAPID key | REJECT, low | **Upheld** | Nothing found that documents Web Push to native `firebase-messaging` tokens. The design objection (a shared private key in the APK) stands on its own. |
| C10b While-idle alarm polling | CONSIDER, low | **Upheld, confidence raised to low-medium** | AlarmManager text confirmed verbatim: "approximately 10 seconds", "about every minute", "such as 15 minutes", "significant battery blame". **New:** Android's power-details page says apps on the temporary allowlist get "Unrestricted network access (unless device is in data saver mode)". So network access in the window is documented. E6 is still needed for the Android 15+ background-network restriction and for OEM behavior. |
| C6, C7, C8, C10a (high-confidence REJECTs) | REJECT, high | **Upheld** | No viable path was missed. C8: even a least-privilege FCM-sender service account can send arbitrary notifications to every token in the project. C10a: the exemption table and Play policy are as stated. |

### Why C2 should be downgraded (evidence the report missed or understated)

1. **The main risk is Play-policy enforcement, not only breakage.** Google Play's Device and Network Abuse policy lists as a violation: "Apps that access or use a service or API in a manner that violates its terms of service." The Google APIs Terms (§**2.c**, "Permitted Access"; the report cites §3.c) require access "by the means described in the documentation". Together these give Google a documented route to act against the *app* on Play, and not just to stop the path working. So the report's "likely outcome is that it stops working, not that the app gets banned" has no evidence behind it. Two things mitigate this:
   - the registration IPC is the same one Google's own open-source `firebase-messaging` uses (`GmsRpc.java`, cited in the library's comments);
   - DAVx5 and Fedilab ship it on Play today.

   Still, it is only undocumented, and nothing sanctions it.
2. **Adoption on Play is thinner than listed.** Checked in the build files:
   - DAVx5 includes `unifiedpush-fcm = "3.1.0"` in `core` for every flavor. That's real Play evidence.
   - Fedilab uses `playstoreImplementation(...embedded-fcm-distributor:3.0.0)`. Also real.
   - **Nextcloud Talk's Play flavor uses `firebase-messaging:25.1.2`.** The embedded distributor is only in its `generic` flavor. A large vendor chose *not* to ship this path on Play.
   - **0xchat uses the older `foss-embedded-fcm-distributor:1.0.0`**, not the gateway-free 3.x VAPID path, so it's no evidence for C2.
   - SchildiChat Next pins 3.0.0.
3. **The license is ambiguous, not simply LGPL-2.1.** The repo `LICENSE` is LGPL-2.1, but the published Maven POM for 3.1.0 declares "The Apache License, Version 2.0". Ask the maintainer to clarify before linking, or re-implement cleanly (still feasible).
4. **Google is preparing a different web-push endpoint path.** Chromium's `push_messaging_constants.cc` has both `https://fcm.googleapis.com/fcm/send/` and `https://fcm.googleapis.com/wp/`, switched by the feature flag `kPushMessagingGcmEndpointWebpushPath` (off by default in the mirror checked). The library hardcodes `/fcm/send/%s`. Treat endpoints as opaque: allow-list the host (`fcm.googleapis.com`), never a path, and expect the endpoint shape to change.
5. **The benefit is unproven.** Firebase's priority page documents `Urgency: high` only for web push, and nothing for native `wp:` registrations. The Android power page says high-priority FCM is downgraded if an app "consistently" sends messages that don't result in a notification. Two cases the report didn't cover:
   - (a) **Users who deny `POST_NOTIFICATIONS` (Android 13+) can never produce user-facing notifications.** For them, rings will be downgraded to normal priority, which is no better than WorkManager in Doze.
   - (b) The library sends firebase-style ACKs (`supports_message_handled`, `RPC_ACK`), but how GMS decides "resulted in a notification" for `wp:` registrations is unknown.
6. **Library health.** It has one maintainer. Codeberg shows 3 stars, 1 issue and releases 3.0.0 (2025-01-31), 3.1.0-rc1 (2026-01-27) and 3.1.0 (2026-06-23). It is small and easy to audit, so this is acceptable for an optional layer. The code (`C2mRequests.kt`, `FirebaseReceiver.kt`, `Constants.kt`) matches the report's description.

### Important things the report missed

- **Data Saver breaks the follow-up sync.** Temporary-allowlist network access is granted "unless device is in data saver mode". On metered networks with Data Saver on, a ring can arrive but the sync job can't fetch. Detect this with `ConnectivityManager.getRestrictBackgroundStatus()` and show it in diagnostics.
- **microG phones may be covered too.** microG's `McsService.java` builds and delivers `rawData` extras, the same field the library reads, so C2 plausibly works on de-Googled phones with microG. microG issue #357 ("Web push notifications support", 2017) is still open, so this is **unverified**. Add microG to E5.
- **FCM registration lifetime.** Firebase: registrations inactive for 270 days are "expired" and garbage-collected; "stale" after one month without connecting. Senders must drop endpoints on 404/400, and receivers must republish their endpoint after every sync.
- **A policy-clean alternative for Chrome users (low value, noted for completeness).** A web page that subscribes via Chrome's documented `PushManager` gets a sanctioned FCM Web Push endpoint, and the same sender (C1) can ring it. It must show a notification (`userVisibleOnly`) and wakes Chrome's service worker, not our app. It also needs a hosted origin, and it's clunky. It's only a fallback if Google ever blocks C2.
- The UX guide's line "should not be used by default if the user has another distributor installed" means C2 is the default only on phones with **no** UnifiedPush distributor. Make it explicit in the design.

### Net effect on D17

The report's conclusion still holds: option (b) no longer needs developer infrastructure. It should come with three caveats:
- Play-policy exposure exists and is documented.
- The wake benefit is unmeasured until E1 and E2 run.
- Users who deny notifications or turn on Data Saver get little or nothing from it.

Recommended order:
1. Build the generic C1 sender and C3 connector support. These are policy-clean.
2. Run E1, E2 and E5 (with microG) on a test APK.
3. Ship C2 only behind an off-switch in settings, with a privacy disclosure.
