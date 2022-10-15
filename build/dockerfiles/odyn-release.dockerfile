FROM scratch
ARG TARGETARCH

COPY --from=artifacts ${TARGETARCH}/odyn /usr/local/bin/odyn

ENTRYPOINT ["/usr/local/bin/odyn"]