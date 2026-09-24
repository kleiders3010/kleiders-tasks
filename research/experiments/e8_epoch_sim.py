"""E8: key-epoch simulator (no central orderer, causal delivery, random delays).

Membership ops are signed by a device and reference the epoch they were made
in. Proposed rules under test (from R7 + A5):
  R1 Strong remove: a removal of T made in epoch E yields a new epoch whose
     recipients exclude T; T can never be re-added by anyone.
  R2 Fork merge: when a device knows several "head" epochs (concurrent
     rotations), the deterministic merger (lowest device id among the
     intersection of the heads' recipients) creates one successor whose
     recipients = intersection of the heads' recipients minus all removed.
     Duplicate merges converge by lowest-hash tie-break.
  R3 Additions are valid only if signed by a device that is a recipient of the
     head epoch the addition references (so a holder of an old/leaked key
     can't add a ghost).
  R4 Mutual removal: if A removes B and B removes A concurrently, both are out.

Invariants checked after every run:
  I1 A removed device never holds the key of an epoch created by a device that
     already knew of its removal.
  I2 After quiescence, all remaining devices agree on one head epoch.
  I3 No ghost: a device added with an old epoch key is never a recipient.
"""
import hashlib
import random

N_RUNS = 3000


class Epoch:
    def __init__(self, eid, parents, recipients, creator, removed):
        self.id, self.parents, self.recipients = eid, tuple(sorted(parents)), frozenset(recipients)
        self.creator, self.removed = creator, frozenset(removed)


def h(*parts):
    return hashlib.sha256("|".join(map(str, parts)).encode()).hexdigest()[:12]


class Device:
    def __init__(self, did, world):
        self.id, self.w = did, world
        self.epochs = {}          # epochs this device knows about (metadata is public within group)
        self.keys = set()         # epoch ids whose key this device holds
        self.removed = set()      # devices known removed
        self.members = set()

    def heads(self):
        known = set(self.epochs)
        parents = {p for e in self.epochs.values() for p in e.parents}
        return sorted(known - parents)

    def receive(self, epoch):
        if epoch.id in self.epochs:
            return
        self.epochs[epoch.id] = epoch
        self.removed |= epoch.removed
        if self.id in epoch.recipients:
            self.keys.add(epoch.id)
        self.maybe_merge()

    def maybe_merge(self):
        hs = self.heads()
        if len(hs) < 2 or not all(h_ in self.keys for h_ in hs):
            return
        inter = frozenset.intersection(*(self.epochs[x].recipients for x in hs)) - self.removed
        if not inter:
            return
        if self.id != min(inter):
            return  # not the deterministic merger
        eid = h("merge", *hs)  # deterministic id: identical merges collapse
        self.w.publish(self, Epoch(eid, hs, inter, self.id, self.removed))

    def remove(self, target):
        hs = self.heads()
        if len(hs) != 1 or hs[0] not in self.keys or target == self.id or target in self.removed:
            return
        cur = self.epochs[hs[0]]
        rec = cur.recipients - {target} - self.removed
        if self.id not in rec:
            return
        eid = h("rm", hs[0], self.id, target, len(self.epochs))
        self.w.publish(self, Epoch(eid, hs, rec, self.id, self.removed | {target}))

    def add(self, newcomer, with_epoch=None):
        hs = self.heads()
        ref = with_epoch or (hs[0] if len(hs) == 1 else None)
        if ref is None or newcomer in self.removed:
            return
        # R3: valid only if the signer is a recipient of the referenced epoch AND it is a current head
        valid = ref in self.keys and ref in hs and self.id in self.epochs[ref].recipients
        if not valid:
            self.w.rejected_adds += 1
            return
        cur = self.epochs[ref]
        eid = h("add", ref, self.id, newcomer)
        self.w.publish(self, Epoch(eid, [ref], (cur.recipients - self.removed) | {newcomer}, self.id, self.removed))


class World:
    def __init__(self, n, seed):
        self.rng = random.Random(seed)
        self.devs = {i: Device(i, self) for i in range(n)}
        self.queue = []  # (deliver_at, target, epoch)
        self.t = 0
        self.rejected_adds = 0
        self.created_by_knower = []  # (epoch, removed set known by creator)
        g = Epoch("genesis", [], set(range(n)), 0, set())
        for d in self.devs.values():
            d.receive(g)

    def publish(self, sender, epoch):
        self.created_by_knower.append((epoch, set(sender.removed) | set(epoch.removed)))
        sender.receive(epoch)
        for d in self.devs.values():
            if d is not sender:
                self.queue.append((self.t + self.rng.randint(1, 30), d.id, epoch))

    def step(self):
        self.t += 1
        due = [q for q in self.queue if q[0] <= self.t]
        self.queue = [q for q in self.queue if q[0] > self.t]
        self.rng.shuffle(due)
        for _, target, ep in due:
            self.devs[target].receive(ep)


def run(seed):
    rng = random.Random(seed)
    n = rng.randint(3, 8)
    w = World(n, seed)
    ghost_attempts = []
    for _ in range(rng.randint(1, 5)):
        # a burst of concurrent operations
        for _ in range(rng.randint(1, 3)):
            actor = w.devs[rng.randrange(n)]
            if rng.random() < 0.6:
                actor.remove(rng.randrange(n))
            else:
                # ghost attempt: add using an old epoch key the actor holds (maybe stale)
                old = rng.choice(sorted(actor.keys))
                ghost = 100 + rng.randrange(50)
                actor.add(ghost, with_epoch=old)
                ghost_attempts.append((ghost, old))
        for _ in range(rng.randint(0, 20)):
            w.step()
    for _ in range(400):  # quiescence
        w.step()
        for d in w.devs.values():
            d.maybe_merge()
    removed = set().union(*(d.removed for d in w.devs.values()))
    # I1
    for ep, known_removed in w.created_by_knower:
        if ep.recipients & known_removed:
            return "I1 violated", seed
    # I2
    remaining = [d for d in w.devs.values() if d.id not in removed]
    if remaining:
        heads = {tuple(d.heads()) for d in remaining}
        if len(heads) != 1 or len(next(iter(heads))) != 1:
            return "I2 violated (no convergence)", seed
    # I3: ghosts only valid if added from the then-current head by a recipient; check no ghost via stale key
    for d in w.devs.values():
        for ep in d.epochs.values():
            for g in ep.recipients:
                if g >= 100 and ep.creator in removed and ep.id.startswith("add"):
                    return "I3 violated", seed
    return "ok", len(remaining) == 0


results = {}
empties = 0
for s in range(N_RUNS):
    r, extra = run(s)
    results[r] = results.get(r, 0) + 1
    if r == "ok" and extra:
        empties += 1
    elif r != "ok" and results[r] <= 3:
        print(f"{r} (seed {extra})")
print(f"runs={N_RUNS} results={results} groups-emptied-by-mutual-removal={empties}")
