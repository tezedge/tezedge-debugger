FROM ubuntu:20.04

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
    ./rustup-init -y --no-modify-path --default-toolchain nightly-2020-12-31 && \
    rm rustup-init && \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME
ENV PATH=/usr/local/cargo/bin:$PATH

# llvm 11
RUN wget https://apt.llvm.org/llvm.sh && chmod +x llvm.sh && ./llvm.sh 11 && rm llvm.sh
ENV LLVM_SYS_110_PREFIX=/usr/lib/llvm-11

RUN wget -cq https://cdn.kernel.org/pub/linux/kernel/v5.x/linux-5.8.18.tar.xz && \
    tar xf linux-5.8.18.tar.xz && cd linux-5.8.18 && make defconfig && make modules_prepare
ENV KERNEL_SOURCE=/home/appuser/linux-5.8.18 KERNEL_VERSION=5.8.18
