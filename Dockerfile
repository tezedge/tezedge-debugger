FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN cargo install --bins --root . --path .
RUN cargo install --bins --root . --path bpf-sniffer

FROM ubuntu:20.10
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/bin ./
