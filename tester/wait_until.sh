#!/usr/bin/env bash

URL="localhost:18732/chains/main/blocks/head"
DEBUGGER_URL="localhost:17732/v2/p2p?limit=1000"

HTTPD="0"
until [ "$HTTPD" == "200" ]; do
  HTTPD=$(curl -A "Web Check" -sL --connect-timeout 3 -w "%{http_code}\n" "$URL" -o /dev/null)
  printf '.'
  sleep 3
done

RESPONSE="0"
until [ "$RESPONSE" == "1000" ]; do
  HTTPD=$(curl -sL --connect-timeout 3 "$URL" | jq length)
  printf '!'
  sleep 0.5
done
