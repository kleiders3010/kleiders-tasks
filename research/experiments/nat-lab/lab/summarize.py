#!/usr/bin/env python3
"""Turn results/<exp>.jsonl into markdown tables (median over reps)."""
import json, sys, os, statistics, glob

R = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "results")


def med(xs):
    xs = [x for x in xs if x is not None]
    return f"{statistics.median(xs):.0f}" if xs else "-"


def fam(p):
    if not p:
        return "-"
    if p == "relay":
        return "relay"
    if p.startswith("Ip([") or p.startswith("Ip(2001"):
        return "direct v6"
    if p.startswith("Ip("):
        return "direct v4"
    if p.startswith("Custom"):
        return "custom (TCP fwd)"
    return p


def table(exp):
    path = f"{R}/{exp}.jsonl"
    if not os.path.exists(path):
        return None
    rows = [json.loads(l) for l in open(path)]
    by = {}
    for r in rows:
        by.setdefault(r["pair"], []).append(r)
    out = [f"### `{exp}`", "",
           "| Pair | Types | Connected | Direct | Connect ms | TTFB ms | Direct after ms | Final path | Notes |",
           "|---|---|---|---|---|---|---|---|---|"]
    for pair, rs in by.items():
        n = len(rs)
        c = sum(r["connected"] for r in rs)
        d = sum(r["direct"] for r in rs)
        paths = sorted({fam(r["final_path"]) for r in rs})
        notes = []
        r0 = rs[0]
        for t in ("a", "b"):
            q = r0.get(f"{t}_qad_v4")
            if q:
                notes.append(f"{t} QAD {q}")
            sp = r0.get(f"{t}_stun_predicted")
            if sp:
                notes.append(f"{t} pred {sp}")
        errs = sorted({(r.get("err") or "")[:40] for r in rs if r.get("err")})
        if errs:
            notes.append("err: " + "; ".join(errs))
        out.append(f"| {pair} | {r0['a_type']} / {r0['b_type']} | {c}/{n} | {d}/{n} | "
                   f"{med([r['connect_ms'] for r in rs])} | {med([r['ttfb_ms'] for r in rs])} | "
                   f"{med([r['direct_ms'] for r in rs])} | {', '.join(paths)} | {'; '.join(notes)} |")
    return "\n".join(out)


for f in sorted(glob.glob(f"{R}/*.jsonl")):
    t = table(os.path.basename(f)[:-6])
    if t:
        print(t, "\n")
