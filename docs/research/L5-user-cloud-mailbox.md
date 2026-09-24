# L5: Members' own cloud storage as a group mailbox and blob store

*Research pass, 2026-09-24. Scope: whether members' own cloud accounts (Google Drive, Dropbox, OneDrive, MEGA), anonymous paste/file hosts, or GitHub Gists can serve as (a) a **per-member mailbox** that other members poll (gap G1: offline store-and-forward that doesn't depend on Nostr) and (b) **blob storage** for images and large text (G6). Web and API documentation only. Nothing was installed or built. No accounts were created, and no files were uploaded to or downloaded from any service. The only traffic was reading documentation, ToS and robots.txt pages, GitHub issues/READMEs, and crates.io/Maven metadata. DESIGN.md and DECISIONS.md are unchanged.*

---

## Summary

- **No candidate here is a clean, constraint-satisfying replacement for Nostr.** Each one fails at least one hard constraint: no accounts, $0 for the developer at any scale, no developer-run infrastructure, or ToS permission.
- **Google Drive is the only candidate worth keeping, and only as an opt-in, scale-capped extra layer.** It piggybacks on the Drive connection the design already plans for backup (§11).
  - **What works (documented):**
    - `permissions.create` accepts the non-sensitive `drive.file` scope, and `type=anyone` is a supported grantee.
    - Google's credentials guide says API keys can "anonymously access publicly available data, such as Google Workspace files shared using the 'Anyone on the Internet with this link' sharing setting".
    - So a member who connected Drive can publish an encrypted slot that other members fetch **without any Google account**.
  - **The blocker:**
    - Since 2026-05-01, **every Drive API call from every user counts against the developer's single Cloud project**: 1,000,000 units/min, and a daily "billing threshold" of 400,000,000 units.
    - Metadata read = 5 units, download = 200, update = 50.
    - Google says: "API usage over standard daily thresholds will generate charges on your Google Cloud bill" (later in 2026, 90 days' notice), and "Quota increase requests will require Google Cloud billing to be enabled".
    - The layer is therefore **free only up to a ceiling** (my estimate: roughly 50k–100k daily-active devices under the polling model below). Beyond that it either stops working each day or costs the developer money. That fails "free at any scale" as a *foundation*, but it's tolerable as one parallel layer that degrades to the others (principle 4).
  - **The unauthenticated "plain download URL" path is not usable:**
    - `drive.usercontent.google.com` serves `robots.txt` = `Disallow: /`.
    - Google's Terms (effective 2026-07-30) prohibit "automated means to access content from any of our services in violation of the machine-readable instructions on our web pages (for example, robots.txt files…)".
    - The path is also undocumented. gdown has to scrape HTML confirmation pages, and hits "Too many users have viewed or downloaded this file recently" throttling.
- **Drive as a blob store (G6) fits better than Drive as a mailbox (G1).** Blobs are fetched once per member (low request volume), sizes are far beyond Nostr's 64 KiB, and the storage is the uploader's own 15 GB. Verdict: **CONSIDER** (medium confidence).
- **Dropbox: REJECT for now** (medium confidence).
  - It technically works: public `?dl=1` links, 20 GB/day and 100k downloads/day per free account.
  - But few users have Dropbox, and production approval is "in its sole discretion".
  - Its Acceptable Use Policy forbids using it "as infrastructure for, your own cloud services".
  - And a limit breach pauses *all* of the user's sharing for 24 h.
- **OneDrive: REJECT** (high). `createLink` needs **full-drive** `Files.ReadWrite` on personal accounts, and the Shares API requires a bearer token (readers need Microsoft accounts). On top of that come the Azure credit-card registration and the open AppFolder 403 regression (issues #1929/#1930, 2026-08-23/25).
- **MEGA: REJECT** (high).
  - The ToS require app approval before publishing an API client, and a MEGA account (email signup) per writer.
  - Free accounts inactive >3 months can be terminated.
  - Download quotas are opaque (per IP, over 6 h).
  - The Rust crate `mega` was last released 2024-10.
- **Anonymous paste/file hosts (0x0.st class): REJECT** (high).
  - 0x0.st's ToS: "NOT a platform for … backups … CI build artifacts … other automated mass uploads".
  - One individual operator, openly hostile to automated clients.
  - Immutable URLs, so it can't act as a replaceable slot.
  - Public instances churn: transfer.sh was unreachable during this pass.
- **GitHub Gists/raw: REJECT** (high): writers need GitHub accounts (not "seamless for non-technical users"), and unauthenticated raw/API access has undisclosed, tightening rate limits.
- **Gaps covered:** L5 touches only G1 (partially, opt-in, asymmetric) and G6. It does **nothing** for G2 (P2P connectivity), G3 (waking phones: Drive push notifications need an HTTPS webhook server), or G4 (relaying). For G5 it gives at best slow, polled presence.

---

## How the layer would work (common model for all candidates)

1. A member who connects a cloud account ("writer") creates one file per group (per epoch) and keeps rewriting it with the **same encrypted slot format as Nostr** (§7: snapshot slot + per-device bundle, XChaCha20-Poly1305 with the group key, signed).
2. The writer makes the file readable by link, and records the link/file ID in the **group document** (CRDT). Members learn it after one sync over any other layer.
3. Other members **poll** the link during their normal sync runs. Pure HTTP GET: no persistent sockets, fits WorkManager/expedited jobs.
4. On each key epoch (D5) the writer creates a **new file** and deletes the old one, so removed members can't keep watching modification times.
5. Blobs (G6): the uploader stores the encrypted image once in their own cloud, and members fetch it once and cache it.

Structural limits that apply to every candidate:
- **Asymmetric:** only members who sign in to a supported cloud can *write*. A group where nobody has connected gets nothing.
- **Poll-only:** no push without a server, so latency equals the Android sync cadence (15 min → hours in Doze, D16). It doesn't help G3.
- **One provider per member:** Google is one company, just as n0 is. The gain is *independence from Nostr*, not decentralisation.

---

## Candidate 1: Google Drive (`drive.file` + "anyone with the link")

### 1a. Mechanism facts (verified in docs)
- **Scope:**
  - `drive.file` is **non-sensitive**: "Create new Drive files, or modify existing files, that you open with an app or that the user shares with an app…" (Drive scopes page, updated 2026-09-03).
  - R8 already established: no verification, no warning screen, no user cap once In production, but an owned verified domain is required for the homepage.
- **Public permission with `drive.file`:**
  - `permissions.create` lists its authorization scopes as `https://www.googleapis.com/auth/drive` **or** `https://www.googleapis.com/auth/drive.file`.
  - The Permission resource: "Supported values include: `user`, `group`, `domain`, `anyone`… The `anyone` type requires no additional information."
  - `allowFileDiscovery` (keep false) applies only to `domain`/`anyone`.
  - **Not verified at runtime:** that Google accepts `anyone` on a `drive.file`-created file on consumer accounts (strongly implied, not demonstrated). Work/school accounts: "your administrator can limit who can access a published file".
- **App data folder can't be used:** "You can't share files or folders inside the application data folder." Mailbox files must therefore be visible files in the user's Drive, which the user could delete or unshare.
- **Reader access without an account (documented path):** "An API key… used to anonymously access publicly available data, such as Google Workspace files shared using the 'Anyone on the Internet with this link' sharing setting" (Create credentials guide, 2026-09-03). Download uses `files.get?alt=media`, with byte ranges supported.
- **Revisions:** each rewrite creates a revision. "Purgeable revisions are typically preserved for 30 days, but can be purged earlier if a file has 100 revisions…" So a ~50 KB slot costs ≤ ~5 MB of hidden revision storage. Fine.
- **Abuse flag:** "Only file owners can download files identified as abusive content". A false positive on an encrypted blob would silently cut readers off.

### 1b. Quotas and cost (the decisive issue)
From *Usage limits* (updated 2026-09-11) and *Google Workspace standardized model for agent tools and APIs* (updated 2026-09-03):
- "As of May 1, 2026, the usage limits for this API were updated… Cloud projects created on or after May 1, 2026 are subject to the new API quotas." A new Kleiders project falls under the new model.
- Per minute per project: **1,000,000 quota units**. Per minute per user per project: **325,000**. Per day per project: **1 TB** egress "before charges apply".
- **Daily billing threshold: 400,000,000 quota units per project.** "You cannot request an increase on this daily threshold limit."
- Per method: "Read items, such as files.get: **5**"; "List items: 100"; "Download items, such as files.download: **200**"; "Edit items, such as files.update: **50**". Whether `files.get?alt=media` costs 5 or 200 is **not stated** (experiment X4).
- "Exceeding the quota request limits is planned to incur charges to your Google Cloud billing account later in 2026." "Later in 2026, following 90 days of notice: Quota increase requests will require Google Cloud billing to be enabled. API usage over standard daily thresholds will generate charges on your Google Cloud bill."
- What happens past the threshold **without** a billing account isn't documented. Presumably requests are refused (a hard ceiling), not billed. **Unverified.**

**Rough load model** (my assumptions, to be measured):
- 2 groups per device, 4 other devices per group, ~40 background syncs/day.
- Each sync does a metadata check per remote slot: 8 × 5 units × 40 = **1,600 units**.
- ~10 changed-slot downloads/day at 200 units = **2,000 units**.
- ~20 slot rewrites/day at 50 units = **1,000 units**.
- Total ≈ **4,600 units per device per day**, so 400 M / 4,600 ≈ **~87k daily-active devices** before the daily threshold.
- With download counted at 5 instead of 200, the ceiling rises to roughly 150–200k.
- **Either way the layer has a hard ceiling, far below "any scale".** Above it, every user shares one exhausted project quota daily, or the developer pays.
- Blob-only use (G6) is far cheaper: each image is uploaded once (50) and downloaded once per member (200). 400 M units covers ~2M blob downloads/day.

### 1c. The unauthenticated plain-download path (`uc?export=download` → `drive.usercontent.google.com`)
- **Not documented** as an API. gdown's parser handles several HTML confirmation-page formats (`uc?export=download` links, a `#download-form`, a `"downloadUrl"` JSON field). Its error text: "Cannot retrieve the public link of the file… or have had many accesses."
- **Throttling observed in the wild:** gdown issue #406 "Too many users have viewed or downloaded this file recently" (2025-07; maintainer: "Google Drive rate limiting — not a gdown bug"). #391 "Gdown stops working after first few files" (2025-03; "Google Drive rate limiting. This is a Google-side restriction"). The README FAQ: "Google throttles downloads when too many people access the same file, but lets a signed-in account through."
- **Virus-scan interstitial:** gdown advertises that it "Skips the virus-scan confirmation page so large downloads actually finish". I found no official size threshold (the help pages fetched don't state one). Small slot files are presumably below it; **unverified**.
- **ToS:** `https://drive.usercontent.google.com/robots.txt` is `User-agent: * / Disallow: /`. Google Terms of Service (effective 2026-07-30) list as abuse: "using automated means to access content from any of our services in violation of the machine-readable instructions on our web pages (for example, robots.txt files that disallow crawling, training, or other activities)". An app polling that host is plausibly inside this clause. The Drive Program Policies add: "Do not engage in actions intended to bypass our policies, subvert restrictions placed on your account…".
- **Sub-verdict: REJECT this path.** Use the API-key path, which is documented and metered.

### 1d. Evaluation

| Criterion | Finding |
|---|---|
| Gaps | **G1 partial** (opt-in, writer must connect Drive; readers need nothing). **G6 yes.** G5: slow polled presence only. G2/G3/G4: no. |
| Cost to users | $0 within the 15 GB free quota (shared with Gmail/Photos). Mailbox slots are KB; images count against the uploader's quota. A full account makes writes fail, which the app must detect. |
| Cost to developer | $0 **only below the per-project daily threshold** (400 M units). Charges "later in 2026" for usage above it; quota increases will require billing. No developer-hosted infrastructure. Needs an owned verified domain for the OAuth homepage (R8). |
| ToS / third-party use | API use permitted under Google APIs ToS (2021-11-09): "you… will not attempt to circumvent, such limitations"; "**Developer credentials may not be embedded in open source projects**" (affects the embedded API key if §14 goes open source). Drive Program Policies: "should not be used as a replacement for a content distribution network… Google Drive will restrict usage when it appears that it's being used for large-scale public streaming." Small encrypted slots between known members are far from that, but nothing explicitly blesses "Drive as an app data relay". User Data Policy: prominent disclosure and consent needed before creating public links on the user's behalf. |
| Operators | One: Google. Highly sustainable. Policy shifts (the 2026 quota/billing model) are the realistic risk. |
| Reliability / limits | Per project 1 M units/min; per user 325k units/min; 403 "User rate limit exceeded" / 429 on backend throttling. Files up to 5 TB; 750 GB/day upload. Link-shared downloads can be throttled when "too many users" fetch one file (unlikely for 2–10 members). Retention: as long as the file and account exist. The user can delete or unshare it, and the app must re-create it. |
| Privacy / metadata | Content is E2E (group key). **Google sees** the writer's Google identity bound to each group mailbox file, plus the IPs and timing of every reader fetching it, so Google can build the group's social graph (worse than Nostr, where keys are throwaway). Anyone holding the file ID (members, ex-members until rotation) can call `files.get` with an API key. The User resource's `emailAddress` "may not be present… if the user has not made their email address visible". Owner **display name** exposure to link holders is likely; email exposure depends on settings (**experiment X3**). |
| Android | Plain HTTPS in WorkManager/expedited jobs; no sockets kept open; negligible battery. Writers need an OAuth token in the background: the Play-services Authorization API can return a token silently once granted (**unverified in a background job after process death**, X6). **Not available in the `foss`/F-Droid flavor** (R8-12: custom URI schemes are unsupported on Android; client credentials can't go in public repos). Readers need only an API key. Restrict it to package + signing cert on Android. The desktop key is extractable, so others could burn our project quota (quota-exhaustion DoS). |
| Libraries | Kotlin: plain REST over OkHttp + `play-services-auth` (already needed for backup). `com.google.apis:google-api-services-drive` v3-rev20260428-2.0.0 (Apache-2.0) exists but is heavy and unnecessary. Rust (desktop): `google-drive3` 7.0.0+20251218 (MIT, 2026-01-01, ~3.0 M downloads, 93 KB crate); `yup-oauth2` 12.1.2 (MIT/Apache-2.0). No GPL/MPL. |
| Abuse resistance | Writers are authenticated Google accounts, so Google's own abuse systems apply. The flip side: **collateral risk to the user's primary Google account** if automated systems misjudge the traffic. Low probability at our volumes, severe impact. Readers can't write. The embedded API key can be abused to exhaust the project quota. |
| Precedent | I found none of an app using members' Drives as a cross-user mailbox. Counter-precedent: Joplin supports Drive/Dropbox/OneDrive for *own-device* sync but offers notebook sharing **only via Joplin Cloud** ("Using Joplin Cloud you can share notebooks between users"). Closest analogue: Delta Chat using users' own email accounts as transport (R5), which later moved to dedicated relays. |
| Failure modes | Project daily quota exhausted (all users lose the layer until reset); a future billing requirement; file deleted or unshared by the user; Workspace admin blocks `anyone` sharing; storage full; false-positive abuse flag ("Only file owners can download files identified as abusive content"); token revoked; `foss` build has no Drive at all. |
| **Verdict** | **Mailbox (G1): CONSIDER as an opt-in, scale-capped parallel layer**, with low-medium confidence. It must never be a load-bearing layer, and must be designed to stay far under the project threshold: metadata-first polling, epoch-rotated files, adaptive back-off on 403/429. **Blobs (G6): CONSIDER**, medium confidence; the best fit in this dimension. **Unauthenticated usercontent URL: REJECT** (robots.txt + Google ToS). |

---

## Candidate 2: Dropbox shared links

| Criterion | Finding |
|---|---|
| Mechanism | App-folder app ("A dedicated folder named after your app… read and write access to this folder only"). `sharing/create_shared_link_with_settings` (scope `sharing.write`, `allow_app_folder_app = true`, default visibility public). Readers use the public link with `dl=1` ("force a browser to download") or `raw=1` ("available to anyone with no restrictions"). The API route `get_shared_link_file` needs `auth = "app, user"` (an app secret or user token), so readers without a Dropbox account must use the web link. |
| Gaps | G1 partial (opt-in writer), G6 yes. Nothing else. |
| Cost | Users: Basic = free 2 GB. Developer: no published per-app fees in the Developer Terms (effective 2025-03-01). |
| ToS | Developer Terms: "Dropbox may set and enforce limitations… in its own discretion"; production status: "Dropbox may, in its sole discretion, deny these requests". The Developer guide says the app works for "up to 500 total Dropbox users. However, once your app links 50 Dropbox users, you will have two weeks to apply for and receive production status approval before… linking additional Dropbox users will be frozen." **Acceptable Use Policy:** prohibits using the Services "to back up, or as infrastructure for, your own cloud services". A sync mailbox for our app's users is arguably exactly that. `www.dropbox.com/robots.txt` disallows `/s/`, `/scl/` for `*`. Dropbox's AUP instead requires "publicly supported interfaces", and shared links are one, so the robots issue is weaker than Google's. |
| Limits | Help center (updated 2026-08-14): Basic "20 GB" per day and "up to 100,000 file downloads per day". On violation, "your sharing activity and link or file requests are paused for 24 hours"; repeats get longer suspensions. The penalty hits **all of the user's shared links**, not just ours. API rate limits are unpublished (`too_many_requests`, `too_many_write_operations`, `retry_after`). |
| Operators | One (Dropbox). |
| Privacy | Same shape as Drive: Dropbox links the writer's account to group files and sees readers' IPs. |
| Android | Plain HTTPS; PKCE OAuth (no secret). Fine. |
| Libraries | Java `com.dropbox.core:dropbox-core-sdk` 7.0.0 (MIT; repo active 2026-09-23). Rust `dropbox-sdk` 0.20.3 (Apache-2.0, 2026-07-29). |
| Reach | Far fewer Android users have Dropbox than Google accounts. The design already lists Dropbox as "only if users ask" (§11). |
| Precedent | None found for cross-user mailboxes. |
| Failure modes | Production approval denied (the layer freezes at 50 users); link paused 24 h; unknown whether a shared link survives `files/upload mode=overwrite` with the same URL and fresh content (**unverified**, X5). |
| **Verdict** | **REJECT for now**, medium confidence. Workable technically, but low reach, discretionary approval, and an AUP clause that plausibly covers this use. Revisit only if Drive proves out and users ask for Dropbox. |

---

## Candidate 3: OneDrive (personal)

| Criterion | Finding |
|---|---|
| Mechanism | `createLink` with `scope: anonymous` ("Anyone with the link has access, without needing to sign in"; "Links always point to the current version of an item"). |
| Blockers | (1) `createLink` least-privileged permission for personal accounts is **`Files.ReadWrite`** (full drive), not AppFolder (Graph docs, updated 2026-08-04). (2) Reading via `GET /shares/{encoded}` requires an `Authorization: Bearer` header, so **readers need a Microsoft account**; for OneDrive for Business the Shares API "always requires authentication". Account-free reading would mean scraping `1drv.ms` web links (undocumented; not evaluated further). (3) Registration needs an Entra tenant via an Azure free account with a credit-card identity check (R8). (4) Open regression: onedrive-api-docs **#1929** (2026-08-23) "newly consented apps get 403 accessDenied / serviceReadOnly ('Database Is Read Only') on ALL drive endpoints" and **#1930** (2026-08-25) "Files.ReadWrite.AppFolder: every drive endpoint returns 403… only workaround is full-drive Files.ReadWrite". Both OPEN, and the reporter's analysis points at a missing one-time provisioning step. |
| Gaps | G1/G6 in theory. |
| Libraries | Rust `onedrive-api` 0.11.0 (MIT, 2026-01-21); MSAL Android 6.0.1. |
| **Verdict** | **REJECT**, high confidence. A full-drive scope for a mailbox is a privacy regression, readers need accounts, and registration isn't free of identity checks. |

---

## Candidate 4: MEGA

| Criterion | Finding |
|---|---|
| Mechanism | Public file links with the key in the URL fragment; E2E by design (redundant with ours). |
| ToS | MEGA Terms (last updated 02.03.2026, effective 02.04.2026): "you may only directly and specifically use the API if you register at the developer registration page and agree that you will only publish or make available your application after we have approved it pursuant to our application approval process and licence agreement available on request". Also: accounts can be terminated if "you are using a free account and that account has been inactive for over 3 months"; "open multiple free accounts" prohibited; Reasonable use: usage "materially exceed[ing] the average… use patterns" may be suspended; "MEGA may… impose limits or restrictions on… Bandwidth usage in respect of any account". |
| Limits | Download quota for non-users and free users: "based on the data transferred to your IP address during the last six hours… influenced by factors such as your geographical location and our current network usage" (no numbers published). On CGNAT, strangers share the quota. |
| Accounts | Writers need a MEGA account (email + password + verification), which fails the no-accounts / seamless constraints far worse than an already-present Google account. |
| Libraries | C++ SDK `meganz/sdk` (BSD-2-Clause, active, v10.19.0 on 2026-08-25): large native dependency. Rust `mega` 0.8.0 (MIT/Apache-2.0), **last release 2024-10-09** (stale). |
| **Verdict** | **REJECT**, high confidence. |

---

## Candidate 5: Anonymous paste/file hosts (0x0.st and similar)

| Criterion | Finding |
|---|---|
| Mechanism | Anonymous HTTP POST returns an immutable URL. 0x0.st: retention 30 days to 1 year by size, 512 MiB max, `X-Token` for delete/expiry. paste.rs: `POST` raw data, `DELETE /<id>`, ~384 KiB web-form limit, "Pasting is heavily rate limited". |
| Fit | **Immutable URLs can't be a replaceable slot.** Every update is a new URL that must be announced over another layer (Nostr/P2P), so it adds nothing to G1's independence. At best it's a G6 blob side-channel. |
| ToS | 0x0.st: "0x0.st is NOT a platform for: … backups, CI build artifacts, other automated mass uploads… Uploads found to be in violation of these rules will be removed, and the originating IP address may be blocked". The client-author notes ask software to "only do that with user consent" and to tell users "it's a public file hoster run by some Internet rando in Germany… you can make no privacy guarantees". The front page is openly hostile to automated clients ("CLANKERS ARE NOT WELCOME HERE"). `robots.txt`: `Disallow: /*.*`. |
| Operators / sustainability | Single individuals. 0x0.st: "about 400 GB" of disk, "10 and 40 TB a month" of traffic, deliberately designed so "it will not grow beyond the very small scale you can handle". transfer.sh: the repo now says public installs won't be advertised, and `https://transfer.sh/` failed to connect during this pass (churn). catbox.moe: its robots.txt disallows Anthropic's agents, so I did **not** read its pages. Not evaluated. |
| Privacy | 0x0.st stores uploader IP + UA per file. |
| Abuse | Anything scale-shaped from our app would look like the "automated mass uploads" these operators ban. |
| **Verdict** | **REJECT**, high confidence (ToS, single volunteer operators, immutability, churn). |

---

## Candidate 6: GitHub Gists / raw

| Criterion | Finding |
|---|---|
| Mechanism | Gist per member (secret gist), updated via API; readers fetch `raw_url` unauthenticated. The API returns "up to 1MB of content per file" (truncated beyond), raw up to 10 MB, clone above. |
| Accounts | "You must be signed into GitHub to create gists"; token scope `gist`. Non-technical users don't have GitHub accounts. **Fails the seamless / no-accounts constraint outright.** |
| ToS / limits | AUP: "If we determine your bandwidth usage to be significantly excessive in relation to other users of similar features, we reserve the right to suspend your Account"; prohibits "automated excessive bulk activity". Changelog 2025-05-08: updated (unspecified) rate limits for unauthenticated requests, covering "anonymously interacting with our REST APIs, and downloading files from raw.githubusercontent.com". Unauthenticated REST is 60 req/h per IP (D41/R8), which is hopeless behind CGNAT. |
| **Verdict** | **REJECT**, high confidence. |

---

## Comparison vs gaps

Legend: ✔ covers · ◐ partial / with caveats · ✘ no.

| Candidate | G1 offline S&F (non-Nostr) | G2 P2P w/o n0 | G3 wake phones | G4 relay failed punches | G5 presence | G6 blobs | $0 dev at any scale | No-account readers | Verdict |
|---|---|---|---|---|---|---|---|---|---|
| Google Drive (API key readers) | ◐ opt-in writers, poll-only, **scale-capped** | ✘ | ✘ (push needs webhook server) | ✘ | ◐ slow polled | ✔ (best fit) | ✘ above ~400 M units/day | ✔ | **CONSIDER** (mailbox: low-med; blobs: med) |
| Google Drive (usercontent URL scraping) | ◐ | ✘ | ✘ | ✘ | ◐ | ◐ | ✔ (unmetered) | ✔ | **REJECT** (robots.txt + Google ToS, undocumented) |
| Dropbox shared links | ◐ | ✘ | ✘ | ✘ | ◐ | ✔ | ✔ (discretionary approval) | ✔ | **REJECT for now** (medium) |
| OneDrive | ◐ | ✘ | ✘ | ✘ | ✘ | ◐ | ✔ | ✘ | **REJECT** (high) |
| MEGA | ◐ | ✘ | ✘ | ✘ | ✘ | ◐ | ✔ | ✔ | **REJECT** (high) |
| 0x0.st-type hosts | ✘ (immutable, ToS) | ✘ | ✘ | ✘ | ✘ | ◐ (ToS) | ✔ | ✔ | **REJECT** (high) |
| GitHub Gists | ◐ (accounts) | ✘ | ✘ | ✘ | ✘ | ◐ | ✔ | ✔ (tight limits) | **REJECT** (high) |

---

## New risks surfaced

1. **The Google Workspace API quota and billing model (2026-05-01) also touches the planned Drive *backup*.** All Drive traffic from all users counts against one developer project, with charges above 400 M units/day "later in 2026" and quota increases gated on billing. Backup volume is tiny (one `files.update` ≈ 50 units per device per day ⇒ ~8M devices), so backup is safe. Any *polling* feature on Drive is not. Add this to D42/§11/§13.
2. **Collateral damage to users' primary accounts.** Automated misclassification by Google or Dropbox (Drive "will restrict usage…"; Dropbox pauses *all* sharing for 24 h) would hit the user's own Gmail/Drive/Dropbox, not an app-specific key. This harm class is new; Nostr keys are throwaway.
3. **The cloud provider learns the group's social graph.** The writer's real identity is bound to each group's mailbox file, and every reader's IP hits that file. That's a metadata regression versus Nostr's per-device-per-group throwaway keys (D23). Needs explicit consent wording.
4. **The embedded API key is a shared-fate DoS surface.** Anyone who extracts it (trivial on desktop) can spend the project's daily quota, knocking the layer out for every user. Android key restrictions (package + cert) help, desktop ones don't. It also conflicts with "Developer credentials may not be embedded in open source projects" (§14).
5. **`foss`/F-Droid builds get no Drive layer at all** (R8-12), so the layer can't be counted on for parity across flavors.
6. **Temptation risk:** the unmetered usercontent URL "works" in quick tests. Shipping it would put users' traffic against a robots `Disallow: /` host, inside Google's automated-access clause. Document the decision so nobody "optimises" into it later.

---

## Suggested SAFE experiments

X1–X6 need the **user's own test Google/Dropbox accounts** and send a few requests to those providers. They are outward traffic, so run them only with the user's go-ahead. X7 is purely local.

- **X1: `drive.file` + `anyone` permission.** In a test Cloud project, create a file with `drive.file` and call `permissions.create {type: anyone, role: reader, allowFileDiscovery: false}` on a personal account. Record the response. Repeat on a Workspace account with external sharing disabled to capture the failure mode.
- **X2: Freshness and conditional reads via API key.** Poll `files.get?fields=version,md5Checksum,modifiedTime&key=…` and `?alt=media` every 10 s for 5 min while the writer rewrites the file. Record staleness, the ETag/`If-None-Match` behaviour, and response sizes. From Wi-Fi and from a phone on mobile data (CGNAT).
- **X3: Metadata exposure.** With only the API key, request `fields=owners,lastModifyingUser,permissions` on the public test file, and record whether the owner's display name, email or photo are visible to link holders.
- **X4: Quota accounting.** After X2, read the Cloud console *Quotas & System Limits* metrics to learn whether `alt=media` is charged 5 or 200 units, and whether API-key calls count as per-user or only per-project.
- **X5: Dropbox link stability** (only if Dropbox is revisited). Create a shared link in an app-folder app, overwrite the file, and check that `dl=1` returns new content at the same URL.
- **X6: Background token.** In a test build, obtain a Drive access token from a WorkManager job after process death with no UI (Play-services Authorization API), on Android 15/16.
- **X7: Local cost model (no network).** A script that computes daily quota units per device for group sizes 2–10, sync cadences (15 min/1 h/6 h), and metadata-first vs download-always polling. Output the device ceiling under 400 M units/day and the per-minute peak vs 1 M units/min. Use it to set the layer's back-off policy.

---

## Sources

Google Drive / Google:
- Drive API scopes (updated 2026-09-03): https://developers.google.com/workspace/drive/api/guides/api-specific-auth
- permissions.create (scopes): https://developers.google.com/workspace/drive/api/reference/rest/v3/permissions/create
- Permission resource: https://developers.google.com/workspace/drive/api/reference/rest/v3/permissions
- Share files guide (updated 2026-09-14): https://developers.google.com/workspace/drive/api/guides/manage-sharing
- Usage limits (updated 2026-09-11): https://developers.google.com/workspace/drive/api/guides/limits
- Standardized tiering model (updated 2026-09-03): https://developers.google.com/workspace/tools-safety
- Workspace Updates blog, 2026-05-01: http://workspaceupdates.googleblog.com/2026/05/agent-tools-and-security-updates-for-workspace-developers.html
- Downloads guide: https://developers.google.com/workspace/drive/api/guides/manage-downloads
- App data folder: https://developers.google.com/workspace/drive/api/guides/appdata
- Revisions: https://developers.google.com/workspace/drive/api/guides/manage-revisions
- User resource: https://developers.google.com/workspace/drive/api/reference/rest/v3/User
- Create credentials (API keys for public files): https://developers.google.com/workspace/guides/create-credentials
- Google APIs Terms of Service: https://developers.google.com/terms
- Google API Services User Data Policy: https://developers.google.com/terms/api-services-user-data-policy
- Google Terms of Service (effective 2026-07-30): https://policies.google.com/terms
- Drive/Docs abuse program policies: https://support.google.com/docs/answer/148505
- Drive sharing help: https://support.google.com/drive/answer/2494822
- robots.txt: https://drive.usercontent.google.com/robots.txt , https://drive.google.com/robots.txt
- gdown README/FAQ and source: https://github.com/wkentaro/gdown (issues #391, #406, #354; `gdown/download.py`)
- rclone Drive docs: https://rclone.org/drive/
- crates: https://crates.io/crates/google-drive3 , https://crates.io/crates/yup-oauth2 ; Maven: com.google.apis:google-api-services-drive

Dropbox:
- Bandwidth limits (updated 2026-08-14): https://help.dropbox.com/share/banned-links
- dl/raw parameters: https://help.dropbox.com/share/force-download
- Acceptable Use Policy: https://www.dropbox.com/acceptable_use
- Developer Terms (effective 2025-03-01): https://www.dropbox.com/developers/terms
- Developer guide (500/50 users, production): https://docs.dropboxapi.com/dropbox-api/docs/developer-resources/developer-guide
- API spec (routes, rate-limit errors): https://github.com/dropbox/dropbox-api-spec (`sharing.stone`, `auth.stone`)
- robots.txt: https://www.dropbox.com/robots.txt
- SDKs: https://github.com/dropbox/dropbox-sdk-java , https://crates.io/crates/dropbox-sdk

OneDrive / Microsoft Graph:
- createLink (updated 2026-08-04): https://learn.microsoft.com/en-us/graph/api/driveitem-createlink?view=graph-rest-1.0
- shares-get: https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0
- AppFolder regression: https://github.com/OneDrive/onedrive-api-docs/issues/1929 , https://github.com/OneDrive/onedrive-api-docs/issues/1930
- Prior research: `R8-policy.md` (Azure registration, R8-13)

MEGA:
- Terms (updated 02.03.2026): https://mega.io/terms
- Transfer quota help: https://help.mega.io/plans-storage/space-storage/transfer-quota
- SDK: https://github.com/meganz/sdk (BSD-2-Clause); Rust crate: https://crates.io/crates/mega

Anonymous hosts:
- 0x0.st (ToS, retention, client notes): https://0x0.st/
- paste.rs: https://paste.rs/
- transfer.sh repo: https://github.com/dutchcoders/transfer.sh

GitHub:
- Acceptable Use Policies: https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies
- Gists REST API: https://docs.github.com/en/rest/gists/gists
- Unauthenticated rate limits changelog (2025-05-08): https://github.blog/changelog/2025-05-08-updated-rate-limits-for-unauthenticated-requests/

Precedent:
- Joplin notebook sharing (Joplin Cloud only): https://github.com/laurent22/joplin/blob/dev/readme/apps/share_notebook.md

---

## Independent verification

*Skeptical second pass, 2026-09-24. I re-fetched the primary sources with WebFetch, checked robots.txt with curl, checked crate metadata through the crates.io API and checked issue status with `gh api`. Nothing was installed, no accounts were used, and no files were uploaded.*

### Verdict checks

| Candidate | Researcher | Result | Notes |
|---|---|---|---|
| Drive public-link mailbox (API-key readers) | CONSIDER, low | **Upheld** (CONSIDER, low) | All the quoted numbers still match *Usage limits* (updated 2026-09-11): 1,000,000 units/min per project, 325,000 per user per project, 400,000,000 units/day and 1 TB/day egress "before charges", and costs of read 5, list 100, download 200, edit 50. Going over returns `403: User rate limit exceeded` or `429`. The *tools-safety* page (2026-09-03) confirms that Drive is in the tiering rollout: "API usage over standard daily thresholds will generate charges" later in 2026, with 90 days' notice. The credentials guide still says API keys "anonymously access… files shared using the 'Anyone on the Internet with this link' sharing setting". `permissions.create` accepts `drive.file`. Drive push (`watch`) needs an HTTPS webhook with a valid certificate, so it doesn't help G3. **Correction:** the API-key read path is not the only documented account-free read path. See "Missed 1" below: a variant that moves the reads off the metered API raises the ceiling roughly 4–5x. The layer stays CONSIDER and is still capped by writes. |
| Drive as blob store (G6) | CONSIDER, medium | **Upheld, with a correction** | The abuse rule is confirmed: "Files identified as abusive… are only downloadable by the file owner". **Missed limit:** the **1 TB/day per-project egress** limit binds before the 400M-unit limit whenever the average blob is over about 500 KB (400M / 200 = 2M downloads, and 1 TB / 2M = 500 KB). At 1–3 MB photos the real ceiling is about 0.3–1M blob downloads/day for all users combined, not 2M. It is still the strongest G6 option in this dimension, because DECISIONS already records that free Blossom servers are media-only or delete after about 60 days. |
| Dropbox shared links | REJECT, medium | **Upheld, but part of the reasoning is weaker than stated** | The Developer Terms (effective 2025-03-01) confirm that production approval is "in its sole discretion", along with the 50-user/2-week freeze and the Basic limit of 20 GB and 100,000 downloads/day (help page updated 2026-08-14). **The AUP argument is overstated.** "Use the Services to back up, or as infrastructure for, your own cloud services" targets services that resell or back onto Dropbox. A per-user **app folder** that the user connects on purpose is the ordinary third-party pattern Dropbox itself documents. The Developer Terms have no clause against app-data storage. The only related ban in the developer guide is "build publicly searchable file sharing networks on top of Dropbox", and encrypted group slots are not that. The REJECT holds on reach, discretionary approval and the 24 h sharing-pause penalty, not on the AUP. |
| OneDrive | REJECT, high | **Upheld** | Issues #1929 (2026-08-23) and #1930 (2026-08-25) are still **open**. `onedrive-api` 0.11.0 was last published 2026-01-21. |
| MEGA | REJECT, high | **Upheld** | Crate `mega` 0.8.0 was last published 2024-10-09 (stale). |
| 0x0.st-type hosts / Gists | REJECT, high | **Upheld** | No new evidence against. |
| usercontent URL scraping | REJECT | **Upheld** | Google ToS (effective 2026-07-30) contains the robots.txt / automated-means clause, verbatim. gdown #406 is closed and was attributed to Google rate limiting. |

Library facts were re-checked on crates.io: `google-drive3` 7.0.0+20251218 (MIT, 2026-01-01, about 3.0M downloads), `yup-oauth2` 12.1.2 (MIT/Apache-2.0), and `dropbox-sdk` 0.20.3 (Apache-2.0, 2026-07-29). All match the report.

### Missed or under-weighted

1. **"Publish to the web" Google Sheet as the read path (not evaluated in the report).**
   - **What it is:** the Drive `revisions` resource has `published` / `publishAuto` / `publishedLink` fields for Docs Editors files. A writer holding `drive.file` could create a Sheet, publish it, and rewrite it.
   - **Readers need no account and no API key.** They fetch `docs.google.com/spreadsheets/d/e/…/pub?output=csv`.
   - **This read path is allowed by robots.txt.** `docs.google.com/robots.txt` has `Allow: /spreadsheet` (a prefix, so it matches `/spreadsheets/…`) and `Crawl-delay: 1`. That is unlike `drive.usercontent.google.com`, which has `Disallow: /`.
   - **Effect on quota:** reads leave the metered project quota entirely. Only writes are metered.
   - **Write cost:** at about 20 writes/device/day × 50 units, the ceiling rises to about 400k devices, versus about 87k. It also removes the embedded reader API key, which is both the quota-DoS surface and the open-source credentials conflict.
   - **Caveats:**
     - Help center: republishing "might take a few minutes". That is acceptable for a poll-only layer.
     - Work and school admins can disable publishing.
     - Slot bytes have to fit in cells, so base64 would need to be split across cells.
     - Writing through the Sheets API is far worse: 300 writes/min per project, about 21k devices. Writes must therefore go through Drive `files.update` with a converting upload. **Unverified:** whether that update keeps the published revision and the `publishAuto` setting.
     - Unknown throttling of heavily polled published CSVs from CGNAT.
   - **Suggested experiment X8:** publish a test Sheet, rewrite it through the Drive API, and measure lag and staleness.
2. **Blob egress cap** (see the table above). Add the 1 TB/day per-project egress limit to the X7 cost model.
3. **Apps Script web app in the writer's own account.**
   - **What it is:** `script.google.com/macros/s/…/exec` with ContentService.
   - **Quotas are per user, not per project:** "Quotas are per user"; consumer accounts get 30 simultaneous executions per user and 50,000 properties reads/writes per day.
   - **Robots:** `script.google.com/robots.txt` returns 404, so no restrictions apply. HTTP access is the documented purpose of web apps.
   - **Why it is probably a REJECT:**
     - Deploying one needs the Apps Script API, which each user must enable themselves ("you must grant them access to your script projects", via the user-settings toggle).
     - It probably needs a sensitive `script.*` scope.
     - It means pushing executable code into users' accounts.
   - Listed so it is not rediscovered later as a "free" path.
4. **Users' own email as the mailbox (the Delta Chat pattern) is closed on Google.** The Gmail scopes page lists `https://mail.google.com/`, `gmail.readonly`, `gmail.modify`, `gmail.insert` and `gmail.metadata` as **restricted**, which requires a security assessment. `gmail.send` is sensitive. The Gmail API is also inside the 2026 tiering model. Record this so the idea isn't reopened.
5. **Grandfathering is temporary.** "Projects created on or after May 1, 2026 are subject to new quotas; earlier projects retain previously set limits", but only "for at least 60 days" per the tiering page. An older Cloud project is not a durable escape from the threshold.

### Bottom line
The researcher's numbers and quotes hold up. No verdict flips. The main thing missed is a Drive variant: publishing the file to the web from a Sheet, which moves the reads onto a robots-allowed host. It is documented and not metered for readers. It should be considered alongside the API-key variant before the Drive mailbox layer is designed.
