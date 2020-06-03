#!/bin/bash
if [ -z "${VOLUME+x}" ]; then
  VOLUME="$PWD/identity"
fi

IDENTITY_FILE="$VOLUME/identity.json"
# RPC endpoints for debugger / node
DEBUGGER_RPC_PORT=17732
NODE_RPC_PORT=18732
# Node specific ports
NODE_P2P_PORT=19732
NODE_WS_PORT=4972
TAG=dev

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

if [ -z "${NODE_TYPE+x}" ]; then
  NODE_TYPE="OCAML"
fi

if [ "$NODE_TYPE" = "OCAML" ] || [ "$NODE_TYPE" = "RUST" ]; then
  echo "Running debugger with $NODE_TYPE node"
else
  err "NODE_TYPE set to invalid value: $NODE_TYPE. Valid are OCAML or RUST"
fi

docker pull simplestakingcom/tezedge-debuger:"$TAG"
docker pull simplestakingcom/tezedge-explorer-ocaml

# Check identity
if [ ! -f "$IDENTITY_FILE" ]; then
  docker run --volume "$VOLUME:/root/identity" -i simplestakingcom/tezedge-tezos:"$TAG" /bin/bash -c "./tezos-node identity generate && cp /root/.tezos-node/identity.json /root/identity"
fi

# == START PROXY IN DETACHED MODE ==
PROXY_ID=$(docker run -d --cap-add=NET_ADMIN -p "$DEBUGGER_RPC_PORT:10000" -p "$NODE_RPC_PORT:8732" -p "$NODE_P2P_PORT:9732" -p "$NODE_WS_PORT:4927" --volume "$VOLUME:/home/appuser/proxy/identity" --device /dev/net/tun:/dev/net/tun -i simplestakingcom/tezedge-debuger:"$TAG")
docker exec "$PROXY_ID" iptables -t nat -A PREROUTING -p tcp --dport 8732 -j DNAT --to-destination 10.0.1.1
echo "Spawned proxy in container $PROXY_ID"
sleep 1

# == START NODE IN DETACHED MODE ==
# 1. make inactive container
if [ "$NODE_TYPE" = "OCAML" ]; then
  docker pull simplestakingcom/tezedge-tezos:"$TAG"
  NODE_ID=$(docker run -d --volume "$VOLUME:/root/identity/" simplestakingcom/tezedge-tezos:"$TAG" sleep inf)
else
  docker pull simplestakingcom/light-node:latest
  NODE_ID=$(docker run -d --volume "$VOLUME:/root/identity/" simplestakingcom/light-node:latest sleep inf)
fi

docker exec "$NODE_ID" cp /root/identity/identity.json /root/.tezos-node/

#docker exec "$NODE_ID" cp /root/identity/identity.json /root/.tezos-node/
docker exec "$NODE_ID" mkfifo /root/identity/tezos.log
echo "Spawned tezos container $NODE_ID"
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
until curl -X GET --output /dev/null --silent --fail "localhost:$DEBUGGER_RPC_PORT/v2/p2p"; do
  sleep 1
done
echo "Proxy running successfully on port $DEBUGGER_RPC_PORT"
unmount_ns "$NODE_ID"
unmount_ns "$PROXY_ID"
# 4. start node in existing container
#docker exec -it "$NODE_ID" /bin/bash
#EXPLORER_ID=$(docker run -d -p "8080:8080" simplestakingcom/tezedge-explorer-ocaml:latest)
#echo "Running explorer on port 8080 in container $EXPLORER_ID"

if [ "$NODE_TYPE" = "OCAML" ]; then
  echo "[+] Running ocaml node"
  docker exec "$NODE_ID" sh -c "./tezos-node run --cors-header='content-type' --log-output=/root/identity/tezos.log --cors-origin='*' --rpc-addr 0.0.0.0:8732 --config-file \"/root/config.json\""
else
  echo "[+] Running rust node"
  docker exec "$NODE_ID" mkdir -p /tmp/tezedge
  docker exec "$NODE_ID" sh -c "./run.sh release --config-file ./tezedge.config --identity-file /root/identity/identity.json --log-file /root/identity/tezos.log"
fi
