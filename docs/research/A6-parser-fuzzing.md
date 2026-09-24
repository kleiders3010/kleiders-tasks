# A6: Fuzzing Loro import (a malicious member's view)

Date: 2026-09-24. Loro 1.16.2, Windows x64 release build. Code: `research/experiments/rust/e1_loro/src/bin/a6_fuzz.rs`, `a6_repro.rs`; crash inputs in `research/experiments/rust/e1_loro/fuzz_out/crashes/`.

## Threat
A group member holds the group key, so anything they send decrypts fine and reaches `LoroDoc::import`. So can bytes corrupted on disk, or bugs in another app version.

## Method
Seed corpus: real Loro exports (snapshot, updates, shallow snapshot, state-only). Mutations: bit flips, byte overwrites, truncation, boundary values, chunk duplication, insertion.
- **Pass 1** (mutating whole files): 65.5 M inputs in 5 min, 0 panics. **Misleading:** Loro's header has an xxHash32 checksum, so almost every mutation was rejected before parsing.
- **Pass 2** (mutate the body, then **recompute the checksum**, as an attacker can): the process **aborted in all 40 of 40 sessions within 20 s each**.

## Crashes found (4 distinct sites)
| Count | First panic site | Kind |
|---|---|---|
| 26 | `oplog/loro_dag.rs:445` | `assert_eq!` failure |
| 6 | `oplog/loro_dag.rs:620` | panic |
| 6 | `oplog/change_store.rs:1040` | `unwrap()` on `Err` |
| 2 | `oplog.rs:1237` | panic |

Smallest crashing input: **85 bytes**. After the first panic, a second panic follows (`sync.rs:34`, "poisoned lock"), so the panic turns into a **process abort** (`0xC0000409`). `catch_unwind` cannot catch it.

## Impact on our design
- **Any member (or corruption) can crash every other member's app** by putting such an update in their mailbox slot.
- Because slots are re-fetched on every sync and app launch, it becomes a **crash loop**: the app dies on every start until the slot is replaced. That's a denial of service against the whole group, and with the planned `panic = "abort"` release profile, even single panics abort.
- The same applies to local files corrupted on disk (crash at launch).

## Options (evaluated)
1. **Isolate untrusted imports in a separate process.** Before importing into the live document, import it into a *copy* of the live document in a disposable process (Android: a service in its own `android:process`; desktop: a child process). Only if that succeeds does the live import proceed. A crash kills only the disposable process.
   - Cost: a second process's memory (the copy is small: KBs to a few MB), IPC, and ~tens of ms latency per sync batch.
   - This is the only option I found that contains the aborts. **Confidence: high** that it contains crashes; **not verified**: the cost of an isolated process on Android and in background jobs.
2. **Crash-loop breaker (needed in any case).** Before importing, persist "importing update <hash> from <author>". If the app starts and finds that marker, the update is quarantined, never imported again, and the author is flagged. This doesn't prevent the first crash, but stops the loop. It's cheap.
3. **Fix upstream.** Report the 4 crash sites with inputs to Loro. They maintain fuzzing and fix such bugs, but new ones may appear, so this complements 1 and 2 rather than replacing them. **Needs your OK:** it publishes a bug report publicly on GitHub.
4. **`catch_unwind` in-process: tested, doesn't work.** The double panic aborts. Not a real option unless Loro removes the lock-poisoning panic.

Also: author signatures on updates (R7) let the breaker name *who* sent a poisoned update.

**Recommendation:** 1 + 2, with 3 if you agree to file publicly.

## Not yet done
- A longer fuzz campaign with coverage guidance (cargo-fuzz / libFuzzer) after crash handling exists.
- Fuzzing our own message formats (they don't exist yet; this must be part of the engine's CI).
