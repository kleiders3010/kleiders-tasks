#!/bin/bash
# NAT lab for E10 / D49: Linux network namespaces + veth + nftables inside WSL2 Ubuntu.
# Nothing here is connected to the root namespace, so no lab packet can reach the real internet.
#
#   lab.sh up      build everything (idempotent: tears down first)
#   lab.sh down    delete all nl-* namespaces
#   lab.sh status  list namespaces and addresses
#
# Topology (all "public" hosts share one bridge in nl-inet; optional delay via "lab.sh delay MS"):
#
#                      nl-inet  (bridge br0 $PUB4.1/24, 2001:db8::1/64, routes LAN /64s)
#        ______________________|____________________________________________
#       |       |       |       |       |                                    |
#     nl-ca  nl-stun  nl-h1   nl-h2  nl-load                     nl-r-<name> x N (WAN .20+i)
#     .5     .6,.7    .10     .11    .12                                    |  NAT / firewall
#     Pebble STUN     relay1  relay2 load gen                          nl-c-<name> 10.i.0.2
#                                                             (v6 routers: 2001:db8:i::2 as well)
#
# Router types:
#   eim   endpoint-independent mapping AND filtering (full cone), port-preserving
#   prc   port-restricted cone: Linux masquerade (port-preserving EIM, addr+port-dependent filter)
#   sym   symmetric/CGNAT-like: masquerade fully-random (endpoint-dependent mapping, random ports)
#   v6    IPv4 symmetric NAT + global IPv6 behind a stateful firewall (phone-like dual stack)
#   upnp  prc + miniupnpd (UPnP IGD + NAT-PMP/PCP)
#   upcgn prc + miniupnpd home router BEHIND a symmetric CGN (nl-r-cgn, 100.64.11.0/24)
set -u

# "Public" IPv4 prefix of the isolated lab. Not a documentation range on purpose: miniupnpd
# (like real routers) refuses to port-map when its WAN address is reserved/private.
PUB4=${PUB4:-7.7.7}
LAB=$(cd "$(dirname "$0")" && pwd)
RUN=/run/natlab; mkdir -p $RUN

# name:index:type
ROUTERS="eim1:1:eim eim2:2:eim prc1:3:prc prc2:4:prc sym1:5:sym sym2:6:sym v6a:7:v6 v6b:8:v6 upnp1:9:upnp upnp2:10:upnp upcgn:11:upcgn"

nsx() { local n=$1; shift; ip netns exec "$n" "$@"; }

mkns() {
  ip netns add "$1"
  nsx "$1" ip link set lo up
  nsx "$1" sysctl -qw net.ipv6.conf.all.accept_dad=0 net.ipv6.conf.default.accept_dad=0
  nsx "$1" sysctl -qw net.ipv6.conf.all.accept_ra=0 net.ipv6.conf.default.accept_ra=0
}

# attach a namespace to the public bridge: attach <ns> <ipv4 last octet list> [v6 suffix]
attach() {
  local ns=$1 v4s=$2 v6=${3:-} tag=${1#nl-}
  local host="p-${tag:0:12}"
  ip -n nl-inet link add "$host" type veth peer name wan netns "$ns"
  nsx nl-inet ip link set "$host" master br0 up
  nsx "$ns" ip link set wan up
  for o in $v4s; do nsx "$ns" ip addr add $PUB4.$o/24 dev wan; done
  nsx "$ns" ip route add default via $PUB4.1
  if [ -n "$v6" ]; then
    nsx "$ns" ip -6 addr add 2001:db8::$v6/64 dev wan nodad
    nsx "$ns" ip -6 route add default via 2001:db8::1
  fi
}

# stateful forward filter shared by all NAT types: LAN->WAN allowed, WAN->LAN only for
# established/related flows (plus extra rules passed in $1)
fw_table() {
cat <<EOF
table inet fw {
  chain filt {
    type filter hook forward priority 0; policy drop;
    iifname "lan" accept
    iifname "wan" ct state established,related accept
$1
  }
  # like real home routers: drop unsolicited WAN input to the router itself. Accepting it
  # would create conntrack entries that break port preservation during simultaneous open.
  chain inp {
    type filter hook input priority 0; policy accept;
    iifname "wan" ct state established,related accept
    iifname "wan" meta l4proto { icmp, ipv6-icmp } accept
    iifname "wan" drop
  }
}
EOF
}

nft_router() {
  local r=$1 i=$2 typ=$3
  local c=10.$i.0.2
  case $typ in
    prc) { cat <<EOF
table ip nat {
  chain postnat {
    type nat hook postrouting priority 100;
    oifname "wan" masquerade
  }
}
EOF
      fw_table ""; } | nsx $r nft -f - ;;
    eim) { cat <<EOF
table ip nat {
  chain postnat {
    type nat hook postrouting priority 100;
    oifname "wan" masquerade
  }
  chain prenat {
    type nat hook prerouting priority -100;
    iifname "wan" udp dport 1024-65535 dnat to $c
  }
}
EOF
      fw_table '    iifname "wan" ct status dnat accept'; } | nsx $r nft -f - ;;
    sym|cgn) { cat <<EOF
table ip nat {
  chain postnat {
    type nat hook postrouting priority 100;
    oifname "wan" masquerade fully-random
  }
}
EOF
      fw_table ""; } | nsx $r nft -f - ;;
    v6) { cat <<EOF
table ip nat {
  chain postnat {
    type nat hook postrouting priority 100;
    oifname "wan" masquerade fully-random
  }
}
EOF
      fw_table '    iifname "wan" icmpv6 type { packet-too-big, time-exceeded, parameter-problem, destination-unreachable } accept'; } | nsx $r nft -f - ;;
    upnp|upcgn) nsx $r nft -f - <<EOF
table inet filter {
  chain forward {
    type filter hook forward priority 0; policy drop;
    iifname "lan" accept
    ct state established,related accept
    jump miniupnpd
  }
  chain inp {
    type filter hook input priority 0; policy accept;
    iifname "wan" ct state established,related accept
    iifname "wan" meta l4proto { icmp, ipv6-icmp } accept
    iifname "wan" drop
  }
  chain miniupnpd {
  }
  chain prerouting {
    type nat hook prerouting priority -100; policy accept;
    jump prerouting_miniupnpd
  }
  chain postrouting {
    type nat hook postrouting priority 100; policy accept;
    jump postrouting_miniupnpd
    oifname "wan" masquerade
  }
  chain prerouting_miniupnpd {
  }
  chain postrouting_miniupnpd {
  }
}
EOF
    ;;
  esac
}

start_miniupnpd() {
  local r=$1 i=$2
  cat > $RUN/miniupnpd-$i.conf <<EOF
ext_ifname=wan
listening_ip=lan
http_port=5000
enable_natpmp=yes
enable_upnp=yes
secure_mode=yes
system_uptime=yes
uuid=00000000-0000-4000-8000-0000000000$(printf %02d $i)
allow 1-65535 10.$i.0.0/24 1-65535
deny 0-65535 0.0.0.0/0 0-65535
EOF
  setsid nohup ip netns exec $r miniupnpd -f $RUN/miniupnpd-$i.conf -P $RUN/miniupnpd-$i.pid -d > $RUN/miniupnpd-$i.log 2>&1 &
}

down() {
  pkill -f "miniupnpd -f $RUN" 2>/dev/null
  for n in $(ip netns list | awk '{print $1}' | grep '^nl-'); do
    ip netns pids "$n" 2>/dev/null | xargs -r kill 2>/dev/null
    ip netns del "$n"
  done
}

up() {
  down; sleep 0.3
  mkns nl-inet
  nsx nl-inet sysctl -qw net.ipv4.ip_forward=1 net.ipv6.conf.all.forwarding=1
  nsx nl-inet ip link add br0 type bridge
  nsx nl-inet ip link set br0 up
  nsx nl-inet ip addr add $PUB4.1/24 dev br0
  nsx nl-inet ip -6 addr add 2001:db8::1/64 dev br0 nodad
  nsx nl-inet sysctl -qw net.ipv4.conf.all.send_redirects=0 net.ipv4.conf.br0.send_redirects=0
  # core port for the delaying userspace switch (unused while the kernel bridge is active)
  nsx nl-inet ip link add p-core type veth peer name core0
  nsx nl-inet ip link set core0 master br0 up
  nsx nl-inet ip link set p-core up

  for h in "ca:5:5" "stun:6 7:6" "h1:10:10" "h2:11:11" "load:12:12"; do
    IFS=: read n o v6 <<<"$h"; mkns nl-$n; attach nl-$n "$o" "$v6"
  done

  # CGN for the upcgn router: public .31, inside 100.64.11.0/24
  mkns nl-r-cgn; attach nl-r-cgn 31 31
  nsx nl-r-cgn sysctl -qw net.ipv4.ip_forward=1
  for R in $ROUTERS; do
    IFS=: read name i typ <<<"$R"
    r=nl-r-$name; c=nl-c-$name
    mkns $r; mkns $c
    nsx $r sysctl -qw net.ipv4.ip_forward=1 net.ipv6.conf.all.forwarding=1
    if [ "$typ" = upcgn ]; then
      ip -n nl-r-cgn link add lan type veth peer name wan netns $r
      nsx nl-r-cgn ip link set lan up
      nsx nl-r-cgn ip addr add 100.64.11.1/24 dev lan
      nsx $r ip link set wan up; nsx $r ip addr add 100.64.11.2/24 dev wan
      nsx $r ip route add default via 100.64.11.1
      nft_router nl-r-cgn 0 cgn
    else
      attach $r $((20+i)) $((20+i))
    fi
    ip -n $r link add lan type veth peer name eth0 netns $c
    nsx $r ip link set lan up; nsx $c ip link set eth0 up
    nsx $r ip addr add 10.$i.0.1/24 dev lan
    nsx $c ip addr add 10.$i.0.2/24 dev eth0
    nsx $c ip route add default via 10.$i.0.1
    if [ "$typ" = v6 ]; then
      nsx $r ip -6 addr add 2001:db8:$i::1/64 dev lan nodad
      nsx $c ip -6 addr add 2001:db8:$i::2/64 dev eth0 nodad
      nsx $c ip -6 route add default via 2001:db8:$i::1
      nsx nl-inet ip -6 route add 2001:db8:$i::/64 via 2001:db8::$((20+i))
    fi
    nft_router $r $i $typ
    case $typ in upnp|upcgn) start_miniupnpd $r $i;; esac
  done
  # guard: the root namespace (WSL VM uplink) must be untouched
  ip link show eth0 2>/dev/null | grep -q "state UP" || echo "WARNING: root-namespace eth0 is not UP"
  echo "lab up"
}

# WAN latency: the kernel bridge is swapped for a userspace delaying switch (natlab wanemu).
# Each frame crossing the core is delayed MS: host<->host RTT = 2*MS, relayed RTT = 4*MS.
delay_on() {
  local ms=${1:-20}; delay_off
  local ports=""
  for p in $(nsx nl-inet ls /sys/class/net | grep "^p-"); do
    ports="$ports --if $p"
    [ "$p" = p-core ] && continue
    nsx nl-inet ip link set $p nomaster
    nsx nl-inet ethtool -K $p gro off gso off tso off tx off rx off >/dev/null 2>&1
    nsx nl-${p#p-} ethtool -K wan gro off gso off tso off tx off rx off tx-udp-segmentation off >/dev/null 2>&1
  done
  nsx nl-inet ethtool -K p-core gro off gso off tso off tx off rx off >/dev/null 2>&1
  nsx nl-inet ethtool -K core0 gro off gso off tso off tx off rx off >/dev/null 2>&1
  setsid nohup ip netns exec nl-inet natlab wanemu --ms $ms $ports > $RUN/wanemu.log 2>&1 &
  sleep 0.5; cat $RUN/wanemu.log
}
delay_off() {
  pkill -f "natlab wanemu" 2>/dev/null; sleep 0.2
  for p in $(nsx nl-inet ls /sys/class/net | grep "^p-" | grep -v "^p-core$"); do
    nsx nl-inet ip link set $p master br0
  done
  true
}

status() {
  for n in $(ip netns list | awk '{print $1}' | grep '^nl-' | sort); do
    echo "$n: $(nsx $n ip -br addr | grep -v '^lo' | awk '{printf "%s %s %s; ", $1, $3, $4}')"
  done
}

case "${1:-}" in
  up) up;; down) down;; status) status;;
  delay) delay_on "${2:-20}";; nodelay) delay_off;;
  *) echo "usage: $0 up|down|status|delay [ms]|nodelay"; exit 1;;
esac
