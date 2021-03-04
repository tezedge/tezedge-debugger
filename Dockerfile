FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN cargo install --bins --root . --path .

FROM ubuntu:20.04
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/bin ./
