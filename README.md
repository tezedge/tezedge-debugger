TezEdge Memory Profiler
=======================

The tool monitor memory usage of each function of TezEdge Light Node.

Memory used by the node itself and by the kernel for caching IO of node.
The profiler can track both.

How does it work
================

The tool consists of two parts.

The first part is `bpf-memprof-user` binary it has embedded
ebpf module. It requires superuser permission. When launch this binary loads the ebpf module
into kernel and create `/tmp/bpf-memprof.sock` socket. The ebpf module is tracking `exec` syscall
to determine which process is the TezEdge node. That is why `bpf-memprof-user` should be running
before the TezEdge node. If `bpf-memprof-user` run when the node already running, it will not
found the node.

The ebpf module is tracking a physical (residential) page allocation, deallocation, adding and
removing such page to the IO cache. Also the ebpf module unwinding stack at each allocation event.
So the profiler has call-stack virtual addresses.

The second part is `tezedge-memprof` binary. It performs multiple tasks.
* Connects to the socket and receiving the stream of kernel events.
* Monitoring `/proc/<pid>/maps` file. This file containing descriptions of each memory area on
the address space of the TezEdge node. Among others it containing descriptions of memory area of
the executable code `light-node` binary and shared libraries used by the node. It allows translation
from virtual address of the function into filename and offset in the file where the function is.
* Loading `.symtab` and `.strtab` sections from `light-node` binary and from shared libraries.
It enable the profiler to resolve function name.
* Counting allocated memory and memory used for cache at each function.
* Serving http requests.

How to run
==========

The application is distributed as a docker image `simplestakingcom/tezedge-memprof`. The image
needs to have privileged permissions. Also it needs `/sys/kernel/debug` and `/proc` directories
mapped from the host system. The application is serving http requests on `17832` port.

For example:

```
docker run --rm --privileged -it -p 17832:17832 -v /proc:/proc:rw -v /sys/kernel/debug:/sys/kernel/debug:rw simplestakingcom/tezedge-memprof:latest
```

In order to determine function names, the memory profiler needs an access to `light-node`
and system shared libraries. It should be identically the same files. That is why Tte docker image
`simplestakingcom/tezedge-memprof:latest` is inherited from `simplestakingcom/tezedge:latest` image.

But if the `tezedge` is updated, but the `tezedge-memprof` image is still old, it is a problem.
To avoid such situation, `tezedge-memprof` image has a docker client inside, and copy `light-node`
binary from the `tezedge` container. Set `TEZEDGE_NODE_NAME` environment variable into
TezEdge node container name and map `/var/run/docker.sock` file from host to enable such behavior.
See `docker-compose.yml` and `memprof.sh` for details.

HTTP API
========

## `/v1/tree`

Return tree-like object. Each node of the tree representing a function in some executable file.
The tree has following structure:

* `name` 
    * `executable` - name of the binary file (ELF), for example `libc-2.31.so`
    * `offset` - offset of the function call in the binary file
    * `functionName` - demangled name of the function, for example
    `<networking::p2p::peer::Peer as riker::actor::Receive<networking::p2p::peer::SendMessage>>::receive::hfe17b4d497a1a6cb`,
    note: rust function name is ending with hash, for example `hfe17b4d497a1a6cb`
    * `functionCategory` - indicated the origin of the function can be one of:
        * `nodeRust` is a function of the TezEdge node written in Rust
        * `nodeCpp` is a function of the TezEdge node written in C++
        * `systemLib` is a function from some system library, usually written in C,
        but might be arbitrary language.
* `value` - positive integer, number of kilobytes of total memory (ordinary + cache)
allocated in this function and all functions from which this function is called
* `cacheValue` - positive integer, number of kilobytes of cache allocated in this function
* `frames` - list of branches of the tree, containing all functions from which this function is called,
or containing all functions which is called from this function (if `reverse` is set in true).

### Parameters

`reverse` - boolean parameter, used to request reversed tree, default value is `false`;

`threshold` - integer parameter, used to filter out functions which allocated smaller amount of memory than some threshold value, default value is `256`.

## `/v1/pid`

Returns the process id of the TezEdge Node process.
