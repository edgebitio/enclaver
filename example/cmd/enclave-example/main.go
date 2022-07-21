package main

import (
	"github.com/mdlayher/vsock"
	"golang.org/x/sys/unix"
)

func main() {
	err := listen()
	if err != nil {
		panic(err)
	}
}

func listen() error {
	listener, err := vsock.ListenContextID(unix.VMADDR_CID_ANY, 8080, nil)
	if err != nil {
		return err
	}

	defer listener.Close()

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
