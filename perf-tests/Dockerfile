FROM golang:1.18-alpine
WORKDIR /app

COPY go.mod ./
COPY server.go ./

RUN go build -o /enclave-server

EXPOSE 8082

CMD [ "/enclave-server" ]