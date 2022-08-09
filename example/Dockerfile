FROM golang:1.18-alpine AS app-builder
WORKDIR /usr/src/enclaver_example
COPY . .
RUN go build -v -o /usr/local/bin/enclaver-example ./cmd/enclaver-example


FROM alpine:latest AS app-container
COPY --from=app-builder /usr/local/bin/enclaver-example /usr/local/bin/enclaver-example
CMD ["/usr/local/bin/enclaver-example"]
