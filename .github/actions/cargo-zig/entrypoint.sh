#!/bin/bash

TARGET="$1"

shift

case $TARGET in
  "aarch64-unknown-linux-musl")
    ZIG_TARGET="aarch64-linux-musl"
    ;;
  "x86_64-unknown-linux-musl")
    ZIG_TARGET="x86_64-linux-musl"
    ;;
  *)
    echo "Unsupported architecture: ${TARGET}"
    exit 1
    ;;
esac

ZIG_COMMAND="zig cc -target ${ZIG_TARGET}"

export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="${ZIG_COMMAND}"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="${ZIG_COMMAND}"
export CC="${ZIG_COMMAND}"

cargo $@ "--target=${TARGET}"