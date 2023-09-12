#!/usr/bin/env bash

VCPUS="${1:-4}"

docker stop nitro_enclave

set -e

image_name=enclave_image_${VCPUS}vcpu

docker rmi --force docker-go ${image_name}
docker build -t docker-go .

config=$(mktemp)
trap "rm --force ${config}" EXIT

cat > ${config} <<- EOF
	version: v1
	name: ${VCPUS}vcpu
	target: ${image_name}
	sources:
	  app: docker-go
	defaults:
	  memory_mb: 3000
	  cpu_count: ${VCPUS}
	egress:
	  allow:
	  - "**"
	ingress:
	- listen_port: 8082
EOF

enclaver build -f ${config}
docker run --detach --rm \
       --name=nitro_enclave \
       --device=/dev/nitro_enclaves:/dev/nitro_enclaves:rw \
       --publish=8082:8082 \
       ${image_name}
