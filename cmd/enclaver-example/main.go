package main

import (
	"context"
	"fmt"
	"github.com/aws/aws-sdk-go-v2/aws"
	"github.com/aws/aws-sdk-go-v2/config"
	"github.com/aws/aws-sdk-go-v2/service/kms"
	"github.com/aws/aws-sdk-go-v2/service/kms/types"
	"github.com/davecgh/go-spew/spew"
	"github.com/go-edgebit/enclaver/crypto/cms"
	"github.com/go-edgebit/enclaver/runtime"
	"net/http"
)

var (
	kmsKeyId = aws.String("arn:aws:kms:us-west-2:899464120550:key/8bd2a0dd-4ca3-4ebc-a8b9-efab2b658a06")
)

func main() {
	ctx := context.Background()

	err := run(ctx)
	if err != nil {
		panic(err)
	}
}

func run(ctx context.Context) error {
	rt, err := runtime.GetOrInitialize()
	if err != nil {
		return err
	}

	config, err := config.LoadDefaultConfig(ctx, config.WithRegion("us-west-2"))
	if err != nil {
		return err
	}

	kmsClient := kms.NewFromConfig(config)

	attestationDoc, err := rt.Attest(runtime.AttestationOptions{})
	if err != nil {
		panic(err)
	}

	dataKeyRes, err := kmsClient.GenerateDataKey(context.Background(), &kms.GenerateDataKeyInput{
		KeyId:   kmsKeyId,
		KeySpec: types.DataKeySpecAes256,
		Recipient: &types.RecipientInfoType{
			AttestationDocument:    attestationDoc,
			KeyEncryptionAlgorithm: types.EncryptionAlgorithmSpecRsaesOaepSha256,
		},
	})
	if err != nil {
		return err
	}
	if dataKeyRes.CiphertextForRecipient == nil {
		return fmt.Errorf("CiphertextForRecipient is nil")
	}

	key, err := cms.DecryptEnvelopedKey(rt.GetPrivateKey(), dataKeyRes.CiphertextForRecipient)
	if err != nil {
		panic(err)
	}

	println("Decrypted Data Key:")
	spew.Dump(key)

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

	return nil
}
