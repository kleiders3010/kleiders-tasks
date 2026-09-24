# A7 + A8: Metadata leakage and invite-secret exposure

Date: 2026-09-24. **Analysis** of the current design (no live testing against third parties).

## A7: What observers can learn
| Observer | Learns | Worst realistic inference |
|---|---|---|
| Nostr relay | Per-(device, group) pubkeys, event timing, sizes, uploader IP; that N pubkeys share a slot "family" (same kind, similar timing) | IP + timing correlation → which IPs are in the same group, roughly how active it is, rough group size |
| iroh relay (n0) | Endpoint IDs, IPs of both ends, when they connect, bytes transferred | **Who syncs with whom** (social graph edges between devices) while traffic goes through the relay |
| n0 DNS/pkarr discovery | Endpoint IDs and current addresses of every device using it | A device's IP history over time |
| Mainline DHT | The group-derived discovery key and the IPs publishing/looking it up | Which IPs share a group (the key links them) |
| Helpers | Slot ciphertexts, sizes, timing, reader and writer IPs | Same as relays; plus which groups a reader fetches |
| STUN | IPs | minimal |

**Findings:**
1. **One stable endpoint ID per device across all groups links everything.** Anyone seeing the device's iroh ID in two contexts (discovery for group A and group B) learns both groups share a member. The per-group Nostr keys don't help if the iroh ID is global.
   - **Option:** a per-group iroh endpoint key.
   - **Cost:** multiple endpoints (sockets, connections) per device, and more battery. iroh endpoints are ~cheap, but N groups means N endpoints. **Not verified:** the actual battery/socket cost on Android.
   - **Alternative I evaluated:** accept the linkage, document it. The observers are relay operators and DHT watchers; they learn *that* devices are connected, never content.
   - Honestly this is a values choice, not a technical one.
2. **The DHT discovery key must rotate** (keyed with the group secret and the date), or it's a permanent, public group identifier that anyone can watch forever.
3. **Timing:** "Leaving the app flushes immediately" produces a burst of writes to every relay at the same instant, which is a correlation signal. It's inherent in low-latency sync and accepted.

## A8: Invite-secret exposure paths
The design puts the **permanent group secret** in the link's URL fragment and QR code. Paths where it leaks:

| Path | Leak |
|---|---|
| **Keyboard / IME clipboard history** (e.g. Gboard) | Copied links are kept in clipboard history; some keyboards sync clipboard across devices to the vendor's cloud |
| **Windows cloud clipboard** (Win+V sync) | Syncs clipboard content to the Microsoft account when enabled |
| **The design's own "copy invite to clipboard" install trick** | Deliberately puts the secret on the clipboard |
| **Browser history and sync** | The full URL *including the fragment* is stored and synced (e.g. Chrome sync) |
| **Chat apps** | Fragments aren't sent in link previews (good), but chat backups hold the link forever |
| **Screenshots / photo backups of QR codes** | Permanent copies in cloud photo libraries |

The common problem: **a leak at any time in the future grants permanent access**, because the link *is* the key.

**Proposal: links and QR codes carry a short-lived invite, not the group secret.** Evaluated with the same rigor as the current design:
- **How:** the same mechanism as invite codes, but with a full-entropy random key (no Argon2 needed). The link holds `invite-id + 256-bit invite key`. The inviter publishes the group secret, encrypted with the invite key, as an invite record on Nostr (+ helpers/P2P), expiring after N days. Joining fetches and decrypts the record.
- **Gain:** a link or QR code found later (history, backups, clipboard) is useless after expiry. Revoking an unused invite = deleting its record.
- **Costs:**
  - joining requires the invite record to be reachable (Nostr / helper / inviter online). Joining needs the network anyway to get any data, so the practical loss is small;
  - links stop working after N days, so old links in chats die. That's arguably a feature;
  - a little more engine logic, which reuses the invite-code path.
- **Current design (secret in link):** works fully offline for the key exchange, never expires, simpler. But every leak is permanent, including leaks through the app's own clipboard trick.
- **Recommendation:** short-lived invites for links and QR, keeping expiry consistent with codes (e.g. 1 day for codes; links perhaps 7 days, since they're shared asynchronously). Confidence: high on the security gain; the expiry lengths are a product call.
- **Also:** mark clipboard content sensitive (Android 13+ `ClipDescription.EXTRA_IS_SENSITIVE`) so previews and some keyboards don't display or store it. That's a mitigation, not a guarantee.
