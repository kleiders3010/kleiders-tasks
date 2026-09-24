# Decided

*Decisions made by the user during the research review. Each links back to its full analysis in `DECISIONS-FULL.md` (by ID). These are the basis for updating `DESIGN.md`.*

| ID | Decision | Date |
|---|---|---|
| D1 | Device identity (keys, PeerID) is never backed up; a restored or new install gets a fresh identity and re-joins the personal group; the desktop app runs as a single instance. | 2026-09-24 |
| D2 | One person = the personal group; one key per device underneath. Every update is signed; a device may publish only its own edits (checked on an isolated copy); the shared snapshot slot is dropped in favor of per-device slots + document epochs. | 2026-09-24 |
| D3 | Untrusted data is imported first in a disposable process; crash-loop breaker; `panic = "unwind"`. **No** upstream report to Loro. To measure later: the helper process's cost on low-end Android. | 2026-09-24 |
| D4 | **Soft** size limits: large updates are split into pieces; priority lanes (small edits first); text above ~100 KB becomes a compressed text attachment; per-author throttling instead of rejection; a hard stop only at absurd sizes, with a clear message. | 2026-09-24 |
| D7 + D8 (partial) | **One invite mechanism, three wrappers:** an invite is one 8-word code; the link (`jurisgames.com/join#…`) and the QR carry that same code, with the same expiry and a date-salted derivation. Device linking stays a **separate** flow, never accepted from the join path. *Still open: expiry length; confirming the EFF short list (D8 option a).* | 2026-09-24 |
| D19 / D20 | **Helper network is core, high priority, on by default on every device.** Options: *Wi-Fi and charging (+ Wi-Fi while open)* / *Wi-Fi while open* / *Off*. Off is easy but discouraged; a minimum level may go in the privacy policy if ever needed. Phones help too (the Play-policy fallback build is ready if needed). | 2026-09-24 |
| D38 | Android package `com.jurisgames.tasks`; invite links and App Links on jurisgames.com. Registration in Play Console in progress (SHA-256 fingerprint provided). | 2026-09-24 |
| D40 | Flathub not pursued. Distribution priority: Google Play (Android); GitHub Releases (PC, Linux, APK). | 2026-09-24 |
| D44 | Signing-key custody: keystore backed up to the private repo `jurisgames-signing-backup` (encrypted, no password); password in Credential Manager and in the user's password manager. | 2026-09-24 |
| Layers | **Preference order when several paths are available** (all still run in parallel). **Live:** same Wi-Fi → direct internet (IPv6, router-mapped, public IP, hole punch) → relay through a member's device → helper relays → n0 relays. **Offline:** member devices (incl. own always-on machine) → helpers → Nostr / second mailbox → own Google Drive (own devices only). Nearby for no-network. | 2026-09-24 |
