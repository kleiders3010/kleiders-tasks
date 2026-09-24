# Pending decisions

*Only what's still open. Full analysis for each ID: `DECISIONS-FULL.md`. Already decided: `DECIDED.md`. Evidence: the `R*`, `L*`, `E*` and `A*` reports in this folder. Only alternatives that were actually evaluated are listed.*

Split in two:
- **Part A: yours.** Product, values, policy and outward contact.
- **Part B: technical.** I settle these with measurements and report the result and the reason. You keep a veto, but you're never asked to pick between options you'd have to take on faith.

---

# Part A: your decisions

**D51. Should default-on helpers also relay *live* connections for strangers?** *(storage helping is already decided: on)*
- (a) Default-on, like storage helping: maximum coverage.
- (b) A separate opt-in relay level.
- (c) Own groups only.
- **The trade-off is exposure, not engineering:**
  - a relaying user's IP address and bandwidth carry strangers' encrypted traffic;
  - some ISPs' terms forbid running servers;
  - abuse complaints are possible;
  - desktops show a firewall prompt.
- How ISPs react at this traffic level is **not verified**.

**D14 + D50. How to treat free "best-effort" public services** (n0 relays, public STUN, IPFS relays, Send instances…)
- (a) Only services with written permission for any app.
- (b) Use them as non-critical parallel layers, never counted as a guarantee.
- (c) Ask operators in writing first.
- **Asking n0 or others is outward contact: your call.**

**D6. Device linking against phishing** *(agreed: link QR only accepted from "Your devices", a full-screen warning, "new device linked" alerts)*
- **Convenience vs safety:**
  - require the devices to be near each other (same Wi-Fi / Bluetooth) by default, which the synthesis recommends,
  - or keep that in reserve (my lean).
- **Never silently merge a device that already has lists?** This touches your earlier "linking merges your lists".

**D23 + D54. Privacy: what observers are acceptable?**
- Relays and DHT nodes can tell which groups one device belongs to (via IP and timing). Content is never visible.
- **Technical option for the network identity:** a separate identity per group. Its battery cost goes to Part B for measurement.
- **Your part:** which observers are acceptable, and how honest the privacy text should be. The draft states them all plainly.

**D29 + D53. Licensing: allow MPL-2.0 dependencies?**
- Loro already pulls in MPL-2.0 crates. MPL is file-level copyleft: fine for a closed-source app as long as those files aren't modified. It's also tied to whether you open-source later (DESIGN §14).
- (a) Allow MPL-2.0.
- (b) Stay MIT/Apache/BSD only, which means replacing Loro (not evaluated).

**D30. Android backup when a phone has no screen lock** (it isn't end-to-end encrypted then)
- (a) No automatic backup for those users.
- (b) A sealed backup, the same protection level you accepted for cloud backups.

**D45. People invited from an iPhone**
- (a) A web page they can join from (it syncs through Nostr only).
- (b) An iOS app later ($99/yr Apple fee).
- (c) Android + desktop only, stated on the invite page.

**D33. The same person on two unlinked devices showing up as two members**
- Proposal: an automatic "Is this your other device?" suggestion.
- Private nicknames stay dropped.

**D52. Images and large attachments**
- Product question: postpone for now, or offer an opt-in Google Drive store for people who want it?
- The technical options (iroh-blobs, Send instances, helpers) go to Part B once the product answer is known.

---

# Part B: technical, settled by me with data

| Item | Plan | Data needed | Status |
|---|---|---|---|
| **D49** Internet P2P and relaying without n0 | Network lab in WSL2 (Ubuntu is available): helper-hosted iroh relays with IP certificates; IPv6 / router-mapped direct; custom-transport variants | NAT lab matrix (LAYERS §6, P1–P2) | Next |
| **D48** Second offline mailbox | Mock mailbox interface, then local Session storage server and isolated Veilid network | LAYERS §6 P3 | After D49 |
| **D25** History checkpoints | Lean (b): full history in a window + small snapshots stored separately | Measure with Loro after document epochs exist | Engine phase |
| **D47 add-on** "More often" setting | Battery cost of idle-alarm checks | Emulator + real phone | Engine phase |
| **D23 technical** Per-group network identity | Battery and socket cost | Emulator | Engine phase |
| **The block** (no real alternative found; details in FULL) | Accept as specified | none | Accepted unless you veto |

**The block:**
- **Engine:** D5 key-rotation rules · D9 Argon2 64 MiB floor · D24 document epochs · D26 compact only at acknowledged versions · D27 ≥ 30 days of undo history + "undo changes by person" · D28 we own backward compatibility.
- **Nostr:** D10 dedicated event kind · D11 write budget, relay subsets, backoff, read-back checks · D12 40 KiB chunks.
- **Android:** D15 lower P2P expectations · D16 expedited-job flush + honest delay wording · D21 fix the permission table · D43 re-request exact alarms after restore.
- **Recovery:** D31 first-launch restore prompt, linking as default recovery · D32 periodic rotation limits stolen backups.
- **UX:** D18 "Saved → Sent → On Alex's phone" · D34 clipboard handling · D35 small UX fixes · D36 diary study + code-entry test.
- **Housekeeping:** D37 Play verification (in progress) · D41 a separate public repo for join page and releases.
- **Superseded:** D17 → D47 · D22 → D52 · D46 → D49.

---

## Later (not needed for launch)
- **D39 Windows distribution:** decide at the Windows release. Unsigned GitHub installers get a SmartScreen warning; the Microsoft Store is free.
- **D42 OneDrive:** postponed (Microsoft registration needs a credit-card check; an API bug since Aug 2026). Google Drive only.
- **Free hosting for operator-manifest select entries (D56):**
  - Cloudflare Workers/Durable Objects (a small Nostr relay or mailbox; no UDP, so no iroh relay);
  - Oracle Cloud Always Free VM (iroh relay + helper + Nostr relay);
  - Google Cloud e2-micro.

  **Not evaluated yet.**
- **Optional bonuses re-eligible under the clarified C2:** Cloudflare Realtime TURN free tier (developer key; bills above 1,000 GB/month) for failed hole punches; a doorbell worker only if D47 is ever reconsidered.

## Outward actions awaiting approval (none run)
- A Drive test on your Google account (D48d / D52).
- Small probes: Nostr ephemeral events on 3–4 relays, one STUN request per operator, a read-only attach to the Veilid network.
- Contacting n0, Session's foundation, chatmail or Send operators.
