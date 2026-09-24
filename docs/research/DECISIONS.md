# Pending decisions

*Only what's still open. Full analysis for each ID: `DECISIONS-FULL.md`. Already decided: `DECIDED.md`. Evidence: the `R*`, `L*`, `E*` and `A*` reports in this folder. Only alternatives that were actually evaluated are listed.*

Split in two:
- **Part A: yours.** Product, values, policy and outward contact.
- **Part B: technical.** I settle these with measurements and report the result and the reason. You keep a veto, but you're never asked to pick between options you'd have to take on faith.

---

# Part A: your decisions

**D51. Should default-on helpers also relay *live* connections for strangers?** *(storage helping is already decided: on)*
- *User direction: relay capability must exist; a developer-run select relay (your PC or a free server) is the early guarantee. Final choice waits for the D49 lab data on how many helpers are reachable.*
- (a) Default-on, like storage helping: maximum coverage.
- (b) A separate opt-in relay level.
- (c) Own groups only.
- **The trade-off is exposure, not engineering:**
  - a relaying user's IP address and bandwidth carry strangers' encrypted traffic;
  - some ISPs' terms forbid running servers;
  - abuse complaints are possible;
  - desktops show a firewall prompt.
- How ISPs react at this traffic level is **not verified**.

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
| **Select relay hosting** (D56 bonus, prioritized by user) | Evaluate free hosting for a developer-run relay: Oracle Cloud Always Free VM, Google Cloud e2-micro (real iroh relay), Cloudflare Workers + Durable Objects (WebSocket-only relaying, no address discovery: feasibility) | Terms, limits and resource needs, informed by the D49 lab | **Queued right after D49** |
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
- Contacting Session's foundation, chatmail or Send operators (n0: not needed, D50).
