FROM kyras/tezedge_base:latest as builder
WORKDIR /home/appuser/
RUN apt-get update && \
    DEBIAN_FRONTEND='noninteractive' apt-get install -y \
    wget lsb-release software-properties-common \
    libarchive-tools flex bison libssl-dev bc libelf-dev \
    && \
    rustup install nightly-2020-12-31 && rustup default nightly-2020-12-31

# llvm 11
RUN wget https://apt.llvm.org/llvm.sh && \
    chmod +x llvm.sh && \
    ./llvm.sh 11 && \
    rm llvm.sh
ENV LLVM_SYS_110_PREFIX=/usr/lib/llvm-11

RUN wget -cq https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-5.8.18.tar.xz && \
    tar xf linux-5.8.18.tar.xz && cd linux-5.8.18 && make defconfig && make modules_prepare
ENV KERNEL_SOURCE=/home/appuser/linux-5.8.18
ENV KERNEL_VERSION=5.8.18

# https://blog.mgattozzi.dev/caching-rust-docker-builds/
# Prepare empty binaries and all the dependencies that we have in Cargo.toml
#RUN mkdir -p {.,sniffer}/src/bin && \
#    echo "fn main() {}" > ./src/bin/debugger.rs && \
#    echo "fn main() {}" > ./src/bin/drone_test_server.rs && \
#    echo "fn main() {}" > ./src/bin/drone_test_client.rs && \
#    echo "fn main() {}" > ./sniffer/src/bin/kprobe.rs && \
#    echo "pub fn foo() {}" > ./sniffer/src/lib.rs
#COPY Cargo.lock .
#COPY Cargo.toml .
#COPY sniffer/Cargo.toml sniffer
# This step cache's our deps!
#RUN cargo install --bins --path . --root . && rm -R ./src
# Copy the rest of the files into the container
COPY . .
RUN cargo install --bins --path . --root .

FROM ubuntu:20.04
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/bin ./
COPY cleanup_probes.sh ./
#CMD ["./tezedge-debugger"]
