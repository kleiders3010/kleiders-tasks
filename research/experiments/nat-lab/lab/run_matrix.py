#!/usr/bin/env python3
"""E10 NAT-lab matrix runner (runs inside WSL Ubuntu as root, after all_up.sh).

usage: run_matrix.py <experiment> [--reps N] [--pairs a-b,c-d] [--out DIR]
experiments:
  baseline   relays disabled, no portmapper, B dials A with A's self-known addresses
  relay      option (a): both clients use ONLY the lab helper relay h1 (Pebble IP cert)
  relay_idonly   option (a): B dials A knowing only A's endpoint id + relay URL
  relay_cross    option (a): A homes on h1, B homes on h2 (two different helpers)
  relay_noportmap option (a) with the portmapper disabled
  direct_one   option (b): relays disabled, portmapper on, one-sided dial with published addrs
  direct_simul option (b): relays disabled, portmapper on, simultaneous dial (go-file)
  direct_stun  option (b): as direct_simul + side-socket STUN prediction (public ip : iroh port)
"""
import json, os, subprocess, sys, time, shutil, argparse

RUN = "/run/natlab"
REPO = "/mnt/c/Users/uri_l/projects/tasklist/research/experiments/nat-lab"
CA1 = f"{RUN}/certs/h1/ca.pem"
H1 = "https://7.7.7.10"
H2 = "https://7.7.7.11"
STUN = ["--stun", "7.7.7.6:3478", "--stun", "7.7.7.7:3478"]

TYPES = {"eim1": "EIM", "eim2": "EIM", "prc1": "PRC", "prc2": "PRC", "sym1": "SYM", "sym2": "SYM",
         "v6a": "V6+SYM4", "v6b": "V6+SYM4", "upnp1": "UPNP", "upnp2": "UPNP", "upcgn": "UPNP-behind-CGN"}

DEFAULT_PAIRS = ["eim1-eim2", "eim1-prc2", "prc1-prc2", "eim1-sym2", "prc1-sym2", "sym1-sym2",
                 "v6a-v6b", "v6a-prc2", "upnp1-prc2", "upnp1-sym2", "upnp1-upnp2", "upcgn-prc2", "upcgn-sym2"]

ROUTERS = ["eim1", "eim2", "prc1", "prc2", "sym1", "sym2", "v6a", "v6b", "upnp1", "upnp2", "upcgn", "cgn"]


def sh(cmd):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout


def flush_state():
    for r in ROUTERS:
        sh(f"ip netns exec nl-r-{r} conntrack -F 2>/dev/null")
    # drop leftover UPnP/NAT-PMP mappings so each case starts from a clean router
    for c in ["upnp1", "upnp2", "upcgn"]:
        out = sh(f"ip netns exec nl-c-{c} upnpc -l 2>/dev/null")
        for line in out.splitlines():
            parts = line.split()
            if len(parts) >= 3 and parts[0].isdigit() and parts[1] in ("UDP", "TCP"):
                ext = parts[2].split("->")[0]
                sh(f"ip netns exec nl-c-{c} upnpc -d {ext} {parts[1]} >/dev/null 2>&1")


def parse(path):
    evs = []
    try:
        for line in open(path):
            line = line.strip()
            if line.startswith("{"):
                try:
                    evs.append(json.loads(line))
                except Exception:
                    pass
    except FileNotFoundError:
        pass
    return evs


def last(evs, name):
    r = [e for e in evs if e.get("ev") == name]
    return r[-1] if r else None


def variant_args(exp):
    common = ["--settle", "3000", "--timeout", "10", "--observe", "6"]
    a, b = [], []
    simul = False
    if exp == "baseline":
        a = b = ["--no-portmap"]
    elif exp == "relay":
        a = b = ["--relay", H1, "--ca", CA1]
    elif exp == "relay_idonly":
        a = b = ["--relay", H1, "--ca", CA1]
        b = b + ["--no-peer-ips"]
    elif exp == "relay_cross":
        a = ["--relay", H1, "--ca", CA1]
        b = ["--relay", H2, "--ca", CA1]  # same Pebble root signs both helpers
    elif exp == "relay_noportmap":
        a = b = ["--relay", H1, "--ca", CA1, "--no-portmap"]
    elif exp == "direct_one":
        a = b = []
    elif exp == "direct_simul":
        a = b = []
        simul = True
    elif exp == "direct_stun":
        a = b = STUN + ["--stun-predict"]
        simul = True
    else:
        raise SystemExit(f"unknown experiment {exp}")
    return common, a, b, simul


def run_case(exp, pair, rep, outdir):
    ca, cb = pair.split("-")
    common, xa, xb, simul = variant_args(exp)
    case = f"{exp}__{pair}__r{rep}"
    w = f"{RUN}/cases/{case}"
    shutil.rmtree(w, ignore_errors=True)
    os.makedirs(w)
    flush_state()
    base = ["natlab", "peer"] + common
    pa = base + ["--seed", "1", "--out", f"{w}/a.json", "--lifetime", "30"] + xa
    pb = base + ["--seed", "2", "--out", f"{w}/b.json", "--lifetime", "30", "--dial",
                 "--peer-file", f"{w}/a.json"] + xb
    if simul:
        pa += ["--dial", "--peer-file", f"{w}/b.json", "--go-file", f"{w}/go"]
        pb += ["--go-file", f"{w}/go"]
    env = dict(os.environ, RUST_LOG="warn")
    fa = open(f"{w}/a.log", "w"); ea = open(f"{w}/a.err", "w")
    fb = open(f"{w}/b.log", "w"); eb = open(f"{w}/b.err", "w")
    procA = subprocess.Popen(["ip", "netns", "exec", f"nl-c-{ca}"] + pa, stdout=fa, stderr=ea, env=env)
    procB = subprocess.Popen(["ip", "netns", "exec", f"nl-c-{cb}"] + pb, stdout=fb, stderr=eb, env=env)
    t0 = time.time()
    if simul:
        while not (os.path.exists(f"{w}/a.json") and os.path.exists(f"{w}/b.json")) and time.time() - t0 < 30:
            time.sleep(0.05)
        with open(f"{w}/go.tmp", "w") as f:
            f.write(str(int(time.time() * 1000) + 500))
        os.rename(f"{w}/go.tmp", f"{w}/go")
    # wait for the dialer summaries
    need = ["b"] + (["a"] if simul else [])
    while time.time() - t0 < 45:
        done = all(last(parse(f"{w}/{x}.log"), "summary") for x in need)
        if done:
            time.sleep(0.3)
            break
        time.sleep(0.2)
    for p in (procA, procB):
        p.terminate()
    for p in (procA, procB):
        try:
            p.wait(5)
        except subprocess.TimeoutExpired:
            p.kill()
    for f in (fa, ea, fb, eb):
        f.close()
    A, B = parse(f"{w}/a.log"), parse(f"{w}/b.log")
    row = {"exp": exp, "pair": pair, "rep": rep, "a": ca, "b": cb, "a_type": TYPES[ca], "b_type": TYPES[cb]}
    for tag, evs in (("a", A), ("b", B)):
        pub = last(evs, "published") or {}
        nr = last(evs, "netreport") or {}
        row[f"{tag}_published"] = pub.get("ips", []) + pub.get("relays", [])
        row[f"{tag}_qad_v4"] = nr.get("global_v4")
        row[f"{tag}_qad_v6"] = nr.get("global_v6")
        st = last(evs, "stun_side")
        if st:
            row[f"{tag}_stun"] = {k: st.get(k) for k in ("mapped", "port_preserved", "varies_by_dest")}
        pr = last(evs, "stun_predicted")
        if pr:
            row[f"{tag}_stun_predicted"] = pr["addr"]
        on = last(evs, "online")
        row[f"{tag}_online_ms"] = on["ms"] if on else None
        s = last(evs, "summary")
        if s:
            s = {k: v for k, v in s.items() if k not in ("ev", "t")}
            row[f"{tag}_summary"] = s
        row[f"{tag}_incoming"] = len([e for e in evs if e.get("ev") == "incoming"])
    sums = [row.get(f"{x}_summary") for x in need if row.get(f"{x}_summary")]
    row["connected"] = any(s.get("connected") for s in sums)
    row["direct"] = any(s.get("direct") for s in sums)
    ok = [s for s in sums if s.get("connected")]
    best = min(ok, key=lambda s: s.get("connect_ms") or 1e9) if ok else (sums[0] if sums else {})
    row["connect_ms"] = best.get("connect_ms")
    row["ttfb_ms"] = best.get("ttfb_ms")
    dm = [s.get("direct_ms") for s in ok if s.get("direct_ms") is not None]
    row["direct_ms"] = min(dm) if dm else None
    fs = best.get("final_selected") or {}
    row["final_path"] = ("relay" if fs.get("relay") else fs.get("remote")) if fs else None
    row["final_rtt_ms"] = fs.get("rtt_ms") if fs else None
    row["err"] = best.get("err")
    # keep raw logs (stdout only) next to the results
    rd = f"{outdir}/raw/{case}"
    os.makedirs(rd, exist_ok=True)
    for x in ("a", "b"):
        shutil.copy(f"{w}/{x}.log", f"{rd}/{x}.log")
    return row


def fmt(v, nd=0):
    if v is None:
        return "-"
    if isinstance(v, float):
        return f"{v:.{nd}f}"
    return str(v)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("exp")
    ap.add_argument("--reps", type=int, default=1)
    ap.add_argument("--pairs", default=",".join(DEFAULT_PAIRS))
    ap.add_argument("--out", default=f"{REPO}/results")
    ap.add_argument("--tag", default="")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    tag = a.tag or a.exp
    rows = []
    with open(f"{a.out}/{tag}.jsonl", "w") as f:
        for pair in a.pairs.split(","):
            for rep in range(a.reps):
                row = run_case(a.exp, pair, rep, a.out)
                rows.append(row)
                f.write(json.dumps(row) + "\n")
                f.flush()
                print(f"{a.exp:16} {pair:12} r{rep} conn={row['connected']!s:5} direct={row['direct']!s:5} "
                      f"connect={fmt(row['connect_ms'])} ttfb={fmt(row['ttfb_ms'])} direct_at={fmt(row['direct_ms'])} "
                      f"path={row['final_path']} err={(row['err'] or '')[:60]}", flush=True)


if __name__ == "__main__":
    main()
