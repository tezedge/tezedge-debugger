#!/usr/bin/env bash

function fail {
    kill -SIGTERM $RECORDER_PID
    exit 1
}

function run_recorder {
    ./target/none/release/tezedge-recorder --run-bpf &
    export RECORDER_PID=$!
    sleep 2
}

function stop_recorder {
    kill -SIGTERM $RECORDER_PID
    sleep 2
}

run_recorder
./target/none/release/pseudonode log 0 # populate first half log messages
./target/none/release/deps/log-???????????????? -- pagination level || fail
stop_recorder

run_recorder
./target/none/release/deps/log-???????????????? -- pagination level || fail
./target/none/release/pseudonode log 1 # populate second half log messages
./target/none/release/deps/log-???????????????? -- pagination level timestamp || fail
stop_recorder

run_recorder
./target/none/release/deps/log-???????????????? -- pagination level timestamp || fail
# populate p2p messages
./target/none/release/pseudonode p2p-responder 29733 29732 & RESPONDER_PID=$!
./target/none/release/pseudonode p2p-initiator 29732 29733 && wait $RESPONDER_PID && sleep 5
./target/none/release/deps/p2p-???????????????? --nocapture -- check_messages || fail
stop_recorder
