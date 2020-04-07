#!/usr/bin/env bash
set -e

function cleanup() {
  echo "Cleaning up rudamentary iptables rules"
  sudo iptables -F
  sudo iptables -F -t nat
  sudo iptables -F -t mangle
}

trap cleanup EXIT

cargo build --release
sudo setcap cap_net_raw,cap_net_admin=eip ./target/release/tezedge_proxy
sudo ./target/release/tezedge_proxy \
  --identity-file ./identity/identity.json \
  --port 9732 --interface eth0 \
  --local-address 192.168.70.132

bash ./clean.sh
