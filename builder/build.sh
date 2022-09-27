#!/usr/bin/env bash

set -e

target_arch=$1

case $target_arch in
  arm64)
    rust_target="aarch64-unknown-linux-musl"
    ;;
  amd64)
    rust_target="x86_64-unknown-linux-musl"
    ;;
esac

rustup target add ${rust_target}

cd enclaver && cargo zigbuild --release --target ${rust_target}