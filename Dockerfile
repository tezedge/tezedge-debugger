FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++

ARG branch
RUN cargo install --bins --root . --git https://github.com/simplestaking/tezedge-debugger --branch ${branch} tezedge_debugger

FROM ubuntu:20.04
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/bin ./
