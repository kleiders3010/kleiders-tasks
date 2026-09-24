# R3 independent verification (loro 1.16.2)

Code behind "Independent verification" in `docs/research/R3-loro.md`. All local; no relays.

- `v3`: shallow-snapshot boundary cases, root placement, PeerID reuse/squatting, tree delete/undo,
  pending persistence (`src/main.rs`); `src/bin/issues.rs` (#1046, #1055, update sizes);
  `src/bin/purge.rs` and `src/bin/churn.rs` (tombstones vs shallow@head, mergeable purge);
  `src/bin/rb2.rs` (reads blobs re-exported by the no-counter build).
- `v3nc`: the same Loro built with `default-features = false` (no Counter), acting as an old client.
- `v3ab`: #1068 and PR #1083 abort repros (`1068`, `1083c` aborts on 1.16.2, `1083d`/`1083e` catch_unwind recovery).
- `v3x` + `js/xtest.js`: cross-version corpus between Rust 1.16.2 and loro-crdt 1.0.9 / 1.10.8 (npm tarballs
  unpacked to `js/v<ver>/`, no npm install). `js/i1055.js`: #1055 on 1.13.7 / 1.13.8 / 1.13.9 / 1.16.3.
- `e6full`: broad-API Loro cdylib for arm64 size (`cargo ndk -t arm64-v8a build --release`).

Run: `cd v3 && cargo run --release -- <blobdir>`, then `cd v3nc && cargo run --release -- <blobdir>`,
then `cd v3 && cargo run --release --bin rb2 -- <blobdir>`.
