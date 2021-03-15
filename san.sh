#!/usr/bin/env bash

if [ $1 == "none" ]; then
    export CARGO_TARGET_DIR="target/none" RUSTFLAGS=""
    cargo build --bin=tezedge-debugger
else
    export CARGO_TARGET_DIR="target/sanitizer-$1" RUSTFLAGS="-Z sanitizer=$1"
    cargo build -Zbuild-std --target x86_64-unknown-linux-gnu --bin=tezedge-debugger
fi
