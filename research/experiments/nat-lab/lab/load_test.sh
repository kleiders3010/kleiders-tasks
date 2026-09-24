#!/bin/bash
# Helper relay resource use: N relay-only endpoints in nl-load (public, no NAT) all homed on h1,
# N/2 relayed connections, ping hold phase, then relayed bulk throughput with 1/5/10 pairs.
# Run with the kernel bridge (lab.sh nodelay) so the userspace delay switch is not the bottleneck.
OUT=${1:-/mnt/c/Users/uri_l/projects/tasklist/research/experiments/nat-lab/results/load}
mkdir -p $OUT
L=$(cd "$(dirname "$0")" && pwd)
bash $L/lab.sh nodelay
CA=/run/natlab/certs/h1/ca.pem
# fresh relay process so baseline RSS is clean
bash $L/services.sh stop relay-h1; sleep 0.5
bash $L/services.sh relay nl-h1 0.0.0.0 /run/natlab/certs/h1 h1 >/dev/null
PID=$(cat /run/natlab/relay-h1.pid)
for n in ${NS:-20 100 200}; do
  echo "== n=$n"
  ip netns exec nl-load natlab load --relay https://7.7.7.10 --ca $CA --n $n --relay-pid $PID \
     --hold 20 --ping-ms 1000 --tp-pairs 1,5,10 --tp-bytes 50000000 > $OUT/load_n$n.log 2> $OUT/load_n$n.err
  cut -c1-400 $OUT/load_n$n.log
  sleep 3
done
