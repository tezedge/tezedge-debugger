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
The easiest way to try the debugger is by running it through the following script: `NODE_TYPE=RUST ./docker.sh`, where `NODE_TYPE` specifies type of node (`OCAML` or `RUST` are allowed). 
This script will setup all the required containers and networking settings. If the required capabilities are not satisfied, the script will prompt the user
for a password to elevate its rights. 

Target ports are determined by the `NODE_TYPE`:
* For `RUST`
    * debugger rpc port: `10000`
    * node rpc port: `10100`
    * node p2p port: `10200`
    * node websocket port: `10300`
* For `OCAML` 
    * debugger rpc port: `11000`
    * node rpc port: `11100`
    * node p2p port: `11200`

Values can be changed by editing `./docker.sh`. 

(V2) Debugger API
==================
## V2 API
### P2P
#### `/v2/p2p`
##### Description
P2P message cursor for presenting p2p messages.

##### Request Fields
Request body must contain valid _JSON object_ (even if no fields are provided).
All fields are _optional_.
* `cursor_id` - integral value specifying current cursor position (default: latest id)
* `limit` - integral value limiting number of results in cursor (default: 100)
* `remote_addr` - string in `<IP:PORT>` format, specifying remote host.
* `types` - list of string specifying message types (see [valid types](#valid-types)).
* `incoming` - boolean specifying from which side came message.

### RPC
#### `/v2/rpc`
##### Description
RPC message cursor for presenting rpc messages.

##### Request Fields
Request body must contain valid _JSON object_ (even if no fields are provided).
All fields are _optional_.
* `cursor_id` - integral value specifying current cursor position (default: latest id)
* `limit` - integral value limiting number of results in cursor (default: 100)
* `remote_addr` - string in `<IP:PORT>` format, specifying remote host.

### Log
#### `/v2/log`
##### Description
RPC message cursor for presenting logs.

##### Request Fields
Request body must contain valid _JSON object_ (even if no fields are provided).
All fields are _optional_.
* `cursor_id` - integral value specifying current cursor position (default: latest id)
* `limit` - integral value limiting number of results in cursor (default: 100)
* `level` - string representation of desired message levels (see [valid levels](#valid-levels)).

## Other
##### Valid types
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
* __get_block_headers__
* __block_header__
* __get_operations__
* __operation__
* __get_protocols__
* __protocol__
* __get_operation_hashes_for_blocks__
* __operation_hashes_for_block__
* __get_operations_for_blocks__
* __operations_for_blocks__

##### Valid levels
* __trace__
* __debug__
* __info__
* __notice__
* __warning__ or __warn__
* __error__
* __fatal__

##### Some Examples
* `/v2/p2p`
* `/v2/p2p?limit=10`
* `/v2/p2p?cursor_id=66780`
* `/v2/p2p?cursor_id=66780&limit=2`
* `/v2/p2p?types=connection_message`
* `/v2/p2p?types=connection_message,metadata`
* `/v2/p2p?types=connection_message,metadata&limit=2`
* `/v2/p2p?remote_addr=18.182.132.42:9732`
* `/v2/p2p?remote_addr=18.182.132.42:9732&types=metadata`

