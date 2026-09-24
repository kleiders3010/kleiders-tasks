# R4: Android platform behavior (research report)

*Dimension R4 of the Kleiders Tasks design review. Researched 2026-09-24 against DESIGN.md §5, §7, §7a, §8, §9–§12 and UX-PRINCIPLES.md. Evidence comes from official Android docs, the Play policy center, and AOSP `main` source read directly from android.googlesource.com (fetched 2026-09-24). Where AOSP code is cited, the behavior is the platform default; OEMs and Google's server-side DeviceConfig can change the constants.*

---

## Summary

- **The widget claim holds in AOSP but it doesn't do what the design says.** `AppStandbyController.getAppMinBucket()` on `main` still returns `STANDBY_BUCKET_ACTIVE` for `isBoundWidgetPackage()`, and the docs list "apps with active widgets" as exempt from the restricted bucket. But the ACTIVE bucket does **not** exempt the app from Doze. In deep Doze, jobs run only in maintenance windows 1 h → 2 h → 4 h → 6 h apart. Since Android 16, ACTIVE also has a job quota (20 min per rolling 60 min). "Periodic sync doesn't slow down" is wrong overnight and on idle phones.
- **Android 15+ cuts background network for every app** (not only apps targeting 15), about 5 s after the app leaves the foreground (2 s, or 20 s for the "last activity" state, under a newer flag), unless a job or FGS is running. On Android 14+ the **cached-app freezer** freezes the process 10 s after it becomes cached and **terminates its TCP sockets**. DESIGN.md doesn't mention either one. Both break any "keep the endpoint alive after leaving" behavior and the phone-helper idea.
- **The `shortService` flush at onStop works**, through a 5-second "activity visibility grace period" in `ActiveServices`. The 10 s notification deferral exists, but it is **rate-limited**: within about 2 min after one deferred FGS notification, the next FGS notification shows **immediately**. The app also shows in the FGS Task Manager. "Usually invisible" is only true for infrequent app switches. An **expedited WorkManager job** does the same job with no notification, no Play FGS declaration, a bypass of Doze network restrictions, and a guaranteed start for the foreground app on Android 13+. It should be the primary mechanism.
- **Phone helpers on "back-to-back jobs while charging" are fragile.** It's true that quota is free while charging and each job may run **30 min**, not 10. But JobScheduler's "charging" isn't the same as "plugged in": it means level ≥ 90 %, or rising, with a 15-min debounce. Charge limiters at 80–85 % can report "discharging". Between jobs the process is firewalled within seconds and frozen within 10 s, and job batching can add delays of up to 31 min. The process is reachable only while a job is actually running.
- **Android 17 `ACCESS_LOCAL_NETWORK` is broader than the design assumes.** It covers TCP and UDP both ways, including *incoming*, multicast and broadcast, mDNS and `.local` resolution, IPv4 private, link-local and **CGNAT (100.64/10)**, plus IPv6 link-local **and directly connected (same-LAN global) prefixes**. So same-Wi-Fi "direct IPv6" is also blocked without it. Without the permission, UDP fails with `EPERM` and TCP times out. **Requesting `NEARBY_WIFI_DEVICES` on SDK 36 is unnecessary**: on 36 local-network protection is off unless a developer enables the compat flag.
- **`SCHEDULE_EXACT_ALARM` works as described**, and a to-do app is **not** eligible for `USE_EXACT_ALARM` (alarm, timer or calendar apps only). Without the permission, reminders degrade to inexact alarms that can be **up to 1 h late** (`maxTriggerTime` caps the window at 1 h; `setWindow` has a 10-min minimum). `setAlarmClock()` also needs the permission.
- **Block Store limits confirmed:** 4 KB per entry, 16 entries, E2E on Android 9+ with a screen lock. But cloud data comes back **only through the new-device restore flow** (cloud restore or device-to-device), or after reinstall on the *same* device with Google Backup on. A user who skips restore during setup gets nothing. The design's main recovery path for shared lists has a silent failure mode.
- **Auto Backup confirmed:** 25 MB (over quota means *no* backup at all), E2E on Android 9+ with a lock screen, at least 24 h apart, and only when idle, on Wi-Fi and with backup enabled. It skips apps in the foreground and kills the app to back it up. The data is restored before first launch. **New critical risk:** restore and D2D clone the per-device identity (Nostr slot keys, iroh key, Loro PeerID) onto a second live device unless those files are excluded.
- **The clipboard deferred invite is workable but leaky.** It must be read after window focus (Android 10+), triggers a "pasted" toast (12+), and the clipboard auto-clears after 1 h (13+ default). The invite puts the **permanent group secret** into keyboard clipboard histories, cross-device clipboard sync and the Android 13 preview.
- **Minor corrections:** Glance *backports* checkboxes below Android 12, so the "tap opens the app" fallback isn't needed. Glance renders every widget update inside a **WorkManager session (45 s)**, which consumes job quota and is deferred by Doze. The Google code scanner needs no camera permission (confirmed). In-App Updates works only for Play-installed copies owned by the account, and update priority can be set **only via the Publishing API, once, at release**.

---

## Claim-by-claim table

| # | Claim (from DESIGN.md / task) | Verdict | Severity if wrong | One-line evidence |
|---|---|---|---|---|
| C1 | WorkManager periodic minimum 15 min; background sync "usually within 15 minutes" under Doze / buckets / OEMs | **Partial** | Medium | 15 min confirmed; deep-Doze windows grow 1→6 h (AOSP `DeviceIdleController`); bucket quotas per docs table; Samsung/Xiaomi/OnePlus/Huawei 5/5 on dontkillmyapp |
| C2 | A bound widget keeps the app in ACTIVE, "so periodic sync doesn't slow down" | **Partial** (mechanism confirmed, conclusion wrong) | Medium | `getAppMinBucket()` returns ACTIVE for `isBoundWidgetPackage()` on AOSP `main`; but Doze still defers; A16 ACTIVE quota 20 min/60 min; user "Restricted" and Samsung sleeping apps are separate systems |
| C3 | `shortService` (3 min) started when leaving the app; notification held back 10 s so "usually invisible" | **Partial** | Medium | 3 min + `onTimeout` confirmed; start allowed within 5 s after the app becomes invisible; deferral 10 s, but rate-limited (2 min exclusion), immediate if the notification has actions; Task Manager always shows it |
| C4 | While charging: no Doze, no bucket limits, ~10 min jobs, chain jobs back-to-back to stay reachable; freezer between jobs; A16/17 changes | **Partial** (several sub-claims wrong) | High (for §7a phone helpers) | Doze off when plugged (confirmed); quota free when "charging" (≥90 % or rising); runtime **30 min** when charging; freezer (10 s) + background firewall (2–5 s) between jobs; batching up to 31 min; A16 quota changes; nothing in A17 |
| C5 | Android 15 dataSync FGS 6 h/24 h; may face Play review | **Confirmed** (and the FGS fallback is likely non-viable) | Medium | 6 h/24 h for target 35+, resets when the app comes to the foreground; every FGS type needs a Play declaration + video; dataSync use cases are user-initiated transfers |
| C6 | `ACCESS_LOCAL_NETWORK` (target 37); `NEARBY_WIFI_DEVICES` on 36; without it, sync goes over the internet | **Partial** | Medium | Scope confirmed and wider (incoming, IPv6 on-link, CGNAT); on SDK 36 no permission is needed (opt-in compat flag only); public internet unaffected |
| C7 | `SCHEDULE_EXACT_ALARM` off by default on 14+; asked when a reminder is set or received | **Confirmed** with refinements | Low | Denied by default for new installs targeting 33+; `USE_EXACT_ALARM` not allowed for task apps; inexact up to 1 h late; `setAlarmClock` needs it; no bucket elevation for target 34+ |
| C8 | Block Store: E2E cloud backup (9+ with lock), restore on a new device, only needs the personal secret | **Partial** | Medium–High | 4 KB × 16 confirmed; E2E needs Android 9+, screen lock (and a Google account); restore only through the setup restore flow or D2D, or same-device reinstall with Backup on; docs don't state the GMS version |
| C9 | Auto Backup: 25 MB, daily, E2E (9+ with lock), restore | **Confirmed** with caveats | Medium (plus a new critical risk) | 25 MB, ≥24 h + idle + Wi-Fi; foreground apps skipped; app killed during backup; restored before first launch, including reinstall; D2D ignores `allowBackup=false` (target 31+) |
| C10 | Clipboard read on first launch as deferred invite | **Partial** | Medium (security / UX) | Focus rule (10+), toast on `getPrimaryClip` (12+), auto-clear 1 h (13+ AOSP default); secret leaks to clipboard ecosystem; toast conflicts with "first launch asks nothing" |
| C11a | Glance checkboxes need Android 12; fallback "tap opens the app" | **Partial** | Low | Glance has a `CheckBoxBackport` layout below S; native compound button on S+ |
| C11b | Google code scanner needs no camera permission | **Confirmed** | Low | ML Kit docs; needs Play services and a scanner module download |
| C11c | Play In-App Updates confirms that a newer version exists | **Confirmed** with caveats | Low | Reports availability, `availableVersionCode`, priority (0–5, set once through the API), staleness; Play-installed + account-owned only |

---

## Details per claim

### C1. WorkManager 15-minute periodic sync under Doze, buckets and OEM killers

**What's confirmed**
- The periodic minimum is 15 min: "The minimum repeat interval that can be defined is 15 minutes (same as the JobScheduler API)." The flex minimum is `MIN_PERIODIC_FLEX_MILLIS` (5 min). [define-work]
- Per-bucket job limits, as of the Android 16 docs table [power-details]:

  | Bucket | Regular jobs | Expedited jobs | Alarms | Network |
  |---|---|---|---|---|
  | Active | 20 min per rolling 60 min (**new in A16**; before A16, unlimited) | 30 min / 24 h | unlimited | unrestricted |
  | Working set | 10 min / 4 h | 15 min / 24 h | 10/h | unrestricted |
  | Frequent | 10 min / 12 h | 10 min / 24 h | 2/h | unrestricted |
  | Rare | 10 min / 24 h | 10 min / 24 h | 1/h | disabled |
  | Restricted | once/day, 10 min | 5 min / 24 h | 1/day | disabled |

- The restricted bucket is entered after **8 days** without use (Android 13+). Apps with active widgets are exempt. [appstandby]
- **Doze:** "Doesn't run JobScheduler jobs (and consequently WorkManager tasks)". Network is suspended outside maintenance windows. [doze-standby]
  - AOSP defaults in `DeviceIdleController`:
    - light idle: 5 min, growing to a 15 min max;
    - deep idle enters after about 30 min inactive + 30 min, then `IDLE_TIMEOUT` = 60 min, doubling (`IDLE_FACTOR` 2) up to `MAX_IDLE_TIMEOUT` = **6 h**.
  - So an unplugged, stationary phone overnight syncs roughly at +1 h, +3 h, +7 h, then every 6 h. A 15-min job has no effect on that.

**What's wrong or incomplete**
- "Usually within 15 minutes, longer if your phone has been idle for a while" is honest, but "longer" can mean **up to about 6 h** on stock Android. On OEM skins it can mean **never**:
  - dontkillmyapp rates Samsung, Xiaomi, OnePlus and Huawei at 5/5.
  - Samsung's own developer page: after "about 3 days" unused, an app goes to "sleeping" with a "bucket restriction" on "Job, Alarm, and Foreground-service". After **16 days** it's "deep sleeping": it "can't perform any activities, including notifications or updates".
  - Samsung's 2023 commitment covers **FGS only** ("foreground services of apps targeting Android 14 will be guaranteed to work as intended"), not jobs. [samsung-dev] [android-dev-blog-2023]
  - Caveat: the dontkillmyapp Samsung page was last updated July 2024, so it's stale for One UI 7/8. Nothing newer and authoritative was found.
- Since Android 16, **jobs started while the app is TOP, or running alongside an FGS, count against quota**. That includes WorkManager and DownloadManager. [a16-behavior-all]

**Fixes**
1. Change the user-facing text to something like "usually within 15 minutes; overnight or when your phone sits unused, it can take a few hours". That keeps the no-technical-words rule.
2. Detect throttling rather than guessing:
   - `UsageStatsManager.getAppStandbyBucket()` (API 28),
   - `ActivityManager.isBackgroundRestricted()` (API 28),
   - `ApplicationExitInfo` (API 30) for OEM kills,
   - `WorkInfo.getStopReason()`, and `JobScheduler.getPendingJobReasonsHistory()` (API 36).

   Show the "blocked background sync" explainer only on hard evidence: RARE or RESTRICTED bucket, background-restricted, or no successful run in more than 24 h while not in Doze.
3. Keep each periodic run short (≤ 30–60 s typical), so that even the 20 min/h ACTIVE quota and the 10 min/4 h WORKING_SET quota are never the limit.

### C2. "An active widget keeps the app in the ACTIVE standby bucket"

**Confirmed in AOSP `main`** (`AppStandbyController.java`, fetched 2026-09-24):

```java
if (mAppWidgetManager != null
        && mInjector.isBoundWidgetPackage(mAppWidgetManager, packageName, userId)) {
    return STANDBY_BUCKET_ACTIVE;
}
```

`getAppMinBucket()` feeds `newBucket = Math.min(newBucket, getAppMinBucket(...))`. So a package with **at least one bound widget instance** can never drop below ACTIVE. The official doc also lists "Apps with active widgets" as exempt from the restricted bucket. [appstandby]

**Why the design's conclusion ("periodic sync doesn't slow down") is only partly true**
- **Doze is independent of buckets.** ACTIVE apps' jobs are still deferred to Doze maintenance windows (see C1).
- **Android 16 quota:** ACTIVE regular jobs get 20 min per rolling 60 min. That's generous, but not unlimited. [power-details]
- **User battery restriction** ("Restricted" in app battery settings) isn't a bucket. Per the docs table it restricts job execution regardless. [power-details]
- **OEM proprietary sleepers** (Samsung sleeping and deep-sleeping apps, Xiaomi autostart, OnePlus deep optimization) run outside AppStandbyController. No evidence was found either way on whether they honor widgets. **Unverified**; this is what DESIGN.md §15 already lists for prototyping.
- The exemption lasts only while a widget is **bound**. Removing the widget or switching launchers drops it.
- A related detail: `shouldGetExactAlarmBucketElevation()` gives WORKING_SET as the minimum to apps holding `USE_EXACT_ALARM`. For `SCHEDULE_EXACT_ALARM` this elevation is disabled for apps targeting 34+ (`SCHEDULE_EXACT_ALARM_DOES_NOT_ELEVATE_BUCKET`, `@EnabledSince(UPSIDE_DOWN_CAKE)`). So exact-alarm permission gives no bucket help to this app.

**Fix:** reword §8: "An active widget keeps the app out of the low-priority standby buckets, so periodic sync runs at full rate whenever the phone isn't in deep idle." Keep the OEM experiment (E1).

### C3. `shortService` flush when leaving the app

**Confirmed**
- Timeout of about 3 min from `startForeground()` (`DEFAULT_SHORT_FGS_TIMEOUT_DURATION = 3 * 60_000`).
- `Service.onTimeout()` (API 34), then a cached state, then an ANR if the service is not stopped.
- It can't start other FGS, isn't sticky, and needs no type-specific permission. [service-types]
- Below Android 14 there is no timeout callback and no ANR. [service-types]
- **Starting at onStop is allowed.**
  - The Android 12+ exemption "Your app transitions from a user-visible state, such as an activity" [restrictions-bg-start] is implemented in `ActiveServices` as `REASON_ACTIVITY_VISIBILITY_GRACE_PERIOD`. It applies when `elapsedRealtime - lastInvisibleTime < mFgToBgFgsGraceDuration`, with `DEFAULT_FG_TO_BG_FGS_GRACE_DURATION = 5 * 1000`.
  - So the FGS must be *started* within about 5 s of the last activity becoming invisible.
  - `ProcessLifecycleOwner`'s ON_STOP (dispatched about 700 ms after the last activity stops) is inside the window. A delayed coroutine or a debounce is not guaranteed to be.
  - Google can change the constant via DeviceConfig.
- **Deferral:** `mFgsNotificationDeferralIntervalForShort` defaults to 10 000 ms. If the service stops before then, the notification is never posted.

**Caveats the design misses**
- **Rate limit.** `mFgsNotificationDeferralExclusionTimeForShort` = 2 min. After one deferred FGS notification, `withinFgsDeferRateLimit()` makes the next FGS notification within (10 s + 2 min) **show immediately**. A user who hops in and out of the app (a common pattern for a shopping list) sees a notification flash.
- **Immediate display** also happens if the notification has action buttons, uses `FOREGROUND_SERVICE_IMMEDIATE`, or has a call, navigation or transport category (`shouldShowForegroundImmediately()`).
- **Task Manager:** on Android 13+ the FGS shows in the "active apps" Task Manager even when notifications are denied. [notification-permission]
- **Play:** "If your app targets Android 14 or higher, you'll need to declare your app's foreground service types in the Play Console". [fgs-types-required] Whether the Play Console form asks for `shortService` (it has no type-specific permission) is **unverified**; assume a declaration and video may be needed.
- **Android 15+ background network cut:** the flush must be in a job or FGS *before* about 5 s after onStop. Otherwise sockets get `UnknownHostException` or `IOException`. [a15-behavior-all] (See risk R1.)

**Better option: an expedited job.** AOSP `JobInfo.setExpedited` javadoc says expedited jobs "Run as soon as possible", are "less restricted during Doze and battery saver", "Bypass Doze, app standby, and battery saver network restrictions (if the job has a connectivity constraint)", and "starting from Android version TIRAMISU, expedited jobs for the foreground app are guaranteed to be started before `schedule()` returns". They get at least 1 min (the AOSP `RUNTIME_MIN_EJ_GUARANTEE_MS` is 3 min), have no notification on 12+ and need no FGS declaration. The ACTIVE EJ quota is 30 min per 24 h; a flush of a few seconds is negligible. On 8–11, WorkManager runs expedited work as an FGS with an **immediate** notification (there's no deferral before 12). [define-work]

**Fix:**
- Make "enqueue an expedited `OneTimeWorkRequest` with `NetworkType.CONNECTED` at `ProcessLifecycleOwner` ON_STOP" the primary path on all versions.
- Use `shortService` only if measurements (E3/E4) show the EJ starts late.
- If `shortService` is kept: start it synchronously in ON_STOP, give the notification no actions, and accept that it's sometimes visible. Change the §8 text from "usually invisible" to "often invisible".

### C4. Charging: Doze, buckets, job runtime, chaining, freezer, A16/17

| Sub-claim | Verdict | Evidence |
|---|---|---|
| Doze doesn't apply while charging | **Confirmed** (plugged in, not "charging") | `DeviceIdleController` uses `EXTRA_PLUGGED != 0`; docs: device exits Doze when "connects charger" |
| Standby-bucket limits lifted while charging | **Confirmed except RESTRICTED** | `QuotaController.isQuotaFreeLocked()`: "Quota constraint is not enforced while charging", `return standbyBucket != RESTRICTED_INDEX`; `AppStandbyController.isAppIdleFiltered()` returns false when `mIsCharging`; docs: "While the device is charging, the system doesn't impose these restrictions" |
| Job execution limit about 10 min | **Refuted (it's longer)** | `getMaxJobExecutionTimeMsLocked()`: when `isBatteryCharging()` it returns `RUNTIME_FREE_QUOTA_MAX_LIMIT_MS` = **30 min** for regular and expedited jobs; 10 min (`RUNTIME_MIN_GUARANTEE_MS`) is the on-battery guarantee |
| Jobs can be chained back-to-back to keep the process reachable | **Mostly refuted** | See the gaps below |
| Freezer freezes the process between jobs | **Confirmed** (A14+) | "App processes in the cached state are frozen 10 seconds after entering the cached state"; "If all processes for a particular app are frozen, the system terminates any active TCP sockets maintained by the app" [freezer] |
| A16/A17 changes to quotas or runtime | **A16: yes; A17: none found** | A16: ACTIVE quota; TOP-started and FGS-concurrent jobs count against quota; `setImportantWhileForeground` ignored; abandoned-job stop reason. The A17 behavior-change pages list no JobScheduler changes (they do add app memory limits). |

**Why chaining doesn't keep a phone reachable**
1. **"Charging" isn't "plugged in."**
   - JobScheduler's `BatteryStateTracker` uses `ACTION_CHARGING`/`ACTION_DISCHARGING` from `BatteryStatsImpl`. When plugged in, it reports charging if the level is ≥ `BATTERY_CHARGING_ENFORCE_LEVEL` (90). Below 90 it reports charging only after the level *rises*, deferred by `BATTERY_CHARGED_DELAY_MS` (15 min), and flips to "discharging" if the level drops while plugged in.
   - AOSP treats a plugged-in device as charging at ≥ 70 % only if the HAL reports an *adaptive* charging policy.
   - So phones holding at 80–85 % with Samsung "Protect battery" or a Pixel charge limit can be "not charging" for hours. Quotas then return, while Doze stays off.
2. **Background network firewall (Android 15+).** `NetworkPolicyManagerService` enables `FIREWALL_CHAIN_BACKGROUND`. A uid loses network when its process state reaches `PROCESS_STATE_TOP_SLEEPING` or worse, after `mBackgroundRestrictionDelayMs` = 5 s (with the newer flag, 2 s, or 20 s for TOP_SLEEPING / LAST_ACTIVITY). A job-bound process sits at SERVICE, which is above that, so it has network only *during* a job.
3. **Freezer:** frozen 10 s after becoming cached, and TCP sockets are killed (Nostr WebSockets, iroh relay connections).
4. **Batching and scheduling gaps.** `JobSchedulerService` can force-batch non-expedited jobs:
   - up to `MAX_NON_ACTIVE_JOB_BATCH_DELAY_MS` / `MAX_CPU_ONLY_JOB_BATCH_DELAY_MS` = 31 min;
   - connectivity jobs up to `CONN_MAX_CONNECTIVITY_JOB_BATCH_DELAY_MS` = 31 min, unless the network is already active;
   - a `batch_active_bucket_jobs` flag extends batching to ACTIVE apps. Whether it's on in production builds is **unverified**.
5. **Concurrency:** overnight, many system and app jobs compete for 8–40 slots (`DEFAULT_CONCURRENCY_LIMIT`, depending on RAM).

**Fix for §7a:**
- Plan phone helpers as **"help during own job windows only"**: each 15-min periodic run on charger and Wi-Fi may stay up to about 25 min *while quota-free*.
- Report reachability probabilistically, as the design's reliability records already allow.
- Drop the promise that a charging phone runs "as a chain of back-to-back jobs".
- The FGS fallback is effectively closed (see C5), so desktops and always-on members are the real helpers.
- Update §15 to measure *duty cycle* (fraction of the night a phone is actually reachable) and not only "how often the process is killed" (E7).

### C5. dataSync FGS 6 h / 24 h and Play review

**Confirmed:**
- "The system permits `dataSync` and `mediaProcessing` foreground services to run for a total of 6 hours in a 24-hour period."
- The timer resets "if the user brings the app to the foreground".
- `onTimeout(int, int)` is called, and if the service isn't stopped: `RemoteServiceException: "A foreground service of type ... did not stop within its timeout"`.
- Applies to apps targeting 15+. [fgs-timeout]
- Apps targeting 15+ can't start dataSync from `BOOT_COMPLETED`. [service-types]

**Play:**
- Every FGS type needs a Play Console declaration with "a link to a video demonstrating each foreground service feature".
- FGS use must be "initiated by the user or ... user perceptible".
- The dataSync rows read "Network transfer: Upload or download", "Backup and restore: use for specifically user-initiated features", and "Local processing: Other: use for specifically user-initiated work and not for regular system or server-initiated tasks". [play-fgs] [play-fgs-preview]
- Relaying *other people's* data as a helper is not an upload or download the user initiated. The only other fit is `specialUse`, which is reviewed case by case.

**Verdict:** the claim is right, and the FGS helper fallback is most likely not approvable, or at least high-risk. **Fix:** remove the FGS fallback from §7a, or reserve it for an explicit user-started "Help now" mode with a visible notification, and even then expect a Play review.

### C6. Local network permission (Android 17) and `NEARBY_WIFI_DEVICES` on 36

**Confirmed scope** [lnp] [lnp-def]:
- Traffic to **and from** local addresses requires the permission: outgoing TCP, **accepting incoming TCP**, and UDP unicast, multicast and broadcast (send and receive).
- Resolving `.local` services and using `NsdManager` also require it.
- DNS to the local resolver (port 53) is exempt.
- "Implemented deep in the networking stack", so it covers native sockets, which includes the Rust/iroh sockets.
- Without permission: "TCP connections: typically result in TIMEOUT error; UDP/General: EPERM". NDK: `android_getnetworkblockedreason()` returns `ANDROID_NETWORK_BLOCKED_REASON_LNP`.
- **What counts as local** (on broadcast-capable interfaces, i.e. Wi-Fi and Ethernet; cellular and VPN are excluded):
  - IPv4 169.254/16, **100.64/10 (CGNAT)**, 10/8, 172.16/12, 192.168/16;
  - IPv6 link-local, **directly-connected routes**, stub networks such as Thread;
  - multicast 224/4 and ff00::/8, and 255.255.255.255.
- The public internet is unaffected.
- A real-world example [memories#1718]: the system DownloadManager can't hold the permission, so LAN downloads through it silently time out on Android 17.

**What's wrong in the design**
- **SDK 36:** "Local network access is open" by default. Restriction happens only if you run `adb shell am compat enable RESTRICT_LOCAL_NETWORK <pkg>`, and only then does `NEARBY_WIFI_DEVICES` restore access. [lnp] [a16-behavior-36] Requesting `NEARBY_WIFI_DEVICES` in production on target 36 adds a "Nearby devices" prompt for no benefit.
- **Enforcement** applies to apps **targeting 37+** on Android 17+. A target-37 build on Android 16 or older has no such permission to request.
- **Same-LAN IPv6 is local too** (directly connected route). Two phones on the same Wi-Fi therefore lose *both* layer 1 (LAN/mDNS) and the layer-2 "direct IPv6" path without the permission, and fall back to hairpin NAT or relays.
- The permission sits in the **NEARBY_DEVICES** group, whose system prompt wording is about "nearby devices". After two denials it's permanently denied.

**Fix:**
- On target 36: request nothing.
- On target 37: declare `ACCESS_LOCAL_NETWORK` and request it only on `SDK_INT >= 37`, at the moment a member appears to share the network. Detect that without LAN traffic, by comparing public reflexive addresses from iroh/STUN, or by matching peer-advertised private prefixes.
- Make iroh's path selection treat `EPERM` on LAN or on-link candidates as "path unavailable", not as a socket error (E8).
- Update §6 to say that the IPv6-direct layer between same-LAN devices also needs the permission.

### C7. Exact alarms

**Confirmed** [exact-alarms-14]:
- `SCHEDULE_EXACT_ALARM` is denied by default for new installs on 14+ of apps targeting 33+. It's granted in Settings, under "Alarms & reminders".
- `setExact()`, `setExactAndAllowWhileIdle()` and **`setAlarmClock()`** throw `SecurityException` without it. The in-process `OnAlarmListener` variant doesn't need it, but only works while the process is alive.
- `USE_EXACT_ALARM` is restricted by Play to "an alarm or timer app" or "a calendar app that shows event notifications". [play-permissions] A task app with reminders is not eligible.

**Inexact behavior** (AOSP `AlarmManagerService`):
- `setWindow()` minimum window `MIN_WINDOW` = 10 min.
- A plain `set()` gets a window of 75 % of futurity, capped at 1 h (`maxTriggerTime`).
- In Doze, while-idle alarms are quota-limited; the docs table says "Limited to 7 per hour". [power-details]

**Refinements:**
- For target 34+, `SCHEDULE_EXACT_ALARM` no longer lifts the standby bucket (see C2).
- When a reminder is *received* in a background sync, the app can't open the Settings page. It must schedule an inexact alarm and ask the next time the app is opened.
- If the user revokes the permission, all exact alarms are cancelled. Re-check `canScheduleExactAlarms()` on every app start and listen for `ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED` (sent on grant).

**Fix:** without the permission, use `setAndAllowWhileIdle(RTC_WAKEUP, t)`. Say honestly in the UI "may be up to an hour late" (in plain words). Re-arm alarms on `BOOT_COMPLETED`, `MY_PACKAGE_REPLACED` and `TIME_SET` / `TIMEZONE_CHANGED`.

### C8. Block Store

**Confirmed** [block-store]:
- Limits: 4 KB per entry, 16 entries.
- E2E: "the device must be running Android 9 or higher, and the user must have set a screen lock (PIN, pattern, or password)". The doc summary also mentions a signed-in Google account. `isEndToEndEncryptionAvailable()` returns false if any condition is missing.
- Cloud backup is periodic and opt-in per `storeBytes` via `setShouldBackupToCloud(true)`. Calling it again with false deletes the cloud copy on the next sync.
- D2D restore: source Android 6+, target Android 9+.
- Library: `play-services-auth-blockstore:16.4.0`. The docs don't state a minimum Play services APK version (**unverified**).
- The docs are internally inconsistent: they say "Android 9 (API 29)", but Android 9 is API 28.

**Restore semantics are narrower than DESIGN.md §11 implies**
- New device: "when a user goes through the restore flow on a new device, Google Play services first verifies the user, then retrieves your Block Store data."
- Same device: "If the user enables Backup services ... Block Store data is persisted across the app uninstall/reinstall."
- **Not covered:** a user who skips "restore from backup" during setup and installs the app later. Also: restoring from a different old device's backup, and a phone lost before the periodic cloud sync ran (the design stores the personal secret on day one).
- The `foss` flavor and non-GMS phones (Huawei) have no Block Store.

**Fix:**
- On first launch, if Block Store is empty and there are no local lists, offer "Link this phone to your lists" (device linking) as the default recovery path.
- Consider adding the (2025+) Credential Manager restore-credential path as a second store (**unverified** for this purpose).
- Tell the user, in plain words, when backup isn't protected. If `isEndToEndEncryptionAvailable()` is false, don't upload the raw secret. Rely on linking or a sealed cloud backup instead.

### C9. Auto Backup

**Confirmed** [autobackup]:
- 25 MB per app; when exceeded, `onQuotaExceeded()` runs and **nothing** is backed up until the data is back under the limit.
- Backups run when backup is enabled, ≥ 24 h since the last one, the device is idle, and it's on Wi-Fi (unless the user allows mobile data), and only if data changed.
- E2E on Android 9+ with backup enabled and a screen lock (`clientSideEncryption` flag).
- "During Auto Backup, the system shuts down the app"; "the backup system ignores apps that are running in the foreground" (see `backupInForeground`).
- Restore happens "after APK installation but before app launch", including on reinstall.
- On Android 12+ with target 31+, `allowBackup=false` does **not** disable D2D.
- Excluded by default: `getCacheDir()`, `getCodeCacheDir()`, `getNoBackupFilesDir()`.

**Caveats:**
- "Daily" is a best case, not a schedule.
- The app process is killed for the backup. It restarts later under WorkManager; background sync resumes at the next job.
- The docs explicitly warn about restoring "device-specific identifiers" (FCM token example). For this app that means device keys, per-device Nostr slot keys, the iroh endpoint key and any persisted Loro PeerID (risk R5).

**Fix:**
- Use `data_extraction_rules.xml` / `full_backup_content`. Put identity material in `noBackupFilesDir` or Keystore.
- Mark data files `requireFlags="clientSideEncryption"`, and add a separate sealed fallback set only if `clientSideEncryption` is absent.
- Keep the included set ≤ about 20 MB, and log `onQuotaExceeded` into diagnostics.

### C10. Clipboard as a deferred invite

**Confirmed:**
- "Unless your app is the default input method editor (IME) or is the app that currently has focus, your app cannot access clipboard data on Android 10 or higher." [a10-privacy] `ClipboardService` enforces this with `mWm.isUidFocused(uid)`. A read in `onCreate` returns null; read in `onWindowFocusChanged(true)`.
- Android 12+: a toast "APP pasted from your clipboard" on `getPrimaryClip()`, but **not** on `getPrimaryClipDescription()` / `hasPrimaryClip()`. [copy-paste]
- Android 13+: a visual confirmation with a **preview** of copied content, and the `EXTRA_IS_SENSITIVE` flag to hide it. [a13-copy-paste]
- AOSP `ClipboardService`: `DEFAULT_CLIPBOARD_TIMEOUT_MILLIS = 3600000` with `auto_clear_enabled` defaulting to true. **The clipboard is wiped after 1 h.**

**Problems:**
1. The payload is the **group secret itself**, the permanent membership key. On the clipboard it's exposed to:
   - keyboard clipboard histories (Gboard, Samsung Keyboard, SwiftKey);
   - cross-device clipboard sync (Samsung/Windows Link, SwiftKey cloud clipboard);
   - the Android 13 preview.

   A web page can't set `EXTRA_IS_SENSITIVE` via `navigator.clipboard.writeText` (**unverified**, but no web API exposes it).
2. The first-launch read shows a toast on every fresh install, invite or not. That clashes with "First launch asks nothing" and looks like snooping.
3. If the user installs later than 1 h after copying, the invite is gone. The fallback "tap the link again" still works.

**Fix:**
- Call `hasPrimaryClip()` + `getPrimaryClipDescription()` first (no toast). Only call `getPrimaryClip()` if the MIME type is `text/plain` and, on API 31+, the description classifies as a URL (`getConfidenceScore(TextClassifier.TYPE_URL)` when the classification status is complete).
- After consuming an invite, call `clearPrimaryClip()` (API 28+).
- Better still: have the web page copy a **short-lived join code** instead of the permanent link. The existing 6-word, 1-day invite-code mechanism fits: the web page can't mint one, but the inviter's app can include a code in the shared link's fragment for the "Get the app" page to copy. Then a leak expires in a day.

### C11. Glance checkboxes, code scanner, In-App Updates

- **Glance checkboxes: partial.**
  - `CheckBoxTranslator.kt` uses `LayoutType.CheckBox` with `setCompoundButtonChecked` on S+, and `LayoutType.CheckBoxBackport` (an image plus a click action) below S. So checkboxes work on API 26–30. The visual state waits for a re-render there, while on S+ the launcher toggles it instantly.
  - Glance runs each widget session as a **WorkManager** `OneTimeWorkRequest` (`SessionManager`) that stays alive `initialTimeout = 45.seconds` (5 s when idle) (`SessionWorker.kt`). Every background redraw therefore uses job quota and waits for Doze windows.
  - Target 37 adds a RemoteViews bitmap memory cap (1.5 × screen w × h × 4 bytes). [a17-behavior-37]
  - **Fix:** drop "tap opens the app" as the < 12 fallback. Batch widget redraws (one `updateAll` per sync run, not per change).
- **Code scanner: confirmed.** "Scanning code without requiring your app to request camera permission." It needs Google Play services and downloads a scanner module on first use unless install-time delivery is set through metadata. [code-scanner] **Fix:** declare the install-time module dependency so the first scan works offline. The `foss` flavor keeps CameraX + ZXing and asks for the camera permission, as designed.
- **In-App Updates: confirmed with caveats.**
  - `AppUpdateInfo` exposes availability, `availableVersionCode`, `updatePriority()` and `clientVersionStalenessDays()`.
  - "Priority can only be set when rolling out a new release and cannot be changed later", through the Publishing API (`inAppUpdatePriority`), not the Console UI.
  - "In-app updates are only available to user accounts that own the app." It needs a Play install with the same signing key. [iau] [iau-kotlin] [iau-test]
  - Consequences: GitHub-APK and F-Droid installs never see updates through it. Staged rollouts can hide an existing newer version. Samsung "deep sleeping" apps don't auto-update, which causes version skew within groups.
  - **Fix:** branch on `PackageManager.getInstallSourceInfo()`: Play means In-App Updates, otherwise GitHub Releases. Gossip `versionCode`, not the version name.

---

## New risks (not considered in DESIGN.md)

| # | Risk | Severity | Evidence | Proposed fix |
|---|---|---|---|---|
| R1 | **Android 15+ background network firewall (all apps).** About 5 s (2 s / 20 s under a newer flag) after leaving the foreground, sockets fail unless a job or FGS runs. It hits iroh, Nostr and any "finish in the background" coroutine. | High | a15-behavior-all: "apps that start a network request outside of a valid process lifecycle receive an exception", all apps; NPMS `FIREWALL_CHAIN_BACKGROUND`, `mBackgroundRestrictionDelayMs = 5 s` | Start the EJ/FGS at ON_STOP synchronously. Treat every background network use as job-scoped. Close sockets cleanly at ON_STOP + 3 s. |
| R2 | **Cached-app freezer kills TCP sockets** and bursts missed timers on unfreeze. Every job run starts cold: TLS to 5–8 relays + iroh relay + DHT. That costs battery and time and makes the phone look flaky to peers. | Medium | freezer doc (10 s, TCP termination, timer bursts) | Per-run connection budget: 2–3 best relays in background runs, all relays only in the foreground. Jittered timers. Idempotent reconnect. |
| R3 | **"Charging" ≠ plugged in** for JobScheduler and standby (≥ 90 % or rising, 15-min debounce; charge limiters report discharging). Quotas come back overnight even though Doze stays off. | Medium | `BatteryStatsImpl` (`BATTERY_CHARGING_ENFORCE_LEVEL = 90`, `BATTERY_CHARGED_DELAY_MS = 15 min`), `JobSchedulerService.isConsideredCharging()` | Gate "heavy help" on `JobScheduler`-observed quota (stop reasons), not on `BatteryManager` plug state. Measure (E7). |
| R4 | **Job batching / flexible scheduling** can delay non-expedited jobs by up to 31 min, even for ACTIVE apps if `batch_active_bucket_jobs` is on. That affects the "connectivity-constrained job uploads as soon as possible" path. | Medium (unverified in production) | `JobSchedulerService` force-batch logic; `job.aconfig` | Use expedited work for user-caused uploads (flush, widget action). Keep periodic work for the backstop only. |
| R5 | **Device identity cloning via Auto Backup / D2D.** The old and new phones both live with the same per-device Nostr slot key (writers overwrite each other's replaceable slot, so mailbox data is lost), the same iroh endpoint ID, and possibly the same Loro PeerID (duplicate (peer, counter) pairs corrupt CRDT merges). | **Critical** if identity is stored in backed-up files | autobackup doc warns about restored device identifiers; D2D ignores `allowBackup=false` (target 31+) | Keep all identity material in `noBackupFilesDir` or Keystore. On start, if a Keystore-bound "install marker" is missing but data exists, it's a restore: mint a new device identity and link through the personal group. Never persist or reuse a Loro PeerID across installs (coordinate with R3-loro). |
| R6 | **Keystore-wrapped secrets inside backed-up files** restore as undecryptable ciphertext (Keystore keys aren't backed up), which can crash on first read. The Jetpack `security-crypto` library (EncryptedSharedPreferences) is reported deprecated. | Medium | DESIGN §9 stores group secrets in Keystore-backed storage; Keystore non-exportability | Exclude the secure-storage files from backup explicitly. Handle `AEADBadTagException` or `KeyStoreException` by treating the store as empty. Don't depend on `security-crypto`. |
| R7 | **Samsung "sleeping" (3 days) / "deep sleeping" (16 days)** may override AOSP's widget minimum bucket. Deep-sleeping apps "can't perform any activities" and miss Play updates, which causes version skew. | Medium (unverified) | Samsung developer page; dontkillmyapp | E1 on One UI 7/8. If widgets don't exempt, detect it through a "no job run in > 24 h" heuristic and show the §8 explainer with the Samsung-specific path. |
| R8 | **Glance redraws are WorkManager jobs** (45 s sessions): widget updates consume job quota, are Doze-deferred, and compete with sync. "Every change redraws the widget" multiplies that per widget. | Low–Medium | Glance `SessionWorker.kt`, `SessionManager.kt` | One `updateAll()` per sync run. Measure job time per update (E12). If needed, render simple RemoteViews directly for background-triggered refreshes. |
| R9 | **Clipboard leak of the permanent group secret** (keyboards, cross-device clipboard sync, Android 13 preview); 1 h auto-clear. | Medium | ClipboardService, copy-paste docs | Copy a 1-day code instead of the secret. `clearPrimaryClip()` after use. Classify before reading. |
| R10 | **iroh / quinn-udp and `EPERM`** on LAN or on-link candidates on Android 17 without permission. A batched `sendmmsg`/GSO send containing a blocked destination could fail the whole batch or be treated as a fatal socket error. | Medium (unverified) | LNP doc: UDP gets EPERM | Test (E8). Filter local candidates in the transport manager when permission is absent. |
| R11 | **Android 16 counts TOP-started jobs against quota once the user leaves.** A large first sync, such as joining a big list, started in the foreground can be cut off in the background. | Low–Medium | a16-behavior-all | Use a user-initiated data transfer job (UIDT) for big first syncs, or keep them short and resumable. |
| R12 | **Android 17 app memory limits** (`MemoryLimiter:AnonSwap` kills). The Rust engine, Loro docs and a 16–32 MB helper RAM cache all add up. | Low–Medium | a17-behavior-all | Watch `ApplicationExitInfo.getDescription()`. Scale the helper cache to `ActivityManager.getMemoryClass()` / `isLowRamDevice()`. |
| R13 | **Excessive partial wake locks** (a Play vitals metric: ≥ 2 h/24 h in > 5 % of sessions "can affect your app's visibility on Play"). JobScheduler is exempt, FGS wake locks are not. | Low (only if an FGS helper fallback is built) | play vitals excessive-wakelock | Don't hold app wake locks. Let jobs keep the CPU awake. |
| R14 | **Reminders received in the background** can't trigger the exact-alarm permission flow and silently degrade to up to 1 h late. | Low | exact-alarms-14, `AlarmManagerService.maxTriggerTime` | Say so in the UI. Ask at the next foreground moment. |
| R15 | **Target-37 TLS changes** (CT on by default, ECH) affect platform TLS stacks. That matters only if the Rust side uses Android's platform verifier. | Low | a17-behavior-37 | Keep rustls with bundled roots, or test with the platform verifier on 37. |
| R16 | **App hibernation (Android 12+, 90 days unused)** revokes permissions and stops background work. It counts `lastTimeAnyComponentUsed`, so running jobs and widgets keep the app "used". It becomes a risk only after background work is already blocked (e.g. Samsung deep sleep). | Low | PermissionController `HibernationPolicy.kt` (`DEFAULT_UNUSED_THRESHOLD_MS = 90 days`) | Nothing to build. Mention it in the blocked-sync explainer. |

---

## Suggested experiments (local devices and emulators, local test relays; no public-relay traffic)

| # | Question | Setup | Measure / pass criterion |
|---|---|---|---|
| E1 | Does a bound Glance widget keep ACTIVE on OEM skins, and does it survive Samsung sleeping / deep sleeping? | Test APK + widget on a Pixel (A16/A17), Samsung One UI 7/8, Xiaomi HyperOS, OnePlus OxygenOS; don't open the app for 3, 8 and 16+ days | `adb shell am get-standby-bucket <pkg>` daily; `dumpsys usagestats`; Samsung Battery → Background usage limits lists; job-run log. Pass: ACTIVE throughout and periodic jobs still run. |
| E2 | Real periodic cadence under Doze | Periodic 15-min worker logging timestamps; phone unplugged and stationary overnight; also `dumpsys deviceidle force-idle` | Histogram of gaps; `getPendingJobReasonsHistory()` on A16+. Decide the user-facing wording from the p50/p90 gaps. |
| E3 | Can `shortService` start at ON_STOP, and how often is its notification visible? | Start at `ProcessLifecycleOwner` ON_STOP, and with artificial delays of 0–8 s; repeat app switches at 30 s, 90 s and 3 min intervals | `ForegroundServiceStartNotAllowedException` rate versus delay (expect a cliff near 5 s); fraction of runs where the notification was posted; Task Manager visibility. |
| E4 | Expedited job as the flush path | Enqueue an expedited worker at ON_STOP; also in battery saver and with the device in forced Doze | Start latency (expect < 1 s on 13+), completion rate, and the absence of any notification on 12+. |
| E5 | When does background network die after leaving the app? | After ON_STOP, loop UDP + TCP sends to a LAN test server every 250 ms, with no job or FGS | Time to first `EPERM`/`UnknownHost`/`IOException` (expect about 2–5 s, or 20 s); repeat with the EJ running. |
| E6 | Freezer effects on iroh and WebSockets | Leave the app idle in cached state; `adb shell dumpsys activity processes` (frozen state); a local Nostr relay + local iroh relay | Socket close events on freeze; reconnect time and data cost per job run. |
| E7 | Phone-helper duty cycle while charging | Chain one-time workers on charger + Wi-Fi overnight, with and without a charge limiter (Samsung Protect battery, Pixel 80 % limit); a peer probes reachability every 30 s over the LAN / local relay | Fraction of the night reachable; gaps between jobs; stop reasons; `isCharging()` transitions; `ApplicationExitInfo` kills. Decide §7a from the duty cycle. |
| E8 | Local network permission with iroh | Target-37 build on an Android 17 device or emulator: permission denied vs granted; two devices on one Wi-Fi with IPv4 and IPv6 | LAN / mDNS / IPv6-direct success; error codes (`android_getnetworkblockedreason`); confirm relay and public paths are unaffected and no batch send failures occur. |
| E9 | Block Store restore paths | Store a 32-byte test secret with cloud backup; then (a) factory reset + cloud restore, (b) D2D cable/wireless to a second phone, (c) new phone set up *without* restore, then install the app, (d) uninstall/reinstall with Google Backup on and off | Whether the secret is retrievable in each case, and the time between `storeBytes` and the cloud copy existing. |
| E10 | Auto Backup / D2D identity cloning | `adb shell bmgr backupnow <pkg>`, then `bmgr restore`; a D2D transfer between two test phones that both keep running | Confirm identity files are excluded, a new device identity is minted, and Loro PeerIDs differ; test `requireFlags="clientSideEncryption"` with no screen lock. |
| E11 | Clipboard deferred invite end to end | Real Chrome on Android → "Get the app" → Play (internal testing) → first launch; Gboard, Samsung Keyboard, SwiftKey | Invite picked up after focus; toast behavior with the description-first check; clipboard-history persistence in each keyboard; the 1 h auto-clear. |
| E12 | Glance cost | `adb shell dumpsys jobscheduler` while forcing widget updates; API 26–30 emulator for backport checkbox latency | Job-seconds per update; tap-to-visual latency before and after S. |
| E13 | In-App Updates behavior | Internal app sharing with two version codes; a GitHub-APK install of the same app ID | `AppUpdateInfo` fields; the error code on non-Play installs; staged-rollout visibility. |

---

## Sources

**Official Android documentation**
- [power-details] Power management restrictions: https://developer.android.com/topic/performance/power/power-details
- [appstandby] App Standby Buckets: https://developer.android.com/topic/performance/appstandby
- [doze-standby] Optimize for Doze and App Standby: https://developer.android.com/training/monitoring-device-state/doze-standby
- [define-work] WorkManager, define work requests: https://developer.android.com/develop/background-work/background-tasks/persistent/getting-started/define-work
- [a15-behavior-all] Android 15 behavior changes (all apps): https://developer.android.com/about/versions/15/behavior-changes-all
- [a16-behavior-all] Android 16 behavior changes (all apps): https://developer.android.com/about/versions/16/behavior-changes-all
- [a16-behavior-36] Android 16 behavior changes (target 36): https://developer.android.com/about/versions/16/behavior-changes-16
- [a17-behavior-all] Android 17 behavior changes (all apps): https://developer.android.com/about/versions/17/behavior-changes-all
- [a17-behavior-37] Android 17 behavior changes (target 37): https://developer.android.com/about/versions/17/behavior-changes-17
- [lnp] Local network permission: https://developer.android.com/privacy-and-security/local-network-permission
- [lnp-def] Local network definition: https://developer.android.com/privacy-and-security/local-network-definition
- [service-types] FGS types: https://developer.android.com/develop/background-work/services/fgs/service-types
- [restrictions-bg-start] Background FGS start restrictions: https://developer.android.com/develop/background-work/services/fgs/restrictions-bg-start
- [fgs-timeout] FGS timeouts: https://developer.android.com/develop/background-work/services/fgs/timeout
- [fgs-types-required] FGS types required (A14): https://developer.android.com/about/versions/14/changes/fgs-types-required
- [notification-permission] Notification runtime permission: https://developer.android.com/develop/ui/views/notifications/notification-permission
- [exact-alarms-14] Exact alarms denied by default: https://developer.android.com/about/versions/14/changes/schedule-exact-alarms
- [block-store] Block Store: https://developer.android.com/identity/block-store
- [autobackup] Auto Backup: https://developer.android.com/identity/data/autobackup
- [copy-paste] Copy and paste: https://developer.android.com/develop/ui/views/touch-and-input/copy-paste
- [a13-copy-paste] Android 13 copy/paste: https://developer.android.com/about/versions/13/features/copy-paste
- [a10-privacy] Android 10 privacy changes: https://developer.android.com/about/versions/10/privacy/changes
- [iau] In-app updates: https://developer.android.com/guide/playcore/in-app-updates
- [iau-kotlin] In-app updates (Kotlin/Java): https://developer.android.com/guide/playcore/in-app-updates/kotlin-java
- [iau-test] Test in-app updates: https://developer.android.com/guide/playcore/in-app-updates/test
- [play-vitals-wakelock] Excessive partial wake locks: https://developer.android.com/google/play/vitals/excessive-wakelock
- [freezer] Cached apps freezer (AOSP docs): https://source.android.com/docs/core/perf/cached-apps-freezer
- [code-scanner] Google code scanner: https://developers.google.com/ml-kit/vision/barcode-scanning/code-scanner

**Google Play policy**
- [play-fgs] FGS and full-screen intent requirements: https://support.google.com/googleplay/android-developer/answer/13392821
- [play-fgs-preview] Preview of FGS requirements (effective 2026-08-26 changes): https://support.google.com/googleplay/android-developer/answer/16965181
- [play-permissions] Permissions and APIs that access sensitive information (exact alarm): https://support.google.com/googleplay/android-developer/answer/9888170

**AOSP source** (`refs/heads/main`, fetched 2026-09-24)
- AppStandbyController: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/service/java/com/android/server/usage/AppStandbyController.java
- AlarmManagerService: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/service/java/com/android/server/alarm/AlarmManagerService.java
- AlarmManager (compat changes): https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/framework/java/android/app/AlarmManager.java
- QuotaController: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/service/java/com/android/server/job/controllers/QuotaController.java
- JobSchedulerService: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/service/java/com/android/server/job/JobSchedulerService.java
- JobServiceContext: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/service/java/com/android/server/job/JobServiceContext.java
- JobInfo (setExpedited javadoc): https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/framework/java/android/app/job/JobInfo.java
- job.aconfig (batching flags): https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/service/aconfig/job.aconfig
- DeviceIdleController: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/apex/jobscheduler/service/java/com/android/server/DeviceIdleController.java
- NetworkPolicyManagerService: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/services/core/java/com/android/server/net/NetworkPolicyManagerService.java
- NetworkPolicyManager: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/java/android/net/NetworkPolicyManager.java
- ActiveServices: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/services/core/java/com/android/server/am/ActiveServices.java
- ActivityManagerConstants: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/services/core/java/com/android/server/am/ActivityManagerConstants.java
- BatteryStatsImpl: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/services/core/java/com/android/server/power/stats/BatteryStatsImpl.java
- ClipboardService: https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/services/core/java/com/android/server/clipboard/ClipboardService.java
- HibernationPolicy: https://android.googlesource.com/platform/packages/modules/Permission/+/refs/heads/main/PermissionController/src/com/android/permissioncontroller/hibernation/HibernationPolicy.kt

**AndroidX Glance source** (`androidx-main`)
- CheckBox.kt: https://github.com/androidx/androidx/blob/androidx-main/glance/glance-appwidget/src/main/java/androidx/glance/appwidget/CheckBox.kt
- CheckBoxTranslator.kt: https://github.com/androidx/androidx/blob/androidx-main/glance/glance-appwidget/src/main/java/androidx/glance/appwidget/translators/CheckBoxTranslator.kt
- SessionWorker.kt: https://github.com/androidx/androidx/blob/androidx-main/glance/glance/src/main/java/androidx/glance/session/SessionWorker.kt
- SessionManager.kt: https://github.com/androidx/androidx/blob/androidx-main/glance/glance/src/main/java/androidx/glance/session/SessionManager.kt

**OEM and third-party**
- dontkillmyapp: https://dontkillmyapp.com/samsung, https://dontkillmyapp.com/xiaomi, https://dontkillmyapp.com/oneplus, https://dontkillmyapp.com/huawei (Samsung page last updated July 2024)
- [samsung-dev] Samsung app management: https://developer.samsung.com/mobile/app-management.html
- [android-dev-blog-2023] Improving consistency of background work: https://android-developers.googleblog.com/2023/05/improving-consistency-of-background-work-on-android.html
- [memories#1718] Android 17 LAN download failure through DownloadManager: https://github.com/pulsejet/memories/issues/1718
- Google security blog on Android 17 network protections: https://blog.google/security/new-android-network-security-protections/

**Could not verify** (research budget exhausted or no primary source found):
- whether Samsung's sleeping apps honor bound widgets;
- whether the Play Console FGS declaration asks for `shortService`;
- the minimum Play services version for Block Store;
- the `security-crypto` deprecation status;
- whether Chrome can set a sensitive-clipboard flag;
- whether Xiaomi's clipboard-access prompt exists on global ROMs;
- whether the JobScheduler batching flags are enabled in production A16/A17 builds.
