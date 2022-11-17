FROM edgebitio/nitro-cli:latest AS nitro_cli
RUN touch /tmp/dummy

###############################

FROM scratch AS build-amd64
COPY --from=nitro_cli /lib64/ld-linux-x86-64.so.2 /lib64/

###############################

FROM scratch AS build-arm64
COPY --from=nitro_cli /lib/ld-linux-aarch64.so.1 /lib/

###############################

FROM build-${TARGETARCH} AS build

ARG TARGETARCH

COPY --from=nitro_cli /lib64/libssl.so.10 /lib64/libcrypto.so.10 /lib64/libgcc_s.so.1 /lib64/librt.so.1 /lib64/libpthread.so.0 /lib64/libm.so.6 /lib64/libdl.so.2 /lib64/libc.so.6 /lib64/libgssapi_krb5.so.2 /lib64/libkrb5.so.3 /lib64/libcom_err.so.2 /lib64/libk5crypto.so.3 /lib64/libz.so.1 /lib64/libkrb5support.so.0 /lib64/libkeyutils.so.1 /lib64/libresolv.so.2 /lib64/libselinux.so.1 /lib64/libpcre.so.1 /lib64/
COPY --from=nitro_cli /usr/bin/nitro-cli /bin/nitro-cli

COPY --from=nitro_cli /tmp/dummy /var/log/nitro_enclaves/
COPY --from=nitro_cli /tmp/dummy /run/nitro_enclaves/

COPY --from=artifacts ${TARGETARCH}/enclaver-run /usr/local/bin/enclaver-run

ENTRYPOINT ["/usr/local/bin/enclaver-run"]
