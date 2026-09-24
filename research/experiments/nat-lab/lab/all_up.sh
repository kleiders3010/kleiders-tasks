#!/bin/bash
# Build the whole lab: namespaces + NATs, STUN, Pebble, ACME IP certificates, two helper relays.
L=$(cd "$(dirname "$0")" && pwd)
pkill -f "natlab " 2>/dev/null; pkill miniupnpd 2>/dev/null; sleep 0.3
bash $L/lab.sh up
bash $L/services.sh stun
bash $L/services.sh pebble
bash $L/services.sh cert nl-h1 /run/natlab/certs/h1 7.7.7.10 2001:db8::10 | tail -1
bash $L/services.sh cert nl-h2 /run/natlab/certs/h2 7.7.7.11 2001:db8::11 | tail -1
bash $L/services.sh relay nl-h1 0.0.0.0 /run/natlab/certs/h1 h1
bash $L/services.sh relay nl-h2 0.0.0.0 /run/natlab/certs/h2 h2
# option (c): blind TCP datagram forwarder on helper h1 (first path for the iroh custom transport)
setsid nohup ip netns exec nl-h1 natlab ctrelay --listen 7.7.7.10:9000 > /run/natlab/ctrelay.log 2>&1 &
echo $! > /run/natlab/ctrelay.pid
