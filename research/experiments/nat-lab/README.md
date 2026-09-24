# E10 NAT lab (D49): iroh 1.2 P2P without n0

Report: [`docs/research/E10-nat-lab.md`](../../../docs/research/E10-nat-lab.md).

Everything runs inside the WSL2 **Ubuntu** distro as root, in Linux network namespaces that have
**no link to the root namespace**, so no lab packet can reach the internet. No n0 relay, n0 DNS or
pkarr server is ever configured: endpoints use `RelayMode::Disabled`, `RelayMode::Custom(<lab
helpers only>)` or `Custom(empty)`, and no address lookup service.

## Layout

| Path | What |
|---|---|
| `natlab/` | Rust test binary (iroh 1.2.0, iroh-relay 1.2.0 `server`, instant-acme 0.8.5). Subcommands: `relay`, `acme`, `selfsign`, `stunsrv`, `stun`, `peer`, `relaymgmt`, `load`, `ctrelay`, `wanemu`. See the header of each `src/*.rs`. |
| `build.sh` | Builds `natlab` with `CARGO_TARGET_DIR=/root/natlab-target` and installs it to `/usr/local/bin`. |
| `lab/lab.sh` | `up` / `down` / `status` / `delay MS` / `nodelay`: namespaces, veths, NAT routers (nftables), miniupnpd. Topology diagram in its header. |
| `lab/services.sh` | STUN server, Pebble (local ACME CA), IP-certificate issuance, relays. |
| `lab/all_up.sh` | Whole lab: `lab.sh up` + STUN + Pebble + ACME certificates for helpers h1/h2 + both relays + the option (c) TCP forwarder. |
| `lab/run_matrix.py` | NAT-pair matrix runner (experiments `baseline`, `relay*`, `direct_*`, `ct_*`). Writes `results/<exp>.jsonl` and raw logs to `results/raw/`. |
| `lab/run_all.sh` | The full matrix as run for the report. |
| `lab/relaymgmt_test.sh` | `insert_relay` / `remove_relay` at runtime (R1–R4). |
| `lab/helper_nat_test.sh` | "Can this desktop be a helper relay?" behind UPnP, UPnP-behind-CGN, and plain NAT routers. |
| `lab/load_test.sh` | Relay RAM/CPU with 20/100/200 relay-only endpoints and relayed throughput. |
| `results/` | Raw outputs used in the report. |

## Prerequisites (inside Ubuntu, once)

```bash
apt-get install -y build-essential pkg-config nftables iproute2 iptables miniupnpd-nftables \
  miniupnpc conntrack tcpdump curl jq python3 ethtool
systemctl disable --now miniupnpd          # the lab starts its own instances per router
curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
# Pebble v2.10.1 release binary
mkdir -p /opt/pebble && cd /opt/pebble && curl -sSLO \
  https://github.com/letsencrypt/pebble/releases/download/v2.10.1/pebble-linux-amd64.tar.gz \
  && tar xzf pebble-linux-amd64.tar.gz && chmod +x pebble-linux-amd64/linux/amd64/pebble
```

## Rerun

From Windows, keep the distro alive in one terminal (WSL stops idle distros and the namespaces
vanish): `wsl -d Ubuntu -u root -- sleep infinity`. Then:

```bash
wsl -d Ubuntu -u root -- bash
N=/mnt/c/Users/uri_l/projects/tasklist/research/experiments/nat-lab
bash $N/build.sh
bash $N/lab/all_up.sh
bash $N/lab/lab.sh delay 20            # 20 ms per core hop: 40 ms direct RTT, ~80 ms relayed RTT
bash $N/lab/run_all.sh                 # ~1 h; or: python3 $N/lab/run_matrix.py relay --pairs prc1-sym2
python3 $N/lab/run_matrix.py ct_only --out $N/results
python3 $N/lab/run_matrix.py ct_stun --out $N/results
bash $N/lab/relaymgmt_test.sh
bash $N/lab/helper_nat_test.sh
bash $N/lab/load_test.sh               # switches the delay off itself
bash $N/lab/lab.sh down
```

## Lab caveats

- WSL2's kernel has no `sch_netem` and no NFQUEUE target, so latency comes from `natlab wanemu`,
  a userspace learning switch (AF_PACKET) that replaces the core bridge during `lab.sh delay`.
- The "public" IPv4 prefix is `7.7.7.0/24` on purpose: miniupnpd, like real routers, refuses to
  map ports when its WAN address is in a reserved range. The prefix only exists inside the
  isolated namespaces. IPv6 uses `2001:db8::/32`.
- NAT behaviour is Linux netfilter: `masquerade` (port-preserving, endpoint-independent mapping,
  address+port-dependent filtering), `masquerade fully-random` (symmetric), and a DNAT-all-UDP rule
  for full cone. Routers drop unsolicited WAN input like real home routers do (accepting it creates
  conntrack entries that break port preservation during simultaneous open; see the report).
