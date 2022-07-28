package main

import (
	"context"
	"fmt"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/davecgh/go-spew/spew"
	"github.com/go-edgebit/enclaver/runtime"
	"net/http"
)

func main() {
	runtime, err := runtime.GetOrInitialize()
	if err != nil {
		panic(err)
	}

	doc, err := runtime.Attest([]byte("nonce"), []byte("userdata"), nil)
	if err != nil {
		panic(err)
	}

	spew.Dump(doc)

	config, err := config.LoadDefaultConfig(context.Background())
	if err != nil {
		panic(err)
	}

	spew.Dump(config)

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
