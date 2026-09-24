# Pending decisions

*Only what's still open. Full analysis for each ID: `DECISIONS-FULL.md`. Already decided: `DECIDED.md`. Evidence: the `R*`, `L*`, `E*` and `A*` reports in this folder. Only alternatives that were actually evaluated are listed.*

Split in two:
- **Part A: yours.** Product, values, policy and outward contact.
- **Part B: technical.** I settle these with measurements and report the result and the reason. You keep a veto, but you're never asked to pick between options you'd have to take on faith.

---

# Part A: your decisions

*Nothing open right now (D51 decided 2026-09-24: see DECIDED.md).*

---

# Part B: technical, settled by me with data

| Item | Plan | Data needed | Status |
|---|---|---|---|
| **D49** Internet P2P and relaying without n0 | **Settled with data (E10):** build **(a) helper-hosted iroh relays** first and **(b) direct paths through our own signaling** alongside. **Don't build (c)**, the custom first path: it adds no direct coverage beyond (a)+(b) and depends on an unstable iroh API. | 13 NAT pairs × 3 runs, with 20 ms per hop | **Done.** (a) connected 13/13 pairs (39/39 runs), 8 direct and 5 relayed. (b) alone reached 8/13 at best, and only with IPv6, UPnP, a port-preserving NAT *and* both sides dialing at once; one-sided dialing got 3/13. Constraints for the build: helper relays need **IPv4 URLs** (iroh's bracketed-IPv6 URL bug is confirmed); a relay added at runtime works only if the endpoint started with an empty custom relay map; getting an IP certificate took ~1 s (local Let's Encrypt test CA). Untested for lack of time: libp2p and HyperDHT's randomized port punching (the only candidate for getting some of the 5 relay-only pairs direct). Confidence: high on iroh's behavior, medium on real networks. |
| **Select relay hosting** (D56 bonus, prioritized by user) | Evaluate free hosting for a developer-run relay: Oracle Cloud Always Free VM, Google Cloud e2-micro (real iroh relay), Cloudflare Workers + Durable Objects (WebSocket-only relaying, no address discovery: feasibility) | Terms, limits and resource needs. From the D49 lab: an iroh relay needs ~48 MB RAM, a public IPv4 address and UDP (so Cloudflare Workers can't host a full one) | **Next** |
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
