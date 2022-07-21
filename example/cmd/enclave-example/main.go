package main

import (
	"github.com/mdlayher/vsock"
)

func main() {
	err := listen()
	if err != nil {
		panic(err)
	}
}

func listen() error {
	listener, err := vsock.Listen(8080, &vsock.Config{})
	if err != nil {
		return err
	}

	println("listening")

	for {
		conn, err := listener.Accept()
		if err != nil {
			return err
		}

		println("accepted")

		go func() {
			for {
				buf := make([]byte, 1024)
				_, err := conn.Read(buf)
				if err != nil {
					println("error reading from socket", err)
				}

				println("received message:", string(buf))
			}
		}()
	}
}
