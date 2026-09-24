"""E5b: helper availability with REALISTIC daily patterns (the user's decided design).

Question: with only a handful of desktops, does the helper network hold a
group's latest slot around the clock, and do phones helping "on Wi-Fi and
charging" fill the night gap when desktops are off?

Model (hourly, 21 days):
- Desktops: in their owner's local day window 08:00-24:00 each desktop is on
  with probability 0.7 for the day (some days not used); off at night.
  Restart/boot each morning wipes RAM (RAM-only helpers).
- Phones: helping while charging on Wi-Fi, typically 23:00-07:00 local; on a
  given night a phone participates with probability 0.75; reachable only
  while a charging job runs: 70% of those hours (R4: gaps between jobs).
  Starting a new charging night = new session (RAM empty).
- Time zones: all helpers in one zone, or spread across 3 zones (0, +6h, -6h).
- A group writes at a random time; its r helpers are chosen by measured
  reliability (desktops first, then phones by past uptime) with at most one
  copy per helper; the writer uploads to online helpers, helpers replicate to
  online set members each hour. Member refresh: with probability 0.3 per hour
  during the members' own day (08-23 local, zone 0) a member re-uploads.
- Reader tries at every hour for 24h after the write; success = some online
  set member holds the latest version.
Output: overall availability and the worst hour-of-day availability.
"""
import numpy as np

rng = np.random.default_rng(7)
DAYS, H = 21, 21 * 24


def schedule(kind, tz, reboot_p=1.0, phone_start=23, phone_len=8):
    on = np.zeros(H, dtype=bool)
    restart = np.zeros(H, dtype=bool)
    for d in range(DAYS):
        if kind == "desktop":
            if rng.random() < 0.7:
                start = d * 24 + 8 - tz
                for h in range(start, start + 16):
                    if 0 <= h < H:
                        on[h] = True
                if 0 <= start < H and rng.random() < reboot_p:
                    restart[start] = True
        else:
            if rng.random() < 0.75:
                start = d * 24 + phone_start - tz
                first = True
                for h in range(start, start + phone_len):
                    if 0 <= h < H and rng.random() < 0.7:
                        on[h] = True
                        if first:
                            restart[h] = True
                            first = False
    return on, restart


def run(n_desktops, n_phones, spread, r=10, groups=400, refresh_p=0.3, reboot_p=1.0, phone_start=23, phone_len=8, handoff=False):
    zones = [0, 6, -6] if spread else [0]
    helpers = [("desktop", zones[i % len(zones)]) for i in range(n_desktops)] + \
              [("phone", zones[i % len(zones)]) for i in range(n_phones)]
    sched = [schedule(k, tz, reboot_p, phone_start, phone_len) for k, tz in helpers]
    online = np.array([s[0] for s in sched])      # helpers x H
    restart = np.array([s[1] for s in sched])
    uptime = online[:, : 7 * 24].mean(axis=1)    # "measured reliability" from the first week
    order = np.argsort(-uptime)
    ok = np.zeros(24)
    tot = np.zeros(24)
    for _ in range(groups):
        # reliability-weighted choice among the top candidates, one copy per helper
        pool = order[: max(r * 3, r)]
        w = uptime[pool] + 1e-6
        chosen = rng.choice(pool, size=min(r, len(pool)), replace=False, p=w / w.sum())
        t0 = rng.integers(7 * 24, H - 30)
        holds = np.zeros(len(helpers), dtype=bool)
        for t in range(t0, t0 + 25):
            if handoff and t > t0:
                # helpers going offline at t hand their copy to set members online at t
                leaving = [h for h in chosen if online[h, t - 1] and not online[h, t] and holds[h]]
                if leaving:
                    for h in chosen:
                        if online[h, t]:
                            holds[h] = True
            holds[restart[:, t]] = False
            on_now = [h for h in chosen if online[h, t]]
            member_awake = 8 <= (t % 24) <= 23
            if t == t0 or (member_awake and rng.random() < refresh_p):
                for h in on_now:
                    holds[h] = True
            if any(holds[h] for h in on_now):
                for h in on_now:
                    holds[h] = True
            if t > t0:
                hod = t % 24
                tot[hod] += 1
                ok[hod] += any(online[h, t] and holds[h] for h in chosen)
    by_hour = ok / np.maximum(tot, 1)
    return ok.sum() / tot.sum(), by_hour.min(), int(by_hour.argmin())



print("| Scenario | Overall | Worst hour |")
print("|---|---|---|")
cases = [
  ("10 desktops, one zone, reboot daily (baseline)", dict(n_desktops=10, n_phones=0, spread=False)),
  ("10 desktops, one zone, SLEEP (reboot 1 in 7 days)", dict(n_desktops=10, n_phones=0, spread=False, reboot_p=1/7)),
  ("10 desktops + 200 phones, one zone (baseline)", dict(n_desktops=10, n_phones=200, spread=False)),
  ("... + graceful handoff", dict(n_desktops=10, n_phones=200, spread=False, handoff=True)),
  ("... + handoff + phones 22-08 (overlap)", dict(n_desktops=10, n_phones=200, spread=False, handoff=True, phone_start=22, phone_len=10)),
  ("... + handoff + overlap + desktops sleep", dict(n_desktops=10, n_phones=200, spread=False, handoff=True, phone_start=22, phone_len=10, reboot_p=1/7)),
  ("10 desktops + 200 phones, 3 zones, handoff + overlap + sleep", dict(n_desktops=10, n_phones=200, spread=True, handoff=True, phone_start=22, phone_len=10, reboot_p=1/7)),
  ("5 desktops + 50 phones, one zone, handoff + overlap + sleep", dict(n_desktops=5, n_phones=50, spread=False, handoff=True, phone_start=22, phone_len=10, reboot_p=1/7)),
]
for label, kw in cases:
    o, w, h = run(**kw)
    print(f"| {label} | {o:.1%} | {w:.1%} at {h:02d}:00 |")
