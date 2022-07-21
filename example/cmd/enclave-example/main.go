package main

import (
	"github.com/mdlayher/vsock"
	"time"
)

func main() {
	cid, err := vsock.ContextID()
	if err != nil {
		panic(err)
	}

	println("Found CID: ", cid)
	for {
		println("Hello, world!")
		time.Sleep(10 * time.Second)
	}
}
