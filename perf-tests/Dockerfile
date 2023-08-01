# Set destination for COPY


# Download Go modules
FROM golang:1.18-alpine
WORKDIR /app

WORKDIR /app

COPY go.mod ./
COPY go.sum ./
RUN go mod download
COPY *.go ./
COPY server.crt /app/server.crt
COPY server.key /app/server.key

RUN go build -o /enclave-server

EXPOSE 8082

CMD [ "/enclave-server" ]