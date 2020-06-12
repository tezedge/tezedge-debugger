FROM kyras/tezedge_base:latest AS builder
WORKDIR /home/appuser
RUN apt-get update -y
RUN apt-get install git -y
RUN git clone https://github.com/simplestaking/tezedge-debugger.git
WORKDIR /home/appuser/tezedge-debugger
RUN git checkout mlacko@PoC_nfqueue
RUN cargo build --release

FROM kyras/tezedge_base:latest
WORKDIR /home/appuser/benchmarks
RUN apt-get update -y
RUN apt-get install iperf3 -y
COPY --from=builder /home/appuser/tezedge-debugger/target/release/poc_tokio /home/appuser/benchmarks
COPY ./bench.sh .
USER root
CMD [ "./bench.sh" ]