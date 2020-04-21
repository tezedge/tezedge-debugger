#!/usr/bin/env bash
set -e

cargo build --release
sudo setcap cap_net_raw,cap_net_admin=eip ./target/release/tezedge_proxy
sudo ./target/release/tezedge_proxy \
  --identity-file ./identity/identity.json \
  --rpc-port 9732 --interface eth0 \
  --local-address 192.168.1.199
