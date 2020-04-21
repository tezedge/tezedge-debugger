#!/bin/bash
IDENTITY=./identity/identity.json
EXECUTABLE=./tezedge_proxy
TUN=/dev/net/tun
TUN0_NAME=tun0
TUN0_ADDR=10.0.1.0
TUN1_NAME=tun1
TUN1_ADDR=10.0.1.0
ADDR_SPACE=24
RPC_PORT=10000
FILES=("$IDENTITY" "$EXECUTABLE" "$TUN")

function clean() {
  del_tun "$TUN0_NAME"
  del_tun "$TUN1_NAME"
}

function set_tun() {
  local TUN_NAME="$1"
  local TUN_ADDR="$2"
  local TUN_SPACE="$3"
  ip tuntap add dev "$TUN_NAME" mode tun
  ip addr add "$TUN_ADDR/$TUN_SPACE" dev "$TUN_NAME"
  ip link set dev "$TUN_NAME" up
}

function del_tun() {
  ip tuntap del dev "$1" mode tun
}

function nextip() {
  IP=$1
  IP_HEX=$(printf '%.2X%.2X%.2X%.2X\n' $(echo $IP | sed -e 's/\./ /g'))
  NEXT_IP_HEX=$(printf %.8X $(echo $((0x$IP_HEX + 1))))
  NEXT_IP=$(printf '%d.%d.%d.%d\n' $(echo $NEXT_IP_HEX | sed -r 's/(..)/0x\1 /g'))
  echo "$NEXT_IP"
}

function test_reachability() {
  local TUN_NAME="$1"
  local TUN_ADDR="$2"
  local RC=1
  ((COUNT = 3))
  while [[ $COUNT -ne 0 ]]; do
    ping -c 1 -W 1 "$TUN_ADDR" &>/dev/null
    RC="$?"
    if [[ "$RC" -eq 0 ]]; then
      ((COUNT = 1))
    fi
    ((COUNT = COUNT - 1))
  done

  if [[ "$RC" -ne 0 ]]; then
    err "Address $TUN_ADDR unreachable, $TUN_NAME set incorrectly"
  fi
}

function err() {
  echo "error: $*" >>/dev/stderr
  exit 1
}

for FILE in ${FILES[*]}; do
  if [ ! -f "$FILE" ] && [ ! -c "$FILE" ]; then
    err "$FILE does not exist"
  fi
done

trap clean EXIT

# === Command Section === #
# Set tun devices
set_tun "$TUN0_NAME" "$TUN0_ADDR" "$ADDR_SPACE"
set_tun "$TUN1_NAME" "$TUN1_ADDR" "$ADDR_SPACE"

# Allow IP forwarding
IP_FORWARD=$(cat "/proc/sys/net/ipv4/ip_forward")
if [ "$IP_FORWARD" -ne 1 ]; then
  sysctl -w "net.ipv4.ip_forward=1"
fi

# Make reverse path filtering more permissive
DEVICES=("tun0" "eth0" "default")
for DEVICE in ${DEVICES[*]}; do
  RP_FILTER=$(cat "/proc/sys/net/ipv4/conf/$DEVICE/rp_filter")
  if [ "$RP_FILTER" -ne 2 ]; then
    sysctl -w "net.ipv4.conf.$DEVICE.rp_filter=2"
  fi
done

# Allow forwarding from tun1
iptables -P FORWARD ACCEPT
iptables -t nat -A POSTROUTING -s "$TUN1_ADDR/$ADDR_SPACE" -j MASQUERADE

# Test internet connectivity
test_reachability "internet connection" "8.8.8.8"

${EXECUTABLE} --identity-file "$IDENTITY" \
  --rpc-port "$RPC_PORT" \
  --interface eth0 \
  --local-address "$(nextip $TUN1_ADDR)" \
  --tun0-name "$TUN0_NAME" --tun0-address-space "$TUN1_ADDR/$ADDR_SPACE" --tun0-address "$(nextip $TUN1_ADDR)" \
  --tun1-name "$TUN1_NAME" --tun1-address-space "$TUN1_ADDR/$ADDR_SPACE" --tun1-address "$(nextip $TUN1_ADDR)"
