#!/bin/bash
VOLUME="$PWD/identity"
IDENTITY_FILE="$VOLUME/identity.json"
PROXY_RPC_PORT=17732
NODE_RPC_PORT=18732
DEBUGGER_TAG=dev
DEBUGGER_IMAGE="simplestakingcom/tezedge-debuger:$DEBUGGER_TAG"

trap clean EXIT

function clean() {
  if docker kill "$PROXY_ID" &>/dev/null; then
    echo "Killed proxy container"
  fi
  if docker kill "$NODE_ID" &>/dev/null; then
    echo "Killed node container"
  fi

  if docker kill "$EXPLORER_ID" &>/dev/null; then
    echo "Killed explorer container"
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

docker pull simplestakingcom/tezedge-tezos:latest &>/dev/null
#docker pull "DEBUGGER_IMAGE"
docker pull simplestakingcom/tezedge-explorer-ocaml &>/dev/null

# Check identity
if [ ! -f "$IDENTITY_FILE" ]; then
  docker run --volume "$VOLUME:/root/identity" -it simplestakingcom/tezedge-tezos:latest /bin/bash -c "./tezos-node identity generate && cp /root/.tezos-node/identity.json /root/identity"
fi

# == START PROXY IN DETACHED MODE ==
PROXY_ID=$(docker run -d --cap-add=NET_ADMIN -p "$PROXY_RPC_PORT:10000" -p "$NODE_RPC_PORT:8732" -p "19732:9732" -p "4927:4927" --volume "$VOLUME:/home/appuser/proxy/identity" --device /dev/net/tun:/dev/net/tun -it "$DEBUGGER_IMAGE")
docker exec "$PROXY_ID" iptables -t nat -A PREROUTING -p tcp --dport 8732 -j DNAT --to-destination 10.0.1.1
echo "Spawned proxy in container $PROXY_ID"
sleep 1

# == START NODE IN DETACHED MODE ==
# 1. make inactive container
NODE_ID=$(docker run -d --volume "$VOLUME:/root/identity/" simplestakingcom/tezedge-tezos:latest sleep inf)
docker exec "$NODE_ID" mkdir /root/.tezos-node
docker exec "$NODE_ID" cp /root/identity/identity.json /root/.tezos-node/
echo "Spawned tezedge container $NODE_ID"
mount_ns "$NODE_ID"
mount_ns "$PROXY_ID"
# 2. move tun0 from PROXY container into NODE container
sudo ip netns exec "$PROXY_ID" ip link set tun0 netns "$NODE_ID"
# 3. setup tun0 in NODE container and set it as a default route
sudo ip netns exec "$NODE_ID" ip route
sudo ip netns exec "$NODE_ID" ip addr add 10.0.1.1/24 dev tun0
sudo ip netns exec "$NODE_ID" ip link set dev tun0 up
sudo ip netns exec "$NODE_ID" ip route del 0/0
sudo ip netns exec "$NODE_ID" ip route del 172.17.0.0/16
sudo ip netns exec "$NODE_ID" ip route add default via 10.0.1.1
echo "Moved tun0 from proxy container to node container"
until curl -X GET --output /dev/null --silent --head --fail "localhost:$PROXY_RPC_PORT/v2/p2p?count=0"; do
  sleep 1
done
echo "Proxy running successfully on port $PROXY_RPC_PORT"
unmount_ns "$NODE_ID"
unmount_ns "$PROXY_ID"
# 4. start node in existing container
#docker exec -it "$NODE_ID" /bin/bash
EXPLORER_ID=$(docker run -d -p "8080:8080" simplestakingcom/tezedge-explorer-ocaml:latest)
echo "Running explorer on port 8080 in container $EXPLORER_ID"
docker exec "$NODE_ID" ./tezos-node run --rpc-addr 0.0.0.0:8732
