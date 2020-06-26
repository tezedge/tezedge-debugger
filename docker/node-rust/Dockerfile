FROM kyras/tezedge_base:latest
WORKDIR /home/appuser/
RUN apt-get update
RUN apt-get install -y pkg-config libsodium-dev git gawk
#RUN apt-get install -y clang libclang-dev llvm llvm-dev linux-kernel-headers libev-dev --fix-missing
ARG CACHEBUST=1
RUN git clone "https://github.com/simplestaking/tezedge"
WORKDIR /home/appuser/tezedge
RUN cargo build --release
COPY ./tezedge.config ./
