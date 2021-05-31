#!/usr/bin/bash

# TODO: rewrite in rust
bpf-memprof-user & sleep 0.5
while ! docker cp $(docker ps -qf name=$TEZEDGE_NODE_NAME):/light-node /; do
    sleep 0.5
done
tezedge-memprof
