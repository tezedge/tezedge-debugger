FROM tezedge/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git libssl-dev pkg-config

COPY . .
RUN rustup update stable && rustup update nightly-2021-03-23
RUN cargo +stable install bpf-linker --git https://github.com/tezedge/bpf-linker.git --branch main
RUN cargo +stable build -p bpf-recorder --release && \
    cargo +nightly-2021-03-23 build -p tezedge-recorder --release

FROM ubuntu:20.10

RUN DEBIAN_FRONTEND='noninteractive' apt-get update && apt-get install -y libelf-dev

COPY --from=builder /home/appuser/target/none/release/bpf-recorder /usr/local/bin
COPY --from=builder /home/appuser/target/none/release/tezedge-recorder /usr/local/bin

WORKDIR /home/appuser/
CMD bpf-recorder & sleep 0.5 && tezedge-recorder
