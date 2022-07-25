package main

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy"
	"github.com/go-edgebit/enclaver/proxy/ifconfig"
	"github.com/go-edgebit/enclaver/proxy/vsock"
	"net"
	"net/http"
	"os"
)

func main() {
	os.Setenv("HTTP_PROXY", "http://localhost:3128")
	os.Setenv("HTTPS_PROXY", "http://localhost:3128")
	err := runInternalProxy(context.Background())
	if err != nil {
		panic(err)
	}

	resp, err := http.Get("https://google.com")
	if err != nil {
		panic(err)
	}

	fmt.Printf("Got status: %d\n", resp.StatusCode)
}

// TODO: improve error handling, logging and context propagation
func runInternalProxy(ctx context.Context) error {
	err := ifconfig.ConfigureEnclaveInterface()
	if err != nil {
		return err
	}

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
				serverConn, err := vsock.DialParent(3128)
				if err != nil {
					panic(err)
				}

				err = proxy.Pump(clientConn, serverConn, ctx)
			}()
		}
	}()

	return nil
}

/*
func listenVsock() error {
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
*/
