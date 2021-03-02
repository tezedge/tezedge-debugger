#!/usr/bin/env bash

if [ $1 == "none" ]; then
    export CARGO_TARGET_DIR="target/none"
    cargo build --bin=tezedge-debugger
    cp $CARGO_TARGET_DIR/debug/tezedge-debugger bin/tezedge-debugger
    cp debugger_config.toml bin/debugger-config.toml
else
    export CARGO_TARGET_DIR="target/sanitizer-$1" RUSTFLAGS="-Z sanitizer=$1"
    cargo build -Zbuild-std --target x86_64-unknown-linux-gnu --bin=tezedge-debugger
    cp $CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/debug/tezedge-debugger bin/tezedge-debugger
    cp debugger_config.toml bin/debugger-config.toml
fi
