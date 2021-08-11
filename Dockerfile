FROM tezedge/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git libssl-dev pkg-config libev-dev

RUN rustup update stable && rustup update nightly-2021-08-04
RUN cargo +stable install bpf-linker --git https://github.com/tezedge/bpf-linker.git --branch main

COPY . .
RUN cargo +stable build -p bpf-recorder --release && \
    cargo +nightly-2021-08-04 build -p tezedge-recorder --release

FROM tezedge/tezedge-libs:latest-profile

COPY --from=builder /usr/lib/x86_64-linux-gnu/libelf.so.1 /usr/lib/x86_64-linux-gnu/libelf.so.1
COPY --from=builder /lib/x86_64-linux-gnu/libz.so.1 /lib/x86_64-linux-gnu/libz.so.1
COPY --from=builder /home/appuser/target/none/release/bpf-recorder /usr/local/bin/bpf-recorder
COPY --from=builder /home/appuser/target/none/release/tezedge-recorder /usr/local/bin/tezedge-recorder

ENTRYPOINT [ "tezedge-recorder", "--run-bpf" ]
