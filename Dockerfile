FROM simplestakingcom/tezedge-ci-builder:latest as builder
WORKDIR /home/appuser/
COPY . .
RUN cargo install --bins --path . --root .

FROM ubuntu:latest
WORKDIR /home/appuser/
RUN apt-get update
RUN apt-get install -y libssl-dev net-tools
COPY --from=builder /home/appuser/bin ./
#CMD ["./tezedge-debugger"]