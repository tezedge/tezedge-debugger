#!/usr/bin/env bash

source secrets.sh

case $1 in
  insert_secret)
    sed -i 's@SLACK_HOOKS_URL.*@SLACK_HOOKS_URL='"${SLACK_HOOKS_URL}"'@' docker-compose.{rust,ocaml}.yml
    ;;
  remove_secret)
    sed -i 's@SLACK_HOOKS_URL.*@SLACK_HOOKS_URL@' docker-compose.{rust,ocaml}.yml
    ;;
esac
