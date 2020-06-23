FROM kyras/tezedge_base:latest as builder
WORKDIR /home/appuser/
COPY . .
RUN cargo install --bins --path . --root .

FROM ubuntu:latest
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/bin ./
#CMD ["./tezedge-debugger"]