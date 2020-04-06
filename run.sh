#!/usr/bin/env bash
set -e

trap ./clean.sh INT

cargo build --release
sudo setcap cap_net_raw,cap_net_admin=eip ./target/release/tezedge_proxy
sudo RUST_BACKTRACE=1 ./target/release/tezedge_proxy \
  --identity-file ./identity/identity.json \
  --port 9732 --interface eth0 \
  --local-address 192.168.70.132
