#!/usr/bin/env bash

while ! echo exit | nc ocaml-node 9732; do sleep 1; done
./node $(getent hosts ocaml-node | awk '{ print $1 }'):9732
