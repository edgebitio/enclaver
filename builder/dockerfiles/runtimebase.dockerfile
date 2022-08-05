FROM golang:latest AS builder

WORKDIR /usr/src/app
COPY . .
RUN go build -v -o /usr/local/bin/enclaver-wrapper ./cmd/enclaver-wrapper

FROM amazonlinux:latest

# TODO: Figure out how to make this way smaller
RUN \
    amazon-linux-extras install aws-nitro-enclaves-cli \
    && mkdir /enclave \
    && yum clean all \
    && rm -rf /var/cache/yum

COPY --from=builder /usr/local/bin/enclaver-wrapper /usr/local/bin/enclaver-wrapper

ENTRYPOINT ["/usr/local/bin/enclaver-wrapper"]