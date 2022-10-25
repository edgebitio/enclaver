FROM edgebitio/nitro-cli:latest
ARG TARGETARCH

COPY --from=artifacts ${TARGETARCH}/enclaver-run /usr/local/bin/enclaver-run

ENTRYPOINT ["/usr/local/bin/enclaver-run"]