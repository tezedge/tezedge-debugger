#!/usr/bin/env bash

sudo iptables -F
sudo iptables -F -t nat
sudo iptables -F -t mangle
sudo ip rule del fwmark 1 table 1
