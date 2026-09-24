#!/bin/bash
# X2-style "can this desktop be a helper relay?" across home-router types.
# For each candidate helper position: (1) ask the router for TCP 80/443 + UDP 7842 mappings via
# UPnP (miniupnpc, as the app would with an IGD client), (2) get an ACME IP certificate for the
# router's WAN address from Pebble (http-01 through the mapping), (3) start the relay, (4) have a
# symmetric-NAT client pair use ONLY this relay (QAD, online, connect, path).
# Positions: H-UPNP (behind nl-r-upnp2), H-UPCGN (UPnP home router behind CGN), H-PRC (no UPnP).
OUT=${1:-/mnt/c/Users/uri_l/projects/tasklist/research/experiments/nat-lab/results/helper_nat}
mkdir -p $OUT
RUN=/run/natlab
W=$RUN/hn; rm -rf $W; mkdir -p $W

test_pos() { # name client-ns lan-ip wan-ip-to-certify
  local name=$1 ns=$2 lan=$3 wan=$4
  echo "=== $name ($ns, lan $lan, public $wan)" | tee $OUT/$name.txt
  {
  echo "-- UPnP mappings"
  ip netns exec $ns upnpc -s 2>&1 | grep -E "ExternalIPAddress|No IGD|Found valid IGD" | head -3
  for m in "80 TCP" "443 TCP" "7842 UDP"; do
    set -- $m
    ip netns exec $ns timeout 10 upnpc -e natlab-helper -a $lan $1 $1 $2 2>&1 | grep -E "is redirected|failed|Error|error" | head -2
  done
  echo "-- ACME IP certificate for $wan"
  ip netns exec $ns timeout 40 natlab acme --dir https://7.7.7.5:14000/dir --root $RUN/pebble/apica.pem \
     --ip $wan --http-bind 0.0.0.0 --out-cert $W/$name-cert.pem --out-key $W/$name-key.pem 2>&1 | grep -E "acme_issued|http01_request|Error|error" | cut -c1-200 | head -4
  } 2>&1 | tee -a $OUT/$name.txt
  if [ ! -s $W/$name-cert.pem ]; then
    echo "RESULT $name: no certificate -> cannot serve as a public relay" | tee -a $OUT/$name.txt
    return
  fi
  cp $RUN/pebble/root.pem $W/ca.pem
  setsid nohup ip netns exec $ns natlab relay --ip 0.0.0.0 --cert $W/$name-cert.pem --key $W/$name-key.pem > $W/$name-relay.log 2>&1 &
  local rp=$!
  sleep 1
  ip netns exec nl-c-sym1 natlab peer --seed 31 --relay https://$wan --ca $W/ca.pem --out $W/$name-a.json --settle 2000 --lifetime 25 > $OUT/$name-peerA.log 2>/dev/null &
  ip netns exec nl-c-sym2 natlab peer --seed 32 --relay https://$wan --ca $W/ca.pem --out $W/$name-b.json --settle 2000 --dial --peer-file $W/$name-a.json --timeout 10 --observe 4 --lifetime 1 > $OUT/$name-peerB.log 2>/dev/null
  kill %1 2>/dev/null
  kill $rp 2>/dev/null; wait 2>/dev/null
  {
  echo "-- client A: $(grep -E '"online"|online_timeout' $OUT/$name-peerA.log | head -1 | cut -c1-120)"
  echo "-- client A QAD: $(grep netreport $OUT/$name-peerA.log | tail -1 | cut -c1-200)"
  echo "-- client B summary: $(grep summary $OUT/$name-peerB.log | cut -c1-300)"
  } | tee -a $OUT/$name.txt
  for m in "80 TCP" "443 TCP" "7842 UDP"; do set -- $m; ip netns exec $ns upnpc -d $1 $2 >/dev/null 2>&1; done
}

test_pos H-UPNP  nl-c-upnp2 10.10.0.2 7.7.7.30
test_pos H-UPCGN nl-c-upcgn 10.11.0.2 7.7.7.31
test_pos H-PRC   nl-c-prc2  10.4.0.2  7.7.7.24
