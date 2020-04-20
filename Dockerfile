FROM kyras/tezedge_base:latest AS builder
WORKDIR /home/appuser/tezedge_proxy
COPY . .
RUN cargo build --release

FROM kyras/tezedge_base:latest
WORKDIR /home/appuser/proxy
RUN apt-get install -y iproute2
RUN apt-get install -y iputils-ping
COPY ./docker/run/ ./
COPY --from=builder /home/appuser/tezedge_proxy/target/release/tezedge_proxy /home/appuser/proxy/tezedge_proxy
#COPY --from=builder /home/appuser/tezedge_proxy/docker/run/identity/ /home/appuser/proxy/identity
#COPY --from=builder /home/appuser/tezedge_proxy/docker/run /home/appuser/proxy/
ENV RUST_BACKTRACE=1
CMD ["./run.sh"]