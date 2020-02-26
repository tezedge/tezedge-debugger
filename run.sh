#!/usr/bin/env bash
cargo build
sudo setcap cap_net_raw,cap_net_admin=eip ./target/debug/tezedge_proxy
cargo run
