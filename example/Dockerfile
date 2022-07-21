FROM golang:1.18

WORKDIR /usr/src/enclave_example

COPY . .
RUN go build -v -o /usr/local/bin/enclave-example ./cmd/enclave-example

CMD ["enclave-example"]