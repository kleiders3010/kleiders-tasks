"""E5: helper-network simulation.

Question: when a group member writes a slot and goes offline, how often can
another member (reading at a random later time within 24 h) get the latest
version from at least one helper, as a function of:
- replication factor r
- helper mix (desktops vs phones-on-charger), with RAM loss on every restart
- refresh by group members (their background sync re-uploads the slot)
- dishonest helpers (accept and drop), with and without reputation
- selection: uniform random vs reliability-weighted

Model (hourly steps, 14 simulated days):
- Each helper alternates online/offline sessions (geometric lengths) with a
  target online fraction p. Coming back online clears its RAM.
- Desktops: p=0.5, mean session 8 h. Phones on charger: p=0.3, mean session 6 h.
- Writer uploads to its group's r helpers that are online at write time (the
  others get the copy later by helper-to-helper replication when online at the
  same time as a holder).
- Refresh: each hour, with probability `refresh_p`, some member device is awake
  and re-uploads the latest slot to all online helpers in the set (members hold
  full data locally). refresh_p=0 models "all members' phones asleep / off".
- Dishonest helpers accept but never serve. With reputation, a dishonest helper
  is detected after its first failed read and replaced by the next writer or
  refresher with a fresh pick.
"""
import numpy as np

rng = np.random.default_rng(42)
HOURS = 14 * 24


def make_helpers(n, desktop_share, bad_share):
    kind = rng.random(n) < desktop_share
    p = np.where(kind, 0.5, 0.3)
    session = np.where(kind, 8.0, 6.0)
    bad = rng.random(n) < bad_share
    return p, session, bad


def simulate_online(p, session):
    """Two-state Markov chain per helper: P(on->off)=1/session, P(off->on) chosen to hit p."""
    n = len(p)
    off_on = (p / (1 - p)) / session
    on_off = 1 / session
    state = rng.random(n) < p
    online = np.zeros((HOURS, n), dtype=bool)
    restarted = np.zeros((HOURS, n), dtype=bool)
    for t in range(HOURS):
        u = rng.random(n)
        new = np.where(state, u >= on_off, u < off_on)
        restarted[t] = new & ~state
        state = new
        online[t] = state
    return online, restarted


def run(r, desktop_share=0.3, bad_share=0.0, refresh_p=0.5, weighted=False, reputation=False,
        n_helpers=2000, n_groups=300):
    p, session, bad = make_helpers(n_helpers, desktop_share, bad_share)
    online, restarted = simulate_online(p, session)
    weights = p / p.sum() if weighted else np.full(n_helpers, 1 / n_helpers)
    successes = trials = 0
    for g in range(n_groups):
        helpers = list(rng.choice(n_helpers, size=r, replace=False, p=weights))
        excluded = set()
        holds = np.zeros(n_helpers, dtype=bool)  # holds latest version
        t_write = rng.integers(0, HOURS - 48)
        for t in range(t_write, t_write + 25):
            holds[restarted[t]] = False  # RAM cleared on restart
            on = [h for h in helpers if online[t, h]]
            if t == t_write or rng.random() < refresh_p:
                for h in on:
                    holds[h] = True  # writer / refreshing member uploads
            # helper-to-helper replication among online members of the set
            if any(holds[h] for h in on):
                for h in on:
                    holds[h] = True
            if t > t_write:
                # a reader tries now (one read per hour after the write)
                ok = any(online[t, h] and holds[h] and not bad[h] for h in helpers)
                successes += ok
                trials += 1
                if reputation:
                    for h in on:
                        if bad[h] and holds[h] and h not in excluded:
                            excluded.add(h)  # observed failing -> replace it
                            pool = weights.copy()
                            pool[helpers] = 0
                            pool /= pool.sum()
                            new = int(rng.choice(n_helpers, p=pool))
                            helpers[helpers.index(h)] = new
    return successes / trials


def table(title, rows):
    print(f"\n### {title}")
    print("| Scenario | " + " | ".join(f"r={r}" for r in RS) + " |")
    print("|---|" + "---|" * len(RS))
    for label, kw in rows:
        vals = [run(r, **kw) for r in RS]
        print(f"| {label} | " + " | ".join(f"{v:.1%}" for v in vals) + " |")


RS = [2, 4, 6, 10, 15, 20, 30]
print("# E5 helper-network simulation (probability a reader finds the latest slot, reads hourly for 24 h after a write)")
table("Helper mix and member refresh (no bad helpers, uniform selection)", [
    ("30% desktops, members refresh 50%/h", dict()),
    ("30% desktops, no refresh (all members offline)", dict(refresh_p=0.0)),
    ("10% desktops, members refresh 50%/h", dict(desktop_share=0.1)),
    ("10% desktops, no refresh", dict(desktop_share=0.1, refresh_p=0.0)),
    ("30% desktops, reliability-weighted, no refresh", dict(refresh_p=0.0, weighted=True)),
])
table("Dishonest helpers (accept and drop), no refresh", [
    ("10% bad, no reputation", dict(bad_share=0.1, refresh_p=0.0)),
    ("30% bad, no reputation", dict(bad_share=0.3, refresh_p=0.0)),
    ("30% bad, with reputation", dict(bad_share=0.3, refresh_p=0.0, reputation=True)),
    ("50% bad, with reputation", dict(bad_share=0.5, refresh_p=0.0, reputation=True)),
])
table("Dishonest helpers WITH member refresh 50%/h (1000 groups for tighter estimates)", [
    ("no bad helpers", dict(n_groups=1000)),
    ("30% bad, no reputation", dict(bad_share=0.3, n_groups=1000)),
    ("30% bad, with reputation", dict(bad_share=0.3, reputation=True, n_groups=1000)),
    ("50% bad, no reputation", dict(bad_share=0.5, n_groups=1000)),
    ("50% bad, with reputation", dict(bad_share=0.5, reputation=True, n_groups=1000)),
])
