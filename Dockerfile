FROM simplestakingcom/tezedge-bpf-builder:latest as builder

RUN apt install -y g++ git

COPY . .
RUN git reset --hard e99c0e47b896ca804875e8213b144fd841e91ff2 && \
    cargo install --bins --root . --path .
# RUN cp debugger_config.toml bin/debugger_config.toml

FROM ubuntu:20.04
WORKDIR /home/appuser/
COPY --from=builder /home/appuser/bin ./
