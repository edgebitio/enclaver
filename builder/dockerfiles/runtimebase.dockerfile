FROM edgebitio/nitro-cli:latest
ARG TARGETARCH

COPY --from=artifacts ${TARGETARCH}/enclaver /usr/local/bin/enclaver

ENTRYPOINT ["/usr/local/bin/enclaver", "run-eif", "--eif-file", "/enclave/application.eif"]