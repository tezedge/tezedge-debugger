FROM ubuntu:latest
SHELL ["/bin/bash", "-c"]
RUN apt-get update
RUN apt-get install -y curl iproute2 iputils-ping iptables
RUN apt-get install -y pkg-config libsodium-dev
RUN apt-get install -y clang libclang-dev llvm llvm-dev linux-kernel-headers libev-dev libssl-dev
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly-2020-02-04 -y
ENV PATH="~/.cargo/bin:$PATH"