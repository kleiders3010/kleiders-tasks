#!/bin/bash
# Runtime relay management (insert_relay / remove_relay) against the lab helpers.
# R1: RelayMode::Disabled, then insert_relay(h1)
# R2: RelayMode::Custom(empty map), then insert_relay(h1)
# R3: start with h1, insert h2, remove h1 (does the home relay move to h2?)
# R4: sym1 <-> sym2 (relay-only path): connected via h1; dialer removes h1, pings, re-adds h2
OUT=${1:-/mnt/c/Users/uri_l/projects/tasklist/research/experiments/nat-lab/results/relaymgmt}
mkdir -p $OUT
CA=/run/natlab/certs/h1/ca.pem
H1=https://7.7.7.10; H2=https://7.7.7.11
W=/run/natlab/rm; rm -rf $W; mkdir -p $W

run() { # name ns args...
  local name=$1 ns=$2; shift 2
  ip netns exec $ns natlab relaymgmt --ca $CA "$@" > $OUT/$name.log 2> $W/$name.err
  echo "== $name"; grep -E '"ev":"step"|relay_status' $OUT/$name.log | cut -c1-330
}
run R1_disabled_insert nl-c-prc1 --seed 11 --steps "sleep 500;insert $H1;online;sleep 1000"
run R2_empty_insert    nl-c-prc1 --seed 12 --empty-relay-map --steps "sleep 500;insert $H1;online;sleep 1000"
run R3_switch          nl-c-prc1 --seed 13 --relay $H1 --steps "online;insert $H2;sleep 1500;remove $H1;sleep 3000;online"

# R4: A = plain peer homed on h1 in sym1; B = relaymgmt in sym2
ip netns exec nl-c-sym1 natlab peer --seed 21 --relay $H1 --ca $CA --out $W/a.json --settle 1000 --lifetime 40 > $OUT/R4_peerA.log 2>$W/a.err &
run R4_remove_while_connected nl-c-sym2 --seed 22 --relay $H1 --steps "online;dial $W/a.json;ping;remove $H1;ping;sleep 3000;ping;insert $H2;sleep 3000;ping;insert $H1;sleep 3000;ping"
kill %1 2>/dev/null; wait
