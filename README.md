Tezedge Debugger
================
Network message debugger for applications running on tezos protocol.

The new debugger is based on Raw Sockets to capture all packets forming communication between (local) node and rest of the (remote) network.
Replacing the TUN  (**tun**nel) devices with Raw Sockets allows much easier networking setups with docker and docker-compose. No custom
scripts needed.

How does it work
================
Debugger relies on Raw Socket and identifying which packets are relevant to the running node. By sharing same network as
node and local identity, debugger is able to decode and deserialize exchanged messages from the node.

Requirements
============
* Docker
* (**RECOMMENDED**)  Steps described in Docker [Post-Installation](https://docs.docker.com/engine/install/linux-postinstall/). 

How to run
==========
Easiest way to run the debugger is it by running it with included docker-compose files. There are two separate files one
for our Rust Tezedege Light Node (docker-compose.rust.yml) and one for original OCaml node (docker-compose.ocaml.yml).
How to change ports is described inside the docker-compose files.
```bash
docker-compose -f docker-compose.rust.yml build
docker-compose -f docker-compose.rust.yml up
```

(WIP) Debugger API
==================
RPC endpoint of debugger are split into two parts P2P messages on `/p2p/*` endpoints and RPC messages on `/rpc/*` endpoint.
### P2P
#### `/p2p/{offset}/{count}(/{host})?`
##### Description
Endpoint for checking all P2P communication on running node. 
Messages are always sorted from newest to oldest.
##### Arguments
* `offset : 64bit integer value` - Skip last `offset` values.
* `count : 64bit integer value` - Return `count` messages.
* OPTIONAL `host : String in format <IP>:<PORT>` - Filter messages by remote address
##### Example
* `/p2p/0/1` - Show last P2P message
* `/p2p/50/50` - Show last 50 RPC messages, skipping first 50
* `/p2p/0/1/51.15.81.27:9732` - Show last message between this node and node running on address `51.15.81.27:9732`.

### RPC
#### `/rpc/{offset}/{count}(/{ip})?`
##### Description
Endpoint for checking all RPC Requests/Responses on running node.
Messages are always sorted from newest to oldest.
##### Arguments
* `offset : 64bit integer value` - Skip last `offset` values.
* `count : 64bit integer value` - Return `count` messages.
* OPTIONAL `IP : String representing valid IP address` - Filter messages by remote ip address
##### Example
* `/rpc/0/1` - Show last RPC message
* `/rpc/50/50` - Show last fifty RPC messages, skipping first 50
* `/rpc/0/1/172.16.0.1` - Show RPC message sent between node and remote running on `172.16.0.1`
