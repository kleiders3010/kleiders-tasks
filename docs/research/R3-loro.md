# R3: Loro CRDT, claim verification

*Research date: 2026-09-24. Research only; nothing was built, installed or run against public relays. Versions examined: Rust `loro` / `loro-internal` **1.16.2** (crates.io, 2026-09-21), npm `loro-crdt` **1.16.3** (2026-09-21), `loro-ffi` **1.16.2**, Loro repo HEAD `ad5b2a6` (2026-09-21), loro-docs HEAD `320dd16` (2026-09-04). Source was read from shallow clones of `loro-dev/loro`, `loro-dev/loro-ffi`, `loro-dev/loro-swift` and `loro-dev/loro-docs`.*

## Summary

- **The version the design cites is out of date.** The current release is 1.16.2 (Rust) / 1.16.3 (JS), not "~1.15". The Rust crate went 1.13.9 → 1.16.0 → 1.16.2, so **there is no Rust 1.14.x or 1.15.x**. The movable-tree convergence fix shipped in **1.13.9** (2026-08-01). It was not in "1.15.1", whose notes only cover JS container-ID caching.
- **The binary format has been stable in practice since 1.0, but nobody promises it forever.** The top-level modes 3/4 are unchanged, Loro runs bidirectional tests against 1.0.0-alpha.4, and it now publishes a normative wire spec. However: 1.9.0 **dropped decoding of the 0.x format**, readers have become stricter over 2026, and the set of nested value kinds is closed, so an older reader rejects the **whole blob** if a future nested value type appears. "Always backward compatible, forever" is therefore a promise **the app must own** (pinning, a golden corpus, a group feature floor). Loro alone doesn't provide it.
- **Shallow snapshots work, but they permanently exclude any edit that is concurrent with the chosen root and not yet received.** Loro documents this as "the one lossy case". Since 1.16.0 such updates are rejected with `ImportUpdatesThatDependsOnOutdatedVersion`. Any other valid changes in the same blob are still applied, and later changes that depend on a rejected one sit **pending forever, silently**. Before 1.16.0 these cases panicked (#1068) or stalled silently (#1040). One more abort path still has an open fix PR (#1083). **The design's per-device compaction at "its own" frontier would silently drop other devices' in-flight offline edits.** The root must be a version that every member device has acknowledged.
- **The design's "state checkpoints at widening intervals" cannot be built inside one Loro document.** Loro keeps either full history after the root or nothing. It cannot keep sparse history. `fork_at` / `SnapshotAt` on shallow documents return `NotImplemented` (PR #1115, opened 2026-09-23). `StateOnly(frontiers)` produces a checkpoint only while history still reaches that version, and each checkpoint is a **full state copy**.
- **"Old clients preserve unknown data" holds for new fields and new keys, but only partly for new Loro container types.** Operations on unknown containers survive in the oplog. However, unknown **state** can be lost when re-encoded after materialization. Shallow and state-only export **fail with `UnknownContainer`**, so compaction on old clients breaks. Loro's own cross-version test skips mixed snapshots because the older build "cannot properly handle" them. Before 1.14.0, reading an unknown container **trapped** the WASM build.
- **Stability of the containers, in order:** maps are the most mature. Movable tree, movable list, shallow snapshots and concurrent-import replay all needed **critical fixes during 2026**: a tree convergence bug (1.13.9), an import panic that "permanently broke sync" (1.14.0), unimportable shallow snapshots that hit a production user (1.16.2), and snapshots that imported fine and then **panicked on first read** (#1088). Open issues remain (#1055, #1068, #1040).
- **PeerIDs must be globally unique, and nothing authenticates them.** On import Loro deduplicates by `(peer, counter)` alone, with no content check, so a reused PeerID causes silent divergence. **Android Auto Backup / restore** is the most likely way the app would reuse one. Any member can forge operations as another peer, or "squat" another peer's future counters so that peer's real edits are dropped everywhere.
- **Performance is probably fine for a task app, with no published measurements on low-end Android.** The Loro WASM binary is 3.27 MB. The native aarch64 `.so` (loro-py) is 10.0 MB unstripped, with 5.0 MB of `.text`. Updates that each carry a single op cost about 88–100 B. Snapshot history costs about 12–17 B/op for continuing peers and about 51 B/op when every op comes from a new peer. Several performance cliffs were fixed only in 2026, so **require ≥ 1.16.2**.
- **FFI:** `loro-ffi` (UniFFI 0.31.1, UDL) exists. There is no maintained Kotlin binding, and `loro-swift` calls itself "experimental". This barely matters, because the engine should link the `loro` crate directly and expose its own domain API through UniFFI. **New licensing finding:** Loro depends on `im` / `sized-chunks` / `bitmaps` (**MPL-2.0+**, unmaintained, RUSTSEC advisories including an unpatched HIGH one). That conflicts with principle 9 as written.
- **Other design-level risks:** the design's own `panic = "abort"` profile turns any Loro panic into an app crash, and a "poison" blob would crash every member in a loop. Undoing or restoring a deleted tree node **creates a new TreeID**. Tasks moving between lists are cross-document copies, not moves. Pending (out-of-order) changes live only in memory.

## Claim-by-claim table

| # | Claim | Verdict | Severity if wrong | One-line evidence |
|---|---|---|---|---|
| 1 | Loro 1.x (~1.15) has a stable, backward-compatible binary encoding across versions | **Partial** | High | Stable in practice (modes 3/4 since 1.0, tests against 1.0.0-alpha.4, normative spec). No perpetual guarantee: 0.x decoding removed in 1.9.0, stricter readers, closed nested-value set. Latest is 1.16.2, not ~1.15. |
| 2 | Shallow snapshots trim history; behavior of dependent imports; checkpoints via checkout/fork_at | **Partial** | High (critical for the long-offline and snapshot-slot paths) | ≥1.16.0: rejected with an error (partial apply, dependents pending forever). <1.16.0: panics or silent pending. `fork_at`/`SnapshotAt` on shallow docs are `NotImplemented`. Sparse history is impossible. |
| 3 | Unknown data from newer versions is preserved by older clients, even when an older Loro reads newer Loro data | **Partial** | High | Unknown container ops are preserved. Unknown state can be dropped on re-encode, shallow/state-only export fails with `UnknownContainer`, and Loro's own test skips this case. New nested value kinds would reject the whole blob. |
| 4 | Movable tree / movable list / maps / rich text are production-ready | **Partial** | High | Maps are solid. Tree convergence fixed in **1.13.9** (not 1.15.1). Movable-list panic (#1046), tree undo identity bug (#1055, open), snapshot panics on read (#1088), and the open convergence PR #1116. |
| 5 | Performance and memory are OK with large docs on low-end devices; encoding size per op; history growth | **Unverified** (partial data) | Medium | No Android benchmarks exist. Desktop numbers are good. 2026 fixed 4 KB-per-container memory retention and several O(n²)/multi-second cliffs. Per-op sizes are in the table under claim 5 in Details. |
| 6 | PeerID uniqueness requirements; is authorship authenticated? | **Confirmed as a risk** (unique IDs required; no authentication) | High (backup restore) / Medium (forgery) | Import trims by counter only (`trim_the_known_part_of_change`); duplicate-ID check is `#[allow(unused)]`. No signatures anywhere in the format. |
| 7 | FFI bindings usable from Kotlin (loro-ffi / UniFFI, loro-swift precedent); WASM/Rust size | **Partial** | Low | loro-ffi 1.16.2 on UniFFI 0.31.1 has no Kotlin config or package, and loro-swift is "experimental". Loro builds for Android (loro-react-native ships arm64/armv7 libs). Native size is roughly 5–7 MB stripped per ABI (estimate). |

## Details per claim

### Claim 1: stable, backward-compatible binary encoding

**What Loro actually promises:**
- The 1.0 announcement: "Loro 1.0 has a stable data format" ([blog/v1.0](https://loro.dev/blog/v1.0)).
- The performance page: "Because we want Loro to have good backward and forward compatibility after reaching version 1.0, we have adopted a more easily extensible encoding method… without introducing breaking changes" ([docs/performance](https://loro.dev/docs/performance)).

That is intent, not a written support policy. No document says "every future version will import every 1.x blob".

**Evidence for stability:**
- **Top-level format unchanged since 1.0.** Only two current modes exist: `FastSnapshot = 3` and `FastUpdates = 4`. The normative spec was verified against code on 2026-07-17 ([docs/encoding.md](https://github.com/loro-dev/loro/blob/main/docs/encoding.md), [context/internal-encoding.md](https://github.com/loro-dev/loro/blob/main/context/internal-encoding.md)).
- **Cross-version tests.** `crates/fuzz/tests/compatibility.rs` imports current snapshots, shallow snapshots and updates **into `loro-crdt@1.0.0-alpha.4`** and the reverse, then compares deep values ([compatibility.rs](https://github.com/loro-dev/loro/blob/main/crates/fuzz/tests/compatibility.rs)). The random-op generator covers only text, map, list, tree and movable_list, with 100 ops per run.
- **A documented spec with independent implementations.** Loro has a pure TypeScript implementation (`loro.js`, since 2026-07-21), a MoonBit codec in the repo (`moon/`), and a pure-Go third-party codec ([loro-go](https://github.com/Deln0r/loro-go)). This is good for longevity: the format outlives the Rust implementation.

**Evidence against "forever":**
- **Precedent for dropping formats.** loro 1.9.0 (2025-11-10) shipped "**Removed legacy v0.x encoding path**… Documents created with v0.x must be migrated via an intermediate ≤1.8.x release before installing 1.9.0" ([changelog v1.9.0](https://loro.dev/changelog/v1.9.0)). Modes 1/2 now return `ImportUnsupportedEncodingMode`. These were pre-1.0 formats, but the precedent is that Loro removes old decoders.
- **Readers keep getting stricter.** This can make a newer reader reject blobs that an older, buggy writer produced:
  - 1.12.0 hardened decoding against malformed input.
  - 1.13.8: "Snapshot import now validates every embedded SSTable block checksum".
  - 1.16.2: "Tree state with duplicate node ids or duplicate sibling positions is rejected as a decode error" ([releases](https://github.com/loro-dev/loro/releases)).
- **Cross-implementation interop has already broken once.** loro.js 0.2.0 tree snapshots "imported into `loro-crdt` without error, then panicked on the first read" ([#1088](https://github.com/loro-dev/loro/issues/1088)). The fix went into both sides in 1.16.2.
- **Nested values are a closed set.** `LoroValueKind::from_u8` accepts only tags 0–9 and returns `None` for anything else, which is a decode error ([value.rs](https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/encoding/value.rs)). New *op* kinds (tags 17–127) and new *container* types (`ContainerType::Unknown(u8)`) have forward-compatibility paths. A new nested value type does not.
- **The Rust API changes within 1.x.** Examples: 1.8.0 "events are not async anymore"; 1.16.0 "Potentially breaking" change to the `cid` output; open PR [#1116](https://github.com/loro-dev/loro/pull/1116) changes the `delete_root_container` signature. 1.9.0 was yanked on crates.io. API churn doesn't break the format, but it raises the cost of every upgrade.

**Proposed fix:**
1. Pin the exact version (`loro = "=1.16.2"`) and upgrade deliberately.
2. Keep a **golden corpus** of blobs written by every shipped app version: snapshots, shallow snapshots, updates, every container type, and mergeable containers. CI imports each blob into the new build, exports from it, and imports that export **into every older shipped build**.
3. If Loro ever drops a format again, vendor the old decoder as a renamed dependency and migrate. Loro's own fuzz crate already does this with `loro-alpha-4`.
4. Add a **group feature floor**: each device advertises its supported Loro and app feature level in the group document, and new Loro features are used only once every active member supports them (see claim 3).

### Claim 2: shallow snapshots, dependent imports and old-version checkpoints

**What trimming does.** `ExportMode::ShallowSnapshot(F)` keeps the state at a root `S` plus all operations after `S`.

`S` is "the latest single-head *critical version*… every retained change is causally before or after it". The spec says it "never lies above `F`", and it can be the empty frontier ([encoding.md §3.3](https://github.com/loro-dev/loro/blob/main/docs/encoding.md)). **The trim point can therefore land well before the requested one** whenever long-lived concurrent heads exist, for example a device that has been offline with unmerged edits.

**What happens to updates that depend on trimmed history.** This follows Loro's own contract ([shallow-snapshot-concurrency.md](https://github.com/loro-dev/loro/blob/main/docs/shallow-snapshot-concurrency.md), verified 2026-09-04; the fix is [PR #1090](https://github.com/loro-dev/loro/pull/1090), in 1.16.0):

| Incoming update | Result on a shallow doc (≥ 1.16.0) |
|---|---|
| Causally after the root | Applies normally |
| Causally before the root (already contained) | No-op, `Ok` |
| Based on a version before the root, or **concurrent with the root frontier** | `Err(ImportUpdatesThatDependsOnOutdatedVersion)`; "dropped: they are not applied and they do not become pending, so they can never be delivered to B later" |
| From a peer that **never synced** (genesis change with no deps) | Also rejected ("treated as rooted before F") |
| Later changes by that forked peer, arriving separately | **Parked as pending forever**: "looks deceptively fine… its edits never become visible" |

Two further details:
- **"Error" can mean "partially applied".** In `import_changes_and_apply_delta_to_state_if_needed`, valid post-root changes in the same blob are applied and committed, and only then is `Err(ImportUpdatesThatDependsOnOutdatedVersion)` returned ([loro.rs](https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/loro.rs)). The release note agrees: "subsequent valid post-root updates still apply". The engine must not treat the error as "nothing applied".
- **Older versions behaved much worse:**
  - ≤ 1.15: the concurrent-with-root case was "queued… and panicking".
  - [#1068](https://github.com/loro-dev/loro/issues/1068) (loro 1.13.9, open) reports a panic that then "poisoned LoroMutex", which **aborts the process**. The reporter's workaround is "place the truncation point one operation behind the fleet-wide minimum version".
  - [PR #1083](https://github.com/loro-dev/loro/pull/1083), still open, fixes a second abort path: "changes parked before shallow snapshot import, then unlocked by subsequent updates". In 1.16.2 the `calc_unknown_lamport_change(..).unwrap()` in `pending_changes.rs` is still present, so this path may still abort. Test it (experiment E2).
  - [#1040](https://github.com/loro-dev/loro/issues/1040) (open): importing a shallow snapshot into a doc that has **any** concurrent local commit "silently delivers an **empty document**" (stuck pending).
  - [#1095](https://github.com/loro-dev/loro/issues/1095) / [PR #1104](https://github.com/loro-dev/loro/pull/1104), fixed in 1.16.2: "shallow snapshots that exported successfully but could not be imported into an empty document". A production user (Macro) hit this: "fail to reopen it with 'You cannot switch a document to a version before the shallow history's start version'" ([macro-inc/macro#6760](https://github.com/macro-inc/macro/pull/6760)).

**Importing a shallow snapshot into an existing doc.** Only its retained operations are used, and "its state sections are used only when initializing an empty doc" ([encoding.md §3.6](https://github.com/loro-dev/loro/blob/main/docs/encoding.md)). A device whose version is older than the root therefore ends up with retained changes it can never apply (their dependencies were trimmed), and they stay **pending silently**. This matters directly for the design's Nostr **snapshot slot** (§7).

**Checkpoints at old versions:**
- `checkout`, `diff` and `revert_to` to versions before the root fail with `SwitchToVersionBeforeShallowRoot`. After the root they work.
- `fork_at` and `ExportMode::SnapshotAt` return `NotImplemented("fork_at on shallow docs")` for **every** shallow doc in 1.16.2 (see `encode_snapshot_at` in [shallow_snapshot.rs](https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/encoding/shallow_snapshot.rs)). [PR #1115](https://github.com/loro-dev/loro/pull/1115) (open, 2026-09-23) would allow them from the root onward, never before it.
- `ExportMode::StateOnly(Some(frontiers))` ("a shallow snapshot with depth=1 at the target version") is the right primitive for a checkpoint. But:
  - it only works while the doc still has history back to that version;
  - each checkpoint is a **complete state**, with no sharing between checkpoints;
  - it fails with `UnknownContainer` if the doc contains container types unknown to this Loro build (see claim 3).
- **Loro can't represent "thinned" history** (daily → weekly → monthly inside one oplog). The retained oplog must be complete from the root onward. The design's §10 "older: state checkpoints… with the operation log compacted away" therefore has to be a set of **separate blobs outside the live document**. Restoring from one means diffing app-level values and applying new ops: new op IDs, and new TreeIDs for deleted nodes (see new risk N8).

**Proposed fix:**
1. Choose each shallow root **only at a causally stable version**: the meet of the acknowledged version vectors of every member device seen within a window (e.g. 30 days), minus a safety margin. Never at a device's local head.
2. Devices whose acknowledged version is older than the root are considered "stale" and take the explicit rebase path.
3. Handle `ImportUpdatesThatDependsOnOutdatedVersion` as "partially applied": re-read `oplog_vv()` and trigger the stale-device protocol towards the sender.
4. After every import, check `ImportStatus.pending`. Pending entries whose dependencies fall below a known shallow root are fatal, so trigger a rebase instead of waiting.
5. Store checkpoints as app-level, compressed, delta-encoded JSON of the deep value (or as `StateOnly` blobs), exported **before** each trim.
6. Budget the checkpoint count by *full state size × N*, not by operation volume.

### Claim 3: older clients preserve unknown data written by newer Loro

**What is designed for forward compatibility:**
- Op values with unknown kinds (17–127) are kept as opaque `Future` values.
- Unknown container kinds become `ContainerType::Unknown(raw)`.
- The JSON schema doc states the goal: "When an `Op` with a newly supported Container from a newer version is decoded into the older version, it will be treated as an unknown type" ([JsonSchema.md](https://github.com/loro-dev/loro/blob/main/docs/JsonSchema.md)).
- Mergeable containers (1.13.0) were designed so that "Older clients… see the parent slot marker as an ordinary binary value… They can preserve and sync the data" ([blog: mergeable containers](https://loro.dev/blog/mergeable-containers)).

**Where it breaks** (quoted from Loro's own spec, [encoding-container-states.md §10](https://github.com/loro-dev/loro/blob/main/docs/encoding-container-states.md)):
- "If the wrapper is materialized as `UnknownState`, its public value is Null… the current `UnknownState` encoder emits no type-specific bytes. **Re-encoding after that point can therefore discard an opaque payload.**" An old client that re-exports a snapshot can therefore ship an empty or mangled state for a container type it doesn't know. A new client that bootstraps from that snapshot would use the state section.
- "A generic root rebuild **rejects unknown materialized live containers**". `export_state_only_snapshot` returns `LoroEncodeError::UnknownContainer`. **Compaction and checkpointing on old clients fail** as soon as a newer container type exists in the document.
- Loro's own cross-version test for an unknown type (a Counter against a build without the counter feature) says: "Skip cross-version snapshot tests when the document contains containers other than the root counter. **loro_without_counter cannot properly handle mixed container snapshots that include Counter**" ([fuzz/src/container/counter.rs](https://github.com/loro-dev/loro/blob/main/crates/fuzz/src/container/counter.rs)).
- 1.14.0 (2026-08-08): "accessing an unknown container (created by a newer version of loro-crdt) now throw catchable JS errors… **instead of trapping the WASM instance**". Until August 2026 this path crashed. It is a rarely exercised code path.
- Nested value kinds are closed (claim 1). A future nested type would make old clients reject **the whole change block**. Everything causally after it from that peer then stays pending, so **the old client stops syncing with that peer entirely** instead of "just not displaying" the unknown data.

**The design's app-level case works.** New *fields* (new map keys holding values of existing types, or existing container types) are fully understood by old Loro and survive. That holds only if old app code never:
- rewrites whole objects;
- copies a task generically but lossily, as in a move to another list (see N9), duplicate, or import/export;
- re-creates nodes, as undo-delete does (see N8).

**Proposed fix:**
1. Distinguish **app schema additions**, which are always allowed, from **Loro feature additions** (new container types, mergeable containers, new value kinds, Loro major upgrades). The second kind is gated by the group feature floor.
2. Old clients must catch `UnknownContainer` on shallow/state-only export and fall back to "no compaction" (a full snapshot).
3. Pick the container set now and never add a new Loro container type unless every member supports it. Keep the `counter` feature enabled in every build flavor, since it is behind a cargo feature (`#[cfg(feature = "counter")]`).
4. Use mergeable containers from day one (≥1.13) for per-task child containers such as descriptions, so no old client ever sees them as unknown.

### Claim 4: production readiness; known convergence bugs

**Timeline of 2026 correctness fixes** (from the [GitHub releases](https://github.com/loro-dev/loro/releases) and issues):

| Version (date) | Fix | Type |
|---|---|---|
| 1.12.0 (Apr) | "Make update imports atomic… malformed updates do not leave the document with oplog/state divergence" | divergence |
| 1.13.7 (Jul) | Checkout **hang** after snapshot import (wrong lamport range in decoded blocks) | liveness |
| **1.13.9 (Aug 1)** | "**convergence bug where a movable tree's incrementally maintained state could diverge from a full replay of its own oplog**", when imported ops were concurrent with part of a multi-head frontier | **convergence (tree)** |
| 1.13.9 (via [PR #1058](https://github.com/loro-dev/loro/pull/1058)) | Movable-list panic on delete plus concurrent edit+move imported in separate batches ([#1046](https://github.com/loro-dev/loro/issues/1046), a regression in 1.13.7; issue still open, fix confirmed in [PR #1079](https://github.com/loro-dev/loro/pull/1079)) | panic (movable list) |
| 1.14.0 (Aug 8) | "Fix a panic on import that could **permanently break a document's sync**" | panic / liveness |
| 1.14.1 (Aug 10) | `importBatch` left the doc **detached** ("later imports stop being reflected in the state"); quadratic time | liveness |
| 1.15.0 (Aug 27) | Rich text: shallow export leaked deleted mark values; O(n²) styled reads; unbounded degradation from redundant marks | privacy / perf |
| 1.16.0 (Sep 6) | Shallow boundary rejection instead of panic; shallow export **resurrecting deleted root containers**; memory retention | panic / correctness |
| 1.16.2 (Sep 21) | Shallow snapshots **unimportable**; `forkAt` losing deleted-container state; panic/**deadlock** when subscribing during emit; tree snapshot decode from loro.js | correctness / panic |

About the claim's "1.15.1 movable-tree fix": 1.15.1 only "Cache[s] immutable container ids per JavaScript wrapper" and handles malformed `cid:` strings. The tree convergence fix is **1.13.9**.

**Open items as of 2026-09-24** ([issue list](https://github.com/loro-dev/loro/issues)):
- [#1055](https://github.com/loro-dev/loro/issues/1055): undoing a post-import LoroTree edit re-creates the content under a **different TreeID**. It cross-references #915 and #938.
- [#1068](https://github.com/loro-dev/loro/issues/1068) / [#1040](https://github.com/loro-dev/loro/issues/1040): shallow-snapshot edge cases (claim 2).
- [#1056](https://github.com/loro-dev/loro/issues/1056): `forkAt` lock-up in 1.13.7.
- [#1009](https://github.com/loro-dev/loro/issues/1009): rich-text mark boundary behavior.
- [#1112](https://github.com/loro-dev/loro/issues/1112): zero-sum counter diffs dropped.
- [PR #1116](https://github.com/loro-dev/loro/pull/1116): purging containers owned by deleted tree nodes. It "ensures… peers that never purge converge on the same value", which implies they currently may not.

**Strengths:**
- Extensive fuzzing (`crates/fuzz`, including tree-specific fuzz tests).
- **Rust/JS differential fuzzing** since July 2026 (PR #1047).
- Fast turnaround on fixes.
- Documented semantics for tree moves: cycles are skipped using Kleppmann's approach, deleted subtrees go to TRASH ([movable-tree blog](https://loro.dev/blog/movable-tree)).

**Semantics the app must accept:**
- A concurrent "delete parent" plus "add subtask under it" leaves the subtask deleted (it is in TRASH).
- A **deleted node cannot be moved back** with the public API: `get_index_by_tree_id` returns `None` when the parent is deleted, so `mov` fails with `TreeNodeDeletedOrNotExist` ([tree_state.rs](https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/state/tree_state.rs)).
- Concurrent creation of regular child containers under the same map key hides one of them. Use `ensureMergeable*` ([tips](https://loro.dev/docs/tutorial/tips)).

**Implications and fix:**
- Treat **maps as mature**, and **tree, movable list and shallow snapshots as mature but recently patched**. Pin ≥ 1.16.2.
- Run the design's simulated-network tests (§12a) **with Loro's own fuzz actions for tree and movable list** under drop, duplicate and reorder conditions.
- Soft-delete tasks instead of calling `tree.delete` (see N8).
- Model sibling order with the tree's fractional index, using `enable_fractional_index` with jitter > 0 to reduce concurrent-position collisions. Avoid a separate movable list for ordering unless it's needed.

### Claim 5: performance, memory, encoding size, history growth

**Published numbers** (desktop only; **no Android or low-end benchmarks exist**):

| Metric | Value | Source |
|---|---|---|
| Update blob that carries a single op (B1.x, N = 6000) | 88–100 B per update (Yjs: 27–36 B) | [docs/performance](https://loro.dev/docs/performance) (1.0.0-beta.2 run) |
| Snapshot bytes per op, list appends / random inserts | 71,568 B / 6000 ≈ **12 B/op**; 100,632 B / 6000 ≈ **17 B/op** (history + state) | same |
| Map set, every op from a **distinct peer** (B3.1, 1,549 peers) | 78,764 B ≈ **51 B/op**; update 132,376 B total ≈ **85 B/update** | same |
| Text trace (260k ops) | Snapshot 273,561 B (≈1.05 B/op), compressed 132,459 B; shallow 63,352 B | [docsize](https://loro.dev/docs/performance/docsize) |
| Import of a 1.66M-op real document | 1.15 ms (lazy snapshot); shallow snapshot 375 µs | [blog/v1.0](https://loro.dev/blog/v1.0) |
| WASM size | 3.27 MB (`loro_wasm_bg.wasm`, 1.16.3); ~0.9 MB gzipped at 1.0-beta | npm tarball; perf page |
| Native aarch64 `.so` (loro-py 1.16.2, includes pyo3 bindings) | 10.0 MB unstripped: `.text` 5.05 MB, `.symtab` + `.strtab` 3.2 MB, unwind tables ~1.1 MB | parsed ELF of the PyPI wheel |
| loro-react-native Android static libs | arm64 17.9 MB, armv7 12.5 MB (static `.a`, unstripped; not final size) | npm `loro-react-native@1.10.3` |

**Memory and performance cliffs fixed only in 2026:**
- **1.16.0:** "about **4 KB per container**" of decoded value was pinned for the document's lifetime on every per-container read. This was in the **core** InnerStore, not only WASM. It is now bounded to 2,048 cached entries. "Tree containers materialize a full State on first value read… tree walks still pin state" ([container-value-cache.md](https://github.com/loro-dev/loro/blob/main/context/container-value-cache.md)). A task app with one metadata map per task walks exactly this path.
- **1.16.1:** with any unmerged concurrent head, "400 small imports on a 2000-note doc… took **~2.4 s** natively; they now take ~70 ms". A low-end phone is 5–10× slower, which would have meant 10–25 s.
- **1.16.0:** shallow export on a container-heavy doc went from ~3.2 s to ~370 ms.
- **1.13.4:** fixed an O(n²) text-editing regression introduced in 1.12.0.
- **1.14.1:** `importBatch` of 12,000 out-of-order updates went from 3.4 s to 0.45 s.
- **PR #1058:** "a 1 KB import at 20 rounds took 86 s vs 168 µs" before the false-concurrency fix.

**Realistic growth estimate.** Derived from the numbers above, to be measured in E6:
- A heavy user making ~200 field edits a day produces ~73k ops a year. At 12–30 B/op in snapshots that is about 1–2 MB a year per heavily edited list, before compaction.
- Descriptions typed character by character are cheap: runs of about 1 B/char.
- The design's "single-digit MB" is plausible. **Per-session PeerIDs** make it worse: about 51 B/op when operations are spread across many peers, plus version-vector growth (N7).

**Proposed fix:**
- Require ≥ 1.16.2.
- Budget the native size at **~4–7 MB per ABI for Loro alone** (estimate). This fits §10's "Rust core 8–15 MB" only if iroh is at most ~8 MB.
- Run compaction and shallow export in a background job with a time budget.
- Never walk every container on the UI thread: use `get_deep_value` / bulk reads, which leave no cache residue.
- Measure on a Cortex-A53 device before phase 1 ends (E5, E6).

### Claim 6: PeerID uniqueness and authenticated authorship

**Uniqueness:**
- Loro's docs: "If PeerId repeats, there's a high probability of duplicate OpId, potentially leading to inconsistent CRDT documents" ([version deep dive](https://loro.dev/docs/advanced/version_deep_dive)).
- Reuse is allowed only if "you persist the document's local data… alongside that ID and load it before fetching or applying any remote updates… **Allow [never] multiple… processes to operate on the same reused Peer ID in parallel**" ([peerid_management](https://loro.dev/docs/concepts/peerid_management); [tips](https://loro.dev/docs/tutorial/tips)).
- The mechanism behind the silence: `OpLog::trim_the_known_part_of_change` drops any incoming range whose counters are at or below the local version vector, **without comparing content**. `check_id_is_not_duplicated` exists but is `#[allow(unused)]` ([oplog.rs](https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/oplog.rs)). `ConcurrentOpsWithSamePeerID` is raised only for local detached editing.
- A duplicated PeerID therefore diverges **silently** and permanently. [#944](https://github.com/loro-dev/loro/issues/944) (open, 1.10.8) reports that restoring "an older backed up loro doc – then tried to sync it with devices, it would keep crashing" with `assertion failed: slice_end > slice_from`. That is consistent with ID reuse, but the cause is not confirmed.

**How this design could reuse a PeerID:**
- Android Auto Backup restores app data (documents and any persisted PeerID) onto a new phone while the old phone still exists, or onto the same phone with a stale snapshot. The design relies on Auto Backup for solo lists (§11).
- A device image is cloned.
- Two desktop instances run at once.
- A crash loses the tail of the local log after ops were already sent. The design's save-before-send rule prevents this last one.

**Authentication:**
- None. Change blocks carry a peer table, counters, lamports, timestamps, deps and an optional message, with **no signature field** ([encoding.md §9–10](https://github.com/loro-dev/loro/blob/main/docs/encoding.md)).
- Anyone holding the group key can:
  - write operations under any PeerID, including crafted blobs or `import_json_updates`;
  - set any timestamp or commit message;
  - **pre-empt ("squat") a victim's future counters**, so every receiver later drops the victim's genuine operations as "already known", and the victim's replica diverges from everyone else's.
- The design's per-device Nostr signatures (§7) only prove who uploaded a slot. Slots legitimately carry other peers' operations (snapshots and relayed data).
- Loro does offer building blocks: `subscribe_pre_commit` (set a message per change) plus `export_json_in_id_span` (hash a change). The 1.5.0 notes show a Merkle-DAG example. **Signing changes this way is possible but not built in.**

**Proposed fix:**
1. **PeerID policy.** Generate a new random PeerID whenever continuity can't be proven. Store `(peer_id, last_counter)` in the **same atomic file** as the document, and keep a "lease" marker in `noBackupFilesDir` (Android) or a local, non-synced location (desktop). Rotate the PeerID when the marker is missing (a restore or clone), after crash recovery, and when the single-instance lock fails.
2. On receiving any operation for one of your own PeerIDs that you didn't write locally, **rotate immediately** and flag it in diagnostics. Random 64-bit IDs can't be predicted, which defeats squatting.
3. Map PeerID → device through the group document (the `first-commit-from-peer` hook), and accept that this map is claimed, not proven.
4. If authenticated "who changed what" is ever needed, sign each change's canonical hash with the device key via the pre-commit message, and verify on import (experiment E9).
5. Never write housekeeping data (last seen, sync markers) into group documents. Every such write creates operations and, with per-session PeerIDs, new peers.

### Claim 7: FFI bindings for Kotlin; WASM and Rust size

**What exists:**
- [loro-ffi](https://github.com/loro-dev/loro-ffi) 1.16.2, published 2026-09-21: UniFFI **0.31.1**, UDL-based (`src/loro.udl`). Its `uniffi.toml` has only a `[bindings.swift]` section.
- Listed bindings: Swift ([loro-swift](https://github.com/loro-dev/loro-swift), "**experimental** Swift bindings", 41 stars), Python (pyo3), React Native ([loro-react-native](https://github.com/loro-dev/loro-react-native), which **does build for Android arm64 and armv7** via uniffi-bindgen-react-native), plus community C# and Go.
- There is **no maintained Kotlin or Java binding**. A GitHub search finds only `moheng233/loro-java` (0 stars, last push 2026-05).
- The loro-ffi README expects each language to write its own wrapper layer (callbacks, `LoroValueLike`, a unified `export`).

**What the design needs:** almost none of this. The engine is Rust (§3) and should depend on the `loro` crate directly, exposing a **domain-level** UniFFI API (tasks, lists, sync) to Kotlin. Exposing Loro types across UniFFI would:
- couple Kotlin to Loro's churning API;
- require a UniFFI version lock-step between loro-ffi and the engine's own UniFFI crate;
- move CRDT semantics into the UI layer.

**Size:** see claim 5. Roughly 4–7 MB stripped per ABI for Loro (estimate from the 5.05 MB `.text` of loro-py). Note that with `minSdk` ≥ 23, AGP stores `.so` files uncompressed in the APK by default, so installed size ≈ stripped `.so` size.

**Proposed fix:** don't use loro-ffi. Keep Loro behind the engine API, and measure the `.so` in phase 1 with the planned profile (E5).

## New risks

| ID | Risk | Severity | Proposed fix |
|---|---|---|---|
| N1 | **Per-device compaction at its own head rejects other devices' in-flight concurrent edits forever.** The design (§10) lets each device compact under a local budget. Any member edit concurrent with that root which hasn't arrived yet is rejected (≥1.16.0) or panics (<1.16.0). Descendants of those edits sit pending silently. | **Critical** | Choose roots only at a causally stable version (all member devices acknowledged, within a window, minus a margin). Track acknowledged version vectors in the group document or in sync handshakes. Laggards take the explicit rebase path. |
| N2 | **Snapshot-slot catch-up gap.** Nostr bundles hold "changes since last snapshot" and are reset afterwards (§7). A device whose version is older than the latest shallow snapshot's root gets retained changes that stay pending forever when imported into its non-empty doc (#1040). Missing one snapshot rotation is enough; "long offline" isn't required. | **High** | Put the mailbox snapshot's root at the causally stable version, so active devices can always catch up incrementally. Detect stale devices (own version < root) and bootstrap into a **fresh** doc from the snapshot, then rebase local unsynced edits (N3). Alert on non-empty `pending` that persists across sync rounds. |
| N3 | **The rebase path inverts conflict outcomes.** Re-applying a stale device's old edits "as new edits" (§10) gives them fresh lamports, so they **win** LWW against newer edits made by others in the meantime. Positional edits (text, list) re-applied by value can land in the wrong place. | Medium–High | Do a three-way app-level merge: re-apply a field only if its value at the fork point equals the current group value (nobody else changed it). Otherwise keep the group value and surface a local "your offline change conflicted" note. For text, rebase with cursors or diff-match-patch, or keep both versions. |
| N4 | **Poison blobs and `panic = "abort"`.** §10 sets `panic = "abort"`. Any Loro panic on import or read crashes the app. The same blob arrives again on the next sync, so every member crash-loops. The 2026 history includes such panics (1.14.0 "permanently break a document's sync", #1068 mutex poisoning, #1088 "silent at import time and fatal at read time"). | **High** | Build the engine with `panic = "unwind"` so UniFFI can turn panics into exceptions. Wrap each document operation in `catch_unwind`. Write an "importing blob X" marker before each import and quarantine X after a crash. Validate snapshots (import into a fresh doc and do a full read) **before publishing and before accepting**. Keep the last-known-good local snapshot. |
| N5 | **PeerID reuse through Auto Backup, Block Store restore or device cloning** causes silent divergence or a crash (claim 6, #944). | **High** | PeerID lease marker in no-backup storage, rotate on restore; single-instance lock on desktop. |
| N6 | **Forged authorship and counter squatting** by any member (claim 6). "Who changed what" is unauthenticated, and a malicious or buggy member can make another member's future edits vanish for everyone. | Medium | Rotate on detection; random per-session IDs; optional per-change signatures via the pre-commit hook (E9). |
| N7 | **Version-vector and peer-table growth.** Random per-session PeerIDs (Loro's default) add a peer entry per editing session: roughly 1,800 a year per device for a heavy user. Version vectors are exchanged in sync and aren't shrunk by shallow snapshots. Ops from a fresh peer cost ~51 B versus ~12 B. | Low–Medium | Persist the PeerID per (install, document) with the lease marker from N5. Rotate only when continuity is uncertain. Use frontiers instead of version vectors in handshakes where possible. |
| N8 | **Undoing or restoring a deleted task changes its identity.** Undoing a tree delete re-creates the node with `__internal__next_tree_id()` and remaps its metadata map ([handler.rs](https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/handler.rs)). A deleted node can't be moved back (claim 4). Reminders, widgets, links and notifications keyed by TreeID break, and concurrent edits to the old node are lost. | Medium | **Soft-delete**: set a `deletedAt` field or move the task under an app-level "trash" node, so Undo is a field clear or a move and identity is kept. Call the real `tree.delete` only at the 30-day purge. Give every task an app-level UUID stored in its metadata and never use TreeID as an external key. |
| N9 | **Moving a task between lists crosses documents** ("one document per list", §4). The move becomes delete-in-A plus copy-into-B, which is not atomic. Concurrent moves to different lists **duplicate** the task. An edit made in A during a move is lost. History and unknown fields are dropped unless the copy is a generic deep copy. | High (a core feature of a task app) | Consider **one Loro document per group** holding all its lists as subtrees. Loro's lazy loading and 1.16 bounded caches make large documents cheaper, and moves become atomic tree moves. Otherwise build an explicit cross-document move protocol: tombstone with a forwarding pointer, deterministic winner for concurrent moves, and a generic deep copy of all keys. |
| N10 | **Licensing and supply chain.** `loro-internal` depends on `im 15.1.0` (MPL-2.0+, "im is unmaintained" RUSTSEC-2026-0248, unsoundness RUSTSEC-2023-0126 / RUSTSEC-2020-0096), `sized-chunks` (MPL-2.0+, **HIGH RUSTSEC-2020-0041, no patched version**, unmaintained RUSTSEC-2026-0251) and `bitmaps` (MPL-2.0+). This conflicts with principle 9 ("MIT, Apache or BSD") as written, and `cargo audit` / `cargo deny` will flag it. | Medium | Decide explicitly: MPL-2.0 is file-level copyleft and fine for a closed app if the (unmodified) MPL sources are offered. Add documented `cargo-deny` exceptions. Ask Loro upstream to replace `im` (for example with `imbl` or its own structures). Check whether Loro uses the vulnerable APIs. |
| N11 | **Pending changes are memory-only.** Out-of-order changes wait in the oplog's pending store. Nothing in the snapshot writer exports them (verify in E11), so a restart loses them. With replace-only Nostr slots, the missing dependency may never be resent. | Medium | Persist raw received blobs (content-addressed) until `ImportStatus.pending` no longer references them, and re-import them at startup. |
| N12 | **Threading and re-entrancy.** 2026 fixes include lock-order panics from re-entrant callbacks (1.12.0) and a "panic followed by a deadlock when… subscribing on one thread while another thread commits or imports" (1.16.2). | Low–Medium | Own each LoroDoc in a single engine actor thread. No Loro calls from UniFFI callbacks. Subscribers must not call back into the document synchronously. |
| N13 | **Timestamps are opt-in and unauthenticated.** "Who changed what when" needs `set_record_timestamp(true)`, which "do[es] not persist in exported Updates or Snapshots" and must be re-applied at every document open. Change merging (default interval 1000 s) coarsens timestamps. Clock skew or forgery produces misleading history. | Low | Set it in the engine's single document factory. Display timestamps as "claimed". Choose the merge interval deliberately. |
| N14 | **Concurrent creation of child containers hides one of them.** Examples: two devices add a description (a LoroText) to the same task offline; two devices' personal groups each initialise a child map at first launch before linking. | Medium | Use `ensureMergeable*` (≥1.13) for every lazily created child container. Never make an "initial state" commit (Loro tip; also the #1040 trigger). Link devices by app-level copying into the existing personal group, not by Loro import, because genesis changes are rejected by shallow replicas. |
| N15 | **Android 16 KB page-size requirement.** Play requires 16 KB-aligned native libraries for apps targeting Android 15+ (from 2025-11-01). This applies to the engine `.so` including Loro. | Low | Build with NDK r28+ or pass the linker flag `-z max-page-size=16384`, and check it in CI. |

## Suggested experiments

All of these are local, with no public relays involved. Pin `loro = "=1.16.2"` unless stated otherwise.

1. **E1: Cross-version compatibility matrix.**
   - Generate documents with every container type, mergeable children and counters using Loro 1.0.x, 1.10.8, 1.13.9 and 1.16.2.
   - For every ordered pair of versions: import the snapshot, shallow snapshot, `StateOnly` and updates, then re-export and re-import. Compare deep values.
   - Unknown-container cases: build one side with `default-features = false` (no `counter`), and use a pre-1.13 build as the "old" side for mergeable containers. Record where state is lost and where export fails with `UnknownContainer`.
   - This corpus becomes the permanent golden corpus.
2. **E2: Shallow-root placement simulation.**
   - Use the design's §12a simulator with 3–6 devices, random offline periods, and the Nostr slot model.
   - Compare compaction at (a) the local head, (b) a causally stable version, (c) stable minus a margin.
   - Count rejected changes, stuck-pending changes and panics.
   - Reproduce #1040, #1068 and the PR #1083 scenario 2 ("parked before shallow import, unlocked later") on 1.16.2.
3. **E3: Snapshot-slot catch-up and rebase.**
   - A device misses one snapshot rotation, then imports the shallow snapshot into its existing doc. Confirm the pending state.
   - Implement the fresh-doc plus three-way rebase. Measure how many LWW outcomes flip under naive rebase compared with three-way rebase.
4. **E4: PeerID reuse after restore.** Copy a device's app data to a second emulator while the original keeps editing with a persisted PeerID. Sync both, observe divergence or a crash (compare with #944), then verify that the lease-marker rotation prevents it.
5. **E5: Native size.**
   - Build the engine `.so` (Loro plus a stub UniFFI API) for aarch64, armv7 and x86_64 Android with `opt-level="z"`, `lto`, `codegen-units=1` and `strip`. Try both `panic="abort"` and `panic="unwind"`.
   - Record sizes and run `cargo bloat --crates`. Repeat with iroh linked in.
   - Verify 16 KB page alignment.
6. **E6: Workload realism.**
   - Build a synthetic one-year task workload (create, edit title, complete, reorder, move subtree, description typing, delete/purge) across 3 devices.
   - Measure bytes/op for individual updates, batched bundles, full snapshot, shallow snapshot and `StateOnly`, with persistent and per-session PeerIDs.
   - Then measure on a Cortex-A53/A55 phone: cold open, full `get_deep_value`, 400 small concurrent imports, shallow export, and peak RSS (`dumpsys meminfo`) at 1k, 5k and 20k tasks.
7. **E7: Checkpoint strategy.**
   - Compare N × `StateOnly` blobs against app-level JSON with zstd delta chains, for daily, weekly and monthly checkpoints over a year.
   - Confirm that `fork_at` / `SnapshotAt` fail on shallow documents in 1.16.2 and retest if PR #1115 lands.
8. **E8: Poison-blob resilience.**
   - Fuzz `import` with mutated real blobs under `panic="unwind"` plus `catch_unwind` through UniFFI.
   - Check whether the document stays usable after a caught panic, given `LoroMutex` poisoning, or must be reloaded from disk.
   - Test the quarantine marker across process restarts.
9. **E9: Forgery and squatting.**
   - Craft changes under another member's PeerID with future counters (binary or `import_json_updates`). Confirm that receivers drop the victim's later genuine operations, and that rotation on detection recovers.
   - Prototype per-change signatures via `subscribe_pre_commit` and measure the overhead.
10. **E10: Tree identity.** Confirm that undoing a delete yields a new TreeID (and reproduce #1055). Implement soft-delete and verify identity is preserved and that concurrent "add subtask to a trashed parent" survives.
11. **E11: Pending persistence.** Import an out-of-order update, export a snapshot, reload, and check whether the pending change is gone.
12. **E12: Moving tasks between lists.**
    - Two devices concurrently move the same task to different lists (separate documents), and one edits it mid-move. Measure duplicates and losses.
    - Compare with a single-document-per-group design, including load time and memory at 20k tasks.

## Sources

**Loro releases, packages and versions**
- GitHub releases (dates via API): https://github.com/loro-dev/loro/releases, including https://github.com/loro-dev/loro/releases/tag/loro-crdt%401.16.2 and https://github.com/loro-dev/loro/releases/tag/loro-crdt%401.16.0
- crates.io versions: https://crates.io/crates/loro/versions, https://crates.io/crates/loro-ffi
- npm `loro-crdt` 1.16.3 (WASM size): https://www.npmjs.com/package/loro-crdt
- npm `loro-react-native` 1.10.3 (Android static libs): https://www.npmjs.com/package/loro-react-native
- PyPI `loro` 1.16.2 wheels (native `.so` size): https://pypi.org/project/loro/

**Loro docs site**
- Loro 1.0 announcement: https://loro.dev/blog/v1.0
- Benchmarks and the compatibility statement: https://loro.dev/docs/performance ; document size: https://loro.dev/docs/performance/docsize
- v1.9.0 changelog (0.x encoding removed): https://loro.dev/changelog/v1.9.0
- Shallow snapshots: https://loro.dev/docs/concepts/shallow_snapshots
- PeerID management: https://loro.dev/docs/concepts/peerid_management ; tips: https://loro.dev/docs/tutorial/tips ; version deep dive: https://loro.dev/docs/advanced/version_deep_dive
- Mergeable containers: https://loro.dev/blog/mergeable-containers ; movable tree: https://loro.dev/blog/movable-tree ; timestamps: https://loro.dev/docs/advanced/timestamp
- Docs source repo: https://github.com/loro-dev/loro-docs

**Loro repository: specs and context notes**
- Normative wire spec: https://github.com/loro-dev/loro/blob/main/docs/encoding.md
- Container-state spec (unknown state §10): https://github.com/loro-dev/loro/blob/main/docs/encoding-container-states.md
- Shallow snapshot concurrency contract: https://github.com/loro-dev/loro/blob/main/docs/shallow-snapshot-concurrency.md
- JSON schema doc (forward/backward compatibility): https://github.com/loro-dev/loro/blob/main/docs/JsonSchema.md
- Context notes: https://github.com/loro-dev/loro/blob/main/context/internal-encoding.md , https://github.com/loro-dev/loro/blob/main/context/mergeable-containers.md , https://github.com/loro-dev/loro/blob/main/context/container-value-cache.md

**Loro repository: source**
- Cross-version tests: https://github.com/loro-dev/loro/blob/main/crates/fuzz/tests/compatibility.rs and https://github.com/loro-dev/loro/blob/main/crates/fuzz/src/container/counter.rs
- Import, dedup and shallow rejection: https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/loro.rs , https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/oplog.rs , https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/oplog/pending_changes.rs , https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/oplog/loro_dag.rs
- Shallow, `StateOnly` and `SnapshotAt`: https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/encoding/shallow_snapshot.rs
- Value kinds: https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/encoding/value.rs ; container types and root-name rules: https://github.com/loro-dev/loro/blob/main/crates/loro-common/src/lib.rs
- Tree handler and undo remap: https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/handler.rs , https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/handler/tree.rs , https://github.com/loro-dev/loro/blob/main/crates/loro-internal/src/state/tree_state.rs
- Dependencies: https://github.com/loro-dev/loro/blob/main/crates/loro-internal/Cargo.toml , https://github.com/loro-dev/loro/blob/main/deny.toml

**Loro issues and PRs**
- Issues: https://github.com/loro-dev/loro/issues/944 , /1040 , /1046 , /1055 , /1056 , /1068 , /1088 , /1092 , /1095 (all under https://github.com/loro-dev/loro/issues/)
- PRs: https://github.com/loro-dev/loro/pull/1058 , /1079 , /1083 , /1090 , /1104 , /1115 , /1116 (all under https://github.com/loro-dev/loro/pull/)

**Downstream and bindings**
- Production shallow-snapshot bug at Macro: https://github.com/macro-inc/macro/pull/6760
- Bindings: https://github.com/loro-dev/loro-ffi , https://github.com/loro-dev/loro-swift , https://github.com/loro-dev/loro-react-native , https://github.com/Deln0r/loro-go

**Security advisories**
- RustSec: https://rustsec.org/packages/im.html , https://rustsec.org/packages/sized-chunks.html
