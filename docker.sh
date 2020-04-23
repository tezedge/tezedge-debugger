#!/bin/bash
VOLUME="$PWD/identity"
IDENTITY_FILE="$VOLUME/identity.json"
CONFIG_FILE="$VOLUME/tezedge.config"
RPC_PORT=10000
FILES=("$IDENTITY_FILE" "$CONFIG_FILE")

trap clean EXIT

function clean() {
  if docker kill "$PROXY_ID" &>/dev/null; then
    echo "Killed proxy container"
  fi
  if docker kill "$NODE_ID" &>/dev/null; then
    echo "Killed node container"
  fi
}

function err() {
  echo "[-] $*" >>/dev/stderr
  exit 1
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

# == CHECK THAT REQUIRED FILES EXISTS ==

if [ ! -d "$VOLUME" ]; then
  err "Required director \"$VOLUME\" does not exists"
fi

if [ ! -d "/var/run/netns" ]; then
  sudo ip netns add make_ns
  sudo ip netns del make_ns
fi

for FILE in ${FILES[*]}; do
  if [ ! -f "$FILE" ]; then
    err "Required file \"$FILE\" does not exists"
  fi
done

# == START PROXY IN DETACHED MODE ==
PROXY_ID=$(docker run -d --cap-add=NET_ADMIN -p "10000:$RPC_PORT" --volume "$VOLUME:/home/appuser/proxy/identity" --device /dev/net/tun:/dev/net/tun -it kyras/tezedge_proxy:latest)
echo "Spawned proxy in container $PROXY_ID"
sleep 1

# == START NODE IN DETACHED MODE ==
# 1. make inactive container
NODE_ID=$(docker run -d --volume "$VOLUME:/root/identity/" kyras/tezedge_tezos sleep inf)
docker exec "$NODE_ID" mkdir /root/.tezos-node
docker exec "$NODE_ID" cp /root/identity/identity.json /root/.tezos-node/
echo "Spawned tezedge container $NODE_ID"
mount_ns "$NODE_ID"
mount_ns "$PROXY_ID"
# 2. move tun0 from PROXY container into NODE container
sudo ip netns exec "$PROXY_ID" ip link set tun0 netns "$NODE_ID"
# 3. setup tun0 in NODE container and set it as a default route
sudo ip netns exec "$NODE_ID" ip addr add 10.0.1.1/24 dev tun0
sudo ip netns exec "$NODE_ID" ip link set dev tun0 up
sudo ip netns exec "$NODE_ID" ip route del 0/0
sudo ip netns exec "$NODE_ID" ip route add default via 10.0.1.1
echo "Moved tun0 from proxy container to node container"
until curl --output /dev/null --silent --head --fail "localhost:$RPC_PORT/data/0/0"; do
  sleep 1
done
echo "Proxy running successfully on port $RPC_PORT"
unmount_ns "$NODE_ID"
unmount_ns "$PROXY_ID"
# 4. start node in existing container
#docker exec -it "$NODE_ID" /bin/bash
docker exec "$NODE_ID" ./tezos-node run
