#!/usr/bin/env bash
# Dependencies:
# Shell  `cgroup-tools` required for cgcreate command
# Python `pypacker` required for ping health-check
net_cls_name="tezedge_proxy"
net_cls_id=0x11000011
net_cls_mark=11
net_cls_table=11
interface="tun0"
cleanup="false"

# === Code ===
create_net_cls() {
  echo "Created new cgroup $net_cls_name"
  sudo cgcreate -g net_cls:/"${net_cls_name}"
}

valued_ping() {
  sudo python3 -c 'import socket
s = socket.socket(socket.AF_INET, socket.SOCK_RAW, socket.IPPROTO_ICMP)
s.setsockopt(socket.SOL_IP, socket.IP_HDRINCL, 0)
s.settimeout(1)
packet = b"\x08\x00\xf7\xd3\x00*\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01"
s.sendto(packet, ("8.8.8.8", 0))
recv = s.recv(56)
print(recv[-1])'
}

setup_net_cls() {
  sudo cgset -r net_cls.classid=${net_cls_id} ${net_cls_name} 2>/dev/null
  echo "Set classid for $net_cls_name to $net_cls_id"
  sudo iptables -t mangle -A OUTPUT -m cgroup --cgroup "${net_cls_id}" -j MARK --set-mark ${net_cls_mark} 2>/dev/null
  echo "Set mark $net_cls_mark for packets from classid $net_cls_id"
  sudo ip rule add fwmark $net_cls_mark table "${net_cls_table}" 2>/dev/null
  echo "Associated marked ($net_cls_mark) packets with table $net_cls_table"
  sudo ip route add default dev "${interface}" table "${net_cls_table}" 2>/dev/null
  echo "Routing all packets from table $net_cls_table through interface $interface"
}

delete_net_cls() {
  sudo cgdelete net_cls:/"${net_cls_name}"
  echo "Deleted net_cls $net_cls_name" &>/dev/null
}

clean() {
  if [ "$cleanup" = "true" ]; then
    echo "Cleaning configuration"
    sudo iptables -t mangle -D OUTPUT -m cgroup --cgroup "${net_cls_id}" -j MARK --set-mark ${net_cls_mark} 2>/dev/null
    sudo ip route del default dev "${interface}" table "${net_cls_table}" 2>/dev/null
    sudo ip rule del fwmark $net_cls_mark table "${net_cls_table}" 2>/dev/null
    echo "Reverted routing on net_cls $net_cls_name through interface $interface"
    delete_net_cls ${net_cls_name}
  fi
}

run_in_net_cls() {
  sudo -E env "PATH=$PATH" cgexec -g net_cls:/"${net_cls_name}" "$@"
}

# === MAIN ===
trap clean EXIT
if [ ! "$(cat /sys/class/net/"${interface}"/operstate 2>/dev/null)" = up ]; then
  echo "Interface $interface is not up or does not exists. Aborting"
  exit 1
fi

cleanup="true"
if ! cgget -g net_cls:${net_cls_name} 2>/dev/null | grep -q net_cls.classid; then
  create_net_cls
else
  echo "Net class $net_cls_name already exists"
  # This should be handled somehow, probably, ...
fi
setup_net_cls
ping_value=$(sudo -E env "PATH=$PATH" cgexec -g net_cls:/"${net_cls_name}" python3 -c 'import socket
s = socket.socket(socket.AF_INET, socket.SOCK_RAW, socket.IPPROTO_ICMP)
s.setsockopt(socket.SOL_IP, socket.IP_HDRINCL, 0)
s.settimeout(1)
packet = b"\x08\x00\xf7\xd3\x00*\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01"
s.sendto(packet, ("8.8.8.8", 0))
recv = s.recv(56)
print(recv[-1])' 2>/dev/null)

if [ "$ping_value" ]; then
  echo "Failed health-check. Proxy not set correctly."
  exit 1
fi

echo "Running $* in net_cls $net_cls_name"
run_in_net_cls "$@"
