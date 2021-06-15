FROM ubuntu:21.04 as build-env

RUN DEBIAN_FRONTEND=noninteractive apt-get update && apt-get install -y tzdata && \
    ln -fs /usr/share/zoneinfo/America/New_York /etc/localtime && \
    dpkg-reconfigure --frontend noninteractive tzdata
RUN apt-get install -y wget build-essential m4 flex gawk bison python python3

ARG GLIBC_VERSION=2.33
ARG CFLAGS=-O2\ -fno-omit-frame-pointer

RUN wget -q https://ftpmirror.gnu.org/glibc/glibc-${GLIBC_VERSION}.tar.gz && \
    tar xzf glibc-${GLIBC_VERSION}.tar.gz
RUN mkdir /glibc-build && cd /glibc-build && \
    CFLAGS="${CFLAGS}" ../glibc-${GLIBC_VERSION}/configure --prefix=/usr/local/lib/glibc-${GLIBC_VERSION} && \
    make -j$(nproc) && make install

RUN wget -q https://ftpmirror.gnu.org/gcc/gcc-10.3.0/gcc-10.3.0.tar.xz && \
    tar xf gcc-10.3.0.tar.xz && cd gcc-10.3.0 && contrib/download_prerequisites
RUN mkdir /gcc-build && cd /gcc-build && \
    CFLAGS="${CFLAGS}" ../gcc-10.3.0/configure -v --build=x86_64-linux-gnu --host=x86_64-linux-gnu \
        --target=x86_64-linux-gnu --prefix=/usr/local/gcc-10.3.0 --enable-checking=release \
        --enable-languages=c,c++ --disable-multilib --program-suffix=-10.3 && \
    make -j$(nproc) && make install

# there must be a way to build it along with gcc
RUN wget -q https://ftpmirror.gnu.org/gnu/gmp/gmp-6.1.0.tar.bz2 && \
    tar xf gmp-6.1.0.tar.bz2
RUN cd /gmp-6.1.0 && \
    CFLAGS="${CFLAGS}" ./configure --prefix=/usr/local/lib/gmp-6.1.0 && \
    make -j$(nproc) && make install && make check

RUN wget -q https://download.libsodium.org/libsodium/releases/libsodium-1.0.18-stable.tar.gz && \
    tar xf libsodium-1.0.18-stable.tar.gz
RUN cd /libsodium-stable && \
    CFLAGS="${CFLAGS}" ./configure --prefix=/usr/local/lib/libsodium-1.0.18-stable && \
    make -j$(nproc) && make install

FROM scratch

ARG GLIBC_VERSION=2.33
COPY --from=build-env /usr/local/lib/glibc-${GLIBC_VERSION}/lib/ld-${GLIBC_VERSION}.so /lib64/ld-linux-x86-64.so.2
COPY --from=build-env /usr/local/lib/glibc-${GLIBC_VERSION}/lib/libc-${GLIBC_VERSION}.so /lib/x86_64-linux-gnu/libc.so.6
COPY --from=build-env /usr/local/lib/glibc-${GLIBC_VERSION}/lib/libdl-${GLIBC_VERSION}.so /lib/x86_64-linux-gnu/libdl.so.2
COPY --from=build-env /usr/local/lib/glibc-${GLIBC_VERSION}/lib/libm-${GLIBC_VERSION}.so /lib/x86_64-linux-gnu/libm.so.6
COPY --from=build-env /usr/local/lib/glibc-${GLIBC_VERSION}/lib/librt-${GLIBC_VERSION}.so /lib/x86_64-linux-gnu/librt.so.1
COPY --from=build-env /usr/local/lib/glibc-${GLIBC_VERSION}/lib/libpthread-${GLIBC_VERSION}.so /lib/x86_64-linux-gnu/libpthread.so.0
COPY --from=build-env /usr/local/gcc-10.3.0/lib64/libgcc_s.so.1 /lib/x86_64-linux-gnu/libgcc_s.so.1
COPY --from=build-env /usr/local/gcc-10.3.0/lib64/libstdc++.so.6 /lib/x86_64-linux-gnu/libstdc++.so.6
COPY --from=build-env /usr/local/lib/gmp-6.1.0/lib/libgmp.so.10 /lib/x86_64-linux-gnu/libgmp.so.10
COPY --from=build-env /usr/local/lib/libsodium-1.0.18-stable/lib/libsodium.so.23 /lib/x86_64-linux-gnu/libsodium.so.23
ENV LD_LIBRARY_PATH=/lib:/lib/x86_64-linux-gnu:/usr/lib:/usr/lib/x86_64-linux-gnu
