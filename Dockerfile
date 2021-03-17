FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN cargo build -p bpf-sniffer --relase && \
    cargo build --bin tezedge-debugger --relase && \
    cargo build --bin tezedge-debugger-db --relase && \
    cargo build --bin tezedge-debugger-parser --relase

FROM ubuntu:20.10
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/target/none/release/bpf-sniffer ./
COPY --from=builder /home/appuser/target/none/release/tezedge-debugger ./
COPY --from=builder /home/appuser/target/none/release/tezedge-debugger-db ./
COPY --from=builder /home/appuser/target/none/release/tezedge-debugger-parser ./
