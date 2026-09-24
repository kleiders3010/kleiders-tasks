# E4: Nostr relay limits and acceptance (measured)

Date: 2026-09-24. Code: `research/experiments/e4_nostr_probe.py`; raw data in `research/experiments/e4/`.

## Published limits (NIP-11)
| Relay | Software | Max message | NIP-40 (expiration) | Notes |
|---|---|---|---|---|
| relay.damus.io | strfry | 1,000,000 B | yes | |
| relay.primal.net | strfry | 1,000,000 B | yes | |
| nos.lol | strfry | 131,072 B | yes | |
| nostr.mom | strfry | 131,072 B | yes | |
| offchain.pub | strfry | 131,072 B | yes | |
| nostr.oxtr.dev | strfry | 131,072 B | yes | |
| relay.snort.social | memlay | 524,288 B | no | |
| nostr-pub.wellorder.net | nostr-rs-relay | (not stated) | yes | no payment, open writes |
| nostr.wine | custom | 524,288 B | yes | **paid, restricted writes** |
| relay.nostr.band, relay.nostr.bg, relay.nostr.net | n/a | no usable NIP-11 | n/a | |

None of the free relays advertise a proof-of-work requirement. **Most relays run strfry**, so relay "diversity" is partly an illusion: one software's policy changes would hit most of them at once.

## Acceptance probe (polite)
Per relay:
- a brand-new throwaway key;
- 3 events, each with a 1-hour NIP-40 expiration, followed by a NIP-09 deletion:
  - kind 30078 (NIP-78 app data), 2 KB of opaque base64 content;
  - the same address again with a newer `created_at`;
  - a custom addressable kind 31234 with ~60 KB of opaque content.

8 relays in total.

| Relay | Small addressable | Replacement correct | 60 KB custom kind | Read back | Deletion |
|---|---|---|---|---|---|
| relay.damus.io | ✅ | ✅ | ✅ | ✅ | ✅ |
| nos.lol | ✅ | ✅ | ✅ | ✅ | ✅ |
| relay.primal.net | ✅ | ✅ | ✅ | ✅ | ✅ |
| nostr.mom | ✅ | ✅ | ✅ | ✅ | ✅ |
| offchain.pub | ✅ | ✅ | ✅ | ✅ | ✅ |
| nostr.oxtr.dev | ✅ | ✅ | ✅ | ✅ | ✅ |
| nostr-pub.wellorder.net | ✅ | ✅ | ✅ | ✅ | ✅ |
| relay.snort.social | ✅ | ✅ | ✅ | ✅ | ✅ |

## Verdicts
- **Confirmed (today):** major free relays accept brand-new keys, custom addressable kinds, opaque encrypted-looking content and 60 KB payloads, and replace addressable events correctly. The slot model (§7) works on real relays.
- **Size limit:** the common strfry limit is **128 KB per message**. With base64 overhead that's ~95 KB of payload. E1's 5-year busy list (155 KB state) needs 2 chunks, so **chunking is required**, as the design says. Fewer chunks become possible after document epochs (E1).
- **Use NIP-78 kind 30078** (application-specific data), not a custom kind. It's the standard, sanctioned place for app data, so relays are least likely to filter it.
- **Risk remains:** acceptance is a policy, not a guarantee. The monoculture (mostly strfry) means one popular policy change (e.g. a default web-of-trust plugin) could affect most relays at once. The runtime relay-health tracking and the helper network stay necessary.
- Local relay load tests were skipped: the questions they would answer (replacement semantics, size) are answered directly by real relays above.
