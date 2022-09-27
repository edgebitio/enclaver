FROM --platform=$BUILDPLATFORM rust:latest AS builder
ARG TARGETARCH

RUN rustup toolchain install nightly

# This is awful; there must be a better way???
RUN case ${TARGETARCH} in \
  "arm64") \
    rustup target add aarch64-unknown-linux-musl \
    ;; \
  "amd64") \
    rustup target add x86_64-unknown-linux-musl \
    ;; \
esac

WORKDIR /usr/src/enclaver
COPY enclaver .

RUN case ${TARGETARCH} in \
  "arm64") \
      cargo install --target=aarch64-unknown-linux-musl --path . \
    ;; \
  "amd64") \
      cargo install --target=x86_64-unknown-linux-musl --path . \
    ;; \
esac

FROM amazonlinux:latest

# TODO: Figure out how to make this way smaller
RUN \
    amazon-linux-extras install aws-nitro-enclaves-cli \
    && mkdir /enclave \
    && yum clean all \
    && rm -rf /var/cache/yum

COPY --from=builder /usr/local/cargo/bin/enclaver /usr/local/bin/enclaver

ENTRYPOINT ["/usr/local/bin/enclaver", "run-eif", "--eif-file", "/enclave/application.eif"]