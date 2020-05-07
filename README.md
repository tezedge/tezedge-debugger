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

## V2 API
### P2P
#### `/v2/p2p[?offset={offset}&count={count}]`
##### Description
Replacement for `/p2p/{offset}/{count}` endpoint, but parameters are passed as optional query arguments.
##### Query Arguments
* (__optional__) `offset` - Id of element, from which to start. (Default value is last message recorded)
* (__optional__) `count` - Number of elements. (Default 100)

#### Examples
* `/v2/p2p` - get last 100 recorded p2p messages.
* `/v2/p2p?offset=10000` - get 100 p2p messages ending with message id `10000` (and going backwards).
* `/v2/p2p?count=10` - get last 10 recorded p2p messages.
* `/v2/p2p?offset=100&count=10` - get p2p message with id `100` to `91` (if exists).

#### `/v2/p2p/host/{host_socket_address}[?offset={offset}&count={count}]`
##### Description
Replacement for `/p2p/{host}/{offset}/{count}` endpoint, but parameters are passed as optional query arguments.
##### Query Arguments
* (__required__) `host_socket_address` - Valid socket address (`{IP}:{PORT}`).
* (__optional__) `offset` - Id of element, from which to start. (Default value is last message recorded)
* (__optional__) `count` - Number of elements. (Default 100)
#### Examples
* `/v2/p2p/10.0.0.0:10000` - Get last hundred messages exchanged with node on address `10.0.0.0:10000`.
* `/v2/p2p/10.0.0.0:10000?offset=10000` - get 100 p2p messages starting index `10000` exhanged with peer `10.0.0.0:10000` (and going backwards).
* `/v2/p2p/10.0.0.0:10000?count=10` - get last 10 recorded p2p messages exhanged with peer `10.0.0.0:10000`.
* `/v2/p2p/10.0.0.0:10000?offset=100&count=10` - get p2p message with id `100` to `91` exchanged with peer `10.0.0.0:10000` (if exists).

#### `/v2/p2p/types[?offset={offset}&count={count}&tags={tag_list}]`
##### Description
Replacement for `/p2p/{host}/{offset}/{count}` endpoint, but parameters are passed as optional query arguments.
##### Query Arguments
* (__optional__) `tag_list` - Comma separated values specifying types desired types.
* (__optional__) `offset` - Id of element, from which to start. (Default value is last message recorded)
* (__optional__) `count` - Number of elements. (Default 100)
##### Valid tags
* __tcp__
* __metadata__
* __connection_message__
* __rest_message__
* __p2p_message__
* __disconnect__
* __advertise__
* __swap_request__
* __swap_ack__
* __bootstrap__
* __get_current_branch__
* __current_branch__
* __deactivate__
* __get_current_head__
* __current_head__
* __get_block_header__
* __block_header__
* __get_operations__
* __operation__
* __get_protocols__
* __protocol__
* __get_operation_hashes_for_blocks__
* __operation_hashes_for_block__
* __get_operations_for_blocks__
* __operations_for_blocks__

# Example
* `/v2/p2p/types?tags=connection_message` - get last 100 received connection messages.
* `/v2/p2p/types?tags=connection_message,metadata&count=10` - get last 10 messages that are either connection messages or metadata messages.