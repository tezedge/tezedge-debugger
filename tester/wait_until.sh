#!/bin/bash

HTTPD="0"
until [ "$HTTPD" == "200" ]; do
  HTTPD=$(curl -A "Web Check" -sL --connect-timeout 3 -w "%{http_code}\n" "$URL" -o /dev/null)
  printf '.'
  sleep 3
done
