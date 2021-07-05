```
docker login docker.io

docker build -t tezedge/tezedge-bpf-builder:latest -f bpf.dockerfile . 
docker push tezedge/tezedge-bpf-builder:latest
```