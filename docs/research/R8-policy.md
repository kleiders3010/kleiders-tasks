# R8: Policy and feasibility

*Research pass, 2026-09-24. Scope: Google Play policies, Google Drive and Microsoft Graph OAuth, clipboard invites, F-Droid, Flathub, winget, Microsoft Store / MSIX, GitHub Pages / Releases. Web research only. Nothing was installed or built, and no traffic was sent to relays. Every claim below is checked against a primary source where one could be fetched. Where only second-hand evidence exists, the text says so.*

## Summary

- **Deadline in 6 days: Android developer verification.** Google's policy-deadline page says Play apps must be registered by **2026-09-30** to "avoid global removal from Google Play". Off-Play installs (GitHub APK, F-Droid) will need a verified developer and a registered package name plus signing key: first in Brazil, Indonesia, Singapore and Thailand on 2026-09-30, then worldwide in 2027. For packages with under 50 installs, registration is first-come-first-served, so `com.kleiders.tasks` can be squatted. Register it now. The design doesn't mention any of this.
- **`kleiders.com` currently serves "Kleiders | AI Fashion Design & Sewing Pattern Generator for Brands"** (domain registered 2026-03-03). If that's the user's own business, it solves several problems below: Google brand verification, Flathub app-ID verification, App Links and a permanent link domain. If it isn't, the permanent package name `com.kleiders.tasks` breaks Flathub's app-ID rule and F-Droid's rule that an app ID must not match someone else's domain, and it creates a trademark risk. Settle this before the first upload.
- **The Google Drive row in §11 is wrong, in the design's favour.** `drive.file` is a **non-sensitive** scope. An app using only non-sensitive scopes needs no verification and gets no "unverified app" screen and no 100-user cap. Brand verification is **optional** and only makes the name and logo appear on the consent screen. Without it, "only your application domain will be visible". The real traps are the **Testing** publishing status (100 users, tokens expire after 7 days) and the **foss** flavour: new Android OAuth clients can't use custom URI schemes by default, and loopback redirects are blocked for Android clients.
- **A GitHub Pages (`*.github.io`) domain is very unlikely to pass Google's domain verification.** Search Console "Domain" properties need a DNS TXT record, which `github.io` subdomains can't have, and practitioners report Google rejecting `github.io` for OAuth brand verification (2025–2026). Plan on a custom domain (about $10–15 a year), or skip brand verification entirely.
- **"Free Microsoft app registration" is only partly true.** Since June 2024 every app registration must live in an Entra tenant. For an individual that means an Azure free account, with **credit-card and phone identity verification** (no charge). Publisher verification is effectively out of reach for an individual, because it needs a verified Partner (CPP) organisation, and apps registered with a personal Microsoft account can't be verified. Personal OneDrive users can still consent. Also, **since about 2026-08-23, newly consented personal accounts using `Files.ReadWrite.AppFolder` get 403 `serviceReadOnly` on every drive call**. The regression is reported by several developers and Microsoft hasn't acknowledged it.
- **Play foreground services:** `shortService` is explicitly **exempt** from Play's FGS policy criteria, so the "flush on leaving the app" design holds. `dataSync` is not exempt. The phone-helper fallback ("a foreground service only if WorkManager fails") fails Play's criteria (the service must benefit *this* user and must not be deferrable). Relaying for strangers also runs into the Device & Network Abuse rule: "Apps that facilitate proxy services to third parties may only do so in apps where that is the primary, user-facing core purpose."
- **Exact alarms:** a task app is **not** eligible for `USE_EXACT_ALARM` (only alarm/timer apps and calendar apps with event notifications are). `SCHEDULE_EXACT_ALARM`, as the design has it, is correct. Tasks.org, the leading FOSS task app, does the same. Beyond what the design covers: the permission is denied after a backup-restore onto a new Android 14+ device, and revoking it kills the app and cancels every exact alarm.
- **P2P and Nostr apps are fine on Play.** Amethyst, Primal (Nostr), Briar, Jami, Syncthing-Fork and Orbot (including its opt-in "Kindness Mode" Snowflake proxy for strangers) were all updated on Play in Jul–Sep 2026. A Data safety listing of "no data shared" is common among them.
- **Desktop distribution:** Microsoft Store registration is **free for individuals** since 2025-09-10 (government ID plus selfie), and the Store re-signs MSIX for free. Outside the Store, code signing costs money: Azure Artifact Signing is about $9.99 a month and open to **individuals in the US or Canada only**, an OV certificate costs $150–300 a year, and unsigned files hit a "strong SmartScreen block". That conflicts with C2. jpackage can't produce MSIX. MSIX virtualises `%APPDATA%` and deletes that data on uninstall. **Flathub** now has a generative-AI policy ("Flathub manifests must not contain AI-generated or AI-assisted content"; AI-generated code must be disclosed and may be rejected) and an "insufficient development history" rule. Both matter for this project.
- **GitHub:** Pages on the Free plan needs a **public** repo. The design has "one private GitHub repo", so the join page and the releases need a separate public repo. Release assets in a private repo aren't publicly downloadable. The unauthenticated REST API limit is 60 requests an hour per IP, so update checks should read a static manifest. The biggest GitHub risk is new: **invite links put group secrets in the fragment of a URL on a host we don't fully control.** If the GitHub account or custom domain is ever lost, renamed or taken over, whoever controls that host can serve JavaScript that reads the secret from every old link opened in a browser.

## Claim-by-claim table

| # | Claim (from DESIGN.md or the task) | Verdict | Severity if wrong | Fix (short) |
|---|---|---|---|---|
| 1 | `shortService` flush after leaving the app is Play-compliant and needs no special review | **Confirmed** | none | Keep. Start it inside the "transitions from a user-visible state" window (R4 to time it). |
| 2 | Phone helper may fall back to a (`dataSync`) foreground service, "may face Play review" | **Refuted** as viable | high (phase 7a) | Never use an FGS for helping others. Phone helpers serve only their own groups, or phone helping is dropped from the `play` flavour. |
| 3 | `specialUse` / persistent FGS for a future "Stay connected" | **Partial** | medium (future) | Allowed only with manifest justification plus Play manual review. `dataSync` is capped at 6 h / 24 h. Keep it out of v1, as planned. |
| 4 | Data safety: "no collected user data (E2E data is exempt)" | **Partial** | low–medium | E2E content and backups to the user's own cloud are exempt. Persistent endpoint IDs and IPs sent to third-party relays aren't E2E: disclose them or document why not. Privacy policy link in-app too. |
| 5 | `SCHEDULE_EXACT_ALARM`, asked when a reminder is first set or received | **Confirmed** (with gaps) | low | Keep. Handle denial after restore, the revocation broadcast and the inexact fallback. Never declare `USE_EXACT_ALARM`. |
| 6 | "`targetSdk` 36, then 37 as Play requires" | **Confirmed** for 2026; 2027 **unverified** | low | API 36 required since 2026-08-31 (extension to 11-01). The 2027 rule isn't published yet (expect API 37, around 2027-08-31). Add the 16 KB page-size check. |
| 7 | Export compliance "declared as standard encryption (exempt)" | **Partial** | low | Play has no such declaration; the obligation is contractual (DDA §4.6, §16.5). Under US EAR it's mass-market 5D992.c with no report needed, and open-source standard crypto needs no notification. Check the developer's own country's law. Reword §9. |
| 8 | No Play problem with P2P / Nostr apps | **Confirmed** | none | Evidence: Amethyst, Primal, Briar, Jami, Syncthing-Fork, Orbot all on Play in 2026. |
| 9 | Apps relaying other users' encrypted data (helpers) are OK on Play | **Partial / at risk** | high (phase 7a) | Play's "proxy services to third parties" clause. Orbot passes because proxying *is* its core purpose. Helper must be opt-in, disclosed, desktop-first. |
| 10 | `drive.file` needs brand verification; unverified → warning screen, 100-user cap | **Refuted** | medium (wrong plan) | Non-sensitive scope: no verification, no warning, no cap. Publish to "In production". Brand verification is optional (name/logo). |
| 11 | A free domain (GitHub Pages) is enough for Google verification (§15 open question) | **Likely refuted** | medium | Use a custom domain you own (or `kleiders.com` if it's yours), or skip brand verification. Experiment E-2 settles it. |
| 12 | `foss` flavour does Google sign-in "through the browser (OAuth)" | **Partial** | medium | Custom-scheme redirects are off by default for new Android clients (can be re-enabled in Advanced settings); loopback is blocked for Android clients. Test E-3. |
| 13 | OneDrive: "Free Microsoft app registration" (App folder only) | **Partial** | medium | Free of charge but needs an Entra tenant (Azure free account, credit-card identity check). Publisher verification is unavailable to individuals. Restrict the audience to personal accounts. The AppFolder 403 regression is open. |
| 14 | Clipboard deferred invite is fine with Play and Android | **Partial** | medium (security) / low (policy) | No Play rule against it. Read only after window focus (Android 10+). A web page can't mark the clip sensitive, so keyboard history / cloud clipboard keep the secret. Copy an expiring wrapper, not the group secret. |
| 15 | F-Droid later via a `foss` flavour (no proprietary deps) | **Partial** | medium | Needs public FOSS source (undecided), public OAuth client IDs, no automatic GitHub fetches (NonFreeNet precedent), a resolved app-ID/domain question, and a developer-verification key strategy (reproducible builds). Rust via cargo-ndk is proven on F-Droid. |
| 16 | Flathub for Linux | **Partial** | medium–high | Network / P2P are fine. Blockers: app-ID domain verification, offline build from source, autostart via portal, the **generative-AI policy**, "insufficient development history". |
| 17 | winget distribution | **Confirmed** feasible | low | Free. Installer from the publisher's release URL, silent install, clean uninstall, AV scans. Unsigned is allowed but loses reputation. |
| 18 | Microsoft Store for an individual (fees in 2026), MSIX | **Confirmed** (free) with caveats | medium | Free since 2025-09-10. Store re-signs MSIX. jpackage has no MSIX output. Test Compose + NIO + iroh inside MSIX. AppData is deleted on uninstall. |
| 19 | MSI / MSIX via GitHub Releases at no cost | **Refuted** for good UX | medium | Unsigned = strong SmartScreen block. Signing costs $120–300 a year, or SignPath (free) if open source. Make the Store the primary Windows channel. |
| 20 | GitHub Pages can host the join-link page for free | **Partial** | medium (high for takeover) | Needs a public repo (Free plan). App Links need a root user site (`<owner>.github.io`). The host becomes a permanent trust anchor for secrets: see new risk N3. |
| 21 | GitHub Releases for desktop update checks ("static, free") | **Partial** | medium | Private repo → assets aren't public. API limit is 60/h/IP. Use a static signed manifest on Pages or a `releases/latest/download/` asset in a *public* repo. |

## Details per claim

### 1. `shortService` and Play review: confirmed

- Play's FGS policy (inside the Device and Network Abuse policy) says a foreground-service permission is allowed only if the use "provides a feature that is beneficial to the user…, is initiated by the user or is user perceptible…, can be terminated…, can't be interrupted or deferred…, runs only for as long as necessary". It then states: "**The following foreground service use cases are exempt from the above criteria: foreground service types `systemExempted` or `shortService`**; foreground service type dataSync only when using Play Asset Delivery features."
- `shortService` needs no type-specific permission (only `FOREGROUND_SERVICE`) and has about a 3-minute timeout. After `onTimeout()` the app must stop or it gets an ANR "even if it has other valid foreground services running".
- Play Console asks for an FGS declaration for types used by apps targeting 14+ ("description, user impact if deferred, video"). The declaration is per FGS *permission*. `shortService` has none, and the help page doesn't mention it. It is very likely absent from the form, but that should be checked in the Console (experiment E-1).
- Background start: one listed exemption is "Your app **transitions from a user-visible state**, such as an activity." That supports starting the service during `onStop` / screen-off. The docs give no grace period, so R4 should measure the timing.
- Android 13 and lower: `onTimeout()` doesn't exist and there's no timeout. The design's fallback (expedited WorkManager below Android 14) is fine.

### 2. Phone helper with an FGS fallback: refuted

- A `dataSync` FGS used to help *other people's groups* fails several of Play's criteria. It isn't "beneficial to the user and relevant to the core functionality of the app" for this user. It isn't user-initiated. It *can* be deferred with no loss to this user. Only `shortService`, `systemExempted` and Play Asset Delivery `dataSync` are exempt.
- The Android 15 behaviour changes make it worse: `dataSync` gets "a total of 6 hours in a 24-hour period" (the timer resets only when the user brings the app to the foreground). After that, `onTimeout()` → `stopSelf()` within seconds or a fatal `RemoteServiceException`. `BOOT_COMPLETED` receivers can't start `dataSync`.
- Device & Network Abuse also says: "Apps that facilitate proxy services to third parties may only do so in apps where that is the primary, user-facing core purpose of the app." A store-and-forward mailbox for strangers isn't literally an IP proxy, but a reviewer could read it as one (see #9).
- **Fix:** delete the FGS fallback from §7a. On Android, "help" should mean helping only the groups the device is a member of (tier 1), or only while the app is open. Put strangers' helping on desktops. If phone helping for strangers is kept, keep it out of the `play` flavour or ship it as an explicit, off-by-default feature with an in-app disclosure, and expect review friction.

### 3. `specialUse`: partial

- `specialUse` "covers any valid foreground service use cases that aren't covered by other foreground service types". It needs a `<property android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE">` explanation that is "reviewed when you submit your app in the Google Play Console". It's allowed, with manual review and a real rejection risk for "keep a sync connection open".
- For a future "Stay connected" mode, `dataSync` is ruled out by the 6 h / 24 h cap. `remoteMessaging` (continuing text messages across devices) doesn't fit a task app. `specialUse` with a strong user-visible justification is the only candidate. The design already defers this ("Not in v1").

### 4. Data safety form: partial

What holds (Play Console Help, "Provide information for Google Play's Data safety section"):
- Collection means "Transmitting data from your app off a user's device", including to third parties, not just to your own server.
- Exempt from collection: "User data that is sent off device, but that is unreadable by you or anyone other than the sender and recipient as a result of end-to-end encryption… The encrypted data must not be readable by any intermediary entity, including the developer." Group documents, Nostr payloads and helper payloads qualify.
- Backups: the FAQ says that if "the user chooses to upload their data directly to their own external drive or cloud storage account (such as Google Drive…)… and your app never collects or accesses the data in question, then your app does not need to declare the collection." That covers the sealed Drive/OneDrive backup even though a sealed backup is **not** E2E (its key is in the file).
- "Even developers with apps that do not collect any user data must complete this form and provide a link to their privacy policy."

What doesn't hold:
- **Metadata isn't E2E.** The Ed25519 device key doubles as the iroh endpoint ID. It is persistent, sent in the clear to iroh relays and published to the DHT. Nostr per-device-per-group pubkeys, IP addresses and timing also reach third-party relays. Google's guidance on IPs is "disclose… based on their particular usage", and "Device or other IDs" covers "Identifiers that relate to an individual device… or app". A conservative, defensible answer is to declare *Device or other IDs: collected, required, app functionality, not shared*, with a line in the privacy policy explaining the relays. Declaring nothing is common among P2P apps (Jami and Syncthing-Fork both show "No data shared with third parties") but is a judgment call.
- The User Data policy also requires "a valid privacy policy in both the app's store listing **and within the app itself**". The design only mentions a privacy policy in general.

### 5. Exact alarms: confirmed, with gaps

- Play policy: "Your app must use the `USE_EXACT_ALARM` functionality only when your app's core, user-facing functionality requires precisely timed actions, such as: The app is an alarm or timer app. The app is a calendar app that shows event notifications." It needs a Play Console declaration. Task/to-do apps aren't listed.
- Precedent: Tasks.org declares `SCHEDULE_EXACT_ALARM` (with `tools:ignore="ProtectedPermissions"`) and listens for `SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED`. It doesn't use `USE_EXACT_ALARM`.
- Android docs: the permission "is not pre-granted to fresh installs of apps targeting Android 13… and higher". "If a user transfers app data to a device running Android 14 through a backup-and-restore operation, the `SCHEDULE_EXACT_ALARM` permission will be denied on the new device." And: "When the `SCHEDULE_EXACT_ALARMS` permission is revoked for your app, your app stops, and all future exact alarms are canceled."
- Gaps in the design: (a) after a Block Store / Auto Backup restore, reminders will silently be inexact until the user grants the permission again, so show the permission screen on first launch after restore if timed reminders exist; (b) implement the state-changed receiver to re-arm alarms; (c) the grant happens on a Settings screen, not a dialog (`ACTION_REQUEST_SCHEDULE_EXACT_ALARM`), so the explanation screen matters; (d) the fallback is `setAndAllowWhileIdle()` (Doze-batched).

### 6. Target API deadlines: confirmed (2026), unverified (2027)

- Play: "Starting August 31, 2026, new apps and app updates must target Android 16 (API level 36) or higher." An extension to **2026-11-01** is available. Existing apps must target API 35+ to stay available to new users on newer Android versions.
- The Policy Deadlines page (which lists items through 2027-01-27) doesn't publish a 2027 target-API date. By the usual pattern, expect API 37 around 2027-08-31. The design's "37 as Play requires" is right.
- **Related requirement the design misses:** since 2025-11-01 (extension to 2026-05-31, now passed), apps targeting Android 15+ with native code must support **16 KB page sizes**. The Rust `.so` from cargo-ndk must be linked with 16 KB `LOAD` alignment. NDK r28+ does this by default, and E0 installed NDK 29, but it must be checked in the build (experiment E-6).

### 7. Export compliance: partial

- Google Play has no encryption or export questionnaire. The obligation is contractual: DDA §4.6 (use Play only in line with laws "including all laws related to the export of data or software") and §16.5 (comply with "export control and sanctions laws… including… the U.S. Department of Commerce's Export Administration Regulations"). So "declared as … exempt" describes a step that doesn't exist on Play.
- US EAR (eCFR, current): publicly available encryption source code (5D002) "is not subject to the EAR". Notification to BIS/NSA is needed **only** for "non-standard cryptography" (§742.15(b)(2)). XChaCha20-Poly1305, Ed25519, HKDF, Argon2id and BLAKE3 are all published algorithms, so they're "standard" under the Part 772 definition.
- Mass-market binaries under §740.17(b)(1) are classified 5D992.c and are "excluded from paragraph (e) reporting requirements", so no self-classification report is needed (the 2021 rule).
- **Not covered here:** the developer's own country's crypto-export rules, and a few import regimes (for example France's declaration regime for confidentiality crypto). Check once where the developer lives. **Fix:** reword §9 to "Play has no export declaration. Under the EAR the app is mass-market 5D992.c with no reporting; open-source release uses standard crypto, so no notification is needed."

### 8. P2P and Nostr apps on Play: confirmed

Play listings checked 2026-09-24 (page title and "Updated on"):

| App | What it is | Updated |
|---|---|---|
| Amethyst | Nostr client | 2026-07-29 |
| Primal | Nostr client | 2026-09-01 |
| Briar | P2P messenger (Tor, Bluetooth) | 2026-09-17 |
| Jami | P2P / DHT messenger | 2026-08-17 |
| Syncthing-Fork | P2P file sync | 2026-08-14 |
| Orbot | Tor; "Kindness Mode" Snowflake proxy for strangers | 2026-07-08 (targets API 37) |

Counterpoint: the official syncthing-android was discontinued in Dec 2024 citing "Google making Play publishing something between hard and impossible". That was over broad file access, not P2P.

### 9. Relaying other users' encrypted data: partial / at risk

- The exact rule is quoted in #2. Orbot's Kindness Mode relays strangers' traffic and is on Play, but proxying is Orbot's core purpose. For a task app, relaying strangers' data is secondary, which is exactly what the clause targets. Its likely origin is residential-proxy SDKs quietly selling users' bandwidth.
- **Fix:** v1 is unaffected: layer-6 relaying among *group members* isn't "third parties". For §7a: helper off by default on Android (the design's "Default undecided" should become **off on Android**), an explicit in-app disclosure, never an FGS, and prefer the desktop. Consider shipping phone helping for strangers only in `foss` / sideload builds.

### 10. `drive.file` verification and user cap: refuted

- Drive API scope table: `drive.file` and `drive.appdata` are **non-sensitive (recommended)**.
- Unverified-app screen: shown when "your app uses sensitive or restricted scopes". User cap: "100 new users in total, after the app presents the unverified app screen", which applies to unapproved sensitive/restricted scopes only.
- Brand verification is needed only if the app is External plus Published *and* "you want to display a logo or name on the OAuth consent screen". Branding page: "Your brand must be verified if you want your application logo and application name to be visible to users on the consent screen. **Without verification, only your application domain will be visible to users.**"
- Trap: **Testing** status is "limited to up to 100 test users" and "Authorizations by a test user will expire seven days from the time of consent". The app must be switched to *In production*, which needs no review for non-sensitive scopes.
- **Fix:** rewrite the §11 row: "`drive.file` (non-sensitive): no verification, no warning screen, no user cap once *In production*. Optional brand verification shows our name and logo instead of a bare domain." What an unbranded consent screen shows for an app with no authorized domain is unknown (E-2).

### 11. GitHub Pages domain for Google verification: likely refuted

- Google verifies "the domain component available for registration on a public suffix: the 'top private domain'", "using the Google Search Console". `github.io` is on the Public Suffix List, so `username.github.io` is technically a top private domain. But a Search Console *Domain* property needs a DNS TXT record, which you can't add under `github.io`. Only a URL-prefix property (HTML file) can be verified there.
- Practitioner evidence (second-hand; Google doesn't document it): a July 2026 GitHub issue says "You **cannot** verify a `*.github.io` URL in Search Console… a custom domain is mandatory". A 2025 write-up (Medium, fetch blocked, quoted via search snippet) says that even though `brightconstruct.github.io` was verified in Search Console, Google didn't accept GitHub Pages as a first-party domain for OAuth verification, and the author moved to a custom domain.
- The home page and the privacy policy must be on the same verified domain.
- **Fix:** if brand verification is wanted, use a custom domain (about $10–15 a year; the same domain can serve join links, App Links and Flathub verification). Otherwise skip brand verification. This answers §15's open question. The cost is small but it isn't zero, so it is a small exception to C2.

### 12. `foss` flavour browser OAuth: partial

- Google (2023-10-02): "By default, new Android apps will no longer be allowed to use Custom URI schemes to make authorization requests"; developers "may enable the Custom URI scheme method… in the 'Advanced Settings' section". The loopback redirect was blocked for Android, iOS and Chrome client types in 2022. Google blocks embedded webviews.
- The `foss` flavour therefore needs either an Android client with custom schemes re-enabled (discouraged, and it may be withdrawn later) or an App Link `https` redirect (needs a verified domain, see #11). F-Droid "does not sign up for any API keys", so the client ID must be in the public source, which is normal for PKCE clients.
- Cross-client access (a file created by the Android client and read by the desktop client in the same project) isn't documented on the pages fetched. Test it (E-2).

### 13. OneDrive App Folder: partial

- Registration: since June 2024 "all applications must be registered in a directory". App Registrations tells a personal-account user to "join the M365 Developer program or sign up for Azure". The M365 developer sandbox has been restricted since Feb 2024 (Visual Studio subscribers and some partners only). An Azure free account creates an Entra ID Free tenant at no charge, but "Microsoft requires a credit card to verify your identity" (a temporary about-$1 hold).
- Publisher verification needs "a Partner One ID for a valid Microsoft AI Cloud Partner Program account that has completed the verification process". "Apps that are registered by using a Microsoft account can't be publisher verified." The publisher domain can't be `*.onmicrosoft.com`. In practice that's out of reach for an individual.
- Consequences: work/school tenants with the default risk-based step-up consent block user consent to unverified multitenant apps registered after 2020-11-08 that request more than basic sign-in (a Jan 2024 Q&A reports exactly this for `Files.ReadWrite.AppFolder`). Personal accounts can consent.
- **Open regression:** Microsoft Q&A, reported 2026-08-23 and still open 2026-09-08: on OneDrive Personal, newly consented apps with `Files.ReadWrite.AppFolder` get "403 accessDenied / serviceReadOnly ('Database Is Read Only') on ALL drive endpoints". Suspected cause: a missing per-app provisioning step. Several developers reproduced it, with no Microsoft response. The workaround needs the broad `Files.ReadWrite` scope once, which defeats least privilege.
- App folder behaviour is as designed: `/drive/special/approot`, named from the app registration, visible and user-editable.
- **Fix:** set the audience to "Personal Microsoft accounts only". Budget one-time Azure identity verification. Keep OneDrive second and re-test the regression before building (E-4).

### 14. Clipboard deferred invite: partial

- No Play policy forbids reading the clipboard. The User Data policy's list of sensitive data doesn't mention it.
- Android 10+: "Unless your app is the default input method editor (IME) or is the app that currently has focus, your app cannot access clipboard data." Read only after `onWindowFocusChanged(true)`, not in `onCreate`.
- Android 12+: toast "APP pasted from your clipboard" on the first read of another app's clip (the design accounts for this). Calling `getPrimaryClipDescription()` first shows no toast, so check the MIME type or label before reading, to avoid a toast when there's no invite.
- The web page copies with `navigator.clipboard.writeText` and **can't** set `ClipDescription.EXTRA_IS_SENSITIVE`. Only Android apps can. Keyboard clipboard histories and cross-device clipboard sync therefore get the raw group secret. That's a security exposure (A8 should measure it), not a policy problem.
- **Fix:** don't put the permanent group secret on the clipboard. The page can copy a short-lived wrapper instead (for example, the invite re-encrypted under a one-day key and published like an invite code), or at minimum a link whose secret is a one-time invite capability. Clear the clipboard after a successful join (the app owns focus then and may overwrite the clip).

### 15. F-Droid: partial

- Inclusion policy: FLOSS license; built by F-Droid from public source with a "100% FLOSS toolchain"; "Google Play Services and Firebase and Crashlytics… are strictly forbidden" (a flavour is fine); no downloading executable code without opt-in; "F-Droid does not sign up for any API keys". Also: "**Any application with a duplicate android application ID matching another domain name will receive automatic rejection.**"
- Rust is proven: Amethyst's F-Droid recipe installs `rustup`, adds the Android targets and runs `cargo install cargo-ndk@4.1.2`. Delta Chat builds its Rust core with rustup.
- Anti-features:
  - *NonFreeNet* ("promote or depend entirely on a proprietary network service"): Amethyst has it for "On first start, the app fetches its privacy policy from Github". **Any automatic GitHub fetch** in the `foss` build (update check, relay list) can earn the same label.
  - Optional Google or Microsoft integrations don't have to: Tasks.org, which syncs optionally with Google Tasks and Microsoft To Do, has no anti-feature on its current builds.
  - *TetheredNet* "would not be applied if there is a simple configuration option that allows pointing the app to… an alternative… self-hostable server". Delta Chat, which uses iroh, has no anti-features. Nostr and iroh relays are replaceable, but the design hides the relay list in diagnostics. Making it **editable** there removes any TetheredNet argument.
- Developer verification (2027 globally): F-Droid signs with its own key unless the build is reproducible. Google added an "other ways to verify" path in April 2026 after F-Droid-related dead ends. The clean route is **reproducible builds signed with the developer's key**, which that key's registration covers.
- **Fix:** keep a `foss` flavour with no automatic GitHub traffic (F-Droid does updates); make relays editable in diagnostics; plan for reproducible builds; settle the domain/app-ID question.

### 16. Flathub: partial

- App ID: reverse-DNS, and "the author or the developer or the project must have control over the domain". Verified via `https://{domain}/.well-known/org.flathub.VerifiedApps.txt` or a DNS TXT record at `_flathub.{domain}`. The GitHub alternative is `io.github.<user>.<repo>` with a reachable (public) repo.
- Build: "All source available submissions must be built entirely from source… no network access during the build." That means vendored Gradle (Compose, Skiko natives) and Cargo sources, which is doable with flatpak-builder-tools but significant work.
- Autostart: the linter says `--filesystem=xdg-config/autostart` "is not granted as an Autostart portal exists". "Start with computer" must use the Background portal.
- "Applications that operate exclusively as tray applets will not be accepted." The app has a main window, so that's OK, but it must not be tray-only.
- **Generative AI policy:** "Submitters must disclose any AI-generated code, documentation, packaging… **Flathub manifests must not contain AI-generated or AI-assisted content.** … Reviewers may reject a submission… based on the extent or role of generated material… AI tools or agents must not open or automate Flathub submission pull requests." This project is being built with an AI assistant, so the Flathub manifest must be written by a human, the submission must be done by a human, and rejection is possible.
- "Insufficient development history": "Applications that have only existed for a very short period of time will generally not be accepted." Expect to wait months after the first release.
- Self-updating: no explicit rule found in the requirements, linter or verification pages. The design already defers to Flatpak for updates. Also suppress the GitHub update notice when `/.flatpak-info` exists.

### 17. winget: confirmed feasible

- Free, by pull request to `microsoft/winget-pkgs`. "The installer supports non-interactive modes"; "installs and uninstalls correctly for both administrators and non-administrators"; "The installer comes directly from the publisher's website" (GitHub Releases URLs are the norm, but URL redirectors are flagged `Validation-Indirect-URL`). Installers are scanned by several antivirus programs, with a `Validation-Uninstall-Error` if the uninstall doesn't clean up.
- Unsigned installers are allowed but can fail URL/SmartScreen reputation checks. Winget policy 1.5 needs a privacy URL if personal information is transmitted. Policy 2.12 covers UGC (see N7).
- A Store (MSIX) listing is also installable through `winget --source msstore`, which avoids signing costs.

### 18. Microsoft Store and MSIX: confirmed (free), with caveats

- "Individual developers can now publish apps to the Microsoft Store without paying any onboarding fees… nearly 200 markets" (Windows Developer Blog, 2025-09-10). Identity check with government ID plus selfie. Company accounts cost $99.
- "If you publish your app as an **MSIX package** through the Microsoft Store, code signing is free… Microsoft re-signs the package." MSI/EXE through the Store must be Authenticode-signed with a trusted-root certificate.
- jpackage outputs only exe/msi (plus Linux and macOS formats), not MSIX. MSIX needs MSIX Packaging Tool / `makeappx`, or Hydraulic Conveyor (free only for OSI-licensed projects, otherwise $45 a month).
- A Compose Desktop app failed to start in MSIX because the JVM's NIO `Selector` couldn't create its internal Unix-domain-socket pipe inside AppContainer (Microsoft Q&A, Oct 2025). The workaround is full trust (`uap10:TrustLevel="mediumIL"`), which Store desktop apps normally use.
- MSIX file-system behaviour for virtualised apps: new files under `AppData` "are written to a private per-user, per-app location" and removed on uninstall. See N5.
- Store policy 10.2.8 requires user consent before changing Windows settings. The default-on "Start with computer" should use the MSIX `StartupTask` (user-visible in Settings → Startup) and be mentioned on first run.

### 19. Code signing outside the Store: refuted for good UX

- Microsoft's comparison table (updated 2026-08-29): no signature means "Strong SmartScreen block; enterprises may block entirely". Azure Artifact Signing costs about $9.99 a month, and "Individual developers are currently limited to the USA and Canada". OV certificates cost $150–300 a year with an HSM. EV "no longer" skips SmartScreen (2024). SignPath Foundation offers "free code signing for qualifying open-source projects".
- **Fix:** Windows channel order: Microsoft Store MSIX first (free, signed, auto-updated), winget via msstore, then a GitHub MSI (unsigned, or SignPath-signed if open source) for users who avoid the Store. Change §12 accordingly.

### 20. GitHub Pages for the join-link page: partial

- "If the account that owns the repository uses GitHub Free… the repository must be public." The design's "one private GitHub repo" can't host Pages on the Free plan, so it needs a separate public site repo.
- Limits: 1 GB site, 100 GB a month soft bandwidth, 10 builds an hour, 429 on excess. Not for "online business… e-commerce… SaaS" or "sensitive transactions like sending passwords or credit card numbers". The join page only *reads* a fragment client-side, so it's compatible.
- Android App Links need `/.well-known/assetlinks.json` at the **host root**. That only works for the user or organisation site repo `<owner>.github.io`, or a custom domain, not a project site (`/<repo>`). Jekyll skips dot-folders unless configured, or use `.nojekyll`.
- Permanence: every link and QR code ever shared embeds this host (principle 8 says forever). GitHub: "After changing your username, your old username becomes available for anyone else to claim." See N3.

### 21. GitHub Releases for update checks: partial

- Release assets in a private repo aren't publicly downloadable. Releases must live in a public repo.
- "The primary rate limit for unauthenticated requests is 60 requests per hour" per IP. Shared NAT/CGNAT IPs can hit it. GitHub also tightened unauthenticated limits for raw content and anonymous API use on 2025-05-08 (numbers not published). The Acceptable Use Policy reserves throttling for "significantly excessive" bandwidth.
- **Fix:** desktop checks fetch a tiny **signed** `latest.json` (the design already has a signed release manifest) from Pages or a custom domain, at most once a day with jitter. Downloads come from `releases/download/…` in a public repo. Treat HTTP 403 or 429 as "no update info", never as an error the user sees.

## New risks

| # | Risk | Severity | Fix |
|---|---|---|---|
| N1 | **Android developer verification.** Play apps not registered by **2026-09-30** are removed from Play. Off-Play installs need a registered package name plus signing key (4 countries now, global in 2027). Packages under 50 installs are first-come-first-served, and impersonators have already squatted real package names. The design's GitHub-APK and F-Droid channels depend on this. | **critical** (timeline) | Register `com.kleiders.tasks` (or its replacement) in Play Console / Android Developer Console now. Register the sideload key (the Play App Signing key differs from the upload/sideload key). Use reproducible builds for F-Droid. |
| N2 | **`kleiders.com` is an existing AI fashion-design SaaS** (registered 2026-03-03). If it isn't the user's: Flathub app-ID verification fails, F-Droid auto-rejects an "application ID matching another domain", and a trademark/impersonation complaint on Play is possible. The package name can't change after publishing. | **high** (if not owned) | Confirm ownership. If owned, use it for OAuth brand verification, Flathub, App Links and join links. If not, pick a domain you own and rename the package before the first upload. |
| N3 | **Join-link host = trust anchor for group secrets.** Anyone who controls the page's origin (lost or renamed GitHub account, lapsed custom domain, account takeover) can ship JavaScript that reads `location.hash` from every old invite link opened in a browser (app not installed, in-app browsers, desktop). | **high** | Long-lived domain with auto-renew, 2FA and hardware keys on the GitHub account. Minimal static page with no third-party scripts and a strict CSP. Links carry a **one-time invite capability**, not the permanent group secret. Verified App Links so installed apps never load the page. Tie in with R7/A8. |
| N4 | **Flathub generative-AI policy and development-history rule.** | medium–high | Human-written manifest and human submission. Disclose AI-generated code honestly. Accept possible rejection; keep AppImage/.deb/Flatpak-from-own-repo as fallbacks. Submit after months of public history. |
| N5 | **MSIX AppData virtualisation.** Local data written under `%APPDATA%` by a Store/MSIX install goes to a per-package location and is **deleted on uninstall**. Switching MSIX→MSI or reinstalling loses solo lists that have no peers or backup. | medium | In MSIX builds, use `ApplicationData.LocalFolder` knowingly and prompt for folder/cloud backup (already prompted when there are no peers). Test uninstall/reinstall (E-7). Or write to an explicitly unvirtualised path. |
| N6 | **Windows signing costs versus C2.** A non-Store MSI is either unsigned (strong SmartScreen block) or costs $120–300+ a year. Azure Artifact Signing is unavailable to individuals outside the US/Canada. | medium | Store-first. SignPath if open-sourced. Document the exception to C2 if a certificate is bought. |
| N7 | **UGC policies with no server.** Play needs terms acceptance "before users can create or upload UGC", in-app reporting and blocking, and "taking action". Microsoft Store 11.12 needs report-to-developer and "Remove or disable UGC when requested by Microsoft". Winget 2.12 is similar. E2E P2P means the developer can't see or remove content. | medium–low | Terms/policy link accepted at first share/join (first launch stays prompt-free). "Remove and block person" (local block of their device keys plus key rotation). A "Report" button that composes an email containing only what the reporter chooses to send. State in listings that content is private and E2E. |
| N8 | **16 KB page-size compliance** for the Rust `.so` (required since 2025-11-01; extension ended 2026-05-31). | medium | Check `LOAD` segment alignment is `0x4000` in CI (`llvm-readelf -l`) and `zipalign -c -P 16`. Keep NDK ≥ r28. |
| N9 | **Google OAuth for the `foss` flavour and Linux** (custom scheme off by default, loopback blocked for Android clients). Testing mode expires tokens after 7 days. | medium | Publish the OAuth app "In production". `foss`: enable the custom scheme on a dedicated Android client or use an App Link redirect on your verified domain. Desktop: "Desktop app" client with loopback + PKCE. |
| N10 | **OneDrive `Files.ReadWrite.AppFolder` 403 regression** for new personal-account consents (since about 2026-08-23), plus the Azure identity-verification requirement for registering. | medium (maybe temporary) | Re-test before phase 6. Don't ship a broad-scope workaround silently. Keep Google Drive first. |
| N11 | **Private repo versus public hosting.** Pages (Free plan) and public release downloads both need public repos. | medium | Create `kleiders.github.io` (or custom-domain site) and a `releases` repo, both public, holding only built artifacts, the join page and the policy pages. |
| N12 | **Exact-alarm permission lost on device migration.** Restored users' reminders become inexact without notice. | low–medium | After a restore, if timed reminders exist, show the explained permission screen on first launch. |
| N13 | **New personal Play accounts** (created after 2023-11-13) must run a closed test with **12 testers for 14 days** before production. | low–medium | Check the account's creation date and type. Recruit 12 testers early (the playtest group from UX-PRINCIPLES can double as this). |
| N14 | **Privacy policy must also be reachable in-app** (Play User Data policy), and the Data safety form is mandatory even for "no data". | low | Add "Privacy" to About. Publish the policy on the owned domain (it doubles as Google brand-verification material). |
| N15 | **Future images in tasks.** Play allows `READ_MEDIA_IMAGES` only "if system pickers… are not sufficient", with a Console declaration. | low | Use the Android Photo Picker. Never request media permissions. |
| N16 | **F-Droid NonFreeNet from automatic GitHub fetches** (Amethyst precedent). | low | `foss` build: no update checks, no fetched policy pages. Relay list bundled and editable. |
| N17 | **EU Digital Services Act trader status** must be declared in Play Console. Traders' contact details are shown publicly. *Not re-verified in this pass.* | low | Declare non-trader (free, non-commercial) if accurate; re-check the current Play Console wording. |
| N18 | **The Play build must not self-update or point to off-Play APKs.** "An app distributed via Google Play may not modify, replace, or update itself using any method other than Google Play's update mechanism." | low | The `play` flavour's Update button opens only the Play listing (the design already says so). Keep GitHub links out of the Play build. |

## Suggested experiments

1. **E-1 Play Console dry run (no publishing).** Create the app entry and check the package-registration status (N1). Open the FGS declaration form and see whether `shortService` is listed. Draft the Data safety form with and without "Device or other IDs". Upload an internal-testing AAB containing `shortService` + `SCHEDULE_EXACT_ALARM` and read the pre-launch report and policy warnings.
2. **E-2 Google OAuth, `drive.file`.** Create a project in *Production* without brand verification and screenshot what the consent screen shows. Try adding `<owner>.github.io` as an authorized domain after verifying a URL-prefix property in Search Console, and record accept or reject. With two clients (Android and Desktop) in the same project, create a file on one and list it from the other.
3. **E-3 `foss` OAuth on an emulator (headless).** Test an Android client with the custom URI scheme enabled, plus AppAuth and the system browser. Confirm the Desktop-type loopback flow isn't used on Android. Record every consent-screen warning.
4. **E-4 Microsoft Graph.** Create an Azure free account/tenant and register a "Personal Microsoft accounts only" public client (PKCE). Consent with a **fresh** personal account using `Files.ReadWrite.AppFolder` and check for the 403 regression. Screenshot the consent screen for "unverified" wording.
5. **E-5 Clipboard invite matrix.** Emulators on API 29/31/33/35/36: page copies → install → first launch. Measure when `getPrimaryClip()` works (before or after focus), toast behaviour, and `getPrimaryClipDescription()` gating. Separately check Gboard clipboard-history retention of a web-copied invite, and whether `navigator.clipboard.writeText` works in the WhatsApp, Telegram and Instagram in-app browsers (A8 overlap).
6. **E-6 16 KB alignment.** On the E6 arm64 build: `llvm-readelf -l libengine.so` (all `LOAD` Align = 0x4000), `zipalign -c -P 16 -v 4 app.apk`, and a run on the 16 KB emulator image.
7. **E-7 MSIX full-trust smoke test.** Package a Compose Desktop plus iroh prototype with `makeappx` (self-signed, local only). Check that NIO `Selector.open()` works, iroh can bind UDP, the Windows Firewall prompt appears, where `%APPDATA%` writes land, that data is removed on uninstall, and how the `StartupTask` behaves.
8. **E-8 Flatpak local build.** `flatpak-builder` fully offline with generated Gradle and Cargo sources. Run `flatpak-builder-lint`. Test the Background portal autostart and the Secret portal (libsecret) inside the sandbox.
9. **E-9 Update-check load.** A handful of manual requests: compare the rate-limit headers of `api.github.com/repos/.../releases/latest` with a Pages-hosted `latest.json`. No automated load.
10. **E-10 Exact alarm after restore.** Emulator API 35: grant, back up, restore to a fresh image, confirm denial, and confirm the app's re-prompt path and inexact fallback.

## Sources

Google Play / Android
- Device and Network Abuse policy (incl. FGS policy text, proxy clause, self-update rule): https://support.google.com/googleplay/android-developer/answer/16559646
- Understanding foreground service and full-screen intent requirements: https://support.google.com/googleplay/android-developer/answer/13392821
- Foreground service types: https://developer.android.com/develop/background-work/services/fgs/service-types
- Restrictions on starting FGS from background: https://developer.android.com/develop/background-work/services/fgs/restrictions-bg-start
- Android 15 behavior changes (dataSync 6 h, BOOT_COMPLETED): https://developer.android.com/about/versions/15/behavior-changes-15
- Data safety section help (E2E exemption, backup FAQ, IP FAQ): https://support.google.com/googleplay/android-developer/answer/10787469
- User Data policy: https://support.google.com/googleplay/android-developer/answer/10144311
- Permissions and APIs that access sensitive information (USE_EXACT_ALARM, photo/video): https://support.google.com/googleplay/android-developer/answer/16558241
- Preview of the permissions policy effective 2027-01-27: https://support.google.com/googleplay/android-developer/answer/16909972
- Schedule alarms (SCHEDULE_EXACT_ALARM behaviour): https://developer.android.com/develop/background-work/services/alarms
- Policy deadlines 2026–2027: https://support.google.com/googleplay/android-developer/answer/12253906
- Target API level requirements: https://support.google.com/googleplay/android-developer/answer/11926878
- 16 KB page size requirement: https://android-developers.googleblog.com/2025/05/prepare-play-apps-for-devices-with-16kb-page-size.html and https://developer.android.com/guide/practices/page-sizes
- User Generated Content policy: https://support.google.com/googleplay/android-developer/answer/9876937
- Testing requirements for new personal accounts: https://support.google.com/googleplay/android-developer/answer/14151465
- Developer verification: https://android-developers.googleblog.com/2026/06/android-developer-verification.html , https://support.google.com/android-developer-console/answer/16561738 , https://developer.android.com/developer-verification/guides/android-developer-console , https://support.google.com/googleplay/android-developer/answer/16984799 , https://piunikaweb.com/2026/04/16/google-android-developer-verification/
- Android 10 clipboard restriction: https://developer.android.com/about/versions/10/privacy/changes
- Copy and paste (Android 12 toast, EXTRA_IS_SENSITIVE): https://developer.android.com/develop/ui/views/touch-and-input/copy-paste
- Google Play Developer Distribution Agreement (§4.6, §16.5): https://play.google/developer-distribution-agreement.html
- Play listings checked 2026-09-24: https://play.google.com/store/apps/details?id=com.vitorpamplona.amethyst , ...id=net.primal.android , ...id=org.briarproject.briar.android , ...id=cx.ring , ...id=org.torproject.android , ...id=com.github.catfriend1.syncthingandroid
- syncthing-android discontinuation: https://github.com/syncthing/syncthing-android
- Tasks.org manifest (SCHEDULE_EXACT_ALARM): https://github.com/tasks/tasks/blob/main/app/src/main/AndroidManifest.xml

Export control
- 15 CFR 742.15 (publicly available encryption source code): https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-742/section-742.15
- 15 CFR 740.17 (License Exception ENC, mass market): https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-B/part-740/section-740.17

Google OAuth / Drive
- Drive API scopes: https://developers.google.com/workspace/drive/api/guides/api-specific-auth
- Brand verification: https://developers.google.com/identity/protocols/oauth2/production-readiness/brand-verification
- Manage OAuth app branding ("only your application domain"): https://support.google.com/cloud/answer/15549049
- Verification requirements: https://support.google.com/cloud/answer/13464321
- Unverified apps / user cap: https://support.google.com/cloud/answer/7454865
- Manage app audience (Testing: 100 users, 7-day expiry): https://support.google.com/cloud/answer/15549945
- Custom URI scheme restrictions (2023-10-02): https://developers.googleblog.com/improving-user-safety-in-oauth-flows-through-new-oauth-custom-uri-scheme-restrictions/
- Loopback migration: https://developers.google.com/identity/protocols/oauth2/resources/loopback-migration
- OAuth for installed apps: https://developers.google.com/identity/protocols/oauth2/native-app
- Practitioner reports on github.io: https://github.com/alvarolobato/obsidian-meeting-copilot/issues/143 ; https://medium.com/@info.brightconstruct/the-real-oauth-journey-getting-a-google-workspace-add-on-verified-fc31bc4c9858 (fetch blocked; quoted from search snippet)

Microsoft
- Publisher verification overview: https://learn.microsoft.com/en-us/entra/identity-platform/publisher-verification-overview
- App folder: https://learn.microsoft.com/en-us/onedrive/developer/rest-api/concepts/special-folders-appfolder
- AppFolder 403 regression (2026-08-23): https://learn.microsoft.com/en-us/answers/questions/5983388/personal-onedrive-newly-consented-apps-get-403-acc
- AppFolder needs a verified publisher for work accounts: https://learn.microsoft.com/en-my/answers/questions/1517983/files-readwrite-appfolder-api-permission-requires
- Registering without a tenant is deprecated: https://learn.microsoft.com/en-us/answers/questions/5508903/how-can-i-register-an-app-for-microsoft-authentica ; https://techcommunity.microsoft.com/blog/microsoft-entra-blog/whats-new-in-microsoft-entra---november-2024/4253152
- Entra ID Free needs a credit card for identity: https://learn.microsoft.com/en-us/azure/cost-management-billing/manage/microsoft-entra-id-free
- M365 developer sandbox eligibility: https://learn.microsoft.com/en-us/office/developer-program/microsoft-365-developer-program-faq
- Free Store registration for individuals: https://blogs.windows.com/windowsdeveloper/2025/09/10/free-developer-registration-for-individual-developers-on-microsoft-store/
- Code signing options (updated 2026-08-29): https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options
- Microsoft Store policies (10.2.8, 11.12): https://learn.microsoft.com/en-us/windows/apps/publish/store-policies
- Packaged desktop app behaviour (AppData virtualisation): https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes
- Compose Desktop NIO in MSIX: https://learn.microsoft.com/en-us/answers/questions/5599711/microsoft-technical-support-request-java-nio-selector-compatibility-in-msix-packages
- Winget policies and submission: https://learn.microsoft.com/en-us/windows/package-manager/package/windows-package-manager-policies ; https://learn.microsoft.com/en-us/windows/package-manager/package/repository
- Hydraulic Conveyor pricing: https://hydraulic.dev/pricing.html

F-Droid / Flathub
- F-Droid Inclusion Policy: https://f-droid.org/docs/Inclusion_Policy/
- F-Droid Anti-Features: https://f-droid.org/docs/Anti-Features/
- F-Droid on developer registration: https://f-droid.org/2025/09/29/google-developer-registration-decree.html ; https://f-droid.org/2026/02/24/open-letter-opposing-developer-verification.html
- fdroiddata metadata: https://gitlab.com/fdroid/fdroiddata/-/raw/master/metadata/com.vitorpamplona.amethyst.yml , .../org.tasks.yml , .../com.b44t.messenger.yml , .../chat.delta.lite.yml
- Flathub requirements (AI policy, history, tray-only): https://docs.flathub.org/docs/for-app-authors/requirements
- Flathub verification: https://docs.flathub.org/docs/for-app-authors/verification
- Flathub linter (autostart exception): https://docs.flathub.org/docs/for-app-authors/linter

GitHub
- Pages limits: https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits
- Pages plan availability: https://docs.github.com/en/pages/getting-started-with-github-pages/creating-a-github-pages-site
- REST API rate limits: https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api
- Unauthenticated rate-limit update (2025-05-08): https://github.blog/changelog/2025-05-08-updated-rate-limits-for-unauthenticated-requests/
- Acceptable Use Policies: https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies
- Changing your username: https://docs.github.com/en/account-and-profile/setting-up-and-managing-your-personal-account-on-github/managing-user-account-settings/changing-your-github-username

Domain check
- `kleiders.com` (page title fetched 2026-09-24) and Verisign RDAP (registered 2026-03-03): https://www.kleiders.com/ , https://rdap.verisign.com/com/v1/domain/kleiders.com
