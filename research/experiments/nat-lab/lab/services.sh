#!/bin/bash
# Lab services on top of lab.sh: STUN, Pebble (local ACME CA), IP-certificate issuance, relays.
#   services.sh stun
#   services.sh pebble                       start Pebble in nl-ca (7.7.7.5), fetch its issuing root
#   services.sh cert <ns> <dir> <ip> [ip..]  ACME IP-address certificate via instant-acme (http-01)
#   services.sh selfsign <dir> <ip> [ip..]   fallback self-signed CA + IP-SAN certificate
#   services.sh relay <ns> <ip> <certdir> <name> [extra natlab relay args]
#   services.sh stop <name>
set -u
PUB4=${PUB4:-7.7.7}
RUN=/run/natlab; mkdir -p $RUN
PEBBLE=/opt/pebble/pebble-linux-amd64/linux/amd64/pebble

case "${1:-}" in
stun)
  pkill -f "natlab stunsrv" 2>/dev/null
  setsid nohup ip netns exec nl-stun natlab stunsrv --listen $PUB4.6:3478 --listen $PUB4.7:3478 > $RUN/stun.log 2>&1 &
  echo $! > $RUN/stun.pid ;;
pebble)
  pkill -f "$PEBBLE" 2>/dev/null
  P=$RUN/pebble; mkdir -p $P; cd $P
  # TLS for Pebble's own ACME API (not the certificates it issues)
  openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 30 \
    -subj "/CN=natlab pebble API CA" -keyout apica.key -out apica.pem 2>/dev/null
  openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -subj "/CN=pebble" \
    -keyout api.key -out api.csr 2>/dev/null
  printf "subjectAltName=IP:$PUB4.5\n" > san.ext
  openssl x509 -req -in api.csr -CA apica.pem -CAkey apica.key -CAcreateserial -days 30 \
    -extfile san.ext -out api.pem 2>/dev/null
  cat > pebble.json <<EOF
{ "pebble": {
  "listenAddress": "$PUB4.5:14000", "managementListenAddress": "$PUB4.5:15000",
  "certificate": "$P/api.pem", "privateKey": "$P/api.key",
  "httpPort": 80, "tlsPort": 443, "ocspResponderURL": "",
  "externalAccountBindingRequired": false,
  "retryAfter": { "authz": 1, "order": 1 },
  "keyAlgorithm": "ecdsa",
  "profiles": {
    "default":    { "description": "90 days", "validityPeriod": 7776000 },
    "shortlived": { "description": "160 h like Let's Encrypt IP certificates", "validityPeriod": 576000 }
  } } }
EOF
  PEBBLE_VA_NOSLEEP=1 PEBBLE_WFE_NONCEREJECT=0 PEBBLE_AUTHZREUSE=0 \
    setsid nohup ip netns exec nl-ca $PEBBLE -config pebble.json > $RUN/pebble.log 2>&1 &
  echo $! > $RUN/pebble.pid
  for i in $(seq 50); do
    ip netns exec nl-ca curl -s --cacert apica.pem https://$PUB4.5:15000/roots/0 > root.pem 2>/dev/null && grep -q BEGIN root.pem && break
    sleep 0.2
  done
  echo "pebble up; issuing root: $P/root.pem" ;;
cert)
  ns=$2; dir=$3; shift 3
  mkdir -p $dir
  args=""; for ip in "$@"; do args="$args --ip $ip"; done
  ip netns exec $ns natlab acme --dir https://$PUB4.5:14000/dir --root $RUN/pebble/apica.pem $args \
     --out-cert $dir/cert.pem --out-key $dir/key.pem
  cp $RUN/pebble/root.pem $dir/ca.pem ;;
selfsign)
  dir=$2; shift 2
  args=""; for ip in "$@"; do args="$args --ip $ip"; done
  natlab selfsign --out-dir $dir $args ;;
relay)
  ns=$2; ip=$3; dir=$4; name=$5; shift 5
  setsid nohup ip netns exec $ns natlab relay --ip $ip --cert $dir/cert.pem --key $dir/key.pem "$@" > $RUN/relay-$name.log 2>&1 &
  echo $! > $RUN/relay-$name.pid
  for i in $(seq 50); do grep -q relay_ready $RUN/relay-$name.log && break; sleep 0.1; done
  cat $RUN/relay-$name.log ;;
stop)
  [ -f $RUN/$2.pid ] && kill $(cat $RUN/$2.pid) 2>/dev/null; rm -f $RUN/$2.pid ;;
*) echo "usage: see header"; exit 1 ;;
esac
