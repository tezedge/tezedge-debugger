Bpf builder:

```
docker login docker.io

docker build -t tezedge/tezedge-bpf-builder:latest -f bpf.dockerfile .
docker push tezedge/tezedge-bpf-builder:latest
```

TezEdge libs:

```
docker login docker.io

docker build -t tezedge/tezedge-libs:latest-profile .
docker push tezedge/tezedge-libs:latest-profile

docker build -t tezedge/tezedge-libs:latest --build-arg CFLAGS=-O2 .
docker push tezedge/tezedge-libs:latest
```
