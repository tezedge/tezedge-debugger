FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN cargo install bpf-linker --git https://github.com/tezedge/bpf-linker.git --branch main
COPY . .
RUN cargo build -p bpf-memprof --release
RUN cargo build -p tezedge-memprof --release

FROM ubuntu:20.10

WORKDIR /home/appuser/

RUN apt update && DEBIAN_FRONTEND='noninteractive' apt install -y libelf-dev
COPY --from=builder /home/appuser/target/none/release/bpf-memprof-user .
COPY --from=builder /home/appuser/target/none/release/tezedge-memprof .

CMD ./bpf-memprof-user & sleep 0.5 && ./tezedge-memprof
