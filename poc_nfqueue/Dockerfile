FROM kyras/tezedge_base:latest AS builder
WORKDIR /home/appuser/poc
COPY . .
RUN cargo install --path . --root /home/appuser

FROM kyras/tezedge_base:latest
WORKDIR /home/appuser/poc
COPY --from=builder /home/appuser/bin/* /home/appuser/poc
ENV RUST_BACKTRACE=1
