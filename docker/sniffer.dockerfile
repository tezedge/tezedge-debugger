FROM ubuntu:20.10

WORKDIR /home/appuser/

COPY bin/bpf-sniffer .

CMD while true; do ./bpf-sniffer; done
