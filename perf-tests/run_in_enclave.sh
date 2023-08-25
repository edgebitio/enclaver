# To run enclave with 2vCPU

# docker stop nitro_enclave
# docker rmi --force docker-go enclave_image_2vcpu
# docker build -t docker-go .
# enclaver build -f enclaver_2vcpu.yaml
# docker run -d --rm --name nitro_enclave --device=/dev/nitro_enclaves:/dev/nitro_enclaves:rw -p 8082:8082 enclave_image_2vcpu


# To run enclave with 4vCPU

docker stop nitro_enclave
docker rmi --force docker-go enclave_image_4vcpu
docker build -t docker-go .
enclaver build -f enclaver_4vcpu.yaml
docker run -d --rm --name nitro_enclave --device=/dev/nitro_enclaves:/dev/nitro_enclaves:rw -p 8082:8082 enclave_image_4vcpu
