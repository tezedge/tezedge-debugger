TezEdge Debugger
================
Network message debugger for applications running on the Tezos protocol.

The new Debugger is based on Raw Sockets and captures all packets that form the communication between the (local) node and rest of the (remote) network.
Replacing the TUN  (**tun**nel) devices with Raw Sockets allows for much easier networking setups with docker and docker-compose. No custom
scripts are needed.

How does it work
================
The Debugger relies on a Raw Socket to identify which packets are relevant to the running node. By sharing the same network as
the node and local identity, the Debugger is able to decode and deserialize the messages that are exchanged between nodes.

Requirements
============
* Docker
* (**RECOMMENDED**)  Steps described in Docker [Post-Installation](https://docs.docker.com/engine/install/linux-postinstall/). 

How to run
==========
Easiest way to launch the Debugger is by running it with the included docker-compose files. There are two separate files: one
for our Rust Tezedege Light Node (docker-compose.rust.yml) and the other for the original OCaml node (docker-compose.ocaml.yml).
A guide on how to change ports is included inside the docker-compose files.
```bash
docker-compose -f docker-compose.rust.yml build
docker-compose -f docker-compose.rust.yml up
```

(WIP) Debugger API
==================
The RPC endpoint of the Debugger is split into two parts: P2P messages on `/p2p/*` endpoints and RPC messages on `/rpc/*` endpoint.
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

Detailed Architecture
=====================
#### Packets, Chunks and Messages
Tezos nodes communicates by exchanging chunked P2P messages over internet. Each part uses its own "blocks" of data.

##### Packet
Packets are used by the higher layers of TCP/IP models to transport application communication over internet 
(there are more type of data blocks on lower levels of the model, like ethernet frames, but we do not work with those).
Debugger captures packets TCP packets, containing IPv4 or IPv6 header and TCP header. Those headers are required to determine
source and destination addresses for each packet.

#### Chunks
Binary chunk is an tezos construct, which represents some sized binary block. Each chunk is a continuous memory, which
first two bytes represent size of the block. Chunks are send over internet in TCP Packets, but not necessarily one
chunk per packet. When receiving new packet, first two bytes represent how many bytes, there should be in the whole chunk,
but not how many packets the chunk is split into.

#### Message
Message is parsed representation of some node command, but to be able to send them over internet, they must be firstly
serialized into binary blocks of data, which are then converted into Binary Chunks and finally split into packets to
be sent over internet. Again, it is not necessary, that single message is split into single binary chunk. It is required
to await enough chunks to deserialize message. 

![Message visualization](./docs/messages.svg)

#### Encryption

Primary feature of debugger, is ability to decrypt all messages, by having access only to the single identity of local
node.

##### Tezos "handshake"
To establish encrypted connection, tezos node exchange `ConnectionMessages` which contains information about itself,
including public key, nonce, and proof-of-stake, node running protocol version(s). Public key is static and is part of
nodes identity as is proof-of-stake. Nonces are generated randomly for each connection message. After `ConnectionMessage`
exchange, each node remembers the received and send nonce, and creates the "precomputed" key (for speedups), which is
calculated from local nodes private key and remote node public key. Nonce is a number incremented after each use.

* To encrypt message, node uses nonce sent in its own `ConnectionMessage` and precomputed key.
* To decrypt message, node uses received nonce and precomputed key.

For debugger to decrypt message, which is coming from remote node to local running node. It needs to know:
* Local node private key - which is part of local identity, to which the debugger has access.
* Remote node public key - which is part of received `ConnectionMessage` and was captured.
* Remote node nonce - which is part of received `ConnectionMessage` and was captured.

But to decrypt message sent by local node, it would require to know private key of remote node, to which it does not have
access. Fortunately, Tezos is internally using the Curve5519 method, which allows to decrypt message with same 
keys which were used to encryption, thus debugger "just" needs the:
* Local node private key - which is part of local identity, to which the debugger has access.
* Remote node public key - which is part of received `ConnectionMessage` and was captured.
* Local node nonce - which is part of sent `ConnectionMessage` and was captured.

#### System architecture

Basic concept of the whole systems, are that captured data are moved through a pipeline of steps, which allow a easier
data processing. The basic pipeline for P2P messages consists of Producer - Orchestrator - Parsers and Processors:


![P2P System description](./docs/system.svg)


All parts of system are defined in the [system module](./src/system)

##### Producers
Purpose of producers is to only capture and filter interesting network data.
`RawSocketProducer` captures all networking traffic on specific networking interface, filtering all non-TCP packets 
(as Tezos communication works only on TCP packets). And sends them further down the line into the Orchestrator

##### Orchestrator
Received packets from producer and orchestrates them into "logical streams", each stream of packets has its own parser.
Creating management and cleanup of parsers is responsibility of the orchestrator.
`PacketOrchestrator` Receives TCP packets, determines which address is remote address, as that is determining factor, 
which parser should process the packet. If no parser for specific remote address exists, new one is created instead.
If packet denotes end of communication with remote address, parser is stopped and cleaned. 

##### Parser
Receives packets which belong to some "logical stream", and process them into the messages (parses packets into messages).
Parsed messages are forwarded into the processor.
`P2PParser` is responsible for aggregating packets into chunks and buffers chunks for final deserialization.
If `ConnectionMessages` are exchanged, parser also decrypts the data first.


##### Processors
All processors resides inside single primary processor, which calls individual processors, to process parsed data.
Currently, only processor which is used is database processor, which stores and indexes parsed messages.


#### Node Logs
To capture node logs, debugger is utilizing the "syslog" protocol (which can be easily enabled in the Docker), which
instead of printing the log into the console, wraps them into the UDP packet and sends them to the some server, this should
be handled by application or administrator of application. Debugger runs syslog server inside, to simply process the generated
logs. This system allows to decouple debugger from node, which prevents debugger from failing if running node fails, 
preserving all captured logs, potentially information also about the failure of the node.

#### Storage
Storage is based running on the RocksDB, utilizing custom [indexes](./src/storage/secondary_index.rs). Which
allows field filtering and cursor pagination.

#### RPC server
RPC server is based on the [warp crate](https://crates.io/crates/warp). All endpoints are based on cursor-pagination, 
meaning it is simple to paginate real-time data. All data are from local storage
