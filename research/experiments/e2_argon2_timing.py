"""E2: Argon2id timing on this PC across parameter sets.

A phone joining a group must derive the invite key in ~1 s. This measures how
long each parameter set takes here (single lane = what one join costs), so we
can scale to low-end phones (typically 3-6x slower single-thread than a
desktop core) and pick the heaviest safe setting.
"""
import os
import statistics
import time

from argon2.low_level import Type, hash_secret_raw

PARAMS = [
    # (label, memory_kib, time_cost, lanes)
    ("OWASP min   m=19MiB t=2 p=1", 19 * 1024, 2, 1),
    ("RFC9106 #2  m=64MiB t=3 p=4", 64 * 1024, 3, 4),
    ("m=64MiB t=3 p=1", 64 * 1024, 3, 1),
    ("m=128MiB t=3 p=1", 128 * 1024, 3, 1),
    ("m=256MiB t=2 p=1", 256 * 1024, 2, 1),
    ("m=256MiB t=4 p=4", 256 * 1024, 4, 4),
]

secret = b"maple-orbit-guitar-seven-river-anchor"
salt = b"kleiders-invite-v1"

print(f"CPU count: {os.cpu_count()}")
for label, m, t, p in PARAMS:
    times = []
    for _ in range(5):
        start = time.perf_counter()
        hash_secret_raw(secret, salt, time_cost=t, memory_cost=m, parallelism=p, hash_len=32, type=Type.ID)
        times.append(time.perf_counter() - start)
    med = statistics.median(times)
    print(f"{label:32s} median {med*1000:7.1f} ms   (low-end phone est. x4: {med*4:5.2f} s)")
