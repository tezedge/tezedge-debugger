
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

## How to use

### Prepare the code and build

Two terminal sessions are needed. One for tezedge node, and another for the debugger and replayer.

#### Clone TezEdge repo.

On the TezEdge terminal, clone the TezEdge repository, or change the directory into the repository if you have already done so.

```
git clone https://github.com/simplestaking/tezedge.git
cd tezedge
```

Optionally, checkout the version you want:

```
git checkout tags/v0.6.0 -b v0.6.0
```

Build the TezEdge node. Please note that this might take a while.

```
cargo build --release
```

#### Clone the debugger repo

On the debugger's terminal, clone the `tezedge-debugger` repository

```
git clone https://github.com/simplestaking/tezedge-debugger.git
cd tezedge-debugger
```

Build the debugger and use this script to allow the debugger to record traffic without superuser permissions. This script requires sudo.

```
cargo build --bin tezedge-debugger
./setcap.sh
```

Also, build the replayer.

```
cargo build --bin replayer
```

### Remove or rename the TezEdge database

If you need to keep the TezEdge node database, rename it:

```
mv /tmp/tezedge /tmp/tezedge-backup
```

And at the end of replaying, take it back:

```
mv /tmp/tezedge-backup /tmp/tezedge
```

If you do not need it, you may remove the database.

```
rm -Rf /tmp/tezedge
```

You can perform these actions on any terminal.

By default the database is located at `/tmp/tezedge`.


### Record

#### Run Debugger and specify IP.

On the Debugger's terminal, run the Debugger on the desired network interface and specify the local IP from which it should capture packets. For example:

```
cargo run --bin tezedge-debugger enp4s0 192.168.0.103
```

You can determine your default interface and IP by using this command:

```
ip route show
```

For example, on my computer it shows:

```
default via 192.168.0.1 dev enp4s0 proto dhcp metric 100 
172.17.0.0/16 dev docker0 proto kernel scope link src 172.17.0.1 linkdown 
172.18.0.0/16 dev br-209e9b70bdea proto kernel scope link src 172.18.0.1 linkdown 
192.168.0.0/24 dev enp4s0 proto kernel scope link src 192.168.0.103 metric 100 
```

#### Run node and bootstrap

On TezEdge's terminal, run the TezEdge node. The peer `51.15.220.7:9732` was chosen to bootstrap with. It is well known tezos node.

```
./run.sh release --peer-thresh-low=1 --peer-thresh-high=1 --peers 51.15.220.7:9732
```

The results of the recording is a database that is located at /tmp/volume/<timestamp>. It is used in the command line of the replay

**Important:** When you want to record, you should first launch the Debugger and then the node. When you want to replay, then first launch the node and then the Replayer.

### Replay

Once again, remove the database of the TezEdge node:

```
rm -Rf /tmp/tezedge
```

Run this command on TezEdge's terminal:

```
./run.sh release --peer-thresh-low=1 --peer-thresh-high=2 --peers 127.0.0.1:9732
```

And then run the replayer. Use the proper database path.

The debugger stores its database in `/tmp/volume/<timestamp>`, with the timestamp being the number of seconds since the 1st of January 1970.

On the Debugger's terminal:

```
cargo run --bin replayer -- --peer-ip 51.15.220.7:9732 --path /tmp/volume/1603113392732618717/ --node-ip 127.0.0.1:9732
```

Also, it is possible to use the database from the repository. To replay without recording, run this command:

```
cargo run --bin replayer -- --peer-ip 51.15.220.7:9732 --path tests/rust-node-record --node-ip 127.0.0.1:9732
```
