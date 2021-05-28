#!/usr/bin/bash

docker inspect --format='{{index .RepoDigests 0}}' ${TEZEDGE_IMAGE} > hash.new

if [[ $(< hash) == $(< hash.new) ]]; then
    bpf-memprof-user & sleep 0.5
    while ! docker cp $(docker ps -qf ancestor=${TEZEDGE_IMAGE}):/light-node /; do
        sleep 0.5
    done
    tezedge-memprof
else
    echo "image hash mismatch\\nexpected: $(< hash)\\nhave: $(< hash.new)"
    exit 123
fi
