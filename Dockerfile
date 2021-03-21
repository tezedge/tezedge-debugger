FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN cargo build -p bpf-sniffer --release && \
    cargo build --bin tezedge-debugger --release && \
    cargo build --bin tezedge-debugger-db --release && \
    cargo build --bin tezedge-debugger-parser --release

FROM ubuntu:20.10

RUN apt update && apt install -y heaptrack

WORKDIR /home/appuser/
COPY --from=builder /home/appuser/target/none/release/bpf-sniffer ./
COPY --from=builder /home/appuser/target/none/release/tezedge-debugger ./
COPY --from=builder /home/appuser/target/none/release/tezedge-debugger-db ./
COPY --from=builder /home/appuser/target/none/release/tezedge-debugger-parser ./

# CMD ./bpf-sniffer & sleep 0.5 ; ./tezedge-debugger-db & sleep 1 ; heaptrack ./tezedge-debugger-parser; cp heaptrack.*.gz /tmp/report
CMD ./bpf-sniffer & sleep 0.5 ; ./tezedge-debugger-db & sleep 1 ; ./tezedge-debugger-parser; sleep inf
# CMD ./bpf-sniffer & sleep 0.5 && ./tezedge-debugger ; sleep inf
