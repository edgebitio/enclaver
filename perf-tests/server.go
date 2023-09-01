package main

import (
	"fmt"
	"log"
	"net/http"
)

func HelloServer(w http.ResponseWriter, req *http.Request) {
	busy := 0
	for i := 0; i < 1000000; i++ {
		busy += i
	}
	fmt.Fprintf(w, "Busy calculation: %d", busy)
}

func main() {
	http.HandleFunc("/busy", HelloServer)
	err := http.ListenAndServe(":8082", nil)
	if err != nil {
		log.Fatal("ListenAndServe: ", err)
	}
}
