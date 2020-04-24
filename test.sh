#!/bin/bash

PROXY=proxy
TESTER=tester

function clean() {
  docker rm -f $PROXY &>/dev/null
  docker rm -f $TESTER &>/dev/null
}

function mount_ns() {
  CONTAINER_ID=$1
  CONTAINER_PID=$(docker inspect -f '{{.State.Pid}}' "$CONTAINER_ID")
  sudo ln -sfT "/proc/$CONTAINER_PID/ns/net" "/var/run/netns/$CONTAINER_ID"
}

function unmount_ns() {
  CONTAINER_ID=$1
  sudo unlink "/var/run/netns/$CONTAINER_ID" &>/dev/null
}

trap clean EXIT

clean
gnome-terminal -- docker run --cap-add=NET_ADMIN --name $PROXY -p "10000:10000" -p "8732:8732" --volume "$VOLUME:/home/appuser/proxy/identity" --device /dev/net/tun:/dev/net/tun -it kyras/tezedge_proxy:latest &>/dev/null
gnome-terminal -- docker run --name $TESTER -it tezedge_tester /bin/bash &>/dev/null
sleep 5
mount_ns "$PROXY"
mount_ns "$TESTER"
#sudo ip netns exec "$PROXY" ip link set tun0 netns "$NODE_ID"
## 3. setup tun0 in NODE container and set it as a default route
#sudo ip netns exec "$TESTER" ip addr add 10.0.1.1/24 dev tun0
#sudo ip netns exec "$TESTER" ip link set dev tun0 up
#sudo ip netns exec "$TESTER" ip route del 0/0
#sudo ip netns exec "$TESTER" ip route add default via 10.0.1.1
#until curl --output /dev/null --silent --head --fail "localhost:10000/data/0/0"; do
#  sleep 1
#done
#echo "Proxy running successfully on port $RPC_PORT"
#unmount_ns "$TESTER"
#unmount_ns "$PROXY"
sleep inf
