FROM rust:latest AS builder

ARG ZIG_VERSION=0.9.1
RUN curl -L "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-$(uname -m)-${ZIG_VERSION}.tar.xz" | tar -J -x -C /usr/local && \
    mv /usr/local/zig-linux-$(uname -m)-${ZIG_VERSION}/zig /usr/local/bin/zig && \
    mv /usr/local/zig-linux-$(uname -m)-${ZIG_VERSION}/lib/* /usr/local/lib/

RUN rustup target add x86_64-unknown-linux-musl && \
    rustup target add aarch64-unknown-linux-musl

RUN cargo install cargo-zigbuild

COPY entrypoint.sh /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]