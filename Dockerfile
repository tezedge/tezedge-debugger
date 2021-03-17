FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN cargo build -p bpf-sniffer && \
    cargo build --bin tezedge-debugger && \
    cargo build --bin tezedge-debugger-db && \
    cargo build --bin tezedge-debugger-parser

FROM ubuntu:20.10
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/target/none/debug/bpf-sniffer ./
COPY --from=builder /home/appuser/target/none/debug/tezedge-debugger ./
COPY --from=builder /home/appuser/target/none/debug/tezedge-debugger-db ./
COPY --from=builder /home/appuser/target/none/debug/tezedge-debugger-parser ./
