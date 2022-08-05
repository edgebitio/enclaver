FROM amazonlinux:latest
RUN amazon-linux-extras install aws-nitro-enclaves-cli
RUN yum install aws-nitro-enclaves-cli-devel -y
WORKDIR /build
ENTRYPOINT ["nitro-cli"]