package main

import (
	"context"
	"fmt"
	"github.com/go-edgebit/enclaver/proxy"
	"net/http"
)

func main() {
	// For now, this call needs to happen early in the startup phase of an enclave app, before any HTTP
	// requests are performed.
	err := proxy.StartEnclaveForwarder(context.Background())
	if err != nil {
		panic(err)
	}

	http.ListenAndServe(":8080", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		println("received a request, fetching google.com...")
		resp, err := http.Get("https://google.com")
		if err != nil {
			panic(err)
		}

		fmt.Printf("Got status: %d\n", resp.StatusCode)
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(fmt.Sprintf("Got %s from Google\n", resp.Status)))
	}))

}
