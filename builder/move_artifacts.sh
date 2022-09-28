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

mkdir out
mv enclaver/target/${rust_target}/release/enclaver out/enclaver
mv enclaver/target/${rust_target}/release/odyn out/odyn