FROM kyras/tezedge_base:latest as builder
WORKDIR /home/appuser/
COPY . .
RUN rustup install nightly-2020-07-12 && rustup default nightly-2020-07-12
RUN cargo install --bins --path . --root .

FROM ubuntu:latest
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/bin ./
#CMD ["./tezedge-debugger"]