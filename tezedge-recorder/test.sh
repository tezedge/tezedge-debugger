#!/usr/bin/env bash

export START_TIME=1626264000

function fail {
    kill -SIGTERM $RECORDER_PID
    exit 1
}

function run_recorder {
    ./target/none/release/tezedge-recorder --run-bpf &
    export RECORDER_PID=$!
    sleep 4
}

function stop_recorder {
    kill -SIGTERM $RECORDER_PID
    sleep 2
}

run_recorder
./target/none/release/pseudonode log 0 && sleep 1 # populate first half log messages
./target/none/release/deps/log-???????????????? --nocapture \
    pagination level timestamp_and_level || fail
stop_recorder

run_recorder
./target/none/release/deps/log-???????????????? --nocapture \
    pagination level timestamp_and_level || fail
./target/none/release/pseudonode log 1 && sleep 1 # populate second half log messages
./target/none/release/deps/log-???????????????? --nocapture \
    pagination level timestamp timestamp_and_level || fail
stop_recorder

run_recorder
./target/none/release/deps/log-???????????????? --nocapture \
    pagination level timestamp timestamp_and_level || fail
# populate p2p messages
./target/none/release/pseudonode p2p-responder 29733 29732 & RESPONDER_PID=$! && sleep 1
./target/none/release/pseudonode p2p-initiator 29732 29733 && wait $RESPONDER_PID && sleep 5
./target/none/release/deps/p2p-???????????????? --nocapture check_messages || fail
./target/none/release/pseudonode log 2 && sleep 4 # populate words log messages
./target/none/release/deps/log-???????????????? --nocapture full_text_search || fail
stop_recorder
