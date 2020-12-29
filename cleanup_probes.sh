#!/usr/bin/env bash

for probe in /sys/kernel/debug/tracing/events/kprobes/*; do
    if [ -d "$probe" ]; then
        echo "removing" "$probe"
        echo 0 > "$probe"/enable
    fi
done

echo 0 > /sys/kernel/debug/tracing/tracing_on
echo '' > /sys/kernel/debug/tracing/kprobe_events
