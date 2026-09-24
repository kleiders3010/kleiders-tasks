# E11: Organic sync session on public Nostr relays (measured)

Date: 2026-09-24, 17:02–17:11 (local). Approved by the user ("make sure that the traffic feels organic"). Script: two `kt` devices (debug build of commit `d71b02b` plus the TLS fix), one shared group "Home" with two lists plus a solo list, 8 edits alternating between the devices, a sync every 35–55 s, then the other device syncs 5–15 s later. The relays are the 8-relay baseline list (E4). The app's real write policy was on: 6 events/min/relay, burst 5, 3 relays per topic.

## Result
- **Every edit arrived**, both directions: add, done, delete and rename on both lists. The final lists on both devices are identical.
- **Kind 35711** (the dedicated slot kind, unregistered) was accepted everywhere it was written: relay.damus.io, nos.lol, relay.primal.net, nostr.mom, offchain.pub, nostr-pub.wellorder.net, relay.snort.social. nostr.oxtr.dev wasn't picked for these topics in this session (it was accepted in an earlier aborted run).
- **No refusals, no `rate-limited:` replies.**
- **Read-back 100%** on every relay written to: stored slots came back on the next read.
- **Traffic:** one slot rewrite per edit, sent to 3 relays, so about 30 events in 9 minutes spread over 7 relays. Unchanged slots were never rewritten; slot versions already read were never reopened.
- **relay.damus.io refused 3 of ~11 connections**, each time on the second device's sync a few seconds after the first device's. Both devices share one IP, so this looks like a per-IP connection limit. The per-topic relay subset (3 relays) covered it: no data was lost or delayed. *Not yet tested: whether it persists with longer gaps.*

## Incidents during the test (no effect on the result)
- The first attempt panicked in TLS setup (no rustls crypto provider). It was fixed by installing the `ring` provider, the one iroh already uses. No events were sent by that attempt.
- A stopped first run kept running in the background and overlapped the second for about 2 minutes (it used the same device folders). That doubled our write rate briefly, still a handful of events per relay. Both runs were killed and a clean third run produced the results above.

## Implications
- The Nostr mailbox design (§7) works on real relays today, with the real write policy.
- Relay health is per device and per relay. The per-IP limit at damus suggests two devices on the same home network should stagger their connections (they already do through jitter; a relay that fails to connect is just skipped for that round).
