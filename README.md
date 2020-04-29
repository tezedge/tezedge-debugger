Tezedge Debugger
================
A network message debugger for applications running on the Tezos P2P and RPC protocols.

The debugger is based on Linux Network Namespaces (heavily used in docker) and TUN  (**tun**nel) devices to capture 
all packets forming the communication between the (local) node and rest of the (remote) network inside the debugger. 
As the TUN devices manipulate Kernel drivers, `NET_ADMIN` capabilities are required to run the proxy.

How does it work
================
The Debugger relies on two TUN devices, where one is used to relay packets into and from the internet, and the second one is
placed inside the specific network's namespace (usually the container running the node), to effectively route all communication 
associated with the node through the Debugger. Inside, with a provided local identity (or a generated one), the Debugger is able to decode and deserialize all P2P messages, as well as present RPC requests/responses from the node.

Requirements
============
* Docker
* Follow the steps described in the Docker [post-installation manual](https://docs.docker.com/engine/install/linux-postinstall/). 

How to run
==========
The easiest way to try the debugger is by running it through the following script: `./docker.sh`. This script will setup
all the required containers and networking settings. If the required capabilities are not satisfied, the script will prompt the user
for a password to elevate its rights. The script checks `./identity` folder for `identity.json` file, if not provided one will be
generated inside said folder. Currently, the script will run three containers:
* `tezedge_debugger` - with RPC on port `17732`
* `tezedge_explorer` - UI for presenting data from debugger on `localhost:8080`
* `tezos_node` - on P2P port `9732` and its own [RPC](https://tezos.gitlab.io/api/rpc.html) on port `18732`

(WIP) Debugger API
==================
The RPC endpoints of the debugger are split into two-part P2P messages on `/p2p/*` endpoints and RPC messages on `/rpc/*` endpoint.
### P2P
#### `/p2p/{offset}/{count}(/{host})?`
##### Description
An endpoint for checking all P2P communication on the running node. 
Messages are always sorted from newest to oldest.
##### Arguments
* `offset : 64bit integer value` - Skip last `offset` values.
* `count : 64bit integer value` - Return `count` messages.
* OPTIONAL `host : String in format <IP>:<PORT>` - Filter messages by remote address.
##### Example
* `/p2p/0/1` - Show last P2P message.
* `/p2p/50/50` - Show last 50 RPC messages, skipping first 50.
* `/p2p/0/1/51.15.81.27:9732` - Show last message between this node and node running on address `51.15.81.27:9732`.

### RPC
#### `/rpc/{offset}/{count}(/{ip})?`
##### Description
Endpoint for checking all RPC Requests/Responses on running node.
Messages are always sorted from newest to oldest.
##### Arguments
* `offset : 64bit integer value` - Skip last `offset` values.
* `count : 64bit integer value` - Return `count` messages.
* OPTIONAL `IP : String representing valid IP address` - Filter messages by remote IP address.
##### Example
* `/rpc/0/1` - Show last RPC message.
* `/rpc/50/50` - Show last fifty RPC messages, skipping the first 50.
* `/rpc/0/1/172.16.0.1` - Show RPC message sent between node and remote running on `172.16.0.1`
