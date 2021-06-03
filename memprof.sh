#!/usr/bin/bash

# TODO: rewrite in rust
bpf-memprof-user & sleep 0.5
if [[ ! -z "${TEZEDGE_NODE_NAME}" ]]; then
    while ! docker cp $(docker ps -qf name=${TEZEDGE_NODE_NAME}):/light-node /; do
        sleep 0.5
    done
fi
tezedge-memprof
