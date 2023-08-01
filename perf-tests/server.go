package main

import (
    // "fmt"
    // "io"
    "net/http"
    "log"
	"crypto/aes"
 	"crypto/cipher"
 	"encoding/base64"
 	"fmt"
)

var bytes = []byte{35, 46, 57, 24, 85, 35, 24, 74, 87, 35, 88, 98, 66, 32, 14, 05}
// This should be in an env file in production
const MySecret string = "abc&1*~#^2^#s0^=)^^7%b34"
func Encode(b []byte) string {
	return base64.StdEncoding.EncodeToString(b)
}

func Encrypt(text, MySecret string) (string, error) {
	block, err := aes.NewCipher([]byte(MySecret))
	if err != nil {
	 return "", err
	}
	plainText := []byte(text)
	cfb := cipher.NewCFBEncrypter(block, bytes)
	cipherText := make([]byte, len(plainText))
	cfb.XORKeyStream(cipherText, plainText)
	return Encode(cipherText), nil
}

func HelloServer(w http.ResponseWriter, req *http.Request) {
    w.Header().Set("Content-Type", "text/plain")
    w.Write([]byte("This is an example server.\n"))
	StringToEncrypt := "Encrypting this string"
    certificates := req.TLS.PeerCertificates
    if len(certificates) > 0 {
        fmt.Println("Found client certificate")
    }

    // To encrypt the StringToEncrypt
    encText, err := Encrypt(StringToEncrypt, MySecret)
    if err != nil {
     fmt.Println("error encrypting your classified text: ", err)
    }
    // fmt.Println(encText)
	w.Header().Set("Content-Type", "text/plain")
    w.Write([]byte(encText))
    // fmt.Fprintf(w, "This is an example server.\n")
    // io.WriteString(w, "This is an example server.\n")
}

func main() {
    http.HandleFunc("/hello", HelloServer)
    err := http.ListenAndServeTLS(":8082", "/app/server.crt", "/app/server.key", nil)
    if err != nil {
        log.Fatal("ListenAndServe: ", err)
    }
}