FROM kyras/tezedge_base:latest
WORKDIR /home/appuser/
RUN apt-get update
RUN apt-get install -y rsync git m4 build-essential patch unzip wget pkg-config libgmp-dev libev-dev libhidapi-dev
RUN wget "https://github.com/ocaml/opam/releases/download/2.0.3/opam-2.0.3-x86_64-linux"
RUN cp "opam-2.0.3-x86_64-linux" "/usr/local/bin/opam"
RUN chmod a+x "/usr/local/bin/opam"
RUN groupadd -g 999 appuser && useradd -r -u 999 -g appuser appuser
RUN chown appuser:appuser /home/appuser
USER appuser
RUN git clone "https://gitlab.com/tezos/tezos.git"
WORKDIR /home/appuser/tezos
RUN git checkout carthagenet
RUN opam init --bare --disable-sandbox && make build-deps && eval $(opam config env) && make
USER root
RUN mkdir "/root/.tezos-node"
COPY config.json "/root/"
ENV PATH="/home/appuser/tezos:$PATH"
ENV TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER=Y
