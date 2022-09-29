FROM amazonlinux:latest
ARG TARGETARCH

# TODO: Figure out how to make this way smaller
RUN \
    amazon-linux-extras install aws-nitro-enclaves-cli \
    && mkdir /enclave \
    && yum clean all \
    && rm -rf /var/cache/yum

COPY --from=artifacts ${TARGETARCH}/enclaver /usr/local/bin/enclaver

ENTRYPOINT ["/usr/local/bin/enclaver", "run-eif", "--eif-file", "/enclave/application.eif"]