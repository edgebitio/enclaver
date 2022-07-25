package main

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy"
	"net/http"
)

func main() {
	err := proxy.StartEnclaveForwarder(context.Background())
	if err != nil {
		panic(err)
	}

	resp, err := http.Get("https://google.com")
	if err != nil {
		panic(err)
	}

	fmt.Printf("Got status: %d\n", resp.StatusCode)
}
