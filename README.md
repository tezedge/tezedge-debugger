# The TezEdge Debugger

- [The TezEdge Debugger](#the-tezedge-debugger)
  * [Memory Profiler](#memory-profiler)
    + [How it works](#how-it-works)
    + [Requirements](#requirements)
    + [How to run](#how-to-run)
    + [How to run tests](#how-to-run-tests)
      - [Unit tests](#unit-tests)
      - [Integration test](#integration-test)
  * [Network Recorder](#network-recorder)
    + [Peer to peer messages](#peer-to-peer-messages)
      - [BPF module](#bpf-module)
      - [Packets, Chunks and Messages](#packets--chunks-and-messages)
      - [Packet](#packet)
      - [Chunks](#chunks)
      - [Message](#message)
      - [Encryption](#encryption)
    + [Node Logs](#node-logs)
    + [Storage](#storage)
    + [RPC server](#rpc-server)
    + [API](#api)
    + [Requirements](#requirements-1)
    + [How to run](#how-to-run-1)
  * [Build from sources](#build-from-sources)
    + [Prepare system dependencies](#prepare-system-dependencies)
    + [Rust](#rust)
    + [BPF linker](#bpf-linker-needed-only-for-memory-profiler)
    + [Kernel sources](#kernel-sources-needed-only-for-network-recorder)
    + [Build](#build)
    + [Run tests](#run-tests)
      - [Unit tests](#unit-tests)
      - [Integration tests](#integration-tests)
    + [Important note before run](#important-note-before-run)
    + [Configure network recorder](#configure-network-recorder)
    + [Run memory profiler](#run-memory-profiler)
    + [Run network recorder](#run-network-recorder)
 

## Memory Profiler  

As developers, we want to see how much memory is used in each piece
of code of the TezEdge node, so that we can evaluate whether a particular
function in code costs us too much memory. We want to minimize memory
consumption, which is always beneficial for the smooth running of any software.

For this purpose, we've created a memory profiler for the TezEdge node that utilizes
extended Berkeley Packet Filters (eBPF), a technology we’ve previously used
in the firewall of the TezEdge node’s validation subsystem.

It track physical (residential) memory usage by the TezEdge node and it can
determine the function and entire call stack where the allocation happened.
The profiler does not store the history of allocations, hence it does not
use disk memory. It only gives us the current moment slice, which preserves more
space on the server for the node itself.

By running this software and its browser-based front end, developers can see a call tree
and how many memory are allocated in the branches of the tree. They can then accurately determine
where it would be worthwhile to decrease memory consumption.

### How it works

The tool consists of two parts.

#### 1. EBPF loader

The first part is `bpf-memprof-user` binary which has an embedded ebpf module.
It requires superuser permission. When launched, this binary loads the ebpf
module into the kernel and creates the `/tmp/bpf-memprof.sock` socket. The ebpf
module tracks the `exec` syscall to determine which process is the TezEdge node.
That is why `bpf-memprof-user` should be running before the TezEdge node
is launched. If `bpf-memprof-user` is launched when the node is already running,
it will not be able to find the node.

The ebpf module is tracking physical (residential) page allocation and
deallocation, either removing or adding such pages to the IO cache.
Additionally, the ebpf module unwinds the stack during each allocation event
so that the profiler has call-stack virtual addresses.

#### 2. TezEdge memprof binary

The second part is the `tezedge-memprof` library.
It performs the following tasks:

* Connects to the socket and receives a stream of kernel events.

* Monitors the `/proc/<pid>/maps` file. This file contains descriptions of
each memory area on the address space of the TezEdge node. Among others,
it contains the descriptions of memory areas of the executable code
`light-node` binary and shared libraries used by the node. It allows
translation from the virtual address of the function into filename and offset
in the file where the function is.

* Loads `.symtab` and `.strtab` sections from `light-node` binary and from
shared libraries. It enables the profiler to resolve function names.

* Counts allocated memory and memory used for cache at each function.

* Serves http requests.

### Requirements

* Linux kernel 5.11 version or higher.
* Docker
* [Docker compose](https://docs.docker.com/compose/install/)
* (**RECOMMENDED**)  Steps described in Docker [Post-Installation](https://docs.docker.com/engine/install/linux-postinstall/). 

### How to run

#### Using docker-compose

* `git clone https://github.com/tezedge/tezedge-debugger.git`

* `cd tezedge-debugger`

* `docker-compose pull && docker-compose up -d`

First two steps are clone source code from github and move to the directory.
The full source core is unneeded. You can take only `docker-compose.yml` file.

Third step is running the TezEdge node along with the memory profiler and
frontend.

Now you can see the result at http://localhost/#/resources/memory in your
browser.

#### Without docker-compose

The application is distributed as a docker image
`tezedge/tezedge-memprof`. The image needs to have privileged
permissions. It also needs `/sys/kernel/debug` and `/proc` directories mapped
from the host system. The application is serving http requests on port `17832`.

For example:

```
docker run --rm --privileged -it -p 17832:17832 -v /proc:/proc:rw -v /sys/kernel/debug:/sys/kernel/debug:rw tezedge/tezedge-memprof:latest
```

In order to determine function names, the memory profiler needs access
to `light-node`

and system shared libraries. The files to which the memory profiler has access
to should be the same files that the Tezedge node is using. That is why
the docker image

`tezedge/tezedge-memprof:latest` is inherited from the `tezedge/tezedge:latest` image.

However, if `tezedge` is updated, but the `tezedge-memprof` image is still old,
it can lead to problems. To avoid such situations, `tezedge-memprof` image has
a docker client inside, and copies the `light-node` binary from the current
`tezedge` container.

Set the `TEZEDGE_NODE_NAME` environment variable into the TezEdge node container
name and map `/var/run/docker.sock` file from host to enable such behavior.

See `docker-compose.yml` and `memprof.sh` for details.

### HTTP API

### `/v1/tree`

Return a tree-like object. Each node of the tree represents a function in some
executable file.

The tree has the following structure:

* `name`

* `executable` - name of the binary file (ELF), for example `libc-2.31.so`

* `offset` - offset of the function call in the binary file

* `functionName` - demangled name of the function, for example
`<networking::p2p::peer::Peer as riker::actor::Receive<networking::p2p::peer::SendMessage>>::receive::hfe17b4d497a1a6cb`,
note: rust function name is ending with hash, for example `hfe17b4d497a1a6cb`

* `functionCategory` - indicates the origin of the function, can be one of
the following:

* `nodeRust` is a function of the TezEdge node written in Rust

* `nodeCpp` is a function of the TezEdge node written in C++

* `systemLib` is a function from a system library, usually written in C,
but it can also be an arbitrary language.

* `value` - positive integer, number of kilobytes of total memory
(ordinary + cache) allocated in this function and all functions from which
this function is called

* `cacheValue` - positive integer, number of kilobytes of cache allocated
in this function

* `frames` - list of branches of the tree, containing all functions from which
this function is called, or containing all functions which are called from this
function (if `reverse` is set in true).

### Parameters

`reverse` - boolean parameter, used to request reversed tree, default value
is `false`;

`threshold` - integer parameter, used to filter out functions which allocate
a smaller amount of memory than some threshold value, default value is `256`.

### `/v1/pid`

Returns the process id of the TezEdge Node process.

## Network Recorder

Network message recorder for applications running on the Tezos protocol.

### Peer to peer messages

First of all, the network recorder should get the raw data from the kernel.

#### BPF module

The network recorder uses the BPF module to intercept network-related syscalls.
It intercepts `read`, `recvfrom`, `write`, `sendto`, `bind`, `listen`,
`connect`, `accept` and `close` syscalls. Those syscalls give a full picture
of network activity of the application. The BPF module configured to know where
the application which we want to record is listening incoming connection.
That is needed to determine an applications PID. It listen `bind` attempts from
any PID on the given port. And once we have one, we know the PID. After that,
the BPF module intercepting other syscalls made by this PID. A single instance of the recorder
can record multiple applications simultaneously. Do not run multiple instance of
the network recorder.

#### Packets, Chunks and Messages
Tezos nodes communicate by exchanging chunked P2P messages over the internet. Each part uses its own "blocks" of data.

#### Packet
Packets are used by the higher layers of TCP/IP models to transport application communication over the internet 
(there are more type of data blocks on lower levels of the model, like ethernet frames, but we do not work with those).
The network recorder does not care about such low-level details, packets are processed by the kernel.

#### Chunks
A binary chunk is a Tezos construct, which represents some sized binary block. Each chunk is a continuous memory, with the
first two bytes representing the size of the block. Chunks are send over internet in TCP Packets, but not necessarily one
chunk per packet, and not necessarily the end of the packet is the end of the chunk. The TCP segment can contain multiple
chunks and it split into packets by the kernel, or network hardware which does not know nothing about Tezos chunks. So
the single TCP packet can contain multiple chunk, and can contain few last bytes of some chunk and few first bytes of the next chunk.
It is not easy to handle properly. We need to bufferize received data and cut chunks from the buffer.

#### Message
A message is parsed representation of some node command, but to be able to send them over internet, they must first be serialized into binary blocks of data, which are then converted into Binary Chunks and finally split into packets to be sent over internet. Again, it is not necessary, that single message is split into single binary chunk. It is required
to await enough chunks to deserialize message. 

#### Encryption

The primary feature of the network recorder is the ability to decrypt all messages while having access only to the single identity of the local
node.

##### Tezos "handshake"
To establish encrypted connection, Tezos nodes exchange `ConnectionMessages` which contain information about the nodes themselves,
including public keys, nonces, proof-of-stake and node running protocol version(s). The public key is static and is part of
a node's identity, as is proof-of-stake. Nonces are generated randomly for each connection message. After the `ConnectionMessage`
exchange, each node remembers the node it received and the nonce it sent, and creates the "precomputed" key (for speedups), which is
calculated from the local node's private key and remote node's public key. The nonce is a number incremented after each use.

* To encrypt a message, the node uses the nonce sent in its own `ConnectionMessage` and a precomputed key.
* To decrypt a message, the node uses the received nonce and a precomputed key.

For the network recorder to decrypt a message that is coming from a remote node to the local running node, it needs to know the following:

* The local node's private key - which is part of its local identity to which the network recorder has access.
* The remote node's public key - which is part of the received `ConnectionMessage` and was captured.
* The remote node's nonce - which is part of the received `ConnectionMessage` and was captured.

However, to decrypt a message sent by the local node, it would be necessary to know the private key of the remote node, to which it does not have
access. Fortunately, Tezos is internally using the Curve5519 method, which allows to decrypt a message with the same 
keys which were used for encryption, thus the network recorder "just" needs the:
* Local node's private key - which is part of its local identity, to which the network recorder has access.
* Remote node's public key - which is part of the received `ConnectionMessage` and was captured.
* Local node's nonce - which is part of the sent `ConnectionMessage` and was captured.

### Node Logs
To capture node logs, the network recorder utilizes the "syslog" protocol
(which can be easily enabled in the Docker), which,
instead of printing the log into the console, wraps them into the UDP packet and sends them to the server. This should
be handled by the application or the administrator of the application.
The network recorder runs a syslog server inside to simply process the generated
logs. This system allows us to decouple the recorder from the node,
which prevents the network recorder from failing if the running node fails.
This preserves all of the captured logs, which can potentially include information about the failure of the node.

### Storage
Storage is based on RocksDB, utilizing custom [indexes](./src/storage/secondary_index.rs), which
allows field filtering and cursor pagination.

### RPC server
RPC server is based on the [warp crate](https://crates.io/crates/warp). All endpoints are based on cursor-pagination, 
meaning it is simple to paginate real-time data. All data are from local storage

### API

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

#### `/v2/log`
##### Description
Endpoint for checking all captured logs on running node
Messages are always sorted from newest to oldest.
##### Query arguments
* `cursor_id : 64bit integer value` - Cursor offset, used for easier navigating in messages. Default is the last message.
* `limit : 64bit integer value` - Maximum number of messages returned by the RPC. Default is 100 messages.
* `level : string` - Log level, should be on of `trace, debug, info, warn, error`
* `timestamp : string` - Unix timestamp representing time from which the logs are shown.
##### Example
* `/v2/log?level=error` - Return all errors in last one hundred logs,

### Requirements

* Linux kernel 5.11 version or higher.
* Docker
* [Docker compose](https://docs.docker.com/compose/install/)
* (**RECOMMENDED**)  Steps described in Docker [Post-Installation](https://docs.docker.com/engine/install/linux-postinstall/). 

### How to run

First, you must clone this repo.
```bash
git clone https://github.com/tezedge/tezedge-debugger.git
```

Then change into the cloned directory
```bash
cd tezedge-debugger
```

The easiest way to launch the Debugger is by running it with the included docker-compose file.
```bash
docker-compose pull
docker-compose up
```

## Build from sources

It is preferable to use Ubuntu 21.04 to run this software since it has kernel 5.11.0.
If you are running another OS with an older kernel, you need to update the kernel.

### Prepare system dependencies

* curl, wget and git to get the code
* zlib, clang and llvm 11 to build the BPF linker
* libelf, make to build the memory profiler
* libsodium, gcc, g++, libssl, pkg-config to build the network recorder
* libarchive-tools, flex, bison to prepare the kernel code (needed for the network recorder).

In Ubuntu 20.04 or Ubuntu 20.10 or Ubuntu 21.04:

```
sudo apt-get update
sudo apt-get install curl wget git zlib1g-dev clang make libelf-dev libsodium-dev gcc g++ libssl-dev pkg-config libarchive-tools flex bison bc lsb-release software-properties-common
wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && sudo ./llvm.sh 11 && rm llvm.sh
export LLVM_SYS_110_PREFIX=/usr/lib/llvm-11
```

### Rust

The Rust version should be nightly-2021-03-23 for building the crates.
But internally also used nightly-2020-12-31 for building bpf module.

If you have no rustup:

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly-2020-12-31
source $HOME/.cargo/env
rustup update nightly-2021-03-23
```

If you have rustup:

```
rustup update nightly-2020-12-31
rustup update nightly-2021-03-23
```

### BPF linker (needed only for memory profiler)

```
cargo install bpf-linker --git https://github.com/tezedge/bpf-linker.git --branch main
```

### Kernel sources (needed only for network recorder)

The minimal required version of Linux kernel is 5.8, but if the version is lower 5.11,
the debugger work unreliable. The recommended version of kernel is 5.11 or higher.

However, for building, you need kernel sources of version 5.8.18
no matter what the actual version you run.

```
export KERNEL_VERSION=5.8.18
wget -cq https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-$KERNEL_VERSION.tar.xz
tar xf linux-$KERNEL_VERSION.tar.xz
cd linux-$KERNEL_VERSION
make defconfig
make modules_prepare
export KERNEL_SOURCE=$(pwd)
cd ..
```

### Build

Get the code:

```
git clone https://github.com/tezedge/tezedge-debugger
cd tezedge-debugger
```

Build memory profiler:

```
cargo +nightly-2021-03-23 build -p bpf-memprof --release
```

Build network recorder:
```
cargo +nightly-2020-12-31 build -p bpf-sniffer --release
cargo +nightly-2021-03-23 build -p tezedge-recorder --release
```

### Run tests

#### Unit tests

`cargo +nightly-2021-03-23 test -p tezedge-memprof -- history`

#### Integration tests

The TezEdge node and the memory profiler should be running to do this tests.

`URL=http://localhost:17832 cargo +nightly-2021-03-23 test -p tezedge-memprof -- positive compare`

The TezEdge node and the network recorder should be running to do this tests.

`DEBUGGER_URL=http://localhost:17742 cargo +nightly-2021-03-23 test -p tester -- p2p_limit p2p_cursor p2p_types_filter`

### Important note before run

Do not run multiple instance of the memory profiler or multiple instance of network recorder
simultaneously on the same computer.

A single instance of the network recorder is able to record
multiple TezEdge and/or Tezos nodes on the same computer.

### Configure network recorder

The network recorder expect `config.toml` file in the directory where it is running.
It contains keys:

```
bpf_sniffer_path = "/tmp/bpf-sniffer.sock"
rpc_port = 17732
```

The `bpf_sniffer_path` is legacy, do not change it.
The `rpc_port` is the port where the network recorder serves http requests (v2).

The `[[nodes]]` section contains settings related to some TezEdge or Tezos node.
There might be multiple such sections.

* `db_path` it is path to the database where debugger store intercepted network data. 

* `identity_path` is the path where node generated or will generate `identity.json` file.

* `p2p_port` is the port where the node will be listening incoming p2p connections.

* `rpc_port` is the port where the network recorder serves http requests (v3).

* `syslog_port` is the UDP port where the network recorder receives nodes logs in syslog format.

### Run memory profiler

If you run the TezEdge node in docker, set environment variable
`TEZEDGE_NODE_NAME` to be equal the name of the docker container of TezEdge node.

Run the memory profiler:

```
sudo TEZEDGE_NODE_NAME=<name of node container> ./target/none/release/bpf-memprof-user
```

or 

```
sudo ./target/none/release/bpf-memprof-user
```

### Run network recorder

Run the network recorder:

```
sudo ./target/none/release/bpf-sniffer & sleep 0.5 && ./target/none/release/tezedge-recorder
```
