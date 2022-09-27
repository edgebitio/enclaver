#!/usr/bin/env bash

set -e

build_arch=$1

zig_version="0.9.1"

case $build_arch in
  arm64)
    zig_bundle="zig-linux-aarch64-${zig_version}"
    ;;
  amd64)
    zig_bundle="zig-linux-x86_64-${zig_version}"
    ;;
  *)
    echo "Unsupported build_arch: ${build_arch}"
    exit 1
    ;;
esac

wget -q https://ziglang.org/download/0.9.1/${zig_bundle}.tar.xz
tar xf ${zig_bundle}.tar.xz
mv ${zig_bundle}/zig /usr/local/bin
mv ${zig_bundle}/lib/* /usr/local/lib
rm -rf ${zig_bundle}.tar.xz

cargo install cargo-zigbuild