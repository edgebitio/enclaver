package main

import (
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"fmt"
	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/aws/aws-sdk-go-v2/service/kms"
	"github.com/aws/aws-sdk-go-v2/service/kms/types"
	"github.com/go-edgebit/enclaver/runtime"
	"net/http"
)

var (
	kmsKeyId = aws.String("arn:aws:kms:us-west-2:899464120550:key/8bd2a0dd-4ca3-4ebc-a8b9-efab2b658a06")
)

func main() {
	runtime, err := runtime.GetOrInitialize()
	if err != nil {
		panic(err)
	}

	println("Generating RSA Key..")

	privateKey, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		panic(err)
	}

	encodedPublicKey, err := x509.MarshalPKIXPublicKey(privateKey.PublicKey)
	if err != nil {
		panic(err)
	}

	attestationDoc, err := runtime.Attest(nil, nil, encodedPublicKey)
	if err != nil {
		panic(err)
	}

	config, err := config.LoadDefaultConfig(context.Background(), config.WithRegion("us-west-2"))
	if err != nil {
		panic(err)
	}

	kmsClient := kms.NewFromConfig(config)

	dataKeyRes, err := kmsClient.GenerateDataKey(context.Background(), &kms.GenerateDataKeyInput{
		KeyId:   kmsKeyId,
		KeySpec: types.DataKeySpecAes256,
		Recipient: &types.RecipientInfoType{
			AttestationDocument:    attestationDoc,
			KeyEncryptionAlgorithm: types.EncryptionAlgorithmSpecRsaesOaepSha256,
		},
	})
	if err != nil {
		panic(err)
	}
	if dataKeyRes.CiphertextForRecipient == nil {
		panic("CiphertextForRecipient is nil")
	}

	println("Got non-nil CiphertextForRecipient")

	plaintext, err := privateKey.Decrypt(rand.Reader, dataKeyRes.CiphertextForRecipient, nil)
	if err != nil {
		panic(err)
	}

	println("Decrypted data key: " + string(plaintext))

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
