FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN cargo build --bin tezedge-debugger --release && cargo build -p bpf-sniffer --release

FROM ubuntu:20.10
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/target/none/release/bpf-sniffer ./
COPY --from=builder /home/appuser/target/none/release/tezedge-debugger ./
