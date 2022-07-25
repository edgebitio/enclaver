package main

import (
	"fmt"
	"github.com/mdlayher/vsock"
	"golang.org/x/sys/unix"
	"io"
	"net"
	"net/http"
	"os"
)

func main() {
	os.Setenv("HTTP_PROXY", "http://localhost:3128")
	os.Setenv("HTTPS_PROXY", "http://localhost:3128")
	runInternalProxy()

	resp, err := http.Get("https://google.com")
	if err != nil {
		panic(err)
	}

	fmt.Printf("Got status: %d\n", resp.StatusCode)

	/*
		err = listen()
		if err != nil {
			panic(err)
		}
	*/
}

// TODO: any error handling or logging at all
func runInternalProxy() error {
	listener, err := net.Listen("tcp", ":3128")
	if err != nil {
		return err
	}

	go func() {
		defer listener.Close()

		for {
			clientConn, err := listener.Accept()
			if err != nil {
				panic(err)
			}

			println("accepted proxy conn...")

			go func() {
				serverConn, err := vsock.Dial(3, 3128, nil)
				if err != nil {
					panic(err)
				}

				errc := make(chan error)

				go func() {
					_, err := io.Copy(serverConn, clientConn)
					errc <- err
				}()

				go func() {
					_, err := io.Copy(clientConn, serverConn)
					errc <- err
				}()

				<-errc
			}()
		}
	}()

	return nil
}

func listen() error {
	listener, err := vsock.ListenContextID(unix.VMADDR_CID_ANY, 8080, nil)
	if err != nil {
		return err
	}

	defer listener.Close()

	println("Listening!")

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
