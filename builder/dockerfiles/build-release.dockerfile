FROM --platform=$BUILDPLATFORM rust:latest AS builder
ARG TARGETARCH
ARG BUILDARCH

WORKDIR /usr/src/enclaver

COPY builder/prepare_builder.sh ./builder/prepare_builder.sh
RUN ./builder/prepare_builder.sh ${BUILDARCH}

# Pre-compile dependencies for caching
COPY builder/build.sh ./builder/build.sh
COPY enclaver/Cargo.toml enclaver/Cargo.toml
COPY enclaver/Cargo.lock enclaver/Cargo.lock
RUN mkdir enclaver/src && touch enclaver/src/lib.rs
RUN ./builder/build.sh ${TARGETARCH}
RUN rm enclaver/src/lib.rs

COPY . .

RUN touch enclaver/src/lib.rs

RUN ./builder/build.sh ${TARGETARCH}

RUN ./builder/move_artifacts.sh ${TARGETARCH}

FROM scratch

COPY --from=builder /usr/src/enclaver/out/* /usr/local/bin/