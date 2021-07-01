FROM tezedge/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git libssl-dev pkg-config

COPY . .
RUN cargo +nightly-2020-12-31 build -p bpf-sniffer --release
RUN rustup update nightly-2021-03-23
RUN cargo +nightly-2021-03-23 build -p tester --release
RUN cargo +nightly-2021-03-23 build -p tezedge-recorder --release

FROM ubuntu:20.10

COPY --from=builder /home/appuser/target/none/release/bpf-sniffer /usr/local/bin
COPY --from=builder /home/appuser/target/none/release/tezedge-recorder /usr/local/bin
COPY --from=builder /home/appuser/target/none/release/tester /usr/local/bin
COPY --from=builder /home/appuser/tester/wait_until.sh /usr/local/bin
COPY --from=builder /home/appuser/config-drone.toml /etc

WORKDIR /home/appuser/
CMD bpf-sniffer & sleep 0.5 && tezedge-recorder
