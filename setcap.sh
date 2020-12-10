#!/usr/bin/env bash

sudo setcap 'CAP_NET_RAW+eip CAP_NET_ADMIN+eip' target/x86_64-unknown-linux-gnu/debug/tezedge-debugger
# sudo setcap 'CAP_NET_RAW+eip CAP_NET_ADMIN+eip' ~/.vscode/extensions/vadimcn.vscode-lldb-1.6.0/lldb/bin/lldb-server
