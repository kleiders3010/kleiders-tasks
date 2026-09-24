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

## Independent verification

*Verifier pass, 2026-09-24. Method: instead of rereading the report's sources, the claims were tested against **running code**: Rust `loro` 1.16.2 built locally (`--offline` from the cargo cache), the same crate built **without** the `counter` feature to act as an "old client", and the published npm `loro-crdt` 1.0.9, 1.10.8, 1.13.7, 1.13.8, 1.13.9 and 1.16.3 WASM builds (tarballs unpacked to a scratch folder and loaded from Node; nothing installed). Also checked: the cached published crate source (`loro-internal-1.16.2`), the crates.io and npm version APIs, the GitHub issue, PR and release APIs, and the RustSec advisory pages. Android size came from arm64 builds made with `cargo ndk`. All code is in `research/experiments/rust/r3_verify/`. Nothing touched a public relay.*

### Verdicts

| # | Claim | R3 verdict | Upheld? | Verifier's verdict |
|---|---|---|---|---|
| 1 | Stable binary encoding across versions | Partial | **Yes** | Partial. It is **more stable in practice** than R3 showed, and still carries no "forever" promise. |
| 2 | Shallow snapshots, dependent imports, checkpoints | Partial | **Yes, stronger** | Partial. Every boundary behavior reproduced. An abort path that R3 marked "may still abort" **does abort on 1.16.2**. |
| 3 | Old clients preserve newer data | Partial | **Yes, worse** | Partial. The "re-encode discards the opaque payload" case **reproduced**, and **R3's own fallback fix triggers it**. Mergeable containers are fine on old clients. |
| 4 | Tree / movable list / maps / rich text production-ready | Partial | **Yes, with corrections** | Partial. #1055 and #1068 no longer reproduce on current versions. PR #1116 is not a convergence bug. |
| 5 | Performance / memory / size on low-end devices | Unverified | **Partly wrong** | Low-end performance is still unverified. The **native size estimate is 2–4× too high**. Measured per-op sizes match. |
| 6 | PeerID uniqueness; unauthenticated authorship | Confirmed | **Yes** | Confirmed. Silent divergence and counter squatting reproduced. |
| 7 | FFI for Kotlin; WASM/Rust size | Partial | **Yes, size corrected** | Partial. The Loro part of the `.so` measures **1.4–1.9 MB** on arm64, not 4–7 MB. |

### What was checked, per claim

**Claim 1 (encoding stability): upheld, with more positive evidence.**
- The version facts hold. crates.io shows `loro` 1.13.9 → 1.16.0 → 1.16.2, with no Rust 1.14.x, 1.15.x or 1.16.1 (1.9.0 is yanked). npm has 1.14.0–1.16.3. Several fixes R3 cites by version number (1.14.0, 1.14.1, 1.15.x, 1.16.1) are **JS-only release numbers**. In Rust they first ship in 1.16.0 or 1.16.2, so "require ≥ 1.16.2" is the right Rust floor.
- **Cross-version round trips, both directions, with real old builds:**
  - Old readers: Rust 1.16.2 wrote snapshots, updates, shallow snapshots and state-only blobs (tree with moves, meta text, map with binary, movable list with move and set, list, text). `loro-crdt` **1.0.9** and **1.10.8** imported every one of them and produced identical values.
  - New reader: blobs written natively by 1.0.9 and 1.10.8 (tree, text, movable list, counter), and blobs re-exported by those old clients, imported into 1.16.2 with the same values. That is 46 of 54 cases. The other 8 are expected pendings: updates exported from a shallow-based doc can't be imported into an empty doc.
- **The code confirms the limits R3 described:**
  - `EncodeMode` accepts only modes 1–4. Any other mode is `IncompatibleFutureEncodingError`, so Loro has an explicit "future format" failure path.
  - Modes 1 and 2 return `ImportUnsupportedEncodingMode`.
  - `LoroValueKind::from_u8` accepts only tags 0–9. Anything else is `DecodeDataCorruptionError`.
- **Net:** past compatibility is demonstrated, and a future guarantee is not given. R3's fix (exact pin, golden corpus, feature floor) stands. **The corpus can be seeded with the npm builds**, as done here, with no need to vendor old crates.

**Claim 2 (shallow snapshots): upheld; several points upgraded from "documented" to "reproduced on 1.16.2".**
- **Partial apply is real.** A blob held a valid post-root change and a change concurrent with the root. The result was `Err(ImportUpdatesThatDependsOnOutdatedVersion)`, **and** the valid change was applied.
- **Pending forever.** The rejected peer's next change imports as `Ok` with `pending = {2: 1..2}` and never shows up.
- **A never-synced peer's genesis change is rejected** with the same error.
- **A stale device can't catch up from a shallow snapshot**, even with no local edits. Importing the snapshot into its existing doc returns `Ok`, puts every retained change in `pending`, and leaves the value unchanged. This is R3's N2, reproduced. It also means E1 §3 ("the offline device *can* import the other's shallow snapshot") is **wrong**: E1 checked only `Ok` and never checked the value or `pending`.
- **The root lands far lower than requested.** With one unmerged op from another peer, asking for a root at `[50@1]` gave a root at `[0@1]`, the fork point, so all 50 ops were kept. After the compacting device made one local commit (a merge, single head), the root landed exactly where requested. **Compaction does nothing until the compacting device commits a change after merging.** That conflicts with R3's "never write housekeeping into group documents" (see M4).
- **Checkpoints:**
  - `fork_at` and `SnapshotAt` fail on a shallow doc even at its head, with `Not implemented: fork_at on shallow docs`.
  - `checkout` below the root gives `SwitchToVersionBeforeShallowRoot`. Above the root it works.
  - `StateOnly(after root)` works. `StateOnly(before root)` gives `FrontiersNotFound`.
  - E1 §4 ("fork_at works") is true only on **non-shallow** documents.
- **Correction on #1068.** The issue's exact reproduction **returns a clean `Err` on 1.16.2**; PR #1090 in 1.16.0 fixed it, and the issue was simply never closed. R3's "#1068 … still open" is accurate about the issue tracker but overstates the risk.
- **PR #1083 case 2 aborts the process on 1.16.2 (reproduced).** Sequence:
  1. A fresh doc receives a change whose dependency it doesn't have yet, so the change is parked.
  2. The doc imports a shallow snapshot whose root trims that dependency.
  3. The doc imports any valid later change from the dependency's peer.

  Result: `pending_changes.rs:338` `unwrap()` on `Err(())`, then `poisoned LoroMutex` while unwinding, then **process abort**. This is exactly the order in which a new or stale device can fetch the design's Nostr slots (a member-device slot before the snapshot slot).
- **Recovery tested** with `panic = "unwind"`. `catch_unwind` around the import, plus either `mem::forget` of the doc or dropping it inside a second `catch_unwind`, kept the process alive. Dropping it normally aborts. This refines R3's N4 fix: after a caught panic, the doc must never be reused or dropped normally.

**Claim 3 (old clients preserve newer data): upheld, and the "state can be lost" sub-point is real and worse than R3 treated it.**
- **Old-client setup:** a build without the `counter` feature imported a doc containing counters (a root counter and a counter inside a map). The import succeeded, and the counters read as `Null`.
  - `StateOnly` and `ShallowSnapshot` exports then fail with `UnknownContainer` (confirmed).
  - Full `Snapshot` and `Updates` exports succeed.
- **Data loss reproduced.** The old client **built its doc from updates**, not from a snapshot, and then exported a full snapshot. A new client importing that snapshot shows `cnt = 0.0` instead of 3.0, and the root counter `c` (5.0) **disappears**. The new client's state then **disagrees with a replay of its own oplog** (`state-path == op-replay: false`). Replicas that bootstrapped from that snapshot therefore diverge permanently and silently from replicas that received updates. When the old client's doc came from a snapshot, the values survived.
- **R3's fix #2 is harmful as written.** It says "old clients catch `UnknownContainer` … and fall back to a full snapshot", and that full snapshot is exactly the corrupting blob. Replacement rule: **a client must never publish a snapshot of a document that contains a container type it doesn't know. It publishes updates only.** Newer clients should prefer updates over snapshots from older app versions, or check a received snapshot by replaying it before trusting its state.
- **Mergeable containers are safe on old clients (tested on 1.0.9 and 1.10.8).**
  - They are not unknown *types*. An old client sees an 8-byte binary marker under the key, plus extra roots named `🤝:$meta>body`.
  - It imports, edits, re-exports and even shallow-exports them without loss, and 1.16.2 resolves them correctly afterwards.
  - The only cost is that an old app's UI sees binary junk and extra roots, so old app versions must ignore `🤝:` roots and binary values they don't expect.

**Claim 4 (production readiness): upheld as partial, with three corrections.**
- **#1046** (movable-list panic): its repro **passes and converges on 1.16.2**, which confirms R3's claim that PR #1058 fixed it.
- **#1055** (undo after import deletes the original tree node): **reproduces on `loro-crdt` 1.13.7 and 1.13.8, and not on 1.13.9 or 1.16.3** (the issue's exact JS repro), nor on Rust 1.16.2. It is fixed. R3's "still open" is true only of the tracker, so drop it from the list of live risks.
- **PR #1116** is a fix to **purging**, not to an existing convergence bug. `delete_root_container` couldn't purge a mergeable container whose owning tree node was deleted; see M1. R3's reading ("implies they currently may not converge") is not supported. The PR's "peers that never purge converge" describes the new purge ops.
- **Confirmed:**
  - Moving a deleted node back gives `TreeNodeDeletedOrNotExist`.
  - Undoing a delete leaves the original ID deleted and creates a new node (`0@21` became `3@21`).
  - The soft-delete advice (N8) stands.

**Claim 5 (performance and size): the size estimate is wrong; low-end performance is still unverified.**
- **Native size, measured on arm64 Android** with the design's release profile (`opt-level = "z"`, LTO, `codegen-units = 1`, `panic = "abort"`, `strip = true`) and NDK r29:

  | Build | Stripped `.so` size | Loro's share |
  |---|---|---|
  | Crypto-only baseline | 304,056 B | none |
  | Baseline + Loro (E6 probe) | 1,735,904 B | **≈ 1.43 MB** |
  | Loro with a broad API surface (tree, meta text with marks, counter, lists, undo, subscriptions, JSON updates, `fork_at`, checkout, diff, shallow / state-only / snapshot export, `import_batch`) | 1,885,720 B | **≈ 1.9 MB** |
  | Baseline + Loro at `opt-level = 3` | 2,509,888 B | **≈ 2.2 MB** |

  R3's "4–7 MB per ABI" came from loro-py, a build with pyo3 bindings and a different profile, and is **2–4× too high**. Loro fits §10's "Rust core 8–15 MB" easily.
- **The WASM size is confirmed** at 3,269,925 B (`loro-crdt` 1.16.3).
- **Per-op sizes, measured and consistent with R3:**
  - A single map-set update is **92 B** from a persistent peer and **101 B** when every op comes from a new peer.
  - Snapshot cost for map sets is 3.9 B/op from one peer (keys overwritten) against **36.7 B/op with a new peer per op**.
  - Creating a tree node and setting its title is a 115 B update.
- **The cache fix is in core code:** the bounded 2048-entry value cache lives in core `loro-internal` (`state/container_store/inner_store.rs`), not in the WASM layer.
- **Performance on a real low-end phone is still unmeasured.**

**Claim 6 (PeerIDs): upheld.**
- **Counter squatting reproduced.** A member imported V's history, called `set_peer_id(V)` and committed. Receiver R accepted the forged op. V's genuine op at the same counter was then **silently dropped** at R, while V's next op was accepted. V and R diverge permanently, and V never learns of the forged op (`converged=false`).
- **The mechanism is in the code:** in the published crate source, `trim_the_known_part_of_change` compares counters only, and `check_id_is_not_duplicated` is `#[allow(unused)]`.
- **The documentation matches:** the loro-docs source for PeerID reuse requires "persist the document's local data … and load it before fetching or applying any remote updates" and says never to let parallel processes share a PeerID.

**Claim 7 (FFI): upheld.**
- `loro-ffi`'s `uniffi.toml` has only a `[bindings.swift]` section.
- A GitHub search finds no Kotlin binding; `moheng233/loro-java` has 0 stars and was last pushed 2026-05-09.
- **Size is corrected** as in claim 5.
- **Built `.so` files are already 16 KB-aligned** (every `LOAD` segment has `p_align = 16384`) with the NDK r29 already installed, so N15 is handled by the toolchain. Keep a CI check anyway.

### Missed or wrong in R3 (and in the sibling E1 report)

- **M1. Shallow snapshots do purge tombstones, except mergeable children.** This overturns part of E1 §2 and conflicts with R3's `ensureMergeable*` advice.
  - Test: E1's two-year churn run again (about 20 live items; 5 added and 5 deleted per day), adding a shallow snapshot at head, which E1 never measured.

    | Model | State-only | Shallow snapshot at head |
    |---|---|---|
    | LoroTree | 113 KB | **1.2 KB** |
    | Movable list of maps | 116 KB | **1.1 KB** |
    | Map of maps | 170 KB | 25.9 KB (deleted-key tombstones stay) |
    | **Tree with one mergeable text per node** | 196 KB | **129.8 KB** |

    A fresh document holding only the live data is 1.1 KB, and every shallow import had the right value.
  - So "deleted items purged at compaction" **does work** for tree and movable-list models when the compaction is a shallow snapshot at a causally stable head. E1's "Refuted: needs document re-creation" was based on `StateOnly`, which keeps dead containers' state.
  - Mergeable child containers are **root** containers and are never trimmed. `delete_root_container` on one whose owning node is already deleted fails in 1.16.2: it prints `Failed to clear handler: ContainerDeleted`, returns nothing, and the content stays in the shallow snapshot. That is PR #1116, still open.
  - **Fix:**
    - Purge each task's mergeable children (`delete_root_container`, which works while the owner is alive) **before** calling `tree.delete` at the 30-day purge.
    - Or wait for PR #1116.
    - Or keep plain child containers where concurrent creation is impossible, for example ones created together with the node.
  - Also, **`StateOnly` is a poor checkpoint format.** It keeps dead-container state (113 KB against 1.2 KB), so R3's "store checkpoints as `StateOnly` blobs" should use shallow-at-version or app-level JSON.
- **M2. The old-client snapshot fallback corrupts data** (claim 3 above). This is the most important correction to R3's fixes.
- **M3. The PR #1083 abort is live on 1.16.2**, and the design's slot fetch order can trigger it. **Fix:**
  - Bootstrap from the snapshot slot into a **fresh** doc **before** importing any device slots.
  - Never import a shallow snapshot into a doc that has parked changes; start a fresh doc, then re-import the buffered blobs.
  - Build with `panic = "unwind"`, wrap every Loro call in `catch_unwind`, quarantine the doc (never reuse or drop it normally), and reload it from disk.
  - Watch PR #1083.
- **M4. An unmerged concurrent head pins the shallow root to the fork point**, so compaction is silently ineffective. The compacting device needs a single-head frontier, which means a local commit after merging. The design has to choose between a tiny app-level op at compaction time and waiting for organic edits; R3's "never write housekeeping data" would need an explicit exception here. After every shallow export, check `shallow_since_frontiers()` against the requested root.
- **M5. Pending changes are lost on restart (N11 confirmed).** An out-of-order update was pending, the doc was exported as a snapshot and reloaded, and the missing dependency was then delivered. The pending change was **gone** (value `{k1}`, not `{k1, k2}`). R3's persist-raw-blobs fix is required.
- **M6. RustSec facts in N10 are wrong in the alarming direction.**
  - **RUSTSEC-2020-0041** (HIGH) affects `sized-chunks` < 0.6.3, patched in ≥ 0.6.3. Loro uses **0.6.5**, so it does **not** apply.
  - **RUSTSEC-2020-0096** (`im`) is patched in ≥ 15.1.0, and Loro uses 15.1.0.
  - What does apply is informational only: `im` unmaintained (2026-0248), `im` `OrdSet` unsoundness (2023-0126), `sized-chunks` unmaintained (2026-0251), and **RUSTSEC-2026-0255**. That last one is a `sized-chunks` panic-safety unsoundness with no patch, "reachable … using `catch_unwind` with types whose destructors can panic", which matters because M3's fix introduces `catch_unwind`.
  - The MPL-2.0+ licensing point stands.
- **M7. The sibling E1 report needs corrections:**
  - §3: the offline device's shallow import returns `Ok` but applies nothing.
  - §4: `fork_at` works only on non-shallow docs.
  - §2: tombstones, per M1.

### Reproduction

Code and a README are in `research/experiments/rust/r3_verify/` (crates `v3`, `v3nc`, `v3ab`, `v3x`, `e6full`, and Node scripts in `js/`).
