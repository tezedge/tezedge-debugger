#!/usr/bin/env bash
set -e
#trap 'sudo iptables -D OUTPUT -p tcp --dport 9732 -j DROP' 0
cargo build
sudo setcap cap_net_raw,cap_net_admin=eip ./target/debug/tezedge_proxy
#sudo iptables -A OUTPUT -p tcp --dport 9732 -j DROP
sudo ./target/debug/tezedge_proxy --identity-file ./identity/identity.json --port 9732
