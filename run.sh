#!/usr/bin/env bash
set -e

trap ./clean.sh INT

cargo build
sudo setcap cap_net_raw,cap_net_admin=eip ./target/debug/tezedge_proxy
sudo RUST_BACKTRACE=1 ./target/debug/tezedge_proxy \
  --identity-file ./identity/identity.json \
  --port 9732 --interface enp34s0 \
  --local-address 192.168.1.199
