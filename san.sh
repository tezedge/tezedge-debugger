#!/usr/bin/env bash

if [ $1 == "none" ]; then
    export CARGO_TARGET_DIR="target/none" RUSTFLAGS=""
    cargo +nightly-2021-03-23 build -p tezedge-recorder
else
    export CARGO_TARGET_DIR="target/sanitizer-$1" RUSTFLAGS="-Z sanitizer=$1"
    cargo +nightly-2021-03-23 build -Zbuild-std --target x86_64-unknown-linux-gnu -p tezedge-recorder
fi
