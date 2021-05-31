#!/usr/bin/bash

# TODO: rewrite in rust
bpf-memprof-user & sleep 0.5
while ! docker cp $(docker ps -qf ancestor=$TEZEDGE_IMAGE):/light-node /; do
    sleep 0.5
done
tezedge-memprof
