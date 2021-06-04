# TezEdge Memory Profiler

  

This tool monitors the memory usage of each function of the TezEdge Light Node.

  

The profiler can track both the memory used by the node itself and by the kernel for the caching IO of the node.

  
  

## How it works

  

The tool consists of two parts.

  

### 1. EBPF loader

  

The first part is `bpf-memprof-user` binary which has an embedded ebpf module. It requires superuser permission. When launched, this binary loads the ebpf module into the kernel and creates the `/tmp/bpf-memprof.sock` socket. The ebpf module tracks the `exec` syscall to determine which process is the TezEdge node. That is why `bpf-memprof-user` should be running before the TezEdge node is launched. If `bpf-memprof-user` is launched when the node is already running, it will not be able to find the node.

  

The ebpf module is tracking physical (residential) page allocation and deallocation, either removing or adding such pages to the IO cache. Additionally, the ebpf module unwinds the stack during each allocation event so that the profiler has call-stack virtual addresses.

  

### 2. TezEdge memprof binary

  

The second part is the `tezedge-memprof` binary. It performs the following tasks:

* Connects to the socket and receives a stream of kernel events.

* Monitors the `/proc/<pid>/maps` file. This file contains descriptions of each memory area on the address space of the TezEdge node. Among others, it contains the descriptions of memory areas of the executable code `light-node` binary and shared libraries used by the node. It allows translation from the virtual address of the function into filename and offset in the file where the function is.

* Loads `.symtab` and `.strtab` sections from `light-node` binary and from shared libraries.

It enables the profiler to resolve function names.

* Counts allocated memory and memory used for cache at each function.

* Serves http requests.

  

## How to run


  

The application is distributed as a docker image `simplestakingcom/tezedge-memprof`. The image needs to have privileged permissions. It also needs `/sys/kernel/debug` and `/proc` directories mapped from the host system. The application is serving http requests on port `17832`.

  

For example:

  

```

docker run --rm --privileged -it -p 17832:17832 -v /proc:/proc:rw -v /sys/kernel/debug:/sys/kernel/debug:rw simplestakingcom/tezedge-memprof:latest

```

  

In order to determine function names, the memory profiler needs access to `light-node`

and system shared libraries. The files to which the memory profiler has access to should be the same files that the Tezedge node is using. That is why the docker image

`simplestakingcom/tezedge-memprof:latest` is inherited from the `simplestakingcom/tezedge:latest` image.

  

However, if `tezedge` is updated, but the `tezedge-memprof` image is still old, it can lead to problems. To avoid such situations, `tezedge-memprof` image has a docker client inside, and copies the `light-node` binary from the updated `tezedge` container.

  

Set the `TEZEDGE_NODE_NAME` environment variable into the TezEdge node container name and map `/var/run/docker.sock` file from host to enable such behavior.

See `docker-compose.yml` and `memprof.sh` for details.

  

## HTTP API



  

## `/v1/tree`

  

Return a tree-like object. Each node of the tree represents a function in some executable file.

The tree has the following structure:

  

* `name`

* `executable` - name of the binary file (ELF), for example `libc-2.31.so`

* `offset` - offset of the function call in the binary file

* `functionName` - demangled name of the function, for example

`<networking::p2p::peer::Peer as riker::actor::Receive<networking::p2p::peer::SendMessage>>::receive::hfe17b4d497a1a6cb`,

note: rust function name is ending with hash, for example `hfe17b4d497a1a6cb`

* `functionCategory` - indicates the origin of the function, can be one of the following:

* `nodeRust` is a function of the TezEdge node written in Rust

* `nodeCpp` is a function of the TezEdge node written in C++

* `systemLib` is a function from a system library, usually written in C,

but it can also be an arbitrary language.

* `value` - positive integer, number of kilobytes of total memory (ordinary + cache)

allocated in this function and all functions from which this function is called

* `cacheValue` - positive integer, number of kilobytes of cache allocated in this function

* `frames` - list of branches of the tree, containing all functions from which this function is called,

or containing all functions which are called from this function (if `reverse` is set in true).

  

### Parameters

  

`reverse` - boolean parameter, used to request reversed tree, default value is `false`;

  

`threshold` - integer parameter, used to filter out functions which allocate a smaller amount of memory than some threshold value, default value is `256`.

  

## `/v1/pid`

  

Returns the process id of the TezEdge Node process.
