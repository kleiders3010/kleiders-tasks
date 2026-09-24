# E5b: Helper network with realistic daily patterns

Date: 2026-09-24. Code: `research/experiments/e5b_diurnal_helpers.py`. Models the **decided** design (D20): helpers on by default on all devices, desktops plus phones helping while charging on Wi-Fi.

**Model (21 days, hourly):**
- **Desktops:** on 08:00–24:00 local on 70% of days.
- **Phones:** help while charging overnight, reachable 70% of those hours (gaps between background jobs, R4).
- **RAM-only:** any restart wipes a helper's copies.
- **Helper sets:** 10 helpers per group, chosen by measured reliability.
- **Member refresh:** members re-upload during their own daytime.
- **Reader:** tries every hour for 24 h after a write.
- Every simulated group lives in time zone 0.

## Results
| Scenario | Overall | Worst hour (local) |
|---|---|---|
| 5 / 10 / 20 desktops, one time zone, daily reboot | 57–58% | **0%** at night |
| 10 desktops spread over 3 time zones | 94% | 79% |
| 10 desktops + 200 phones (charging 23–07), one zone | 85% | **0%** at 07:00 (handoff gap) |
| + graceful handoff (a leaving helper passes its copy to an online set member) | 84% | 0% |
| + phones' window 22–08 (overlaps desktops' 08:00 start) | 89% | 26% |
| **+ desktops sleep instead of rebooting (reboot 1 day in 7)** | **97%** | **93%** |
| Same, helpers spread over 3 zones | 96% | 86% |
| **Small community: 5 desktops + 50 phones, same setup** | **94%** | **90%** |
| 200 phones, no desktops | 14% | 0% |

## What this shows
1. **Count doesn't fix correlated downtime.** 5, 10 and 20 desktops in one time zone perform the same: all off at night.
2. **Phones charging overnight are the natural complement to desktops,** which is exactly the case your default-on decision enables. Desktops cover the day, phones the night.
3. **The weak moment is the handoff.** Data held only in RAM dies when the night shift ends before the day shift starts. What fixes it:
   - **overlap** between phone and desktop windows (phones help until they leave the charger, often after 08:00);
   - **desktops keeping RAM across the night** (sleep, not shutdown);
   - **handoff before going offline** (only useful if someone is online to receive it).
4. **Phones alone can't carry it** (14%): too little reachable time. A handful of desktops is essential.
5. With these behaviors, **a small community (5 desktops + 50 phones) reaches ~94% around the clock.** Nostr and member devices run in parallel for the rest.

## Implications for the design (proposals)
- Desktop helpers keep data across sleep. That's automatic, since the process is suspended with RAM intact. On shutdown or quit, hand copies to online set members first.
- Phones hand off before a charging job ends, and keep helping while on Wi-Fi after unplugging if the setting allows ("Wi-Fi while open" covers morning use).
- Helper selection should prefer **complementary schedules** (day desktops + night phones) over raw uptime. Uptime-only selection picks only desktops, which are all off at night.
- Optional small **encrypted disk persistence for helpers**, e.g. a few MB, to survive reboots, would remove the handoff dependency. This conflicts with your earlier RAM-only choice (avoiding SSD writes, not storing strangers' data on disk), so it's listed only as an option.

**Limits:** a simplified model. Real schedules vary more, members in several time zones weren't modeled, and phone reachability during charging is an estimate (R4).
