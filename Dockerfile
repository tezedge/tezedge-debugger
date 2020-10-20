FROM kyras/tezedge_base:latest as builder
WORKDIR /home/appuser/
RUN apt-get update && \
    DEBIAN_FRONTEND='noninteractive' apt-get install -y libpcap-dev && \
    rustup install nightly-2020-07-12 && rustup default nightly-2020-07-12

# https://blog.mgattozzi.dev/caching-rust-docker-builds/
# Prepare empty binaries and all the dependencies that we have in Cargo.toml
RUN mkdir -p ./src/bin && \
    echo "fn main() {}" > ./src/bin/debugger.rs && \
    echo "fn main() {}" > ./src/bin/drone_test_server.rs && \
    echo "fn main() {}" > ./src/bin/drone_test_client.rs
COPY Cargo.lock .
COPY Cargo.toml .
# This step cache's our deps!
RUN cargo install --bins --path . --root . && rm -R ./src
# Copy the rest of the files into the container
COPY . .
RUN cargo install --bins --path . --root .

FROM ubuntu:latest
WORKDIR /home/appuser/
RUN apt-get update && \
    DEBIAN_FRONTEND='noninteractive' apt-get install -y libpcap-dev
COPY --from=builder /home/appuser/bin ./
#CMD ["./tezedge-debugger"]