#!/bin/bash

OUTPUT="./sizes"

function pid_size() {
  PID="$1"
  pmap "$PID"|grep total|grep -o "[0-9]*"
}

function get_pids() {
  NAME="$1"
  ps | grep "$NAME" |
}

function store_process_size() {
  PROCESS="$1"
  PIDS="$(get_pids "$PROCESS")"
  for PID in $PIDS;
  do
    SIZE=$(pid_size "$PID")
    LINE="$(date -u --rfc-3339=seconds): $PROCESS ($PID) - ${SIZE}KB"
    echo "$LINE"
    echo "$LINE" >> "$OUTPUT"
  done
}

while :
do
  store_process_size light-node
  store_process_size protocol-runner
  sleep 10s
done
