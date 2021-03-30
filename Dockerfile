FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN cargo +nightly-2020-12-31 build -p bpf-sniffer --release
RUN rustup update nightly
# TODO: freeze the version
RUN cargo +nightly build -p tezedge-recorder --release

FROM ubuntu:20.10

RUN apt update && apt install -y heaptrack

WORKDIR /home/appuser/
COPY --from=builder /home/appuser/target/none/release/bpf-sniffer ./
COPY --from=builder /home/appuser/target/none/release/tezedge-recorder ./

CMD ./bpf-sniffer & sleep 0.5 && ./tezedge-recorder
