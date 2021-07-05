FROM kyras/tezedge_base:latest as builder
WORKDIR /home/appuser/
RUN rustup install nightly-2020-10-24 && rustup default nightly-2020-10-24 && \
    cargo install --git https://github.com/tezedge/tezedge-bootstrap-poc --bin node --root .

FROM ubuntu:20.04
WORKDIR /home/appuser/
RUN apt-get update && \
    DEBIAN_FRONTEND='noninteractive' apt-get install -y netcat
COPY --from=builder /home/appuser/bin ./
COPY run.sh ./
COPY identity.json ./
RUN mkdir target
