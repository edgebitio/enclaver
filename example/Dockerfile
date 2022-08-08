FROM --platform=$BUILDPLATFORM golang:1.18-alpine AS app-builder
WORKDIR /usr/src/enclaver_example
COPY . .
ARG TARGETOS TARGETARCH
RUN GOOS=$TARGETOS GOARCH=$TARGETARCH go build -v -o /usr/local/bin/enclaver-example ./cmd/enclaver-example


FROM alpine:latest AS app-container
COPY --from=app-builder /usr/local/bin/enclaver-example /usr/local/bin/enclaver-example
CMD ["/usr/local/bin/enclaver-example"]
