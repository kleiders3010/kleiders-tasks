# E12: Invite and join over public networks (measured)

Date: 2026-09-24. Devices: this PC (A, `kt` debug build, default relay seed list) and the test tablet (B, Samsung SM-T290, Android 11, release build). Organic volume: one group, one list, a handful of events.

## Flow
1. A creates the group "Weekend trip" with a list, then `kt invite` (valid 3 days) → 8 words.
2. A syncs: relay discovery learns **188 candidate relays** from NIP-66 monitors and gives each group a relay set; the invite record is published for today plus copies for the remaining days.
3. B, a **fresh device knowing only the 8 words**, runs `kt join-code`: record found **via Nostr in 6.9 s** (including the Argon2id derivations on the slow tablet); join request posted. A's process wasn't running, so no direct join.
4. A syncs: sees the request, adds B (`DeviceJoined`).
5. B syncs: `joined 'Weekend trip' (invited by Alex)`, reads the list.
6. Edits both ways over public relays: B → A and A → B arrived on the next sync.

## Findings
- Works end to end on public infrastructure, with the self-sustaining relay list active.
- Bug found by the multi-process flow and fixed before this run: the invite key lived only in memory, so a joiner whose app restarted before being let in could read nothing. Now persisted with the group; regression test added.
- The day copies ahead are published within the Nostr write budget (a few per sync; the rest on later syncs), as intended.
