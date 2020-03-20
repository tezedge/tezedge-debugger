#!/usr/bin/env bash
set -e
trap 'sudo iptables -t nat -F' INT
cargo build
sudo setcap cap_net_raw,cap_net_admin=eip ./target/debug/tezedge_proxy
sudo ./target/debug/tezedge_proxy --identity-file ./identity/identity.json --port 9732
