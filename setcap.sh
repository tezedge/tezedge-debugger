#!/usr/bin/env bash

sudo setcap 'CAP_NET_RAW+eip CAP_NET_ADMIN+eip CAP_SYS_ADMIN+eip' target/debug/tezedge-debugger
# sudo setcap 'CAP_NET_RAW+eip CAP_NET_ADMIN+eip' ~/.vscode/extensions/vadimcn.vscode-lldb-1.6.0/lldb/bin/lldb-server
