package main

import (
	"fmt"
	"github.com/davecgh/go-spew/spew"
	runtime2 "github.com/go-edgebit/enclaver/runtime"
	"net/http"
)

func main() {
	runtime, err := runtime2.GetOrInitialize()
	if err != nil {
		panic(err)
	}

	doc, err := runtime.Attest([]byte("nonce"), []byte("userdata"), nil)
	if err != nil {
		panic(err)
	}

	spew.Dump(doc)

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
