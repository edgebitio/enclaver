#!/bin/bash

set -eu

local_arch=$(uname -m)
case $local_arch in
	x86_64)
		rust_target="x86_64-unknown-linux-musl"
		;;
	aarch64)
		rust_target="aarch64-unknown-linux-musl"
		;;
	*)
		echo "Unsupported architecture: $local_arch"
		exit 1
		;;
esac

enclaver_dir="$(dirname $(dirname ${BASH_SOURCE[0]}))/enclaver"
rust_target_dir="./target/${rust_target}/debug"

odyn_tag="odyn-dev:latest"
wrapper_base_tag="enclaver-wrapper-base:latest"

cd $enclaver_dir

docker_build_dir=$(mktemp -d)
trap "rm --force --recursive ${docker_build_dir}" EXIT

cargo build --target $rust_target --all-features

cp $rust_target_dir/odyn $docker_build_dir/
cp $rust_target_dir/enclaver-run $docker_build_dir/

docker build \
	-f ../build/dockerfiles/odyn-dev.dockerfile \
	-t ${odyn_tag} \
	${docker_build_dir}

docker build \
	-f ../build/dockerfiles/runtimebase-dev.dockerfile \
	-t ${wrapper_base_tag} \
	${docker_build_dir}

echo "To use dev images, merge the following into enclaver.yaml:"
echo ""
echo "sources:"
echo "   supervisor: \"${odyn_tag}\""
echo "   wrapper: \"${wrapper_base_tag}\""
