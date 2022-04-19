FROM ubuntu:20.04 as builder

WORKDIR /home/appuser/
RUN apt-get update && \
    DEBIAN_FRONTEND='noninteractive' apt-get install -y \
    git wget curl gcc libsodium-dev make zlib1g-dev \
    lsb-release software-properties-common \
    libarchive-tools flex bison libssl-dev bc libelf-dev

# rust
ENV RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
RUN set -eux && \
    wget "https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init" && \
    chmod +x rustup-init && \
    ./rustup-init -y --no-modify-path --default-toolchain nightly-2022-04-15 && \
    rm rustup-init && \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME
ENV PATH=/usr/local/cargo/bin:$PATH

# llvm 12
RUN wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && ./llvm.sh 12 && rm llvm.sh
ENV LLVM_SYS_120_PREFIX=/usr/lib/llvm-12

RUN apt install -y g++ git libssl-dev pkg-config libev-dev

RUN rustup update stable
RUN cargo +stable install bpf-linker --git https://github.com/tezedge/bpf-linker.git --branch develop

COPY . .
RUN cargo +stable build -p bpf-recorder --release && \
    cargo +stable build -p tezedge-recorder --release

FROM tezedge/tezedge-libs:latest-profile

COPY --from=builder /usr/lib/x86_64-linux-gnu/libev.so.4 /usr/lib/x86_64-linux-gnu/libev.so.4
COPY --from=builder /usr/lib/x86_64-linux-gnu/libffi.so.7 /usr/lib/x86_64-linux-gnu/libffi.so.7
COPY --from=builder /usr/lib/x86_64-linux-gnu/libelf.so.1 /usr/lib/x86_64-linux-gnu/libelf.so.1
COPY --from=builder /lib/x86_64-linux-gnu/libz.so.1 /lib/x86_64-linux-gnu/libz.so.1
COPY --from=builder /home/appuser/target/none/release/bpf-recorder /usr/local/bin/bpf-recorder
COPY --from=builder /home/appuser/target/none/release/tezedge-recorder /usr/local/bin/tezedge-recorder

ENTRYPOINT [ "tezedge-recorder", "--run-bpf" ]
