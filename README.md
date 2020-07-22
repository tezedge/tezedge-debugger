Tezedge Debugger
================
Network message debugger for applications running on tezos protocol.

The new debugger is based on Raw Sockets to capture all packets forming communication between (local) node and rest of the (remote) network.
Replacing the TUN  (**tun**nel) devices with Raw Sockets allows much easier networking setups with docker and docker-compose. No custom
scripts needed.

How does it work
================
Debugger relies on Raw Socket and identifying which packets are relevant to the running node. By sharing same network as
node and local identity, debugger is able to decode and deserialize exchanged messages from the nodes.

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
#### `/v2/p2p`
##### Description
Endpoint for checking all P2P communication on running node. 
Messages are always sorted from newest to oldest.
##### Query arguments
* `cursor_id : 64bit integer value` - Cursor offset, used for easier navigating in messages. Default is the last message.
* `limit : 64bit integer value` - Maximum number of messages returned by the RPC. Default is 100 messages.
* `remote_addr : String representing socket address in format "<IP>:<PORT>"` - Filter message belonging to communication with given remote node.
* `incoming : Boolean` - Filter messages by their direction
* `types : comma separated list of types` - Filter messages by given types
* `source_type: "local" or "remote"` - Filter messages by source of the message
##### Example
* `/v2/p2p` - Return last 100 P2P messages
* `/v2/p2p?cursor_id=100&types=connection_message,metadata` - Return all connection and metadata messages from first 100 messages.

### RPC
#### `/v2/rpc`
##### Description
Endpoint for checking all RPC Requests/Responses on running node.
Messages are always sorted from newest to oldest.
##### Query
* `cursor_id : 64bit integer value` - Cursor offset, used for easier navigating in messages. Default is the last message.
* `limit : 64bit integer value` - Maximum number of messages returned by the RPC. Default is 100 messages.
* `remote_addr : String representing socket address in format "<IP>:<PORT>"` - Filter message belonging to communication with given remote node.
##### Example
* `/v2/rpc?remote_addr=192.168.1.1:4852` - Show all requests made by the client with address 192.168.1.1:4852

### Logs
#### `/v2/log`
##### Description
Endpoint for checking all captured logs on running node
Messages are always sorted from newest to oldest.
##### Query arguments
* `cursor_id : 64bit integer value` - Cursor offset, used for easier navigating in messages. Default is the last message.
* `limit : 64bit integer value` - Maximum number of messages returned by the RPC. Default is 100 messages.
* `level : string` - Log level, should be on of `trace, debug, info, warn, error`
* `timestamp : string` - Unix timestamp representing time, from which to show logs
##### Example
* `/v2/log?level=error` - Return all errors in last one hundred logs
