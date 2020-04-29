Tezedge Debugger
================
Network message debugger for applications running on tezos P2P and RPC protocol.

Debugger is based around Linux Network Namespaces (heavily used in docker) and TUN  (**tun**nel) devices to capture 
all packets forming communication between (local) node and rest of the (remote) network inside the debugger. 
As TUN devices manipulates Kernel drivers, `NET_ADMIN` capabilities are required to run the proxy.

How does it work
================
Debugger relies on two TUN devices, where one is used to relay packet into/from internet, and second one is
placed inside specific network namespace (usually container running the node), to effectively route all communication 
associated with node through debugger. Inside, with provided local identity (or generated one), debugger is able to decode and deserialize
all P2P messages, as well as present RPC Requests/Responses from the node.

Requirements
============
* Docker
* Steps described in Docker [Post-Installation](https://docs.docker.com/engine/install/linux-postinstall/). 

How to run
==========
Easiest way to try the debugger is it by running it through provided script `./docker.sh`. This script will setup
all required containers and networking settings. If required capabilities are not satisfied, script will prompt for user
password to elevate its rights. Scripts checks `./identity` folder for `identity.json` file, if not provided one will be
generated inside said folder. Currently, script will run three containers:
* `tezedge_debugger` - with RPC on port `17732`
* `tezedge_explorer` - UI for presenting data from debugger on `localhost:8080`
* `tezos_node` - on P2P port `9732` and its own [RPC](https://tezos.gitlab.io/api/rpc.html) on port `18732`

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
