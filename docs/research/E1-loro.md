# E1: Loro history, compaction, trimmed-history imports, peer IDs (measured)

Date: 2026-09-24. Loro 1.16.2. Code: `research/experiments/rust/e1_loro` (`src/main.rs`, `src/bin/e1b.rs`). Desktop release build.

## Results

### 1. History growth (shared shopping list, 10 edits/day, 2 devices, ~20 live items)
| Years | Full snapshot | State-only | Ops |
|---|---|---|---|
| 1 | 70 KB | 31 KB | 5.6k |
| 3 | 211 KB | 92 KB | 16.9k |
| 5 | 353 KB | 155 KB | 28.2k |

**The size estimate held and was pessimistic:** 5 years of daily use is ~0.35 MB, not 1–3 MB.

### 2. Deleted items never leave the state (new finding)
With ~20 live items and heavy churn over 2 years:
| Data model | State-only | Full snapshot |
|---|---|---|
| LoroTree | 114 KB | 250 KB |
| LoroMap of maps | 171 KB | 324 KB |
| LoroMovableList of maps | 116 KB | 217 KB |
| **Fresh document with only live data** | **1.1 KB** | **1.7 KB** |

In every Loro model, deleted items (and their child containers) stay in the document state, needed for concurrent-edit semantics. **Shallow snapshots trim history but not this.**

**Consequences:**
- "Deleted items stay in trash 30 days, then are purged at compaction" is **wrong as written.** Loro can't purge them from a live document.
- The only real compaction is **re-creating the document from its live state**: a "document epoch", like key epochs.
- Sizes stay small for years, so this can be rare: triggered when state is many times larger than the live data *and* above a size floor.

### 3. Importing updates that depend on trimmed history
- A device compacts with a shallow snapshot. A device that went offline *before* that point sends its offline edits → **import fails**: "The dependencies of the importing updates are not included in the shallow history of the doc."
- The offline device *can* import the other's shallow snapshot.
- **Rebase path** (re-apply the offline edits as new ops on the new state) works.

**The design's long-offline rebase path is confirmed necessary, and it works.** The same path serves document epochs (§2).

### 4. State checkpoints at old versions
`fork_at(frontiers)` and `StateOnly(Some(frontiers))` work, so the "thinning history into checkpoints" plan is technically supported. Because of §2, each checkpoint carries all tombstones up to its date (13 monthly checkpoints of a busy list ≈ 248 KB). Checkpoints should be taken from document-epoch starts, or kept few.

### 5. Large lists
10,000 tasks: created in 31 ms, snapshot 445 KB exported in 26 ms, loaded in 28 ms (desktop). Low-end phones are maybe 5–10× slower: still well under a second.

### 6. Two devices sharing a PeerID: silent divergence (critical)
Two docs with the same PeerID made different edits and exchanged snapshots. **Both imports "succeeded", and the documents permanently disagree** (A shows "from A", B shows "from B"). No error.

**Real-world ways this happens in our design:**
- **Auto Backup / device-to-device restore** copies the app's data to a new phone, including any persisted PeerID. The old and new phones then share it.
- Two instances of the desktop app running at once.
- Restoring a full backup on a second device.

**Fix (mandatory):**
- The PeerID is random per *document session*, or at least per install.
- It's never stored in backed-up storage (Android: `noBackupFilesDir`), and is regenerated after any restore or import.
- The desktop app is single-instance.

### 7. Typing history
13k random keystrokes, each committed separately: state 12.5 KB, full history 27 KB (2.2×). **Character-level history is not a real storage risk;** Loro merges consecutive inserts efficiently.

## Verdicts
| Design claim | Verdict |
|---|---|
| Text data with history stays single-digit MB for years | **Confirmed** (0.35 MB after 5 years of heavy use) |
| Trash purged at compaction | **Refuted**: needs document re-creation (document epochs) |
| Long-offline devices need a rebase path | **Confirmed**, and the rebase path works |
| History thinning via checkpoints is possible | **Confirmed** (fork_at / StateOnly at frontiers), costlier than assumed because of tombstones |
| Large lists perform fine | **Confirmed** on desktop; verify on a low-end phone |
| (new) PeerID reuse silently corrupts sync | **Critical new risk**: fix above |
