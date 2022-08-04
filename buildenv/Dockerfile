FROM amazonlinux:latest AS base
RUN amazon-linux-extras install aws-nitro-enclaves-cli

FROM base AS devel
RUN yum install aws-nitro-enclaves-cli-devel -y