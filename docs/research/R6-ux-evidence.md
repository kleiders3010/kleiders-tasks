# R6: User experience evidence

*Date: 2026-09-24. Scope: the UX assumptions in `docs/UX-PRINCIPLES.md` and the user-facing parts of `docs/DESIGN.md` (§5 identity/sharing/invites, §7 write behavior, §8 Android lifecycle and widgets, §11 recovery, §12 desktop firewall). Method: web research (vendor docs, AOSP source, issue trackers, community forums, app-store review excerpts, peer-reviewed usability studies), plus one local data analysis of candidate invite-code word lists (Appendix A). No software was installed; no traffic was sent to relays.*

**Evidence grades used below:** **A** = official docs, platform source code, peer-reviewed study. **B** = project announcements, issue trackers, several independent user reports. **C** = a single forum post, app-store review excerpt, or third-party blog.

**Limitations.** The session's web-search quota ran out partway through, so some leads were followed only by fetching known URLs. Reddit and Play Store review pages could not be fetched directly, so real-user voice comes from vendor forums, GitHub issues and review excerpts. **I found no study that measures how long users will tolerate shared-list delays at the scale of minutes.** That question stays open (experiment E-UX5).

---

## Summary

1. **There is a real market gap, but server apps fail on reliability too.** Google Tasks still can't share a list (only tasks in Chat spaces or Docs). Todoist's free plan is capped at 5 projects and 5 collaborators. Any.do puts recurring tasks behind the paywall. Every shared-list app needs an account. Yet the most common complaint, even about server-based apps (Microsoft To Do 2024–2025, Apple Reminders), is still "shared list doesn't sync". A P2P design only wins if sync *feels* at least as reliable as OurGroceries or AnyList, which promise updates "instantly" or "within seconds".
2. **Free P2P apps keep failing on the same four problems, and the design already avoids most of them.** Briar went into maintenance mode on 2026-07-09, citing "high battery usage and unreliable background operation on Android", "missing ... account backup", and a "difficult user experience for adding contacts and communicating offline". Anytype and Keet users lose everything when they lose their recovery phrase. Nostr onboarding treats relays as "a high friction step". Seedless recovery (Block Store), no always-on service, hidden relays and a mailbox for offline members are the right calls.
3. **Biggest UX gap: nothing tells a user when someone else changed a list while the app is closed.** A shopper who has the app open sees changes within seconds. But the design has no push doorbell, so "Alex added eggs" notifications and home-screen widgets wait for the 15-minute periodic sync. Doze, standby buckets and OEM battery managers stretch that to hours, and to about a day for people who rarely open the app. Competitors sell exactly this moment ("add milk ... your partner sees it immediately, even if they're already shopping at the store"). **The design should not wait for user complaints.** Add per-member freshness, a one-tap "Tell Alex" through the user's own messenger, and honest wording now.
4. **"Up to date" can't be known in a P2P system, and claiming it backfires.** Anytype users reported that "Both devices say 'Synced' but they show different information". The status should say what is actually known, WhatsApp-style: "Saved" → "Sent" → "On Alex's phone". This can be derived from Loro version vectors.
5. **The invite-code word list has defects, and a typo fails silently.** In the EFF large list, 4 hyphenated words (`t-shirt`, `yo-yo`, `drop-down`, `felt-tip`) clash with the hyphen separator. At least 13 same-sounding pairs survive despite EFF's claim to have removed homophones (`cache/cash`, `capital/capitol`, `carat/carrot`, `cannon/canon`, `shone/shown`, `flier/flyer`…). It has 2,364 pairs of words one edit apart, 985 words missing from a 126k-word pronouncing dictionary (`greedless`, `judgingly`, `unretired`), and only US spellings (`gray`, `theater`, `defense`). A code averages about 47 characters. A mistyped word gives the same "not found" as an expired code. Fix the list and the entry UI (autocomplete chips, separator and spelling tolerance). This complements E7, which proposes 8 words from the EFF short 2.0 list; that choice costs about 20 syllables when read aloud (Appendix A).
6. **CRITICAL: device linking can be phished.** The approval step is on the wrong device for the attack that state actors used against Signal in 2025: "linked device" QR codes "masquerade[d] as group invites". Here, a victim who scans an attacker's *link* QR, thinking it's a list invite, has their personal group merged into the attacker's. The attacker approves on their own phone, gets every list, and keeps getting every future list. Fix: require the two devices to be near each other, put a full warning on the scanning device, and never accept a link payload from the join path.
7. **The clipboard handoff for "install from invite" is fragile.** Android 10+ lets an app read the clipboard only while it has focus. Android 12+ shows an "APP pasted from your clipboard" toast. AOSP clears the clipboard after 1 hour by default (`PROPERTY_AUTO_CLEAR_ENABLED` defaults to `true`, timeout 3,600,000 ms). Any later copy overwrites it. Fixes: check `getPrimaryClipDescription()` first (it shows no toast); send a secret-free "came from an invite" flag through the Install Referrer; add a clear "tap the link again" step.
8. **Display names: hex tags like "Alex · 4F2A" don't help people.** Discord dropped discriminators after finding that ">40%" of users didn't know theirs and "nearly half of all friend requests failed". The most likely duplicate is the *same person* on a second, unlinked device who joined through the invite. Add local nicknames (Signal added these in 2024) and "Is this your other device?" detection.
9. **Smaller fixes.** A widget's "synced X ago" label freezes between redraws, so use absolute times. Merges that arrive after the splash screen can move a row under the user's finger. The desktop assumption that Windows Firewall will prompt is contradicted by a Delta Chat report for iroh's UDP.
10. **Validation plan.** Five-person playtests find flow problems but can't find sync problems. Add a multi-day diary study with couples on several OEMs, and a code-entry study with 20+ people that includes non-native English speakers.

---

## Claim-by-claim table

| # | Claim tested (source) | Verdict | Severity if wrong | Key evidence | Proposed fix |
|---|---|---|---|---|---|
| 1 | Mainstream task apps leave a gap: sharing, accounts, paywalls, widgets, clutter (mission) | **Confirmed** | none | Google Tasks: shared tasks only in Chat spaces / Docs; Todoist free: 5 projects, 5 collaborators; Any.do: recurring tasks paid; To Do and Reminders shared-list sync threads 2024–2025 | Compete on reliability *perception* as much as price; see #3, #10 |
| 2 | P2P/local-first failure modes (battery, delays, onboarding, key loss) are avoided by the design (§2, §5, §8, §11) | **Partial** (mostly confirmed) | medium | Briar maintenance mode 2026 (battery, background, backup, contact adding, offline); Syncthing-Android discontinued 2024 (Play + upkeep); Anytype/Keet phrase loss; Anytype "Synced" while different | Keep seedless recovery and no always-on service. Fix status honesty (#10), notification latency (#3), desktop firewall (#18) |
| 3 | Minutes-to-hours delay is acceptable; "Stay connected" can wait for user reports (§8) | **Partial / likely wrong for shared shopping lists** | **high** | OurGroceries "instantly", "within seconds... even if they're already shopping"; Bring! push on every change; users abandon apps over sync failures | Per-member freshness, "Tell Alex" via share sheet, suppress stale notifications, keep screen on while shopping, honest store copy |
| 4 | "Usually within 15 minutes, longer if your phone has been idle" is honest (§8) | **Partial** | medium | Standby buckets: frequent = 10 min of jobs per 12 h, rare = 10 min per 24 h, restricted = once a day; OEM killers | Don't promise a number. Show the *receiver's* last check-in |
| 5 | 6 words from the EFF large list are a usable invite code (§5) | **Refuted as specified** | **high** | Local analysis: hyphenated words, 13+ homophone pairs, 2,364 edit-1 pairs, 985 rare words, US-only spellings, ~47 chars; Shay et al.: spacing/case/typo/order errors; E7: 92% of codes have a typo-ambiguous word | Curated list + chip autocomplete + tolerant parsing + "check the words" copy; choose the list by experiment (6×EFF-large-fixed vs 8×EFF-short-2.0 vs 7×BIP39) |
| 6 | Clipboard handoff reliably carries the invite through install (§5) | **Partial** | medium | Android 10 focus rule; Android 12 toast; AOSP auto-clears after 1 h; Firebase Dynamic Links shut down 2025-08-25 | `getPrimaryClipDescription()` first; secret-free referrer flag; "tap your link again" screen; landing-page copy |
| 7 | First launch asks nothing; the name is asked at first share/join; permissions just in time (UX-PRINCIPLES) | **Confirmed** | none | Android notification-permission guidance ("users are more likely to grant"); Nostr design guide; Signal PIN study (44% of casual users couldn't explain the PIN) | Put the name field *on* the join screen, not in an extra step |
| 8 | Display names + generated avatar + short tag handle duplicates (§5) | **Partial** | medium | Discord: >40% didn't know their discriminator, ~half of friend requests failed; Matrix disambiguates with full IDs; Signal added local nicknames (2024) | Local nicknames, "invited by", same-name detection with "Is this your other device?", name changes shown in history |
| 9 | Approval on an existing device makes linking safe (§5) | **Refuted** | **critical** | GTIG 2025-02-19: Signal linked-device QR codes disguised as group invites; here the attacker holds the approving device | Proximity requirement, warning on the scanning device, link payloads rejected from the join path, "new device linked" alert on all devices |
| 10 | "State is always visible... whether you're up to date" (UX-PRINCIPLES) | **Refuted in wording** | medium-high | Anytype: "Both devices say 'Synced' but they show different information" (2022); GitHub #933 (2023); To Do: notification arrives, list unchanged | Claim only what's known: Saved / Sent / On Alex's phone (version vectors); per-member "checked in X ago" |
| 11 | "No technical words, ever" (UX-PRINCIPLES) | **Partial** | low | Microsoft To Do exposes "Sync status → Sync now"; users say "sync" | Allow everyday words (sync, offline, updated); ban transport jargon; errors say what to do |
| 12 | "Undo instead of confirmation dialogs" (UX-PRINCIPLES) | **Partial** | medium | NN/g: undo is best for *your own* mistakes; with shared data, other people's deletes surprise you; a shopping app review: "Collaboration generates multiple copies of some items" | Shared "Recently deleted" with who/when; banner on others' devices after bulk deletes; "Delete list" on a shared list = Leave |
| 13 | Widget with a "synced X ago" label (§8) | **Refuted as specified** | low-medium | Widgets redraw only on events or ≥15–30 min jobs, so relative text freezes | Absolute time ("Updated 14:05") or show only when stale |
| 14 | Seedless recovery (Block Store / Auto Backup) avoids the #1 local-first failure (§11) | **Confirmed** direction, **partial** coverage | medium | Phrase loss in Anytype, Keet, Nostr; Block Store cloud restore needs the same Google account and, on non-Pixel phones, an Android 12+ target; E2EE needs a screen lock | Visible backup state under "Your devices"; one-time backup prompt also when Block Store E2EE is unavailable |
| 15 | ~5-person playtests validate the rules (UX-PRINCIPLES) | **Partial** | medium | NN/g: 5 users per *homogeneous* group, qualitative only; 20 for metrics | Add user groups; multi-day multi-OEM diary study; quantitative code study |
| 16 | Invite links open the app from chat apps (§5) | **Unverified** | medium | In-app browsers in Instagram, TikTok, Facebook, LinkedIn and email apps; Instagram deep-link bug May 2026 | Test matrix (E-UX4); landing page with an explicit "Open in app" and "open in browser" hint |
| 17 | Splash held ≤1.5 s, then later changes appear live (§8) | **Partial** | low-medium | Layout shifts make users "click the wrong link or button" (web.dev CLS) | Don't move the row under a finger; animate inserts; mark "new from Alex" |
| 18 | "Windows Firewall shows its own 'Allow access' dialog" (§12) | **Unverified**, contradicted by one report | low | Delta Chat #3218: iroh/QUIC over UDP didn't trigger the prompt; Linux UFW blocks incoming UDP (Delta Chat, July 2026) | MSI adds a firewall rule; don't pre-explain a prompt that may not appear; test (E-UX9) |

---

## Details per claim

### 1. Mainstream task apps: what users complain about (confirmed)

**Sharing needs accounts and is limited or missing.**
- Google Tasks can't share a list. Google's own help covers shared tasks only in "a space in Google Chat" and "Google Docs for eligible Google Workspace plans", and says "You can neither create a subtask nor repeat shared tasks" (A: support.google.com/tasks/answer/11549608).
- Todoist's free ("Beginner") plan has 5 personal projects and 5 collaborators per project. It needs an account, and invitees must sign up (A: todoist.com/pricing; Todoist help). Third-party 2026 write-ups report that the free plan dropped from 80 to 5 projects and prices rose in December 2025 (C: usecarly.com).
- OurGroceries sharing works only if the whole household uses "a single email address" (A: ourgroceries.com/user-guide).
- Microsoft To Do's moderators blame shared-list failures on "different Microsoft accounts or aliases" and tell all members to accept invites "using the same Microsoft account" (B: learn.microsoft.com Q&A 5536244).

**Paywalls.**
- Any.do: "the free plan lacks recurring tasks, an essential feature for to-do list apps" (C: Cloudwards review, updated May 2026).
- TickTick reviewers complain it "hides too much behind the subscription" and that free collaboration is limited to one other person (C: App Store review excerpts).

**Shared-list sync failures, even with central servers.**
- Microsoft To Do, April 2024: "Changes made by a users are not updated to any other users ... Notifications are received ... but lists are not updated", and "This happened last year also" (B: Q&A 5340706).
- Microsoft To Do, August 2025: three brothers' shared list stopped syncing across Windows, Mac, iPhone and Android (B: Q&A 5536244).
- Microsoft To Do, September 2025: "notifications arrive but the list on my account doesn't update", not fixed by reinstalling; the suggested workaround is to recreate the list (B: techcommunity 4451581).
- Apple Reminders 2025, one-way sync on a shared grocery list for "months", 14 "me too" votes (B: discussions.apple.com 256069453).
- Grocery-app reviews: "The sync function fails consistently ... we have abandoned use for family" and "Collaboration generates multiple copies of some items" (C: App Store / Play excerpts).

**Widgets.** Microsoft To Do's widget froze and disappeared on One UI 6.1; the cause turned out to be a launcher setting (C: techcommunity 4210290). A Keep update stopped its widget from updating (C: XDA). Widgets fail in OEM-specific ways.

**Takeaway.** The gap is real: free, no accounts, unlimited sharing. But reliability is what users judge, and here even Google, Microsoft and Apple visibly fail. A P2P app adds failure modes (NAT, relays, background limits), so it needs *more* visible reliability, not less (see #3, #10).

### 2. P2P / local-first apps: what goes wrong (partial: mostly avoided by design)

| Product | Evidence | Design status |
|---|---|---|
| **Briar** (maintenance mode, 2026-07-09) | "high battery usage and unreliable background operation on Android", "missing features like account backup", "difficult user experience for adding contacts and communicating offline" (B: briarproject.org) | Avoided: no always-on service, Block Store recovery, links/QR/codes, Nostr mailbox for offline |
| **Syncthing (Android)** | Discontinued Dec 2024: "Google making Play publishing something between hard and impossible" plus no maintainers (B: forum.syncthing.net). Syncthing-Fork: "20%+ of my drain", battery life from 2 days to 18–20 h on a Pixel 8 (C) | Avoided: no special storage permission, periodic sync only. The risk of Play friction remains (new risk N10) |
| **Keet** | "While offline, you cannot receive messages" (A: Keet support). Review: "Some messages randomly get dropped", notifications appear but messages don't until restart, "still feels like a beta" (C: stacker.news, 2025-06). Lost 24-word phrase means the account is "permanently unrecoverable" (A: Keet support) | Avoided (mailbox, no phrase). **Notification/data mismatch** is a warning (N4) |
| **Anytype** | "Both devices say 'Synced' but they show different information" (B: forum 2022); "Everything should be perfectly synced as long as green dot is glowing" (expected behavior, GitHub #933, 2023). Forgotten recovery phrase: "There is no way the devs can see your recovery phrase or retrieve it" (B: forum 2023) | Phrase avoided. **Status honesty not yet addressed** (#10) |
| **Nostr clients** | "During onboarding we'll likely want to minimize the topics of relays as this is a high friction step" (B: nostrdesign.org); lost nsec means a lost account | Avoided: relays only in diagnostics, no user keys |
| **Delta Chat** (uses iroh for "Add second device") | Timeouts: "failed connecting to remote endpoint: timed out" from a desktop firewall blocking incoming UDP (B: forum, July 2026). Windows Firewall didn't prompt for QUIC/UDP (B: GitHub #3218, 2023). User testing: "I don't know if it was successful or not" (B: forum, 2022–24) | Partly relevant: device linking over iroh (#18) |
| **Manyverse / SSB** | Creator stepped back in 2024 (B: Open Collective) | Maintainer-burnout risk (N9) |
| **SimpleX** | "sharing an invite link is awkward compared to searching a username" (C: privacytools.io); links too long "to speak them properly to someone on the phone" (B: GitHub #3863) | Addressed by 6-word codes, but see #5 |

**Verdict.** The design already avoids the killers that shut down or crippled these projects: phrase-based recovery, always-on services, exposed relays, and requiring both peers online. What's left: an honest status (#10), latency while the app is closed (#3), desktop firewall behavior (#18), and sustainability (N9).

### 3. Delay tolerance in shared lists (partial; high)

**What the design actually delivers, by scenario:**

| Scenario | Mechanism | Expected delay |
|---|---|---|
| Both people have the app open | iroh live connection / Nostr subscription | seconds |
| Sender edits then leaves the app; receiver opens the app later | flush on background (`shortService`); fetch on open, splash waits ≤1.5 s | seconds after opening |
| Receiver's app is closed and they wait for "Alex added eggs" | WorkManager periodic (≥15 min), deferred by Doze, standby bucket and OEM killers | 15 min to hours; about a day for rarely used apps |
| Receiver glances at the home-screen widget | redrawn only when a sync runs | same as above |

**What users are promised elsewhere.**
- OurGroceries: "any change made to a list by either person will be seen by the other instantly" (A: user guide). Its Play listing adds "visible within seconds... Add milk to the grocery list and your partner sees it immediately, even if they're already shopping at the store!" (C: Play listing excerpt).
- AnyList advertises changes that "show up instantly" (C).
- Bring! pushes a notification on every change: "everyone knows immediately when something has been purchased or a new food has been added" (C).

**Users react to missing or late updates by leaving.** "We have abandoned use for family and sync function" (C). Microsoft To Do and Apple Reminders threads show people recreating lists and reinstalling (B).

**What isn't known.** No source measures tolerance at the scale of *minutes*. Even Google Keep's Android app "does not synchronize instantly"; one integration recommends waiting about a minute (C: home-assistant-google-keep-sync README). So a few minutes may be tolerated, but "15 min to hours" for notifications almost certainly won't be, because a *notification* is the product for the "remind my partner at the store" case.

**The platform gives no free way to push instantly.**
- FCM needs a sending server (DESIGN §6 rightly rejects running one).
- UnifiedPush requires "a distributor" app installed separately (A: unifiedpush.org/users/distributors), which is not seamless.

The design's "reconsider if users report delays" is therefore betting on a problem the evidence already shows.

**Fixes (no server needed):**
1. **Per-member freshness** on the list's member sheet: "Alex · checked in 3 min ago" / "Alex · last seen yesterday". This needs a small heartbeat in each device's slot (bounded writes, e.g., at most hourly).
2. **"Tell Alex" nudge.** After someone adds to a shared list while no other member is connected, show a quiet inline hint: "Alex will see this next time their phone checks in · Tell Alex". The action opens Android's share sheet with text such as "Added eggs, bread to Groceries". The user's own messenger delivers the push instantly, for free, with no infrastructure of ours.
3. **Notifications that respect time:** batch them ("Alex added 3 items"), show age when late ("40 min ago"), and drop notifications for items already checked or older than a few hours.
4. **Keep the screen on while shopping:** while a list is open and items are being checked, keep the screen awake for a limited time, as navigation apps do. The live connection then stays up with no background tricks. Validate in E-UX5.
5. **Honest store copy:** "Instant when you both have the app open; otherwise updates arrive when phones check in".

### 4. "Usually within 15 minutes" (partial; medium)

- The Android power table lists job budgets per standby bucket (A: developer.android.com/topic/performance/power/power-details):
  - working set: 10 min per rolling 4 h;
  - frequent: 10 min per 12 h;
  - rare: 10 min per 24 h;
  - restricted: "once per day for up to 10 minutes".
- It also lists network as "Disabled" for the rare and restricted buckets, and says jobs are deferred to Doze maintenance windows.
- It notes that from Android 16, execution quotas also apply to the active bucket.
- OEM battery managers make this worse (B: dontkillmyapp.com; Samsung ranks worst).

For the family member who opens the shared list once a week, which is exactly who needs notifications, "within 15 minutes" is false. **Fix:** remove the number. Show the *other* person's freshness (#3.1). The one-time "background sync blocked" explanation stays. R4 should measure actual delivery on OEM phones.

### 5. Invite codes: 6 words from the EFF large list (refuted as specified; high)

**Local analysis** (Appendix A; E7 measured overlapping numbers independently):

- **Hyphen clash:** `t-shirt`, `yo-yo`, `drop-down`, `felt-tip` are list words, and the design separates words with `-` (`maple-orbit-guitar-...`). `shirt` and `down` are also list words. The code `maple-t-shirt-...` splits into 7 tokens, and reading "t-shirt" aloud invites `tshirt` or `t shirt`.
- **Homophones remain** despite EFF saying homophones were removed. Exact CMUdict matches include `cache/cash`, `cannon/canon`, `capital/capitol`, `carat/carrot`, `shone/shown`, `flier/flyer`, `phoney/phony`, `boney/bony`, `lilly/lily`, `caddie/caddy`, `baggie/baggy`, `banister/bannister`, `sulfate/sulphate`, and near-homophone `accent/ascent`. A listener *cannot* tell these apart when a code is read aloud.
- **One typo from another valid word:** 2,364 pairs (e.g., `action/auction`, `aground/around`, `acid/avid`). E7 counts 2,647 affected words (34%) and 91.8% of 6-word codes.
- **Rare or invented-looking words:** 985 of 7,776 (12.7%) aren't in the 126k-word CMU pronouncing dictionary, e.g., `greedless`, `judgingly`, `unretired`, `freeness`, `unflawed`, `backboned`, `skimpily`. Keyboards are likely to autocorrect them (to be measured in E-UX1). Some are awkward to read to family: `jockstrap`, `lustfully`, `feminize`.
- **US-only spellings:** `gray`, `theater`, `catalog`, `defense`, `mold`, `plow`, `pajamas`, `aluminum` are in the list, and `grey`, `theatre`, `defence`… are not. British, Commonwealth and non-native users will type non-words.
- **Length:** mean word 6.99 characters, so about 47 characters per code. Identifying a word needs 5.29 characters on average and up to 9, so autocomplete helps less than with BIP39 (≤4) or EFF short 2.0 (3).

**Error research.**
- Shay et al. (SOUPS 2012, 1,476 participants): among related entry errors, 25.2% were spacing only, 7.9% capitalization only, 15.2% simple typos, and 9.3% "correct words appearing in the wrong order". Passphrase entry took a median 7 s vs 3 s for passwords. The authors recommend lists where "every word ... had an edit distance of at least three" so typos can be corrected (A).
- Discord: "nearly half of all friend requests failed" because of forgotten discriminators and capitalization (B). Short codes spoken aloud ("Phibi Eight Nine Three Six") went to the wrong account.
- Magic Wormhole uses a *phonetically distinct* list (PGP words) with tab completion (A). The PGP list alternates two lists by syllable count so that swapped, duplicated and missing words are detected (A: Wikipedia, PGP word list).
- BIP39 guarantees a unique 4-letter prefix and avoids similar words (A: bips wordlists doc).

**Silent failure.** A wrong valid word derives a different Argon2id key. The app then finds no invite after Argon2 plus relay timeouts, and can't say which word is wrong, or even whether the code was mistyped or has expired (relays drop expired events). E7 reached the same conclusion.

**Correction to DESIGN §5.** "A smaller 2,048-word list would give only years to decades, so the large list is required" compares *6 words* from each list. What matters is total bits: **7 BIP39 words = 77.0 bits ≈ 6 EFF-large words (77.5 bits)**. The large list isn't *required*. It's one option among several (table below).

| Option | Bits | Chars typed in full | Keystrokes with autocomplete | Syllables read aloud | Known list defects |
|---|---|---|---|---|---|
| 6 × EFF large (design) | 77.5 | ~47 | ~32 (prefix up to 9) | ~13.4 | hyphens, 13+ homophones, rare words, 2,364 edit-1 pairs |
| 8 × EFF short 2.0 (E7 proposal) | 82.7 | ~66 | ~24 (3-letter) | ~19.8 | `yo-yo` hyphen; no homophones found; long words (`wobbliness`, `ergonomic`) |
| 7 × BIP39 English | 77.0 | ~44 | ~26 (≤4-letter) | ~11.9 | 5 homophone pairs (`pair/pear`, `peace/piece`, `right/write`, `wear/where`, `when/win`); 806 edit-1 pairs; looks like a crypto seed phrase |

**Fixes, whichever list is chosen:**
- **Clean the list:** remove hyphenated words. Remove one word of each homophone or spelling-variant pair and accept the removed spelling as an **alias** for the kept one, so each spoken form maps to one index. Accept UK spellings as aliases (`grey`→`gray`, `theatre`→`theater`). Replace awkward words.
- **Entry UI:**
  - one chip per word, with suggestions after 2–3 letters;
  - accept any separator (space, hyphen, comma, newline), any case, and pasted full codes;
  - reject non-list words instantly with the nearest suggestions;
  - use `TYPE_TEXT_FLAG_NO_SUGGESTIONS` and `IME_FLAG_NO_PERSONALIZED_LEARNING` so the keyboard neither autocorrects nor *learns* the code;
  - set `EditorInfo.hintLocales` to the list's language so Hebrew, Arabic and Cyrillic users get a Latin layout.
- **Readout UI:** show codes as 2 lines of 3–4 words with position numbers (reduces order errors).
- **After decryption, confirm before committing:** "Join *Groceries*, shared by Uri" already does this. Keep it.
- **Failure copy:** "We couldn't find this invite. Check the words with Uri, or ask for a new code. Codes work for 1 day."
- **Choose the list by experiment** (E-UX1). If reading aloud matters as much as typing, E7's 8 short words cost about 50% more syllables than today.

### 6. Clipboard handoff through install (partial; medium)

**Platform facts:**
- On Android 10+, an app can read the clipboard only if it's the IME or "the app that currently has focus" (A: Android 10 privacy changes). Reading in `onCreate` or `onResume` returns empty. The read must happen after window focus.
- On Android 12+, the first read of another app's clip shows "*APP* pasted from your clipboard". `getPrimaryClipDescription()` shows **no** toast, and on 12+ can classify text such as URLs with a confidence score (A: Android 12 behavior changes).
- The primary clip is auto-cleared after one hour: `DeviceConfig.getBoolean(NAMESPACE_CLIPBOARD, PROPERTY_AUTO_CLEAR_ENABLED, true)`, `DEFAULT_CLIPBOARD_TIMEOUT_MILLIS = 3600000` (A: AOSP `ClipboardService.java`). A user who taps "Get the app" and installs later on home Wi-Fi loses the invite.
- Android 13+ shows a system preview of copied content. Our own app can hide it with `EXTRA_IS_SENSITIVE`, but a web page can't (A: copy-paste docs). The invite secret is then shown on screen and lands in keyboard clipboard histories (overlaps A8).
- Firebase Dynamic Links shut down on 2025-08-25. Google points to paid vendors for deferred deep links (A: Firebase FAQ). There's no free, secret-safe deferred link service.

**Fixes:**
- On first launch, after focus, call `getPrimaryClipDescription()`. Call `getPrimaryClip()` only if the clip looks like our URL, so organic installs never see the toast.
- Use the **Play Install Referrer only for a secret-free flag** (`from_invite=1`). When the clipboard is empty and the flag is set, show "Open your invite link again to join" with a "Paste invite" button. That paste is user-initiated.
- Landing page copy: "1. Install · 2. Come back here and tap **Open in app**". Signal handles group links for non-installed users the same way.
- Measure success rates across OEMs, browsers and delays (E-UX3).

### 7. First launch asks nothing; name at first share/join; permissions in context (confirmed)

- Google: request notification permission "in the correct context ... users are more likely to grant" it (A: Android notification-permission docs).
- Nostr design guidance keeps technical setup out of onboarding (B).
- Setup that users don't understand gets done badly: in the Signal PIN study (235 users), 44% of "casual users" couldn't explain what the PIN was for (A: SOUPS 2021).
- Industry data on sign-up walls says deferring registration cuts drop-off (C, weak, marketing blogs).

**Refinement.** On the join screen, put the name field inline ("Your name in this list: [____] **Join**") so joining stays one screen.

### 8. Display names without usernames (partial; medium)

**Evidence:**
- Discord removed four-digit discriminators because ">40% ... either don't remember your discriminator or don't even know what a discriminator is" and "nearly half of all friend requests failed" (B: discord.com/blog/usernames).
- Matrix disambiguates duplicate names by showing the full user ID and still has open grammar and normalization problems (B: matrix-spec #177).
- Signal added private, per-user **nicknames and notes** in 2024, "useful for adding missing letters in a lazy friend's profile name" (B).
- WhatsApp's July 2026 usernames immediately raised impersonation concerns (B: TechCrunch).
- Petname systems are the known answer to Zooko's triangle (A: Stiegler).

**Confusion cases specific to this design:**
- **(a) Same person, second unlinked device.** Someone opens the invite on their tablet instead of linking it, and becomes a *second member with the same name*. This is the most likely duplicate, and is identity fragmentation like Microsoft To Do's "different accounts or aliases".
- **(b)** Two different people with the same name ("Mom", "Alex").
- **(c)** Renaming rewrites history ("Changing it updates everywhere"), so past edits suddenly show a new name.
- **(d)** A member renames themselves to another member's name. The avatar hue helps, but hues can be close and colorblind users can't use them.
- **(e)** Hex tags ("4F2A") mean nothing to people.

**Fixes:**
- **Local nicknames** for any member, kept in the personal group so they sync across the user's own devices, and used everywhere once set.
- When a join brings a name equal to an existing member's (ignoring case and accents): on the joining device, "Someone named Alex is already in this list. Is that you on another device? → Link this device instead". Other members see "Alex (2)" until someone sets a nickname.
- Show "invited by Uri · joined 3 Sep" in member details.
- Show renames as history events ("Alex is now 'Dad'").
- Keep the tag only in member details (for support or verification), not in everyday UI.

### 9. Device-link approval (refuted; critical, security-UX; for R7/A8 to confirm)

**The design (§5):** "Linking requires confirmation on an existing device ('Link Pixel 8? Only approve if it's yours'), and the link QR expires after 5 minutes." Also "Linking a device that already has its own data merges it".

**Attack:**
1. An attacker sends a link QR or URL styled as a list invite ("scan to join our family list").
2. The victim's phone, which already holds lists, scans it and becomes the *new device* of the attacker's personal group.
3. The approval prompt appears on the *attacker's* existing device, and the attacker approves.
4. The victim's personal group is merged in, so the attacker gets every list secret and every future list.
5. A fresh-install victim is captured too: all their future lists flow to the attacker.

The 5-minute expiry doesn't help, because the attacker makes a fresh QR during a live chat. This is exactly what Google's Threat Intelligence Group reported on 2025-02-19: malicious Signal "linked devices" QR codes that "masquerade as group invites, security alerts, or legitimate device pairing instructions", used by Russia-aligned actors. Signal then shipped "hardened features" (A: cloud.google.com GTIG blog). The UX principle "Scanning or opening any link states what will happen" is necessary but not enough: Signal's flow also said "link device".

**Fixes:**
- **(1) Proximity:** linking completes only over a direct local path (same LAN via mDNS; later Bluetooth/Nearby). Linking over the internet needs a second, explicit warning.
- **(2) Warning on the scanning device:** "This makes this phone part of **Dana's** account. Dana's devices will see all **5** lists on this phone, and every list you create later. Only continue if Dana is you." Show the other account's avatar and device names, and require a slide to confirm.
- **(3) Separate entry points:** reject link payloads opened from chats or the join scanner ("This code links devices. To link your own phone, go to Your devices → Link"). Link payloads can't be mistaken for invites by accident.
- **(4) Alerts:** show "New device linked: *Pixel 8* · Remove" on all devices for a day after any link.
- **(5) Never merge a non-empty device silently:** show what will merge.

### 10. "Whether you're up to date" (refuted in wording; medium-high)

A P2P device can't know it's up to date, because another member may hold changes not yet published. Anytype's "Synced" indicator was trusted and was wrong (#2). In Microsoft To Do, notifications arrived while the list stayed stale (B). NN/g asks for feedback that is *accurate* and timely (A). Users already understand messaging states (sent, delivered, read).

**Fixes:**
- For your own changes: **Saved** (local) → **Sent** (reached a relay or peer) → **On Alex's phone**. The last state is derived when Alex's device publishes its Loro version vector (frontier) in its slot. This costs one small replaceable write when a device merges remote changes.
- For others: "Alex · checked in 3 min ago".
- Never display "up to date". Use "Updated 14:05" or nothing.

### 11. "No technical words, ever" (partial; low)

Mainstream apps use "sync" freely. Microsoft To Do's own support flow is "Settings > Sync Status > Sync Now" (B). The harmful jargon in P2P apps is transport detail: relay lists (Nostr), network-status grades (Keet), "P2P" badges. **Keep the ban on transport words** (relay, peer, key, P2P, CRDT). **Allow everyday words** (sync, offline, updated), which users search for when something breaks. Add a plain-language "Why isn't this updating?" screen, e.g., "Alex's phone hasn't checked in since yesterday. It may be off or saving battery."

### 12. Undo instead of confirmation (partial; medium)

NN/g supports undo over habitual "Are you sure?" dialogs, but notes that some errors remain (A). In a shared list, the Undo snackbar appears only for the person who acted. Everyone else sees items vanish, and with no roles (DESIGN §5), any member can delete everything. **Fixes:**
- Shared **Recently deleted** (30 days, "deleted by Alex, 14:05"), restorable by anyone.
- After a bulk change by someone else, a one-time banner on other devices: "Alex removed 12 items · Undo".
- On a shared list, "Delete list" means **Leave** by default; "Delete for everyone" is a separate, restorable action.

### 13. Widget "synced X ago" (refuted as specified; low-medium)

A widget is redrawn only on events, by `updatePeriodMillis` ("up to once every 30 minutes"), or by WorkManager (every 15 min or more) (A: Glance docs). A relative label drawn at 14:05 still says "just now" at 16:00. **Fix:** use an absolute time ("Updated 14:05"). Omit it when recent, and mark stale only at redraw time.

### 14. Seedless recovery (confirmed direction; partial coverage; medium)

- Phrase- and PIN-based recovery fails ordinary users: Anytype, Keet and Nostr account loss (B); Signal PIN confusion (A); crypto-wallet UX problems across 6,859 reviews (A: CHI 2021).
- Block Store fits the design:
  - 4 KB per entry, 16 entries (A: Block Store docs).
  - It is E2E-encrypted only with a screen lock on Android 9+.
  - Cloud restore needs the **same Google account**, and on non-Pixel devices an **Android 12+** target. Device-to-device restore needs an Android 9+ target.
  - Data survives reinstall only if backup is enabled.
- **Gaps:** users who set up a new phone without restoring, phones without a screen lock, and phones switched to a different Google account.
- **Fixes:** show "Backed up to your Google account" / "Not backed up: add a device or connect Drive" under *Your devices*. Extend the "prompted once" backup rule to phones where `isEndToEndEncryptionAvailable()` is false.

### 15. Five-person playtests (partial; medium)

Nielsen's 5-user rule applies to qualitative testing of *one* user group. Distinct groups need 3–4 users each, and metrics need about 20 (A: NN/g). Sync, latency and OEM problems appear only across days and devices. **Add:**
- groups: older family members, non-native English speakers, low-end Samsung/Xiaomi owners;
- a 2-week diary study with couples (E-UX5);
- a quantitative code-entry study (E-UX1).

### 16. Invite links from chat apps (unverified; medium)

Instagram, TikTok, Facebook, LinkedIn and most email apps open links in an embedded in-app browser (C: u2l.ai). Instagram had a deep-link bug in May 2026 (C: PiunikaWeb). Whether a verified App Link on a GitHub Pages domain opens the app from WhatsApp, Telegram, Signal, SMS, Gmail and Instagram DM hasn't been checked (E-UX4). Also, App Links need `/.well-known/assetlinks.json` at the host root, and Jekyll skips dot-folders unless configured (for R8). **Fix:** the landing page offers an explicit "Open in app" link targeting our package, plus the hint "Opened inside another app? Tap ⋮ → Open in browser".

### 17. Splash, then live merges (partial; low-medium)

A sync round usually lands *after* the 1.5 s splash cap. Items then shift while the user taps, and "unexpected layout shifts ... [make users] click the wrong link or button" (A: web.dev CLS). In a checklist that means checking the wrong item. **Fix:**
- never move the row under an active touch, and delay reorders until 500 ms after the last input;
- animate inserts;
- mark "new from Alex".

### 18. Windows Firewall prompt (unverified; contradicted by one report; low)

Delta Chat desktop #3218 (2023) reports that iroh's QUIC (UDP) listener **did not** trigger the Windows Firewall prompt; users had to add a rule by hand. A July 2026 Delta Chat thread traced timeouts to Linux UFW blocking incoming UDP (B). Sync still works over relays, so the effect is "slower", not "broken", in line with principle 5. **Fix:** the MSI installer (already elevated) adds an inbound rule for the app. Don't show an "about to see a firewall prompt" screen unless testing shows the prompt appears (E-UX9). On Linux, fall back to relays silently.

---

## New risks (not in the design)

| # | Risk | Severity | Fix |
|---|---|---|---|
| N1 | **Same person, two memberships.** A tablet joins through the invite instead of linking, creating two "Uri"s, split history, and "who changed what" confusion | medium | Same-name detection with "Link this device instead" (#8); in the join flow, ask "Already use Kleiders on another device?" |
| N2 | **Duplicate items from concurrent adds.** Two people add "milk" offline, and the CRDT correctly keeps both. Reviewers of other apps call this a sync bug ("Collaboration generates multiple copies of some items") | medium | For open items with the same text, show "Milk ×2 · merge"; auto-merge only on explicit user action |
| N3 | **No-roles destructive actions:** any member can clear or delete a shared list for all | medium | #12 (shared trash, Leave by default, restore banner) |
| N4 | **Notification/data mismatch** (To Do, Keet): a notification shows but the list doesn't | medium | Generate notifications only from merged local state; the tap opens the exact item |
| N5 | **Stale notifications:** "Alex added eggs" arrives after the shopping trip | medium | Batch, show age, drop if checked or old (#3) |
| N6 | **English-only codes for non-English users** (Hebrew/Arabic/Cyrillic keyboards, spelling, pronunciation). Mixing future language lists risks cross-list word collisions | medium | `hintLocales`, aliases, all words from one list per code, localized lists later with a collision check |
| N7 | **Keyboard learning and clipboard exposure** of codes and links: Gboard personalization, clipboard history, Android 13 copy preview, cloud clipboard sync | medium (with A8) | `IME_FLAG_NO_PERSONALIZED_LEARNING`, `NO_SUGGESTIONS`, `EXTRA_IS_SENSITIVE` on in-app copies; the web page can't, so prefer "Open in app" over copy |
| N8 | **Awkward words read aloud** to family (`jockstrap`, `lustfully`, `feminize` are in EFF large) | low | Curate the list (#5) |
| N9 | **Maintainer and support burden kills free P2P apps:** Briar (unfunded, maintenance mode), Syncthing-Android (no contributors plus Play), Manyverse (creator left). C2 covers infrastructure cost, not labor | high (strategic) | Self-serve diagnosis in plain words, small scope, open the engine to attract contributors (§14), budget maintainer time explicitly |
| N10 | **Play review friction for background/P2P apps** (Syncthing-Android's publishing struggles) | medium | No special-access permissions; document every foreground-service use; keep the `foss` flavor on F-Droid as a fallback |
| N11 | **Rarely opening members fall into rare/restricted buckets:** senders think "Alex ignores the list" | medium | Per-member freshness; "Tell Alex" nudge (#3) |
| N12 | **Layout shift during merges** leads to wrong items checked | low-medium | #17 |
| N13 | **Linking two devices that each have a "Groceries" list** yields duplicate lists after the merge | low-medium | After the merge, offer "You have 2 lists named Groceries · Combine?" |

---

## Suggested experiments

| ID | Question | Method | Decides |
|---|---|---|---|
| E-UX1 | Which code format is fastest and least error-prone to type *and* read aloud? | 20+ participants (≥1/3 non-native English, including RTL-keyboard users). Phones with default Gboard and Samsung Keyboard. Conditions: 6×EFF-large (as is), 6×EFF-large-cleaned with chips, 8×EFF-short-2.0 with chips, 7×BIP39 with chips. Tasks: type from another screen; receive by phone call. Measure time, first-try success, autocorrect substitutions, mishearings | List choice (#5), with E7's security numbers |
| E-UX2 | How clean can a 7,776+ list be? | Script (Appendix A method): remove hyphens, homophones (CMUdict), rare words (frequency list), edit-1 pairs; report remaining size, prefix uniqueness, syllables | Whether "EFF-large-cleaned" keeps ≥12.9 bits/word |
| E-UX3 | Does the clipboard handoff work? | Android 10–16 on Pixel/Samsung/Xiaomi. Tap "Get the app" in Chrome, Samsung Internet, and WhatsApp/Instagram/Telegram in-app browsers. Install at once vs after 65 min. Record pickup rate and toasts | #6 |
| E-UX4 | Do invite links open the app from chat apps? | Verified App Link on a GitHub Pages domain; tap from WhatsApp, Telegram, Signal, SMS, Gmail, Instagram DM, Messenger | #16 |
| E-UX5 | What delay do real couples tolerate for a shared shopping list? | 2-week diary study, 8–10 households, mixed OEMs. Prototype logs locally the edit→visible latency (app open/closed) and notification latency. Weekly interviews. A/B: with vs without "Tell Alex" and per-member freshness | #3, #4; whether "Stay connected" or keep-screen-on is needed |
| E-UX6 | How stale do widgets get? | Pixel + Samsung + Xiaomi, widget on the home screen, 48 h idle and normal use; log redraw times | #13 and DESIGN §15's Glance/standby question |
| E-UX7 | How do people add a second device? | Moderated test: "you also have a tablet; use the list there too". Count link vs invite-join; test same-name detection copy | N1, #8 |
| E-UX8 | Does the link warning stop phishing? | Moderated test: a "join our list" message carrying a *link* QR; measure how many complete it with and without the fixes in #9 | #9 |
| E-UX9 | Does Windows show a firewall prompt for iroh's UDP socket? What happens on Cancel? | Fresh Windows 11 VM, prototype engine; then with an MSI-installed rule | #18 |
| E-UX10 | Do people correctly identify "who changed what" with duplicate names? | Show history with (a) "Alex · 4F2A", (b) "Alex (2)", (c) a local nickname, (d) "Alex, invited by Uri"; measure correct identification | #8 |

---

## Sources

**Mainstream task and list apps**
- Google Tasks, shared tasks: https://support.google.com/tasks/answer/11549608?hl=en
- Todoist pricing: https://www.todoist.com/pricing ; free-plan history (third party): https://www.usecarly.com/blog/todoist-free-plan-limits/
- Todoist, collaborating with family: https://www.todoist.com/help/articles/collaborate-with-friends-or-family-in-todoist-tzkGUy
- Any.do review (May 2026): https://www.cloudwards.net/any-do-review/
- TickTick App Store listing and reviews: https://apps.apple.com/app/tick-tick/id626144601
- Microsoft To Do shared-list sync, Sep 2025: https://techcommunity.microsoft.com/discussions/to-do/microsoft-to-do-shared-list-%E2%80%93-notifications-arrive-but-the-list-on-my-account-do/4451581
- Microsoft To Do shared-list sync, Aug 2025: https://learn.microsoft.com/en-us/answers/questions/5536244/i-m-having-syncing-problems-with-a-shared-microsof
- Microsoft To Do shared-list sync, Apr 2024: https://learn.microsoft.com/en-us/answers/questions/5340706/microsoft-to-do-is-not-syncing-across-users-of-sha
- Microsoft To Do widget (One UI): https://techcommunity.microsoft.com/discussions/to-doinsiders_android/andorid-widget-not-available/4210290
- Apple Reminders one-way sync (14 "me too"): https://discussions.apple.com/thread/256069453
- OurGroceries user guide: https://www.ourgroceries.com/user-guide ; Play listing: https://play.google.com/store/apps/details?id=com.headcode.ourgroceries&hl=en_US
- AnyList: https://www.anylist.com/ ; Bring!: https://play.google.com/store/apps/details?id=ch.publisheria.bring&hl=en_US
- Grocery app reviews: https://apps.apple.com/us/app/grocery-smart-shopping-list/id1195676848?see-all=reviews ; https://play.google.com/store/apps/details?id=com.fivefly.android.shoppinglist
- Google Keep sync delay note: https://github.com/watkins-matt/home-assistant-google-keep-sync

**P2P / local-first apps**
- Briar maintenance mode (2026-07-09): https://briarproject.org/news/2026-maintenance-mode/
- Syncthing-Android discontinued: https://forum.syncthing.net/t/discontinuing-syncthing-android/23002
- Syncthing-Fork battery: https://forum.syncthing.net/t/syncthing-fork-on-android-uses-lots-of-battery/23645
- Keet support: https://support.keet.io/technical-support-and-troubleshooting/other-issues/ ; https://support.keet.io/technical-support-and-troubleshooting/profile-recovery
- Keet review (2025-06-26): https://stacker.news/items/1015915
- Anytype: https://github.com/anyproto/anytype-heart/issues/933 ; https://community.anytype.io/t/syncing-barely-works/5511 ; https://community.anytype.io/t/forgot-my-recovery-phrase-unable-to-login/9400
- Nostr design guide, relays: https://nostrdesign.org/docs/how-to/relays/
- Delta Chat: https://support.delta.chat/t/difficulties-when-using-multiple-devices/2375 ; https://support.delta.chat/t/adding-a-second-device-problems-disable-firewall-on-desktop/5530 ; https://support.delta.chat/t/second-device-error-get-backup-failed-timed-out/4990 ; https://github.com/deltachat/deltachat-desktop/issues/3218
- Manyverse: https://opencollective.com/manyverse
- SimpleX: https://github.com/simplex-chat/simplex-chat/issues/3863 ; https://privacytools.io/app/simplex-chat

**Word codes**
- EFF wordlists: https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases ; https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt ; https://www.eff.org/files/2016/09/08/eff_short_wordlist_2_0.txt
- BIP39: https://github.com/bitcoin/bips/blob/master/bip-0039/bip-0039-wordlists.md ; https://raw.githubusercontent.com/bitcoin/bips/master/bip-0039/english.txt
- CMU Pronouncing Dictionary: https://github.com/cmusphinx/cmudict
- Magic Wormhole: https://magic-wormhole.readthedocs.io/en/latest/welcome.html
- PGP word list: https://en.wikipedia.org/wiki/PGP_word_list
- Shay et al., SOUPS 2012: https://cups.cs.cmu.edu/soups/2012/proceedings/a7_Shay.pdf
- Dechand et al., USENIX Security 2016: https://www.usenix.org/conference/usenixsecurity16/technical-sessions/presentation/dechand
- Orchard Street wordlists: https://github.com/sts10/orchard-street-wordlists
- Incognito keyboard flag discussion: https://github.com/flutter/flutter/issues/73046
- E7 (this project): `docs/research/E7-invite-codes.md`

**Onboarding, clipboard, links, linking**
- Android 12 behavior changes (clipboard toast): https://developer.android.com/about/versions/12/behavior-changes-all
- Android 10 privacy changes (clipboard focus): https://developer.android.com/about/versions/10/privacy/changes
- Copy and paste (sensitive flag, Android 13 preview): https://developer.android.com/develop/ui/views/touch-and-input/copy-paste
- AOSP ClipboardService (auto-clear default): https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/services/core/java/com/android/server/clipboard/ClipboardService.java
- 9to5Google, Android 13 clipboard: https://9to5google.com/2022/08/16/android-13-clipboard/
- Firebase Dynamic Links FAQ: https://firebase.google.com/support/dynamic-links-faq
- In-app browsers: https://u2l.ai/blog/whatsapp-link-opens-in-browser ; https://piunikaweb.com/2026/05/21/instagram-links-open-home-screen-bug/
- Notification permission: https://developer.android.com/develop/ui/compose/notifications/notification-permission
- GTIG, Signal linked-device phishing (2025-02-19): https://cloud.google.com/blog/topics/threat-intelligence/russia-targeting-signal-messenger/ ; https://thehackernews.com/2025/02/hackers-exploit-signals-linked-devices.html

**Names and identity**
- Discord usernames: https://discord.com/blog/usernames
- Matrix display names: https://github.com/matrix-org/matrix-spec/issues/177
- Signal nicknames: https://support.signal.org/hc/en-us/articles/360007319011-Manage-Contacts-Nicknames-and-Notes ; https://x.com/signalapp/status/1777702983406416000
- WhatsApp usernames (2026-07-01): https://techcrunch.com/2026/07/01/whatsapp-usernames-are-already-raising-impersonation-red-flags/
- Petnames: https://www.skyhunter.com/marcs/petnames/IntroPetNames.html

**Platform and background**
- Power management limits: https://developer.android.com/topic/performance/power/power-details
- Glance widget updates: https://developer.android.com/develop/ui/compose/glance/glance-app-widget ; https://appsmfk.com/blog/jetpack-glance-widget-android-2026/
- Don't kill my app: https://dontkillmyapp.com/samsung
- Block Store: https://developer.android.com/identity/block-store
- UnifiedPush distributors: https://unifiedpush.org/users/distributors/

**Usability research**
- NN/g: https://www.nngroup.com/articles/visibility-system-status/ ; https://www.nngroup.com/articles/confirmation-dialog/ ; https://www.nngroup.com/articles/why-you-only-need-to-test-with-5-users/
- web.dev CLS: https://web.dev/articles/cls
- Signal PINs (SOUPS 2021): https://arxiv.org/abs/2106.09006
- Crypto wallet UX (CHI 2021): https://dl.acm.org/doi/abs/10.1145/3411764.3445407
- Trustless key management (CHI 2024): https://doi.org/10.1145/3613904.3642464

---

## Appendix A: word-list measurements (local analysis, 2026-09-24)

**Method.** The EFF large, EFF short 2.0 and BIP39 English lists and CMUdict were downloaded from their public URLs into the session scratchpad. Python computed:
- word lengths;
- the prefix length needed to identify each word (longest common prefix with sorted neighbors + 1);
- pairs at Levenshtein distance 1;
- exact homophones (identical CMUdict phonemes, stress removed);
- syllables (vowel phonemes, minimum over variants).

Nothing was added to the project repo.

| Metric | EFF large | EFF short 2.0 | BIP39 English |
|---|---|---|---|
| Words | 7,776 | 1,296 | 2,048 |
| Mean / max length | 6.99 / 9 | 7.32 / 10 | 5.40 / 8 |
| Hyphenated words | `drop-down`, `felt-tip`, `t-shirt`, `yo-yo` | `yo-yo` | none |
| Chars needed to identify a word (mean / max) | 5.29 / 9 | 3 / 3 (by design) | 3.65 / 4 |
| Words needing ≥6 chars to identify | 3,165 (40.7%) | 0 | 0 |
| Pairs at edit distance 1 | 2,364 | 0 (by design) | 806 |
| Exact homophone groups (CMUdict) | 18 (incl. `cache/cash`, `capital/capitol`, `carat/carrot`, `cannon/canon`, `shone/shown`, `flier/flyer`; a few are dialect artifacts such as `capped/kept`) | 0 | 5 (`pair/pear`, `peace/piece`, `right/write`, `wear/where`, `when/win`) |
| Words not in CMUdict (~126k distinct words) | 985 (12.7%) | 145 (11.2%) | 1 |
| Mean syllables per word | 2.24 | 2.48 | 1.70 |
| US-only spellings present | `gray`, `theater`, `catalog`, `defense`, `mold`, `plow`, `pajamas`, `aluminum` | – | – |
| Code length for ~77–83 bits | 6 words ≈ 47 chars, ≈ 13.4 syllables | 8 words ≈ 65.5 chars, ≈ 19.8 syllables | 7 words ≈ 43.8 chars, ≈ 11.9 syllables |
