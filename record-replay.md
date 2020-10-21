
# Recording and replaying messages in the TezEdge Debugger

Debugging is an essential part of any developer's work. However, debugging can be challenging when working with encrypted networks. Messages sent across the Tezos network are encrypted and serialized, which makes debugging more difficult.

As developers, we want to retrieve these message(s) and replay them in order to simulate the error. This allows us to have detailed bug reports that contain the messages associated with the error(s). Error(s) can be replicated by replaying these messages. This significantly speeds up bug fixing and improves the stability of the system. 

For this purpose, we’ve added a record and replay for communication between nodes on the Tezos network.

**Record**

The three primary functions of record are:



*   Capturing packets using raw socket
*   Converting packets into chunks and then into messages
*   Storing the chunks and messages in the database

A chunk is a piece of data that includes the first two bytes which represent the amount of bytes the chunk contains.

Messages are good for debugging purposes, but in order to understand and simulate various attacks, we need to replay chunks. Another reason for replaying chunks is because we want to replicate the way the node communicates across the network as accurately as possible. 

We are storing messages only for the last chunk. If chunk does not fully represent the message, then we only store a part of this message.

Although we can recreate the messages from the chunks, we store the already-deserialized messages in the database in order to speed up the process. 

When recording, we record the interactions made with our target node - the node we want to debug. We record all of the communication done between the target node and other peers on the network. 

However, the node may be communicating with more than one peer - it might be communicating with several peers. When we replay, it is very hard to force the TezEdge node to connect to the same simulated nodes in the same order. For this reason, we restrict the TezEdge node to communicate with only one node.

**Replay**

The three primary functions of replay are:



*   Taking messages from the database
*   Advertising the target node to talk with us.
*   Replaying messages one-by-one and noticing any differences between the recorded and actual conversations.

When we replay, we only take the communication from the database that is relevant (being just one conversation between two nodes). 

It is hard to debug, because there are many sources of randomness, because any time you run the node, you receive more information from the ‘outer’ world. Biggest source of this information is another node (p2p communication).

One of the reasons that we build replayer is that we want to identify and debug sources of randomness. For example, when we launch the TezEdge node and the replayer, the TezEdge node initiates the handshake, replayer answers the handshake and this is not deterministic.

By ‘not deterministic’ we mean random. In this case, the nonce is always different. If we just replay what we record, this cannot work, because the nonce has already changed since we’ve recorded it.

When we are replaying chunks there is a high chance of replaying chunks that are not deterministic, which are not expected by the remote node. 

Either we avoid this randomness during communication or we can remove the source of randomness in the node. Neither option is ideal, because we are changing what has been recorded. However, this is sufficient for debugging purposes.

If you want to debug the TezEdge node, we can remove sources of randomness from the node, but with the OCaml node we cannot do the same. With the OCaml node, one of the solutions is to replace the randomness from the recorded messages with predetermined data.

**How to build**

Build the debugger and use this script in order for the debugger to have capabilities for recording traffic without superuser permissions. This script requires sudo.

`cargo build --bin tezedge-debugger`

`./setcap.sh`

Also build the replayer.

`cargo build --bin replayer`

**How to record**

**It is convenient to record the traffic on the host OS without using docker.**



1. We launch the debugger in order to listen to the communication between nodes.
2. We use a fully synced OCaml node that is already active. We chose 51.15.220.7:9732, a well known Tezos node.
3. Now we launch an empty TezEdge node. This is the node we want to debug.

In order to perform a bootstrap, the TezEdge node initiates an interaction by sending a connection message and several requests to the OCaml node.

To read more about the bootstrapping process, please refer to our [past article about the P2P layer.](https://medium.com/simplestaking/tezos-rust-node-a-deep-dive-into-the-tezos-p2p-layer-98e3b3e3b704)

Run the debugger on the desired network interface and specify the local IP from which you want to capture packets. This should be run from the directory where the debugger is 

_For example:_


```
cargo run --bin tezedge-debugger enp4s0 192.168.0.103
```


In which `192.168.0.103` is the local IP, but **not** local host (127.0.0.1)

Run the TezEdge node. The peer `51.15.220.7:9732` was chosen for bootstrapping. It is a well-known Tezos node.

```
./run.sh release --peers 51.15.220.7:9732
```

The debugger stores its database in `/tmp/volume/&lt;timestamp>`, with the timestamp being the number of seconds since the 1st of January 1970.

When you record, you should first run Debugger and then node. If you are replaying, then first launch the node and then the Replayer.

**Replay**

To replay, first run the node with local IP as a peer.

```

./run.sh release --peers 127.0.0.1:9732

```

Then run the replayer. Use the database directory with the proper timestamp.

```

cargo run --bin replayer -- --peer-ip 51.15.220.7:9732 --path /tmp/volume/&lt;timestamp>/ --node-ip 127.0.0.1:9732

```

**Bundled record**

One recorded interaction has already been bundled in the directory `tests/rust-node-record`.

```

cargo run --bin replayer -- --peer-ip 51.15.220.7:9732 --path tests/rust-node-record --node-ip 127.0.0.1:9732

```



